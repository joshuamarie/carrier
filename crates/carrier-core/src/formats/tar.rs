use std::fs::File;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use flate2::{write::GzEncoder, Compression};
use tar::Builder;

use crate::manifest::Manifest;
use crate::carrier_toml::{ModuleDep, PackageDep};

/// Bundle a module into a `.tar.gz` archive.
///
/// Archive structure:
/// ``` text
/// tstk_0.1.0/
/// └── tstk/
///     ├── __init__.R
///     ├── decomp/
///     └── ...
/// ```
///
/// `carrier.toml` is intentionally excluded — it is a project manifest,
/// not part of the installable module, just as `pyproject.toml` is not
/// included inside `site-packages/pandas/`.
pub fn bundle(
    src_path: &Path,
    _project_root: &Path,
    output_path: &Path,
    manifest: &Manifest,
) -> Result<()> {
    let file = File::create(output_path)
        .with_context(|| format!("Failed to create: {}", output_path.display()))?;

    let enc = GzEncoder::new(file, Compression::default());
    let mut archive = Builder::new(enc);

    let top = format!("{}_{}", manifest.name, manifest.version);

    for entry in all_files(src_path) {
        let rel = entry
            .strip_prefix(src_path)
            .with_context(|| format!("Failed to strip prefix from {}", entry.display()))?;

        let tar_name = format!(
            "{}/{}/{}",
            top,
            manifest.name,
            rel.to_string_lossy().replace('\\', "/")
        );

        archive
            .append_path_with_name(&entry, &tar_name)
            .with_context(|| format!("Failed to add to archive: {tar_name}"))?;
    }

    // Write manifest.json inside the module directory so it travels
    // with the bundle and can be extracted into .dist-info on install.
    let manifest_json = manifest.to_json()?;
    let manifest_bytes = manifest_json.as_bytes();
    let manifest_tar_name = format!("{}/{}/manifest.json", top, manifest.name);
    let mut header = tar::Header::new_gnu();
    header.set_size(manifest_bytes.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    archive
        .append_data(&mut header, &manifest_tar_name, manifest_bytes)
        .context("Failed to add manifest.json to archive")?;

    archive.finish().context("Failed to finalize tar.gz archive")?;
    Ok(())
}

/// Unpack a `.tar.gz` carrier archive into the install directory.
///
/// Strips the top-level `{name}_{version}/` prefix so the result is:
/// ``` text
/// <install_dir>/tstk/
///     __init__.R
///     decomp/
///     ...
/// <install_dir>/tstk-0.1.0.dist-info/
///     manifest.json
/// ```
pub fn unpack(tar_path: &Path, install_dir: &Path, name: &str, version: &str) -> Result<()> {
    let file = File::open(tar_path)
        .with_context(|| format!("Failed to open: {}", tar_path.display()))?;

    let gz = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(gz);

    let dist_info_dir = install_dir.join(format!("{}-{}.dist-info", name, version));
    std::fs::create_dir_all(&dist_info_dir)
        .with_context(|| format!("Failed to create dist-info dir: {}", dist_info_dir.display()))?;

    for entry in archive.entries().context("Failed to read tar.gz entries")? {
        let mut entry = entry.context("Failed to read tar.gz entry")?;
        let raw_path = entry.path()
            .context("Failed to get entry path")?
            .to_path_buf();

        // Strip top-level {name}_{version}/ prefix
        let stripped = strip_top_level(&raw_path)?;

        if stripped == Path::new("") || stripped == Path::new(".") {
            continue;
        }

        // manifest.json goes into .dist-info, everything else into place
        let file_name = stripped.file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("");

        let dest = if file_name == "manifest.json" {
            dist_info_dir.join("manifest.json")
        } else {
            install_dir.join(&stripped)
        };

        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create dir: {}", parent.display()))?;
        }

        entry
            .unpack(&dest)
            .with_context(|| format!("Failed to unpack: {}", dest.display()))?;
    }

    Ok(())
}

