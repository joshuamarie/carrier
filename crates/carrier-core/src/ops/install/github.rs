use anyhow::{bail, Context, Result};
use std::fs::File;
use std::path::PathBuf;
use tempfile::TempDir;

use ::tar::Archive as TarArchive;
use crate::carrier_toml::CarrierToml;
use crate::lockfile;
use crate::ops::module_graph::ModuleFetcher;

use super::archive::install_from_tar;

pub(super) struct GitHubSource {
    pub(super) user: String,
    pub(super) repo: String,
    pub(super) git_ref: Option<String>,
    pub(super) subpath: Option<String>,
}

pub(super) fn parse_github_source(rest: &str) -> Result<GitHubSource> {
    let parts: Vec<&str> = rest.splitn(2, '/').collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
        bail!("Invalid GitHub source. Expected: gh:username/repo or gh:username/repo/tree/branch/subpath");
    }
    let user = parts[0].to_owned();
    let remainder = parts[1];
    let repo_and_rest: Vec<&str> = remainder.splitn(2, '/').collect();
    let repo = repo_and_rest[0].to_owned();

    let (git_ref, subpath) = if repo_and_rest.len() == 2 {
        let after_repo = repo_and_rest[1];
        if let Some(rest) = after_repo.strip_prefix("tree/") {
            let mut segments = rest.splitn(2, '/');
            let git_ref = segments.next().filter(|s| !s.is_empty()).map(str::to_owned);
            let subpath = segments.next().filter(|s| !s.is_empty()).map(str::to_owned);
            (git_ref, subpath)
        } else {
            let subpath = if after_repo.is_empty() { None } else { Some(after_repo.to_owned()) };
            (None, subpath)
        }
    } else {
        (None, None)
    };

    Ok(GitHubSource { user, repo, git_ref, subpath })
}

#[cfg(feature = "network")]
pub(super) fn install_from_github(user: &str, repo: &str, git_ref: Option<&str>, subpath: Option<&str>, install_deps: bool) -> Result<()> {
    let url = match git_ref {
        Some(git_ref) => format!("https://api.github.com/repos/{}/{}/tarball/{}", user, repo, git_ref),
        None => format!("https://api.github.com/repos/{}/{}/tarball", user, repo),
    };
    match git_ref {
        Some(git_ref) => println!("Fetching {}/{}@{}...", user, repo, git_ref),
        None => println!("Fetching {}/{} (default branch)...", user, repo),
    }

    let tmp = TempDir::new().context("Failed to create temp directory")?;
    let tarball_path = tmp.path().join("repo.tar.gz");

    download_file(&url, &tarball_path)
        .with_context(|| format!("Failed to download {}/{}", user, repo))?;

    let extract_dir = tmp.path().join("extracted");
    std::fs::create_dir_all(&extract_dir)
        .context("Failed to create extraction directory")?;

    extract_tarball(&tarball_path, &extract_dir)
        .context("Failed to extract tarball")?;

    let extracted_root = find_single_subdir(&extract_dir)
        .context("Could not find module directory in downloaded archive")?;

    let project_root = match subpath {
        Some(sub) => extracted_root.join(sub),
        None => extracted_root,
    };

    if !project_root.exists() {
        match subpath {
            Some(sub) => bail!("Subpath '{}' not found in the downloaded archive", sub),
            None => bail!("Extracted archive root does not exist"),
        }
    }

    if !project_root.join("carrier.toml").exists() {
        bail!(
            "No carrier.toml found in {}/{}. \
             This repository is not a carrier module.",
            user, repo
        );
    }

    let lock = lockfile::read(&project_root).with_context(|| {
        format!("Failed to read carrier.lock in {}", project_root.display())
    })?;

    let output_path = tmp.path().join(format!("{}.tar.gz", repo));
    crate::ops::bundle::bundle_to(&project_root, &output_path)
        .context("Failed to bundle downloaded module")?;

    install_from_tar(&output_path, install_deps, lock.as_ref())
}

#[cfg(not(feature = "network"))]
pub(super) fn install_from_github(_user: &str, _repo: &str, _git_ref: Option<&str>, _subpath: Option<&str>, _install_deps: bool) -> Result<()> {
    bail!(
        "GitHub install requires the 'network' feature.\n\
         Rebuild with: cargo build --features network"
    )
}

/// The real ModuleFetcher used by resolve_transitive() outside of tests.
/// Only understands `gh:user/repo` sources today, anything else is a
/// clear error rather than a silent no-op, since a resolver that
/// pretends to resolve something it can't fetch is worse than one that
/// says so plainly.
#[cfg(feature = "network")]
pub struct GitHubFetcher;

#[cfg(feature = "network")]
impl ModuleFetcher for GitHubFetcher {
    fn fetch(&self, source: &str) -> Result<CarrierToml> {
        let rest = source.strip_prefix("gh:").ok_or_else(|| {
            anyhow::anyhow!("Unsupported module source '{source}', only gh: sources can be fetched right now.")
        })?;
        let gh = parse_github_source(rest)?;

        let url = match &gh.git_ref {
            Some(git_ref) => format!("https://api.github.com/repos/{}/{}/tarball/{}", gh.user, gh.repo, git_ref),
            None => format!("https://api.github.com/repos/{}/{}/tarball", gh.user, gh.repo),
        };

        let tmp = TempDir::new().context("Failed to create temp directory")?;
        let tarball_path = tmp.path().join("repo.tar.gz");
        download_file(&url, &tarball_path)
            .with_context(|| format!("Failed to download module source '{source}'"))?;

        let extract_dir = tmp.path().join("extracted");
        std::fs::create_dir_all(&extract_dir)
            .context("Failed to create extraction directory")?;
        extract_tarball(&tarball_path, &extract_dir)
            .context("Failed to extract tarball")?;

        let extracted_root = find_single_subdir(&extract_dir)
            .context("Could not find module directory in downloaded archive")?;

        let project_root = match &gh.subpath {
            Some(sub) => extracted_root.join(sub),
            None => extracted_root,
        };

        CarrierToml::from_dir(&project_root)
            .with_context(|| format!("Module source '{source}' does not contain a valid carrier.toml"))
    }
}

#[cfg(not(feature = "network"))]
pub struct GitHubFetcher;

#[cfg(not(feature = "network"))]
impl ModuleFetcher for GitHubFetcher {
    fn fetch(&self, _source: &str) -> Result<CarrierToml> {
        bail!(
            "GitHub module fetching requires the 'network' feature.\n\
             Rebuild with: cargo build --features network"
        )
    }
}

#[cfg(feature = "network")]
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
