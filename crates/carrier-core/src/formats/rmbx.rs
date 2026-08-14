use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use zip::{write::SimpleFileOptions, ZipArchive, ZipWriter};

use crate::manifest::Manifest;

const MANIFEST_FILENAME: &str = "manifest.json";

/// Bundle a module into a `.rmbx` archive.
///
/// Archive structure:
/// ``` text
/// manifest.json
/// tstk/
///     __init__.R
///     decomp/
///     ...
/// ```
///
/// `carrier.toml` is intentionally excluded — it is a project manifest,
/// not part of the installable module.
pub fn bundle(
    src_path: &Path,
    _project_root: &Path,
    output_path: &Path,
    manifest: &Manifest,
    exclude: &[PathBuf],
) -> Result<()> {
    let file = File::create(output_path)
        .with_context(|| format!("Failed to create: {}", output_path.display()))?;

    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default();

    // Write manifest.json at archive root
    zip.start_file(MANIFEST_FILENAME, options)
        .context("Failed to write manifest entry")?;
    zip.write_all(manifest.to_json()?.as_bytes())
        .context("Failed to write manifest content")?;

    // Write source files inside <name>/
    for entry in all_files(src_path, exclude) {
        let rel = entry
            .strip_prefix(src_path)
            .with_context(|| format!("Failed to strip prefix from {}", entry.display()))?;

        let zip_name = format!(
            "{}/{}",
            manifest.name,
            rel.to_string_lossy().replace('\\', "/")
        );

        zip.start_file(&zip_name, options)
            .with_context(|| format!("Failed to start zip entry: {zip_name}"))?;

        let mut buf = Vec::new();
        File::open(&entry)
            .with_context(|| format!("Failed to open: {}", entry.display()))?
            .read_to_end(&mut buf)
            .with_context(|| format!("Failed to read: {}", entry.display()))?;

        zip.write_all(&buf)
            .with_context(|| format!("Failed to write: {zip_name}"))?;
    }

    zip.finish().context("Failed to finalize archive")?;
    Ok(())
}

/// Unpack a `.rmbx` archive into the install directory.
///
/// Result:
/// ``` text
/// <install_dir>/tstk/
///     __init__.R
///     ...
/// <install_dir>/tstk-0.1.0.dist-info/
///     manifest.json
/// ```
pub fn unpack(rmbx_path: &Path, install_dir: &Path) -> Result<()> {
    let manifest = read_manifest(rmbx_path)?;
    let dist_info_dir = install_dir.join(
        format!("{}-{}.dist-info", manifest.name, manifest.version)
    );
    std::fs::create_dir_all(&dist_info_dir)
        .with_context(|| format!("Failed to create dist-info dir: {}", dist_info_dir.display()))?;

    // Write manifest.json into dist-info
    let manifest_path = dist_info_dir.join(MANIFEST_FILENAME);
    std::fs::write(&manifest_path, manifest.to_json()?)
        .with_context(|| format!("Failed to write manifest to {}", manifest_path.display()))?;

    let file = File::open(rmbx_path)
        .with_context(|| format!("Failed to open: {}", rmbx_path.display()))?;

    let mut archive = ZipArchive::new(file)
        .with_context(|| format!("Failed to read archive: {}", rmbx_path.display()))?;

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .with_context(|| format!("Failed to read zip entry at index {i}"))?;

        let name = entry.name().to_owned();

        // Skip manifest — already written to dist-info above
        if name == MANIFEST_FILENAME {
            continue;
        }

        let dest = install_dir.join(&name);

        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create dir: {}", parent.display()))?;
        }

        let mut buf = Vec::new();
        entry
            .read_to_end(&mut buf)
            .with_context(|| format!("Failed to read entry: {name}"))?;

        File::create(&dest)
            .with_context(|| format!("Failed to create: {}", dest.display()))?
            .write_all(&buf)
            .with_context(|| format!("Failed to write: {}", dest.display()))?;
    }

    Ok(())
}

/// Read the manifest from a `.rmbx` file without fully extracting it.
pub fn read_manifest(rmbx_path: &Path) -> Result<Manifest> {
    let file = File::open(rmbx_path)
        .with_context(|| format!("Failed to open: {}", rmbx_path.display()))?;
    let mut archive = ZipArchive::new(file)
        .with_context(|| format!("Failed to read archive: {}", rmbx_path.display()))?;

    let mut entry = archive.by_name(MANIFEST_FILENAME).with_context(|| {
        format!("Archive is missing '{MANIFEST_FILENAME}' — may be corrupt or not a carrier bundle")
    })?;

    let mut s = String::new();
    entry
        .read_to_string(&mut s)
        .context("Failed to read manifest")?;

    Manifest::from_json(&s)
}

pub fn collect_files(base: &Path) -> Result<Vec<String>> {
    all_files(base, &[])
        .iter()
        .map(|p| {
            p.strip_prefix(base)
                .map(|r| r.to_string_lossy().replace('\\', "/"))
                .with_context(|| format!("Failed to strip prefix from {}", p.display()))
        })
        .collect()
}

fn all_files(base: &Path, exclude: &[PathBuf]) -> Vec<PathBuf> {
    walkdir::WalkDir::new(base)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| !exclude.iter().any(|ex| e.path().starts_with(ex)))
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
