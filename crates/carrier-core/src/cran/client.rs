mod download;
mod graph;

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use anyhow::{Context, Result};
use semver::Version;

use crate::lockfile::CarrierLock;
use crate::ops::resolve::ResolvedPackage;

use download::download_and_unpack;
use graph::{resolve_all, topo_order, RepoResolution};

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

    for (_repo, RepoResolution { index, to_install }) in &per_repo {
        let order = topo_order(to_install, index);

        for pkg in &order {
            let resolved = &to_install[pkg];
            let pkg_dir = lib_path.join(pkg);

            if pkg_dir.is_dir() {
                let desc_path = pkg_dir.join("DESCRIPTION");
                match read_installed_version(&desc_path) {
                    Ok(installed_version) => {
                        if installed_version == resolved.version {
                            println!(" [ok] {} {} (already satisfied)", pkg, installed_version);
                            continue;
                        }
                        println!(" [switching] {} {} → {}...", pkg, installed_version, resolved.version);
                    }
                    Err(_) => {
                        println!(" [reinstalling] {} (could not read installed version)...", pkg);
                    }
                }
            } else {
                println!(" [installing] {} {}...", pkg, resolved.version);
            }

            match download_and_unpack(pkg, &resolved.version.to_string(), &resolved.repo, lib_path) {
                Ok(()) => {
                    println!(" [done] {} {}", pkg, resolved.version);
                }
                Err(e) => {
                    let is_direct = packages.contains_key(pkg.as_str());
                    if is_direct {
                        return Err(e.context(format!("Failed to install {}", pkg)));
                    } else {
                        eprintln!(" [warn] skipping transitive dep {} — {}", pkg, e);
                        globally_resolved.remove(pkg);
                    }
                }
            }
        }
    }

    Ok(globally_resolved)
}

/// Read the installed version of a package from its `DESCRIPTION` file.
///
/// `pub(crate)`: reused by `ops/compile.rs`'s local-satisfied check,
/// so it can't drift from what `install_packages` itself considers
/// "already satisfied".
pub(crate) fn read_installed_version(desc_path: &Path) -> Result<Version> {
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
