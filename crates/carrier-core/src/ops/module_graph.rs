use std::collections::{BTreeMap, HashSet, VecDeque};

use anyhow::{bail, Context, Result};

use crate::carrier_toml::{CarrierToml, ModuleDep};
use crate::ops::resolve::{resolve_packages_from_specs, ResolvedPlan};
use crate::version::VersionSpec;

/// Fetches a module's carrier.toml given the `source` string declared
/// in its dependent's `module_deps`. resolve_transitive() only knows
/// how to walk the dependency graph. It never touches the network
/// itself, so who implements this decides that policy: a real fetch
/// over GitHub in production, a HashMap in a test.
pub trait ModuleFetcher {
    fn fetch(&self, source: &str) -> Result<CarrierToml>;
}

/// Walk the full module dependency graph, starting from `root`,
/// fetching each declared `module_dep`'s `carrier.toml` through `fetcher`
/// and folding its `package_deps/module_deps` into the same queue.
///
/// Two invariants that must hold from the first version of this
/// function, not added later: no infinite loop on a dependency cycle,
/// and no double-fetch of a module two different dependents both need.
pub fn resolve_transitive(
    root: &CarrierToml,
    fetcher: &dyn ModuleFetcher,
) -> Result<ResolvedPlan> {
    let mut pkg_specs: BTreeMap<String, Vec<VersionSpec>> = BTreeMap::new();
    let mut pkg_repos: BTreeMap<String, String> = BTreeMap::new();

    for (name, dep) in root.package_deps.as_ref().unwrap_or(&BTreeMap::new()) {
        let spec = VersionSpec::parse(dep.version())?;
        pkg_specs.entry(name.clone()).or_default().push(spec);
        pkg_repos.insert(name.clone(), dep.repo().to_owned());
    }

    let mut queue: VecDeque<(String, ModuleDep)> = root
        .module_deps
        .as_ref()
        .unwrap_or(&BTreeMap::new())
        .iter()
        .map(|(name, dep)| (name.clone(), dep.clone()))
        .collect();

    // name => (currently-in-progress marker). If we try to enqueue
    // a name that's in here, that's a cycle. It fail loudly instead of
    // looping forever (good).
    let mut in_progress: HashSet<String> = HashSet::new();

    // name => (resolved version, source it came from). Once a module
    // is finished, a second request for it is checked against this
    // instead of being fetched again.
    let mut resolved_modules: BTreeMap<String, (semver::Version, String)> = BTreeMap::new();

    while let Some((name, dep)) = queue.pop_front() {
        if let Some((existing_version, existing_source)) = resolved_modules.get(&name) {
            let source = dep.source().ok_or_else(|| {
                anyhow::anyhow!(
                    "'{name}' has no source declared — carrier has no default module registry.\n\
                     Declare it as: {name} = {{ version = \"...\", source = \"gh:user/repo\" }}"
                )
            })?;
            if source != existing_source {
                bail!(
                    "'{name}' is required from two different sources: '{}' and '{}' — \
                     carrier doesn't merge these.",
                    existing_source, source
                );
            }
            let spec = VersionSpec::parse(dep.version())?;
            if !spec.matches(existing_version) {
                bail!(
                    "'{name}' has conflicting version constraints: already resolved to {} \
                     but this dependent requires {}.",
                    existing_version, spec
                );
            }
            continue;
        }

        if in_progress.contains(&name) {
            bail!(
                "Dependency cycle detected: '{name}' depends on itself, \
                 directly or transitively."
            );
        }
        in_progress.insert(name.clone());

        let source = dep.source().ok_or_else(|| {
            anyhow::anyhow!(
                "'{name}' has no source declared — carrier has no default module registry.\n\
                 Declare it as: {name} = {{ version = \"...\", source = \"gh:user/repo\" }}"
            )
        })?;

        let fetched = fetcher
            .fetch(source)
            .with_context(|| format!("Failed to fetch module '{name}' from '{source}'"))?;

        let fetched_version = semver::Version::parse(&fetched.module.version).with_context(|| {
            format!(
                "Module '{name}' at '{source}' has an invalid version '{}'",
                fetched.module.version
            )
        })?;

        let spec = VersionSpec::parse(dep.version())?;
        if !spec.matches(&fetched_version) {
            bail!(
                "'{name}' requires version {spec} but the source at '{source}' declares {fetched_version}."
            );
        }

        for (pkg_name, pkg_dep) in fetched.package_deps.unwrap_or_default() {
            let pkg_spec = VersionSpec::parse(pkg_dep.version())?;
            pkg_specs.entry(pkg_name.clone()).or_default().push(pkg_spec);
            pkg_repos.insert(pkg_name, pkg_dep.repo().to_owned());
        }
        for (dep_name, dep_dep) in fetched.module_deps.unwrap_or_default() {
            queue.push_back((dep_name, dep_dep));
        }

        in_progress.remove(&name);
        resolved_modules.insert(name, (fetched_version, source.to_owned()));
    }

    let packages = resolve_packages_from_specs(pkg_specs, &pkg_repos)?;
    let modules = resolved_modules
        .into_iter()
        .map(|(name, (version, _source))| (name, version.to_string()))
        .collect();

    Ok(ResolvedPlan { packages, modules })
}
