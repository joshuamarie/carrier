use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::path::Path;

use anyhow::{bail, Context, Result};
// use flate2::read::GzDecoder;
use semver::Version;

use crate::cran::packages::{fetch, fetch_archive_versions, PackageRecord};
use crate::lockfile::CarrierLock;
use crate::ops::resolve::ResolvedPackage;
use crate::version::VersionSpec;
use crate::paths::{detect_r_platform, RPlatformOs};
use crate::cran::binary_install;

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

struct RepoResolution {
    index: HashMap<String, PackageRecord>,
    to_install: HashMap<String, ResolvedInstall>,
}

/// Resolve every package (direct and transitive) needed to satisfy
/// `packages`, without downloading or installing anything. Shared by
/// `install_packages` (resolve, then install) and `resolve_packages()`
/// (resolve only — what `carrier lock` calls). Packages are grouped by
/// repo so each PACKAGES.gz is fetched only once per repository.
///
/// If `lock` is `Some`, any requested package it pins is used at that
/// exact version without touching `resolve_install_set` at all. There'll be 
/// no constraint solving, no archive fallback, no index-based resolution
/// for that name. A package the lock doesn't mention still resolves
/// fresh, the same way it would with no lock present. This lets a
/// newly added dependency work before the lock is re-written to cover
/// it.
fn resolve_all(
    packages: &BTreeMap<String, ResolvedPackage>,
    lock: Option<&CarrierLock>,
) -> Result<(BTreeMap<String, RepoResolution>, HashMap<String, (Version, String)>)> {
    let mut by_repo: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    for (name, pkg) in packages {
        by_repo
            .entry(pkg.repo.clone())
            .or_default()
            .insert(name.clone(), pkg.version_spec.clone());
    }

    // Shared across all repo groups, so a package appearing as a
    // transitive dep under more than one repo isn't independently
    // re-resolved (and potentially silently downgraded) by whichever
    // repo group happens to process it last.
    let mut globally_resolved: HashMap<String, (Version, String)> = HashMap::new();
    let mut per_repo: BTreeMap<String, RepoResolution> = BTreeMap::new();

    for (repo, pkgs) in &by_repo {
        println!("Fetching package index from {}...", repo);
        let index = fetch(repo)?;

        let mut to_install: HashMap<String, ResolvedInstall> = HashMap::new();
        let mut unlocked: BTreeMap<String, String> = BTreeMap::new();

        for (name, spec) in pkgs {
            let locked = match lock {
                Some(l) => l.locked_version(name)?,
                None => None,
            };
            match locked {
                Some((version, locked_repo)) => {
                    if locked_repo != *repo {
                        bail!(
                            "carrier.lock pins '{name}' to repo {locked_repo}, but carrier.toml \
                             now points at {repo}. Re-run with --write-lock (or `carrier lock`) \
                             to update the lock, or revert carrier.toml's repo for this package."
                        );
                    }
                    let required = VersionSpec::parse(spec)?;
                    if !required.matches(&version) {
                        bail!(
                            "carrier.lock pins '{name}' to {version}, but carrier.toml now \
                             requires '{spec}' — the lock is stale. Re-run with --write-lock \
                             (or `carrier lock`) to update it."
                        );
                    }
                    globally_resolved.insert(name.clone(), (version.clone(), repo.clone()));
                    to_install.insert(name.clone(), ResolvedInstall { version });
                }
                None => {
                    unlocked.insert(name.clone(), spec.clone());
                }
            }
        }

        if !unlocked.is_empty() {
            let resolved = resolve_install_set(&unlocked, &index, repo, &mut globally_resolved)?;
            for (name, r) in &resolved {
                globally_resolved.insert(name.clone(), (r.version.clone(), repo.clone()));
            }
            to_install.extend(resolved);
        }

        per_repo.insert(repo.clone(), RepoResolution { index, to_install });
    }

    Ok((per_repo, globally_resolved))
}

/// Resolve every package (direct and transitive) to exact versions and
/// repos, without downloading or installing anything — what `carrier
/// lock` calls. Only the PACKAGES.gz indices get fetched; no individual
/// package's source or binary is ever transferred, which is what makes
/// this cheap enough to run just to check or refresh a lock.
pub fn resolve_packages(
    packages: &BTreeMap<String, ResolvedPackage>,
    lock: Option<&CarrierLock>,
) -> Result<HashMap<String, (Version, String)>> {
    let (_, globally_resolved) = resolve_all(packages, lock)?;
    Ok(globally_resolved)
}

