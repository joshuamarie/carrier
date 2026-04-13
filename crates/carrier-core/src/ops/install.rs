use anyhow::{bail, Context, Result};
use std::fs::File;
use std::path::PathBuf;
use tempfile::TempDir;

use ::tar::Archive as TarArchive;
use crate::formats::{rmbx, tar};
use crate::paths::resolve_install_dir;

enum InstallSource {
    Rmbx(PathBuf),
    Tar(PathBuf),
    Dir(PathBuf),
    GitHub { user: String, repo: String },
}

pub fn run(source: &str) -> Result<()> {
    match parse_source(source)? {
        InstallSource::Rmbx(path) => install_from_rmbx(&path),
        InstallSource::Tar(path) => install_from_tar(&path),
        InstallSource::Dir(path) => install_from_dir(&path),
        InstallSource::GitHub { user, repo } => install_from_github(&user, &repo),
    }
}

fn parse_source(s: &str) -> Result<InstallSource> {
    // Directory path — carrier install . or carrier install ./my-proj
    let path = PathBuf::from(s);
    if path.is_dir() {
        return Ok(InstallSource::Dir(path));
    }

    if let Some(rest) = s.strip_prefix("gh:") {
        let parts: Vec<&str> = rest.splitn(2, '/').collect();
        if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
            bail!("Invalid GitHub source '{}'. Expected: gh:username/repo", s);
        }
        return Ok(InstallSource::GitHub {
            user: parts[0].to_owned(),
            repo: parts[1].to_owned(),
        });
    }

    match path.extension().and_then(|e| e.to_str()) {
        Some("rmbx") => Ok(InstallSource::Rmbx(path)),
        Some("gz")   => Ok(InstallSource::Tar(path)),
        _ => bail!(
            "Expected a directory, .tar.gz, .rmbx, or gh:username/repo — got '{}'.",
            s
        ),
    }
}

fn install_from_rmbx(rmbx_path: &PathBuf) -> Result<()> {
    if !rmbx_path.exists() {
        bail!("File not found: {}", rmbx_path.display());
    }

    let manifest = rmbx::read_manifest(rmbx_path)
        .with_context(|| format!("Failed to read manifest from {}", rmbx_path.display()))?;

    let install_dir = resolve_install_dir()?;
    let output_path = install_dir.join(&manifest.name);

    std::fs::create_dir_all(&install_dir)
        .context("Failed to create install directory")?;

    if output_path.exists() {
        std::fs::remove_dir_all(&output_path)
            .with_context(|| format!("Failed to remove existing: {}", output_path.display()))?;
    }

    rmbx::unpack(rmbx_path, &install_dir)
        .with_context(|| format!("Failed to unpack {}", rmbx_path.display()))?;

    println!(
        "Installed '{}' ({}) -> {}",
        manifest.name,
        manifest.version,
        output_path.display()
    );

    Ok(())
}

fn install_from_tar(tar_path: &PathBuf) -> Result<()> {
    if !tar_path.exists() {
        bail!("File not found: {}", tar_path.display());
    }

    let name = tar::read_name(tar_path)
        .with_context(|| format!("Failed to read carrier.toml from {}", tar_path.display()))?;

    let install_dir = resolve_install_dir()?;
    let output_path = install_dir.join(&name);

    std::fs::create_dir_all(&install_dir)
        .context("Failed to create install directory")?;

    if output_path.exists() {
        std::fs::remove_dir_all(&output_path)
            .with_context(|| format!("Failed to remove existing: {}", output_path.display()))?;
    }

    tar::unpack(tar_path, &install_dir)
        .with_context(|| format!("Failed to unpack {}", tar_path.display()))?;

    println!("Installed '{}' -> {}", name, output_path.display());

    Ok(())
}

fn install_from_github(user: &str, repo: &str) -> Result<()> {
    let url = format!("https://api.github.com/repos/{}/{}/tarball", user, repo);

    println!("Fetching {}/{}...", user, repo);

    let tmp = TempDir::new().context("Failed to create temp directory")?;
    let tarball_path = tmp.path().join("repo.tar.gz");

    download_file(&url, &tarball_path)
        .with_context(|| format!("Failed to download {}/{}", user, repo))?;

    let extract_dir = tmp.path().join("extracted");
    std::fs::create_dir_all(&extract_dir)
        .context("Failed to create extraction directory")?;

    extract_tarball(&tarball_path, &extract_dir)
        .context("Failed to extract tarball")?;

    let project_root = find_single_subdir(&extract_dir)
        .context("Could not find module directory in downloaded archive")?;

    if !project_root.join("carrier.toml").exists() {
        bail!(
            "No carrier.toml found in {}/{}. \
             This repository is not a carrier module.",
            user, repo
        );
    }

    let output_path = tmp.path().join(format!("{}.tar.gz", repo));

    crate::ops::bundle::bundle_to(&project_root, &output_path, false)
        .context("Failed to bundle downloaded module")?;

    install_from_tar(&output_path)
}

fn download_file(url: &str, dest: &PathBuf) -> Result<()> {
    let response = reqwest::blocking::Client::new()
        .get(url)
        .header("User-Agent", "carrier")
        .send()
        .with_context(|| format!("HTTP request failed: {url}"))?;

    if !response.status().is_success() {
        bail!("HTTP {} from {}", response.status(), url);
    }

    let bytes = response.bytes().context("Failed to read response bytes")?;
    std::fs::write(dest, &bytes)
        .with_context(|| format!("Failed to write to {}", dest.display()))?;

    Ok(())
}

fn extract_tarball(tarball_path: &PathBuf, dest: &PathBuf) -> Result<()> {
    let file = File::open(tarball_path)
        .with_context(|| format!("Failed to open: {}", tarball_path.display()))?;

    let gz = flate2::read::GzDecoder::new(file);
    let mut archive = TarArchive::new(gz);

    archive
        .unpack(dest)
        .with_context(|| format!("Failed to unpack to {}", dest.display()))?;

    Ok(())
}

fn find_single_subdir(dir: &PathBuf) -> Result<PathBuf> {
    let entries: Vec<_> = std::fs::read_dir(dir)
        .with_context(|| format!("Failed to read: {}", dir.display()))?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .collect();

    match entries.len() {
        0 => bail!("Extracted archive is empty"),
        1 => Ok(entries.into_iter().next().unwrap().path()),
        _ => bail!("Expected one top-level directory in archive, found multiple"),
    }
}

fn install_from_dir(project_root: &PathBuf) -> Result<()> {
    if !project_root.join("carrier.toml").exists() {
        bail!(
            "No carrier.toml found in {}. \
             Is this a carrier module project?",
            project_root.display()
        );
    }

    let tmp = TempDir::new().context("Failed to create temp directory")?;
    let output_path = tmp.path().join("module.tar.gz");

    crate::ops::bundle::bundle_to(project_root, &output_path, false)
        .context("Failed to bundle project")?;

    install_from_tar(&output_path)
}
