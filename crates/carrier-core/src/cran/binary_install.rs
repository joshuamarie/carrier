//! Install a pre-built (binary) R package archive without invoking R.
//!
//! A binary package archive is already the finished output of `R CMD
//! INSTALL` — configure has run, any compiled code is built, help/Rd
//! files are converted. Installing one is just "put these files in the
//! library", so this module extracts directly instead of shelling out
//! to R a second time.
//!
//! Source packages still go through `R CMD INSTALL` in the source
//! fallback path — that one genuinely needs R's build machinery.

use std::fs::File;
use std::path::{Component, Path, PathBuf};

use anyhow::{bail, Context, Result};
use flate2::read::GzDecoder;
use tar::Archive as TarArchive;
use tempfile::TempDir;

/// Extract a binary archive (`.zip` or `.tgz`/`.tar.gz`) for `package_name`
/// into `lib_path`, replacing any existing install of that package.
///
/// Extraction lands in a temp staging directory first. Only after the
/// staged tree passes `verify_built_package` is it moved into place —
/// so a bad or truncated download never overwrites a working install.
pub fn install_binary_package(
    archive_path: &Path,
    lib_path: &Path,
    package_name: &str,
) -> Result<()> {
    let staging = TempDir::new().context("Failed to create staging dir for binary install")?;

    let is_zip = archive_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("zip"))
        .unwrap_or(false);

    if is_zip {
        extract_zip(archive_path, staging.path())
            .with_context(|| format!("Failed to extract zip for {}", package_name))?;
    } else {
        extract_tgz(archive_path, staging.path())
            .with_context(|| format!("Failed to extract tarball for {}", package_name))?;
    }

    let staged_pkg = staging.path().join(package_name);
    verify_built_package(&staged_pkg, package_name)?;

    std::fs::create_dir_all(lib_path)
        .with_context(|| format!("Failed to create lib dir: {}", lib_path.display()))?;

    let dest = lib_path.join(package_name);
    if dest.exists() {
        std::fs::remove_dir_all(&dest)
            .with_context(|| format!("Failed to remove existing install at {}", dest.display()))?;
    }

    std::fs::rename(&staged_pkg, &dest).or_else(|_| {
        // Staging dir and lib_path may be on different filesystems (temp dir
        // vs a user-configured lib path), which makes rename() fail with
        // EXDEV. Fall back to copy + remove in that case.
        copy_dir_recursive(&staged_pkg, &dest)?;
        std::fs::remove_dir_all(&staged_pkg).ok();
        Ok::<(), anyhow::Error>(())
    })?;

    Ok(())
}

/// A properly built R package — binary tarball or zip — always contains
/// `Meta/package.rds`; a source tree never does. Without this check, a
/// source tarball served where a binary was expected (or a truncated
/// download) would extract cleanly, report success, and only fail later
/// at a `library()` call — possibly in a different session entirely.
fn verify_built_package(staged_pkg: &Path, package_name: &str) -> Result<()> {
    if staged_pkg.join("Meta").join("package.rds").exists() {
        return Ok(());
    }
    bail!(
        "'{}' does not look like a built binary package (no Meta/package.rds — \
         likely a source tarball served as binary, or a truncated download). \
         Retry the download, or install from source instead.",
        package_name
    );
}

/// Reject any archive entry path that is absolute or contains a `..`
/// component. Returns the sanitized relative path on success.
fn sanitize_entry_path(raw: &Path) -> Option<PathBuf> {
    let mut clean = PathBuf::new();
    for part in raw.components() {
        match part {
            Component::Normal(seg) => clean.push(seg),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    if clean.as_os_str().is_empty() {
        None
    } else {
        Some(clean)
    }
}

fn extract_tgz(tgz_path: &Path, dest_root: &Path) -> Result<()> {
    let file = File::open(tgz_path)
        .with_context(|| format!("Failed to open {}", tgz_path.display()))?;
    let decoder = GzDecoder::new(file);
    let mut archive = TarArchive::new(decoder);

    for entry in archive.entries().context("Failed to read tar entries")? {
        let mut entry = entry.context("Failed to read tar entry")?;
        let raw_path = entry.path().context("Invalid entry path")?.into_owned();
        let Some(rel_path) = sanitize_entry_path(&raw_path) else {
            bail!("Refusing to extract unsafe path in archive: {}", raw_path.display());
        };
        let dest = dest_root.join(&rel_path);

        match entry.header().entry_type() {
            tar::EntryType::Directory => {
                std::fs::create_dir_all(&dest)
                    .with_context(|| format!("Failed to create dir {}", dest.display()))?;
            }
            tar::EntryType::Regular | tar::EntryType::Continuous => {
                if let Some(parent) = dest.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let mut out = File::create(&dest)
                    .with_context(|| format!("Failed to create file {}", dest.display()))?;
                std::io::copy(&mut entry, &mut out)
                    .with_context(|| format!("Failed to write {}", dest.display()))?;
            }
            #[cfg(unix)]
            tar::EntryType::Symlink => {
                let Some(target) = entry.link_name().ok().flatten().map(|t| t.into_owned()) else {
                    continue;
                };
                let escapes = target.is_absolute()
                    || target.components().any(|c| c == Component::ParentDir);
                if escapes {
                    continue;
                }
                if let Some(parent) = dest.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let _ = std::fs::remove_file(&dest);
                std::os::unix::fs::symlink(&target, &dest)
                    .with_context(|| format!("Failed to symlink {}", dest.display()))?;
            }
            // Hardlinks, devices, FIFOs, PAX globals, etc. don't show up in
            // R package tarballs in practice — skip anything unrecognized
            // rather than fail the whole install over it.
            _ => {}
        }
    }

    Ok(())
}

fn extract_zip(zip_path: &Path, dest_root: &Path) -> Result<()> {
    let file = File::open(zip_path)
        .with_context(|| format!("Failed to open {}", zip_path.display()))?;
    let mut archive = zip::ZipArchive::new(file).context("Failed to read zip archive")?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).context("Failed to read zip entry")?;
        let Some(rel_path) = entry.enclosed_name().map(|p| p.to_path_buf()) else {
            bail!("Refusing to extract unsafe path in archive: {}", entry.name());
        };
        let dest = dest_root.join(&rel_path);

        if entry.is_dir() {
            std::fs::create_dir_all(&dest)
                .with_context(|| format!("Failed to create dir {}", dest.display()))?;
            continue;
        }

        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut out = File::create(&dest)
            .with_context(|| format!("Failed to create file {}", dest.display()))?;
        std::io::copy(&mut entry, &mut out)
            .with_context(|| format!("Failed to write {}", dest.display()))?;
    }

    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}
