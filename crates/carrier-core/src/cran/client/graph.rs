use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

use anyhow::{bail, Context, Result};
use semver::Version;

use crate::cran::packages::{fetch, fetch_archive_versions, PackageRecord};
use crate::lockfile::CarrierLock;
use crate::ops::resolve::ResolvedPackage;
use crate::version::VersionSpec;

pub(super) struct ResolvedInstall {
    pub(super) version: Version,
    pub(super) repo: String,
}

pub(super) struct RepoResolution {
    pub(super) index: HashMap<String, PackageRecord>,
    pub(super) to_install: HashMap<String, ResolvedInstall>,
}

pub(super) fn topo_order(
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

/// Resolve every package (direct and transitive) needed to satisfy
/// `packages`, without downloading or installing anything. Shared by
/// `install_packages` (resolve, then install) and `resolve_packages()`
/// (resolve only what `carrier lock` calls). Packages are grouped by
/// repo so each PACKAGES.gz is fetched only once per repository.
///
/// If `lock` is `Some`, any requested package it pins is used at that
/// exact version without touching `resolve_install_set` at all. There'll be
/// no constraint solving, no archive fallback, no index-based resolution
/// for that name. A package the lock doesn't mention still resolves
/// fresh, the same way it would with no lock present. This lets a
/// newly added dependency work before the lock is re-written to cover
/// it.
pub(super) fn resolve_all(
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
                             requires '{spec}', so the lock is stale. Re-run with --write-lock \
                             (or `carrier lock`) to update it."
                        );
                    }

                    globally_resolved.insert(name.clone(), (version.clone(), repo.clone()));
                    to_install.insert(name.clone(), ResolvedInstall { version, repo: repo.clone() });
                }
                None => {
                    unlocked.insert(name.clone(), spec.clone());
                }
            }
        }

        if !unlocked.is_empty() {
            let resolved = resolve_install_set(&unlocked, &index, repo, &mut globally_resolved)?;
            for (name, r) in &resolved {
                globally_resolved.insert(name.clone(), (r.version.clone(), r.repo.clone()));
            }
            to_install.extend(resolved);
        }

        per_repo.insert(repo.clone(), RepoResolution { index, to_install });
    }

    Ok((per_repo, globally_resolved))
}

/// Walk the dep graph breadth-first, validating version specs against the
/// index and collecting the full set of packages to install.
fn resolve_install_set(
    requested: &BTreeMap<String, String>,
    index: &HashMap<String, PackageRecord>,
    repo_url: &str,
    globally_resolved: &mut HashMap<String, (Version, String)>,
) -> Result<HashMap<String, ResolvedInstall>> {
    let direct: HashSet<String> = requested.keys().cloned().collect();

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
                eprintln!(" [warn] transitive dep '{}' not found in index, skipping", pkg);
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

            // A package only reached here transitively (e.g. purrr pulling
            // in rlang) never claims it, since it's already owned by whichever
            // group resolved it as an actual direct dependency, and this
            // group's own to_install has no business installing it a
            // second time under the wrong repo. Only a group where pkg is
            // itself one of `requested` may correct a stale repo tag —
            // that's the one place carrier.toml's own declaration for
            // this package lives.
            if direct.contains(pkg) {
                resolved.insert(pkg.clone(), ResolvedInstall {
                    version: existing_version.clone(),
                    repo: repo_url.to_owned(),
                });
            }
            continue;
        }

        let pkg_specs = match pkg_specs {
            Some(s) => s,
            None => {
                resolved.insert(pkg.clone(), ResolvedInstall {
                    version: record.version.clone(),
                    repo: repo_url.to_owned(),
                });
                continue;
            }
        };

        if VersionSpec::resolve(pkg_specs, std::slice::from_ref(&record.version)).is_some() {
            resolved.insert(pkg.clone(), ResolvedInstall {
                version: record.version.clone(),
                repo: repo_url.to_owned(),
            });
            continue;
        }

        println!(" [checking] {} — index version doesn't satisfy constraints, searching archive...", pkg);
        let archive_versions = fetch_archive_versions(repo_url, pkg)
            .with_context(|| format!("fetching archive versions for '{}'", pkg))?;

        match VersionSpec::resolve(pkg_specs, &archive_versions) {
            Some(v) => {
                resolved.insert(pkg.clone(), ResolvedInstall {
                    version: v.clone(),
                    repo: repo_url.to_owned(),
                });
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
