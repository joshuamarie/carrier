use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::path::Path;

use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use semver::Version;

use crate::cran::packages::{fetch, PackageRecord};
use crate::version::VersionSpec;

/// Install a set of R packages (and their transitive deps) from a
/// CRAN-like repository into `lib_path`.
///
/// `requested` maps package name → version spec string from `carrier.toml`
/// (e.g. `"dplyr" → ">=1.1.0"`).
pub fn install_packages(
    requested: &BTreeMap<String, String>,
    repo_url: &str,
    lib_path: &Path,
) -> Result<()> {
    println!("Fetching package index from {}...", repo_url);
    let index = fetch(repo_url)?;

    // Resolve the full transitive install set
    let to_install = resolve_install_set(requested, &index, repo_url)?;

    std::fs::create_dir_all(lib_path)
        .with_context(|| format!("Failed to create R lib dir: {}", lib_path.display()))?;

    for (pkg, record) in &to_install {
        let pkg_dir = lib_path.join(pkg);

        if pkg_dir.is_dir() {
            let desc_path = pkg_dir.join("DESCRIPTION");
            match read_installed_version(&desc_path) {
                Ok(installed_version) => {
                    // Look up the spec for this package — direct deps have an
                    // explicit spec; transitive deps default to "*" (any version).
                    let spec_str = requested
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
                    // DESCRIPTION missing or unparseable — reinstall to be safe
                    println!("  [reinstalling] {} (could not read installed version)...", pkg);
                }
            }
        } else {
            println!("  [installing] {} {}...", pkg, record.version);
        }

        download_and_unpack(pkg, &record.version.to_string(), repo_url, lib_path)
            .with_context(|| format!("Failed to install {}", pkg))?;

        println!("  [done] {} {}", pkg, record.version);
    }

    Ok(())
}

/// Walk the dep graph breadth-first, validating version specs against the
/// CRAN index and collecting the full set of packages to install.
fn resolve_install_set<'a>(
    requested: &BTreeMap<String, String>,
    index: &'a HashMap<String, PackageRecord>,
    repo_url: &str,
) -> Result<HashMap<String, &'a PackageRecord>> {
    let mut result: HashMap<String, &PackageRecord> = HashMap::new();
    let mut visited: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<String> = VecDeque::new();

    // Seed queue with direct deps, validating specs immediately
    for (pkg, spec_str) in requested {
        let spec = VersionSpec::parse(spec_str)?;
        let record = index.get(pkg.as_str()).with_context(|| {
            format!("Package '{}' not found in index at {}", pkg, repo_url)
        })?;
        if !spec.matches(&record.version) {
            anyhow::bail!(
                "Version conflict: '{}' requires {} but CRAN has {}",
                pkg, spec_str, record.version
            );
        }
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
                // Transitive dep missing from index — warn and skip rather
                // than hard-fail, since some packages list base deps that
                // aren't in PACKAGES.gz.
                eprintln!("  [warn] transitive dep '{}' not found in index, skipping", pkg);
                continue;
            }
        };

        result.insert(pkg.clone(), record);

        for dep in &record.deps {
            if !visited.contains(dep) {
                queue.push_back(dep.clone());
            }
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
    let url = format!(
        "{}/src/contrib/{}_{}.tar.gz",
        repo_url.trim_end_matches('/'),
        pkg,
        version
    );

    let response = reqwest::blocking::get(&url)
        .with_context(|| format!("Failed to download {}", url))?;

    if !response.status().is_success() {
        anyhow::bail!("HTTP {} downloading {}", response.status(), url);
    }

    let bytes = response.bytes()
        .with_context(|| format!("Failed to read bytes for {}", pkg))?;

    let gz = GzDecoder::new(bytes.as_ref());
    let mut archive = tar::Archive::new(gz);

    archive.unpack(lib_path)
        .with_context(|| format!("Failed to unpack {} into {}", pkg, lib_path.display()))?;

    Ok(())
}