/// Read and parse the `carrier.toml` embedded in a `.tar.gz` without
/// fully extracting the archive.
pub fn read_toml(tar_path: &Path) -> Result<crate::carrier_toml::CarrierToml> {
    let file = File::open(tar_path)
        .with_context(|| format!("Failed to open: {}", tar_path.display()))?;

    let gz = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(gz);

    // carrier.toml is no longer bundled — read manifest.json instead
    // and reconstruct what we need, or bail with a clear message.
    for entry in archive.entries().context("Failed to read tar.gz entries")? {
        let mut entry = entry.context("Failed to read entry")?;
        let raw_path = entry.path()?.to_path_buf();
        let stripped = strip_top_level(&raw_path)?;

        if stripped.file_name().and_then(|f| f.to_str()) == Some("manifest.json") {
            let mut s = String::new();
            std::io::Read::read_to_string(&mut entry, &mut s)
                .context("Failed to read manifest.json from archive")?;
            let manifest = crate::manifest::Manifest::from_json(&s)
                .context("Failed to parse manifest.json from archive")?;
            // Reconstruct a minimal CarrierToml from the manifest
            return Ok(crate::carrier_toml::CarrierToml {
                native: manifest.native.map(|n| crate::carrier_toml::NativeConfig {
                    path: None,
                    build_deps: if n.build_deps.is_empty() {
                        None
                    } else {
                        Some(
                            n.build_deps
                                .into_iter()
                                .map(|entry| {
                                    let dep = match entry.repo {
                                        Some(repo) => PackageDep::Extended { version: entry.version, repo: Some(repo) },
                                        None => PackageDep::Simple(entry.version),
                                    };
                                    (entry.name, dep)
                                })
                                .collect()
                        )
                    },
                }),
                module: crate::carrier_toml::ModuleMeta {
                    name: manifest.name,
                    version: manifest.version,
                    description: manifest.description,
                    authors: manifest.authors
                        .into_iter()
                        .map(|n| crate::carrier_toml::Author::Simple(n.to_string()))
                        .collect(),
                    license: manifest.license,
                    r_version: manifest.r_version,
                    src: None,
                },
                package_deps: Some(
                    manifest.dependencies.packages
                        .into_iter()
                        .map(|entry| {
                            let dep = match entry.repo {
                                Some(repo) => PackageDep::Extended { version: entry.version, repo: Some(repo) },
                                None => PackageDep::Simple(entry.version),
                            };
                            (entry.name, dep)
                        })
                        .collect()
                ),
                module_deps: Some(
                    manifest.dependencies.modules
                        .into_iter()
                        .map(|entry| {
                            let dep = match entry.source {
                                Some(source) => ModuleDep::Extended { version: entry.version, source: Some(source) },
                                None => ModuleDep::Simple(entry.version),
                            };
                            (entry.name, dep)
                        })
                        .collect()
                ),
                test: None,
            });
        }
    }

    anyhow::bail!(
        "No manifest.json found in {}. Is this a valid carrier package?",
        tar_path.display()
    )
}

pub fn collect_files(base: &Path) -> Result<Vec<String>> {
    all_files(base)
        .iter()
        .map(|p| {
            p.strip_prefix(base)
                .map(|r| r.to_string_lossy().replace('\\', "/"))
                .with_context(|| format!("Failed to strip prefix from {}", p.display()))
        })
        .collect()
}

fn strip_top_level(path: &Path) -> Result<PathBuf> {
    let mut components = path.components();
    components.next();
    Ok(components.as_path().to_path_buf())
}

fn all_files(base: &Path) -> Vec<PathBuf> {
    walkdir::WalkDir::new(base)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| {
            e.path()
                .strip_prefix(base)
                .unwrap_or(e.path())
                .components()
                .filter_map(|c| {
                    let s = c.as_os_str().to_string_lossy();
                    if s == "." || s == ".." { None } else { Some(s.starts_with('.')) }
                })
                .all(|is_hidden| !is_hidden)
        })
        .map(|e| e.path().to_owned())
        .collect()
}
