use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use walkdir::WalkDir;
use zip::{write::SimpleFileOptions, ZipArchive, ZipWriter};

use crate::manifest::Manifest;

const MANIFEST_FILENAME: &str = "manifest.json";

/// Pack a module directory into a `.rmbx` archive.
pub fn bundle(module_path: &Path, output_path: &Path, manifest: &Manifest) -> Result<()> {
    let file = File::create(output_path)
        .with_context(|| format!("Failed to create: {}", output_path.display()))?;

    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default();

    // Write manifest.json first
    zip.start_file(MANIFEST_FILENAME, options)
        .context("Failed to write manifest entry")?;
    zip.write_all(manifest.to_json()?.as_bytes())
        .context("Failed to write manifest content")?;

    // Write every file, preserving relative paths
    for entry in all_files(module_path) {
        let rel = entry
            .strip_prefix(module_path)
            .with_context(|| format!("Failed to strip prefix from {}", entry.display()))?;

        let zip_name = rel.to_string_lossy().replace('\\', "/");

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

/// Unpack a `.rmbx` archive into a destination directory.
/// This is the install step — it extracts R files but skips manifest.json.
pub fn unpack(rmbx_path: &Path, output_path: &Path) -> Result<()> {
    let file = File::open(rmbx_path)
        .with_context(|| format!("Failed to open: {}", rmbx_path.display()))?;

    let mut archive = ZipArchive::new(file)
        .with_context(|| format!("Failed to read archive: {}", rmbx_path.display()))?;

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .with_context(|| format!("Failed to read zip entry at index {i}"))?;

        let name = entry.name().to_owned();

        // Skip the manifest — it's internal to the bundle format
        if name == MANIFEST_FILENAME {
            continue;
        }

        let dest = output_path.join(&name);

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

#[allow(dead_code)]
/// Read the manifest from a `.rmbx` file without fully extracting it.
pub fn read_manifest(rmbx_path: &Path) -> Result<Manifest> {
    let file = File::open(rmbx_path)
        .with_context(|| format!("Failed to open: {}", rmbx_path.display()))?;
    let mut archive = ZipArchive::new(file)
        .with_context(|| format!("Failed to read archive: {}", rmbx_path.display()))?;
    read_manifest_from_archive(&mut archive)
}

/// Collect relative paths of all files in a module directory.
pub fn collect_r_files(module_path: &Path) -> Result<Vec<String>> {
    all_files(module_path)
        .iter()
        .map(|p| {
            p.strip_prefix(module_path)
                .map(|r| r.to_string_lossy().replace('\\', "/"))
                .with_context(|| format!("Failed to strip prefix from {}", p.display()))
        })
        .collect()
}

// ── internal helpers ────────────────────────────────────────────────────────

/// Collect all files recursively, excluding hidden files/dirs (e.g. .git)
fn all_files(module_path: &Path) -> Vec<PathBuf> {
    WalkDir::new(module_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| {
            // Skip hidden files and directories (e.g. .git, .github)
            // Only check individual path components, not the full path
            e.path()
                .components()
                .filter_map(|c| {
                    let s = c.as_os_str().to_string_lossy();
                    // Only consider normal named components, not . or ..
                    if s == "." || s == ".." {
                        None
                    } else {
                        Some(s.starts_with('.'))
                    }
                })
                .all(|is_hidden| !is_hidden)
        })
        .map(|e| e.path().to_owned())
        .collect()
}

fn read_manifest_from_archive(archive: &mut ZipArchive<File>) -> Result<Manifest> {
    let mut entry = archive.by_name(MANIFEST_FILENAME).with_context(|| {
        format!("Archive is missing '{MANIFEST_FILENAME}' — may be corrupt or not a carrier bundle")
    })?;

    let mut s = String::new();
    entry
        .read_to_string(&mut s)
        .context("Failed to read manifest")?;

    Manifest::from_json(&s)
}
