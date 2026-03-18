use anyhow::{bail, Context, Result};
use std::fs::File;
use std::path::PathBuf;
use tempfile::TempDir;

use crate::formats::rmbx;

pub struct InstallArgs {
    pub source: String,
    pub force:  bool,
    pub global: bool,
}

enum InstallSource {
    Rmbx(PathBuf),
    GitHub { user: String, repo: String },
}

pub fn run(args: InstallArgs) -> Result<()> {
    let source = parse_source(&args.source)?;

    match source {
        InstallSource::Rmbx(path) => {
            install_from_rmbx(&path, args.force, args.global)
        }
        InstallSource::GitHub { user, repo } => {
            install_from_github(&user, &repo, args.force, args.global)
        }
    }
}

fn parse_source(s: &str) -> Result<InstallSource> {
    if let Some(rest) = s.strip_prefix("gh:") {
        let parts: Vec<&str> = rest.splitn(2, '/').collect();
        if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
            bail!("Invalid GitHub source '{}'. Expected: gh:username/repo", s);
        }
        Ok(InstallSource::GitHub {
            user: parts[0].to_owned(),
            repo: parts[1].to_owned(),
        })
    } else {
        let path = PathBuf::from(s);
        match path.extension().and_then(|e| e.to_str()) {
            Some("rmbx") => Ok(InstallSource::Rmbx(path)),
            _ => bail!(
                "Expected a .rmbx file or gh:username/repo, got '{}'.\n\
                 To bundle your own module, use `carrier bundle` instead.",
                s
            ),
        }
    }
}

fn resolve_mod_dir(global: bool) -> Result<PathBuf> {
    if global {
        Ok(dirs::home_dir()
            .context("Cannot find home directory")?
            .join(".carrier")
            .join("modules"))
    } else {
        Ok(std::env::current_dir()
            .context("Failed to get current working directory")?
            .join(".mod"))
    }
}

fn install_from_rmbx(rmbx_path: &PathBuf, force: bool, global: bool) -> Result<()> {
    if !rmbx_path.exists() {
        bail!("File not found: {}", rmbx_path.display());
    }

    let manifest = rmbx::read_manifest(rmbx_path)
        .with_context(|| format!("Failed to read manifest from {}", rmbx_path.display()))?;

    let mod_dir = resolve_mod_dir(global)?;
    let output_path = mod_dir.join(&manifest.name);

    if output_path.exists() && !force {
        bail!(
            "Module '{}' is already installed. Use --force to reinstall.",
            manifest.name
        );
    }

    if output_path.exists() {
        std::fs::remove_dir_all(&output_path)
            .with_context(|| format!("Failed to remove existing: {}", output_path.display()))?;
    }

    std::fs::create_dir_all(&output_path)
        .context("Failed to create install directory")?;

    rmbx::unpack(rmbx_path, &output_path)
        .with_context(|| format!("Failed to unpack {}", rmbx_path.display()))?;

    println!("Installed '{}' -> {}", manifest.name, output_path.display());

    Ok(())
}

fn install_from_github(user: &str, repo: &str, force: bool, global: bool) -> Result<()> {
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

    let module_path = find_single_subdir(&extract_dir)
        .context("Could not find module directory in downloaded archive")?;

    if !module_path.join("carrier.toml").exists() {
        bail!(
            "No carrier.toml found in {}/{}. \
             This repository is not a carrier module.",
            user, repo
        );
    }

    let rmbx_path = tmp.path().join(format!("{}.rmbx", repo));

    crate::commands::bundle::bundle_to(&module_path, &rmbx_path)
        .context("Failed to bundle downloaded module")?;

    install_from_rmbx(&rmbx_path, force, global)
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
    let mut archive = tar::Archive::new(gz);

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
