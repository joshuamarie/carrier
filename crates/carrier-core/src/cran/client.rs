use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::path::Path;

use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use semver::Version;

use crate::cran::packages::{fetch, PackageRecord};
use crate::ops::resolve::ResolvedPackage;
use crate::version::{check_conflicts, VersionSpec};

/// Install a set of resolved R packages into `lib_path`.
///
/// Packages are grouped by repo so each PACKAGES.gz is fetched only once
/// per repository.
pub fn install_packages(
    packages: &BTreeMap<String, ResolvedPackage>,
    lib_path: &Path,
) -> Result<()> {
    // Group packages by repo
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

        for (pkg, record) in &to_install {
            let pkg_dir = lib_path.join(pkg);

            if pkg_dir.is_dir() {
                let desc_path = pkg_dir.join("DESCRIPTION");
                match read_installed_version(&desc_path) {
                    Ok(installed_version) => {
                        let spec_str = pkgs
                            .get(pkg.as_str())
                            .map(|s| s.as_str())
                            .unwrap_or("*");
                        let spec = VersionSpec::parse(spec_str)?;

                        if spec.matches(&installed_version) {
                            println!(
                                "  [ok] {} {} (already satisfied)",
                                pkg, installed_version
                            );
                            continue;
                        }

                        println!(
                            "  [upgrading] {} {} → {}...",
                            pkg, installed_version, record.version
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
                println!("  [installing] {} {}...", pkg, record.version);
            }

            match download_and_unpack(pkg, &record.version.to_string(), repo, lib_path) {
                Ok(()) => println!("  [done] {} {}", pkg, record.version),
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
fn resolve_install_set<'a>(
    requested: &BTreeMap<String, String>,
    index: &'a HashMap<String, PackageRecord>,
    repo_url: &str,
) -> Result<HashMap<String, &'a PackageRecord>> {
    let mut result: HashMap<String, &PackageRecord> = HashMap::new();
    let mut visited: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<String> = VecDeque::new();
    let mut specs: HashMap<String, Vec<VersionSpec>> = HashMap::new();

    for (pkg, spec_str) in requested {
        specs.entry(pkg.clone()).or_default().push(VersionSpec::parse(spec_str)?);
        queue.push_back(pkg.clone());
    }

    // First pass: walk the full graph, collecting every spec placed
    // on each package before checking any of them.
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

        result.insert(pkg.clone(), record);

        for (dep, dep_spec_str) in &record.deps {
            if let Ok(dep_spec) = VersionSpec::parse(dep_spec_str) {
                specs.entry(dep.clone()).or_default().push(dep_spec);
            }
            if !visited.contains(dep) {
                queue.push_back(dep.clone());
            }
        }
    }

    // Second pass: every package now has its complete spec list, so
    // conflicts between direct and transitive requirers are caught here.
    for (pkg, record) in &result {
        if let Some(pkg_specs) = specs.get(pkg) {
            check_conflicts(pkg, pkg_specs, std::slice::from_ref(&record.version))
                .with_context(|| format!("resolving '{}' from {}", pkg, repo_url))?;
        }
    }

    Ok(result)
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
    let primary_url = format!(
        "{}/src/contrib/{}_{}.tar.gz",
        repo_url.trim_end_matches('/'),
        pkg,
        version
    );

    let response = reqwest::blocking::get(&primary_url)
        .with_context(|| format!("Failed to download {}", primary_url))?;

    let response = if response.status() == reqwest::StatusCode::NOT_FOUND {
        // Try CRAN archive path first
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

        // If archive also 404s and this looks like r-universe, try the .9000
        // dev version suffix that r-universe uses
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

    let gz = GzDecoder::new(bytes.as_ref());
    let mut archive = tar::Archive::new(gz);

    archive.unpack(lib_path)
        .with_context(|| format!("Failed to unpack {} into {}", pkg, lib_path.display()))?;

    Ok(())
}
