use std::collections::BTreeMap;

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
/// fetching each declared module_dep's carrier.toml through `fetcher`
/// and folding its `package_deps/module_deps` into the same queue.
///
/// Two invariants that must hold from the first version of this
/// function, not added later: no infinite loop on a dependency cycle,
/// and no double-fetch of a module two different dependents both need.
///
/// This is DFS with an explicit ancestor path, not BFS with a queue,
/// deliberately. A "seen before" set alone can't distinguish a real
/// cycle (A depends on B depends on A) from a legitimate diamond (X
/// and Y both depend on the same Z): both look identical as "this name
/// showed up twice." Only the ancestor chain, who is currently
/// resolving whom — tells them apart. A queue has no notion of
/// ancestry; a call stack does, which is why this walks the graph
/// recursively with `path` instead of draining a VecDeque.
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

    // name => (resolved version, source it came from). A module already
    // in here is finished. A second request for it is checked for
    // consistency instead of being fetched again. This is what makes a
    // diamond dependency cheap: the second path to reach it is a lookup,
    // not a re-fetch.
    let mut resolved_modules: BTreeMap<String, (semver::Version, String)> = BTreeMap::new();

    // The chain of module names currently being resolved, root to leaf.
    // If resolving some module's deps leads back to a name already on
    // this path, that's a genuine cycle. The graph has no valid order
    // to fetch these in.
    let mut path: Vec<String> = Vec::new();

    for (name, dep) in root.module_deps.as_ref().unwrap_or(&BTreeMap::new()) {
        resolve_module(
            name,
            dep,
            fetcher,
            &mut resolved_modules,
            &mut path,
            &mut pkg_specs,
            &mut pkg_repos,
        )?;
    }

    let packages = resolve_packages_from_specs(pkg_specs, &pkg_repos)?;
    let modules = resolved_modules
        .into_iter()
        .map(|(name, (version, _source))| (name, version.to_string()))
        .collect();

    Ok(ResolvedPlan { packages, modules })
}

/// Resolve one module dep and recurse into its own module_deps.
/// Package deps discovered along the way are folded into `pkg_specs`/
/// `pkg_repos` for the caller to finalize once the whole graph is walked.
fn resolve_module(
    name: &str,
    dep: &ModuleDep,
    fetcher: &dyn ModuleFetcher,
    resolved_modules: &mut BTreeMap<String, (semver::Version, String)>,
    path: &mut Vec<String>,
    pkg_specs: &mut BTreeMap<String, Vec<VersionSpec>>,
    pkg_repos: &mut BTreeMap<String, String>,
) -> Result<()> {
    if let Some((existing_version, existing_source)) = resolved_modules.get(name) {
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
        return Ok(());
    }

    if path.iter().any(|ancestor| ancestor == name) {
        let mut chain = path.clone();
        chain.push(name.to_owned());
        bail!("Dependency cycle detected: {}", chain.join(" -> "));
    }

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

    path.push(name.to_owned());
    for (dep_name, dep_dep) in fetched.module_deps.unwrap_or_default() {
        resolve_module(&dep_name, &dep_dep, fetcher, resolved_modules, path, pkg_specs, pkg_repos)?;
    }
    path.pop();

    resolved_modules.insert(name.to_owned(), (fetched_version, source.to_owned()));
    Ok(())
}
