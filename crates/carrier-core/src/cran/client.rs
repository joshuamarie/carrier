use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::path::Path;

use anyhow::{bail, Context, Result};
// use flate2::read::GzDecoder;
use semver::Version;

use crate::cran::packages::{fetch, fetch_archive_versions, PackageRecord};
use crate::ops::resolve::ResolvedPackage;
use crate::version::VersionSpec;
use crate::paths::{detect_r_platform, RPlatformOs};

use std::io::Write as _;

fn topo_order(
    to_install: &HashMap<String, ResolvedInstall>,
    index: &HashMap<String, PackageRecord>,
) -> Vec<String> {
    fn visit(
        pkg: &str,
        to_install: &HashMap<String, ResolvedInstall>,
        index: &HashMap<String, PackageRecord>,
        visited: &mut HashSet<String>,
        order: &mut Vec<String>,
    ) {
        if visited.contains(pkg) {
            return;
        }
        visited.insert(pkg.to_owned());
        if to_install.contains_key(pkg) {
            if let Some(record) = index.get(pkg) {
                for (dep, _) in &record.deps {
                    visit(dep, to_install, index, visited, order);
                }
            }
            order.push(pkg.to_owned());
        }
    }

    let mut visited = HashSet::new();
    let mut order = Vec::new();
    for pkg in to_install.keys() {
        visit(pkg, to_install, index, &mut visited, &mut order);
    }
    order
}

struct ResolvedInstall {
    version: Version,
}

/// Install a set of resolved R packages into `lib_path`.
///
/// Packages are grouped by repo so each PACKAGES.gz is fetched only once
/// per repository.
pub fn install_packages(
    packages: &BTreeMap<String, ResolvedPackage>,
    lib_path: &Path,
) -> Result<()> {
    let mut by_repo: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    for (name, pkg) in packages {
        by_repo
            .entry(pkg.repo.clone())
            .or_default()
            .insert(name.clone(), pkg.version_spec.clone());
    }

    std::fs::create_dir_all(lib_path)
        .with_context(|| format!("Failed to create R lib dir: {}", lib_path.display()))?;

    for (repo, pkgs) in &by_repo {
        println!("Fetching package index from {}...", repo);
        let index = fetch(repo)?;
        let to_install = resolve_install_set(pkgs, &index, repo)?;
        let order = topo_order(&to_install, &index);

        for pkg in &order {
            let resolved = &to_install[pkg];
            let pkg_dir = lib_path.join(pkg);

            if pkg_dir.is_dir() {
                let desc_path = pkg_dir.join("DESCRIPTION");
                match read_installed_version(&desc_path) {
                    Ok(installed_version) => {
                        if installed_version == resolved.version {
                            println!("  [ok] {} {} (already satisfied)", pkg, installed_version);
                            continue;
                        }
                        println!(
                            "  [switching] {} {} → {}...",
                            pkg, installed_version, resolved.version
                        );
                    }
                    Err(_) => {
                        println!(
                            "  [reinstalling] {} (could not read installed version)...",
                            pkg
                        );
                    }
                }
            } else {
                println!("  [installing] {} {}...", pkg, resolved.version);
            }

            match download_and_unpack(pkg, &resolved.version.to_string(), repo, lib_path) {
                Ok(()) => println!("  [done] {} {}", pkg, resolved.version),
                Err(e) => {
                    let is_direct = pkgs.contains_key(pkg.as_str());
                    if is_direct {
                        return Err(e.context(format!("Failed to install {}", pkg)));
                    } else {
                        eprintln!("  [warn] skipping transitive dep {} — {}", pkg, e);
                    }
                }
            }
        }
    }

    Ok(())
}

