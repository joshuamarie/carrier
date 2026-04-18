// Remove read_name entirely. Any call site that was:
//
//   let name = tar::read_name(tar_path)?;
//
// becomes:
//
//   let name = tar::read_toml(tar_path)?.module.name;

use std::fs::File;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use flate2::{write::GzEncoder, Compression};
use tar::Builder;

use crate::manifest::Manifest;

pub fn bundle(
    src_path: &Path,
    project_root: &Path,
    output_path: &Path,
    manifest: &Manifest,
) -> Result<()> {
    let file = File::create(output_path)
        .with_context(|| format!("Failed to create: {}", output_path.display()))?;

    let enc = GzEncoder::new(file, Compression::default());
    let mut archive = Builder::new(enc);

    let top = format!("{}_{}", manifest.name, manifest.version);

    let toml_path = project_root.join("carrier.toml");
    archive
        .append_path_with_name(&toml_path, format!("{top}/carrier.toml"))
        .context("Failed to add carrier.toml to archive")?;

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

    archive.finish().context("Failed to finalize tar.gz archive")?;
    Ok(())
}

pub fn unpack(tar_path: &Path, install_dir: &Path) -> Result<()> {
    let file = File::open(tar_path)
        .with_context(|| format!("Failed to open: {}", tar_path.display()))?;

    let gz = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(gz);

    for entry in archive.entries().context("Failed to read tar.gz entries")? {
        let mut entry = entry.context("Failed to read tar.gz entry")?;
        let raw_path = entry.path()
            .context("Failed to get entry path")?
            .to_path_buf();

        let stripped = strip_top_level(&raw_path)?;

        if stripped == Path::new("") || stripped == Path::new(".") {
            continue;
        }

        let dest = install_dir.join(&stripped);

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
/// fully extracting the archive. Use `.module.name` if you only need
/// the module name — `read_name` has been removed as redundant.
pub fn read_toml(tar_path: &Path) -> Result<crate::carrier_toml::CarrierToml> {
    let file = File::open(tar_path)
        .with_context(|| format!("Failed to open: {}", tar_path.display()))?;

    let gz = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(gz);

    for entry in archive.entries().context("Failed to read tar.gz entries")? {
        let mut entry = entry.context("Failed to read entry")?;
        let raw_path = entry.path()?.to_path_buf();
        let stripped = strip_top_level(&raw_path)?;

        if stripped == Path::new("carrier.toml") {
            let mut s = String::new();
            std::io::Read::read_to_string(&mut entry, &mut s)
                .context("Failed to read carrier.toml from archive")?;
            return toml::from_str(&s)
                .context("Failed to parse carrier.toml from archive");
        }
    }

    anyhow::bail!(
        "No carrier.toml found in {}. Is this a valid carrier package?",
        tar_path.display()
    )
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
