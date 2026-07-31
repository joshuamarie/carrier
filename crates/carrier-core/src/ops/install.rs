use anyhow::{bail, Context, Result};
use std::fs::File;
use std::path::PathBuf;
use tempfile::TempDir;

use ::tar::Archive as TarArchive;
use crate::carrier_toml::PackageDep;
use crate::formats::{rmbx, tar};
use crate::ops::resolve;
use crate::paths::resolve_install_dir;

enum InstallSource {
    Rmbx(PathBuf),
    Tar(PathBuf),
    Dir(PathBuf),
    GitHub { user: String, repo: String, subpath: Option<String> },
}

pub fn run(source: &str, install_deps: bool) -> Result<()> {
    match parse_source(source)? {
        InstallSource::Rmbx(path) => install_from_rmbx(&path, install_deps),
        InstallSource::Tar(path) => install_from_tar(&path, install_deps),
        InstallSource::Dir(path) => install_from_dir(&path, install_deps),
        InstallSource::GitHub { user, repo, subpath } => {
            install_from_github(&user, &repo, subpath.as_deref(), install_deps)
        }
    }
}

struct GitHubSource {
    user: String,
    repo: String,
    subpath: Option<String>,
}

fn parse_github_source(rest: &str) -> Result<GitHubSource> {
    let parts: Vec<&str> = rest.splitn(2, '/').collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
        bail!("Invalid GitHub source. Expected: gh:username/repo or gh:username/repo/tree/branch/subpath");
    }
    let user = parts[0].to_owned();
    let remainder = parts[1];
    let repo_and_rest: Vec<&str> = remainder.splitn(2, '/').collect();
    let repo = repo_and_rest[0].to_owned();

    let subpath = if repo_and_rest.len() == 2 {
        let after_repo = repo_and_rest[1];
        let subpath = if let Some(s) = after_repo.strip_prefix("tree/") {
            s.splitn(2, '/').nth(1).unwrap_or("").to_owned()
        } else {
            after_repo.to_owned()
        };
        if subpath.is_empty() { None } else { Some(subpath) }
    } else {
        None
    };

    Ok(GitHubSource { user, repo, subpath })
}

fn parse_source(s: &str) -> Result<InstallSource> {
    let path = PathBuf::from(s);
    if path.is_dir() {
        return Ok(InstallSource::Dir(path));
    }

    if let Some(rest) = s.strip_prefix("gh:") {
        let gh = parse_github_source(rest)?;
        return Ok(InstallSource::GitHub {
            user: gh.user,
            repo: gh.repo,
            subpath: gh.subpath,
        });
    }

    match path.extension().and_then(|e| e.to_str()) {
        Some("rmbx") => Ok(InstallSource::Rmbx(path)),
        Some("gz") => Ok(InstallSource::Tar(path)),
        _ => bail!(
            "Expected a directory, .tar.gz, .rmbx, or gh:username/repo — got '{}'.",
            s
        ),
    }
}

fn install_from_rmbx(rmbx_path: &PathBuf, install_deps: bool) -> Result<()> {
    if !rmbx_path.exists() {
        bail!("File not found: {}", rmbx_path.display());
    }

    let manifest = rmbx::read_manifest(rmbx_path)
        .with_context(|| format!("Failed to read manifest from {}", rmbx_path.display()))?;

    let name = manifest.name.clone();
    let version = manifest.version.clone();
    let install_dir = resolve_install_dir()?;
    let module_path = install_dir.join(&name);

    std::fs::create_dir_all(&install_dir)
        .context("Failed to create install directory")?;

    if module_path.exists() {
        std::fs::remove_dir_all(&module_path)
            .with_context(|| format!("Failed to remove existing: {}", module_path.display()))?;
    }

    // Also clean up old dist-info if present
    let dist_info = install_dir.join(format!("{}-{}.dist-info", name, version));
    if dist_info.exists() {
        std::fs::remove_dir_all(&dist_info)?;
    }

    rmbx::unpack(rmbx_path, &install_dir)
        .with_context(|| format!("Failed to unpack {}", rmbx_path.display()))?;

    println!(
        "Installed '{}' ({}) -> {}",
        name, version, module_path.display()
    );

    let package_deps = Some(
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
    );

    let plan = resolve::resolve(&package_deps, &None)?;
    println!("Dependencies:");
    resolve::print_plan(&plan);
    resolve::execute_plan(&plan, !install_deps)?;

    Ok(())
}

fn install_from_tar(tar_path: &PathBuf, install_deps: bool) -> Result<()> {
    if !tar_path.exists() {
        bail!("File not found: {}", tar_path.display());
    }

    let toml = tar::read_toml(tar_path)
        .with_context(|| format!("Failed to read manifest from {}", tar_path.display()))?;
    let name = toml.module.name.clone();
    let version = toml.module.version.clone();

    let install_dir = resolve_install_dir()?;
    let module_path = install_dir.join(&name);

    std::fs::create_dir_all(&install_dir)
        .context("Failed to create install directory")?;

    if module_path.exists() {
        std::fs::remove_dir_all(&module_path)
            .with_context(|| format!("Failed to remove existing: {}", module_path.display()))?;
    }

    // Also clean up old dist-info if present
    let dist_info = install_dir.join(format!("{}-{}.dist-info", name, version));
    if dist_info.exists() {
        std::fs::remove_dir_all(&dist_info)?;
    }

    tar::unpack(tar_path, &install_dir, &name, &version)
        .with_context(|| format!("Failed to unpack {}", tar_path.display()))?;

    println!("Installed '{}' ({}) -> {}", name, version, module_path.display());

    let plan = resolve::resolve(&toml.package_deps, &toml.module_deps)?;
    println!("Dependencies:");
    resolve::print_plan(&plan);
    resolve::execute_plan(&plan, !install_deps)?;

    Ok(())
}

fn install_from_dir(project_root: &PathBuf, install_deps: bool) -> Result<()> {
    if !project_root.join("carrier.toml").exists() {
        bail!(
            "No carrier.toml found in {}. Is this a carrier module project?",
            project_root.display()
        );
    }

    let tmp = TempDir::new().context("Failed to create temp directory")?;
    let output_path = tmp.path().join("module.tar.gz");

    crate::ops::bundle::bundle_to(project_root, &output_path, false)
        .context("Failed to bundle project")?;

    install_from_tar(&output_path, install_deps)
}

#[cfg(feature = "network")]
fn install_from_github(user: &str, repo: &str, subpath: Option<&str>, install_deps: bool) -> Result<()> {
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

    let output_path = tmp.path().join(format!("{}.tar.gz", repo));
    crate::ops::bundle::bundle_to(&project_root, &output_path, false)
        .context("Failed to bundle downloaded module")?;

    install_from_tar(&output_path, install_deps)
}

#[cfg(not(feature = "network"))]
fn install_from_github(_user: &str, _repo: &str, _subpath: Option<&str>, _install_deps: bool) -> Result<()> {
    bail!(
        "GitHub install requires the 'network' feature.\n\
         Rebuild with: cargo build --features network"
    )
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