/// Walk the dep graph breadth-first, validating version specs against the
/// index and collecting the full set of packages to install.
fn resolve_install_set(
    requested: &BTreeMap<String, String>,
    index: &HashMap<String, PackageRecord>,
    repo_url: &str,
) -> Result<HashMap<String, ResolvedInstall>> {
    let mut result: HashMap<String, ()> = HashMap::new();
    let mut visited: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<String> = VecDeque::new();
    let mut specs: HashMap<String, Vec<VersionSpec>> = HashMap::new();

    for (pkg, spec_str) in requested {
        specs.entry(pkg.clone()).or_default().push(VersionSpec::parse(spec_str)?);
        queue.push_back(pkg.clone());
    }

    while let Some(pkg) = queue.pop_front() {
        if visited.contains(&pkg) {
            continue;
        }
        visited.insert(pkg.clone());

        let record = match index.get(pkg.as_str()) {
            Some(r) => r,
            None => {
                eprintln!("  [warn] transitive dep '{}' not found in index, skipping", pkg);
                continue;
            }
        };

        result.insert(pkg.clone(), ());

        for (dep, dep_spec_str) in &record.deps {
            if let Ok(dep_spec) = VersionSpec::parse(dep_spec_str) {
                specs.entry(dep.clone()).or_default().push(dep_spec);
            }
            if !visited.contains(dep) {
                queue.push_back(dep.clone());
            }
        }
    }

    let mut resolved: HashMap<String, ResolvedInstall> = HashMap::new();

    for pkg in result.keys() {
        let record = &index[pkg];
        let pkg_specs = match specs.get(pkg) {
            Some(s) => s,
            None => {
                resolved.insert(pkg.clone(), ResolvedInstall { version: record.version.clone() });
                continue;
            }
        };

        if VersionSpec::resolve(pkg_specs, std::slice::from_ref(&record.version)).is_some() {
            resolved.insert(pkg.clone(), ResolvedInstall { version: record.version.clone() });
            continue;
        }

        println!("  [checking] {} — index version doesn't satisfy constraints, searching archive...", pkg);
        let archive_versions = fetch_archive_versions(repo_url, pkg)
            .with_context(|| format!("fetching archive versions for '{}'", pkg))?;

        match VersionSpec::resolve(pkg_specs, &archive_versions) {
            Some(v) => {
                resolved.insert(pkg.clone(), ResolvedInstall { version: v.clone() });
            }
            None => {
                bail!(
                    "Version conflict for '{}': no version (including archive) satisfies all constraints.\n\
                     Constraints: {}",
                    pkg,
                    pkg_specs.iter().map(|s| format!("{}", s)).collect::<Vec<_>>().join(", ")
                );
            }
        }
    }

    Ok(resolved)
}

/// Read the installed version of a package from its `DESCRIPTION` file.
fn read_installed_version(desc_path: &Path) -> Result<Version> {
    let content = std::fs::read_to_string(desc_path)
        .with_context(|| format!("Failed to read DESCRIPTION at {}", desc_path.display()))?;

    for line in content.lines() {
        if let Some(ver_str) = line.strip_prefix("Version:") {
            let normalized = ver_str.trim().replace('-', ".");
            return Version::parse(&normalized)
                .with_context(|| format!("Failed to parse installed version: {}", ver_str.trim()));
        }
    }

    anyhow::bail!(
        "No Version field found in DESCRIPTION at {}",
        desc_path.display()
    )
}

/// Download `{repo}/src/contrib/{pkg}_{ver}.tar.gz` and extract it
/// directly into `lib_path`, so `lib_path/{pkg}/` is the result.
fn download_and_unpack(
    pkg: &str,
    version: &str,
    repo_url: &str,
    lib_path: &Path,
) -> Result<()> {
    let platform = detect_r_platform();
    
    if let Ok(platform) = &platform {
        if let Some(binary_url) = binary_url_for(pkg, version, repo_url, platform) {
            match try_install_binary(pkg, &binary_url, lib_path) {
                Ok(()) => {
                    println!("  [binary] {} {} (no compilation needed)", pkg, version);
                    return Ok(());
                }
                Err(e) => {
                    eprintln!("  [warn] binary install failed for {} ({}), falling back to source...", pkg, e);
                }
            }
        }
    }
    
    let primary_url = format!(
        "{}/src/contrib/{}_{}.tar.gz",
        repo_url.trim_end_matches('/'),
        pkg,
        version
    );

    let response = reqwest::blocking::get(&primary_url)
        .with_context(|| format!("Failed to download {}", primary_url))?;

    let response = if response.status() == reqwest::StatusCode::NOT_FOUND {
        let archive_url = format!(
            "{}/src/contrib/Archive/{}/{}_{}.tar.gz",
            repo_url.trim_end_matches('/'),
            pkg,
            pkg,
            version
        );
        eprintln!("  [warn] {} not in src/contrib, trying archive...", pkg);
        let archive_resp = reqwest::blocking::get(&archive_url)
            .with_context(|| format!("Failed to download {}", archive_url))?;

        if archive_resp.status() == reqwest::StatusCode::NOT_FOUND {
            let dev_url = format!(
                "{}/src/contrib/{}_{}.9000.tar.gz",
                repo_url.trim_end_matches('/'),
                pkg,
                version
            );
            eprintln!("  [warn] {} not in archive, trying dev version...", pkg);
            reqwest::blocking::get(&dev_url)
                .with_context(|| format!("Failed to download {}", dev_url))?
        } else {
            archive_resp
        }
    } else {
        response
    };

    if !response.status().is_success() {
        anyhow::bail!("HTTP {} downloading {} {}", response.status(), pkg, version);
    }

    let bytes = response.bytes()
        .with_context(|| format!("Failed to read bytes for {}", pkg))?;

    let mut tmp = tempfile::Builder::new()
        .suffix(".tar.gz")
        .tempfile()
        .with_context(|| format!("Failed to create temp file for {}", pkg))?;
    tmp.write_all(&bytes)
        .with_context(|| format!("Failed to write temp tarball for {}", pkg))?;
    let tmp_path = tmp.path();

    std::fs::create_dir_all(lib_path)
        .with_context(|| format!("Failed to create lib dir: {}", lib_path.display()))?;

    let lib_arg = format!(
        "--library={}",
        lib_path.to_str().context("lib_path contains invalid UTF-8")?
    );

    let status = std::process::Command::new("R")
        .args(["CMD", "INSTALL", "--no-multiarch", "--no-docs", "--no-help", &lib_arg])
        .arg(tmp_path)
        .status()
        .with_context(|| format!("Failed to run R CMD INSTALL for {} — is R on PATH?", pkg))?;

    if !status.success() {
        anyhow::bail!("R CMD INSTALL failed for {} {} (exit: {})", pkg, version, status);
    }

    Ok(())
}