/// Install a set of resolved R packages into `lib_path`, resolving first
/// via `resolve_all`.
///
/// Returns the full resolved set (direct and transitive) so a caller can
/// write it out as a new `carrier.lock`, minus anything that resolved
/// successfully but then failed to actually download/install as a
/// transitive dep (skipped with a warning below): a lock entry for a
/// package that isn't actually there would be worse than no entry.
pub fn install_packages(
    packages: &BTreeMap<String, ResolvedPackage>,
    lib_path: &Path,
    lock: Option<&CarrierLock>,
) -> Result<HashMap<String, (Version, String)>> {
    let (per_repo, mut globally_resolved) = resolve_all(packages, lock)?;

    std::fs::create_dir_all(lib_path)
        .with_context(|| format!("Failed to create R lib dir: {}", lib_path.display()))?;

    for (repo, RepoResolution { index, to_install }) in &per_repo {
        let order = topo_order(to_install, index);

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
                        println!("  [switching] {} {} → {}...", pkg, installed_version, resolved.version);
                    }
                    Err(_) => {
                        println!("  [reinstalling] {} (could not read installed version)...", pkg);
                    }
                }
            } else {
                println!("  [installing] {} {}...", pkg, resolved.version);
            }

            match download_and_unpack(pkg, &resolved.version.to_string(), repo, lib_path) {
                Ok(()) => {
                    println!("  [done] {} {}", pkg, resolved.version);
                }
                Err(e) => {
                    let is_direct = packages.contains_key(pkg.as_str());
                    if is_direct {
                        return Err(e.context(format!("Failed to install {}", pkg)));
                    } else {
                        eprintln!("  [warn] skipping transitive dep {} — {}", pkg, e);
                        globally_resolved.remove(pkg);
                    }
                }
            }
        }
    }

    Ok(globally_resolved)
}

/// Walk the dep graph breadth-first, validating version specs against the
/// index and collecting the full set of packages to install.
fn resolve_install_set(
    requested: &BTreeMap<String, String>,
    index: &HashMap<String, PackageRecord>,
    repo_url: &str,
    globally_resolved: &mut HashMap<String, (Version, String)>,
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
        let pkg_specs = specs.get(pkg);

        // Already resolved by an earlier repo group in this same run (don't 
        // re-resolve independently). Verify it still satisfies what
        // THIS repo's graph requires; reuse it if so, fail loudly if not.
        if let Some((existing_version, existing_repo)) = globally_resolved.get(pkg) {
            if let Some(pkg_specs) = pkg_specs {
                if VersionSpec::resolve(pkg_specs, std::slice::from_ref(existing_version)).is_none() {
                    bail!(
                        "Cross-repo version conflict for '{}': already resolved to {} via {}, \
                         but {} has constraints that version doesn't satisfy: {}",
                        pkg, existing_version, existing_repo, repo_url,
                        pkg_specs.iter().map(|s| format!("{}", s)).collect::<Vec<_>>().join(", ")
                    );
                }
            }
            resolved.insert(pkg.clone(), ResolvedInstall { version: existing_version.clone() });
            continue;
        }

        let pkg_specs = match pkg_specs {
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
            match try_install_binary(pkg, &binary_url, lib_path, &platform.arch) {
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
        RPlatformOs::MacOs => {
            // CRAN split macOS binaries by CPU architecture years ago.
            // bin/macosx/contrib (no arch) is a legacy path that some
            // mirrors still serve, populated with x86_64-only builds. Using
            // it unconditionally installs an Intel binary on an Apple
            // Silicon machine, which fails at dlopen() time, not at
            // install time. R.version$arch is "aarch64" on Apple Silicon
            // and "x86_64" on Intel; CRAN's own directory names are
            // "arm64" and "x86_64" respectively.
            let macos_arch = match platform.arch.as_str() {
                "aarch64" | "arm64" => "big-sur-arm64",
                "x86_64" => "big-sur-x86_64",
                other => {
                    eprintln!(
                        "  [warn] unrecognized macOS architecture '{}', skipping binary install for {}",
                        other, pkg
                    );
                    return None;
                }
            };
            Some(format!(
                "{}/bin/macosx/{}/contrib/{}/{}_{}.tgz",
                base, macos_arch, platform.r_version_short, pkg, version
            ))
        }
        // On Linux, CRAN has no generic binaries, always source
        RPlatformOs::Other => None,
    }
}

fn try_install_binary(pkg: &str, url: &str, lib_path: &Path, expected_arch: &str) -> Result<()> {
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

    let ext = if is_zip { "zip" } else { "tgz" };
    let mut tmp = tempfile::Builder::new()
        .suffix(&format!(".{}", ext))
        .tempfile()
        .with_context(|| format!("Failed to create temp file for {}", pkg))?;
    tmp.write_all(&bytes)
        .with_context(|| format!("Failed to write temp archive for {}", pkg))?;

    binary_install::install_binary_package(tmp.path(), lib_path, pkg, expected_arch)
}