fn binary_url_for(pkg: &str, version: &str, repo_url: &str, platform: &crate::paths::RPlatform) -> Option<String> {
    let base = repo_url.trim_end_matches('/');
    match platform.os {
        RPlatformOs::Windows => Some(format!(
            "{}/bin/windows/contrib/{}/{}_{}.zip",
            base, platform.r_version_short, pkg, version
        )),
        RPlatformOs::MacOs => Some(format!(
            "{}/bin/macosx/contrib/{}/{}_{}.tgz",
            base, platform.r_version_short, pkg, version
        )),
        // On Linux, CRAN has no generic binaries, always source
        RPlatformOs::Other => None, 
    }
}

fn try_install_binary(pkg: &str, url: &str, lib_path: &Path) -> Result<()> {
    let response = reqwest::blocking::get(url)
        .with_context(|| format!("Failed to download binary: {}", url))?;

    if !response.status().is_success() {
        anyhow::bail!("HTTP {} downloading binary {}", response.status(), url);
    }

    let bytes = response.bytes()
        .with_context(|| format!("Failed to read binary bytes for {}", pkg))?;

    let is_zip = bytes.starts_with(b"PK");
    let is_gzip = bytes.len() >= 2 && bytes[0] == 0x1f && bytes[1] == 0x8b;
    if !is_zip && !is_gzip {
        anyhow::bail!(
            "Response for {} was not a valid archive (likely no binary available for this version)",
            pkg
        );
    }

    // Written to a directory carrier controls, not the OS temp dir —
    // files freshly written to AppData\Local\Temp on Windows can be
    // locked/scanned by antivirus before R CMD INSTALL reads them,
    // causing an intermittent "cannot open compressed file" failure.
    let scratch_dir = dirs::home_dir()
        .context("Cannot find home directory")?
        .join(".carrier")
        .join("tmp");
    std::fs::create_dir_all(&scratch_dir)
        .with_context(|| format!("Failed to create scratch dir: {}", scratch_dir.display()))?;

    let ext = if url.ends_with(".zip") { "zip" } else { "tgz" };
    let scratch_path = scratch_dir.join(format!("{}.{}", pkg, ext));

    std::fs::write(&scratch_path, &bytes)
        .with_context(|| format!("Failed to write binary for {}", pkg))?;

    std::fs::create_dir_all(lib_path)
        .with_context(|| format!("Failed to create lib dir: {}", lib_path.display()))?;

    let lib_arg = format!(
        "--library={}",
        lib_path.to_str().context("lib_path contains invalid UTF-8")?
    );

    let status = std::process::Command::new("R")
        .args(["CMD", "INSTALL", &lib_arg])
        .arg(&scratch_path)
        .status()
        .with_context(|| format!("Failed to run R CMD INSTALL for binary {} — is R on PATH?", pkg))?;

    let _ = std::fs::remove_file(&scratch_path);

    if !status.success() {
        anyhow::bail!("R CMD INSTALL failed for binary {} (exit: {})", pkg, status);
    }

    Ok(())
}
