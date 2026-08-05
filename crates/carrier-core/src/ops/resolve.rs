use std::collections::BTreeMap;
use anyhow::{bail, Result};

use crate::carrier_toml::{PackageDep, DEFAULT_CRAN_MIRROR};
use crate::version::VersionSpec;
use crate::paths::{resolve_install_dir, resolve_r_lib_dir};

/// A resolved package dep — version spec plus the repo it comes from.
#[derive(Debug, Clone)]
pub struct ResolvedPackage {
    pub version_spec: String,
    pub repo: String,
}

pub struct ResolvedPlan {
    /// package name => resolved package (spec + repo)
    pub packages: BTreeMap<String, ResolvedPackage>,
    /// module name => version spec
    pub modules: BTreeMap<String, String>,
}

/// Walk the dependency graph breadth-first, collecting all version specs
/// for each package/module across the full graph, then resolve to one
/// version per dep.
///
/// STUB: transitive module resolution is not yet implemented.
pub fn resolve(
    package_deps: &Option<BTreeMap<String, PackageDep>>,
    module_deps: &Option<BTreeMap<String, String>>,
) -> Result<ResolvedPlan> {
    let mut pkg_specs: BTreeMap<String, Vec<VersionSpec>> = BTreeMap::new();
    let mut pkg_repos: BTreeMap<String, String> = BTreeMap::new();
    let mut mod_specs: BTreeMap<String, Vec<VersionSpec>> = BTreeMap::new();

    for (name, dep) in package_deps.as_ref().unwrap_or(&BTreeMap::new()) {
        let spec = VersionSpec::parse(dep.version())?;
        pkg_specs.entry(name.clone()).or_default().push(spec);
        // Last writer wins for repo 
        // Fine since duplicate deps are unusual
        pkg_repos.insert(name.clone(), dep.repo().to_owned());
    }

    for (name, spec_str) in module_deps.as_ref().unwrap_or(&BTreeMap::new()) {
        let spec = VersionSpec::parse(spec_str)?;
        mod_specs.entry(name.clone()).or_default().push(spec);

        // TODO: load the installed module's carrier.toml and push its
        // deps onto the queue for transitive resolution.
    }

    let mut packages = BTreeMap::new();
    for (name, specs) in pkg_specs {
        // Only one spec per name is possible today. `package_deps` is a
        // BTreeMap, so a duplicate key already collapses upstream during
        // TOML parsing. This stays a hard error rather than a silent
        // pick-the-first once transitive module resolution (the TODO
        // above) starts pushing a second spec for the same package: a
        // real conflict should surface here, not resolve to whichever
        // constraint happened to arrive first.
        let version_spec = match specs.as_slice() {
            [] => "*".to_owned(),
            [only] => format!("{only}"),
            multiple => {
                // NOTE: this branch can't be exercised by a test today. 
                // `package_deps` is a BTreeMap, so a duplicate name can
                // never actually reach `pkg_specs` with more than one
                // entry. It only matters once transitive module
                // resolution (the TODO above) can push a second spec for
                // the same package. Add a real test for it the same day
                // that lands (don't let this gap outlive the reason
                // for it).
                let constraints: Vec<String> = multiple.iter().map(|s| s.to_string()).collect();
                bail!(
                    "'{name}' has {} conflicting version constraints ({}) — \
                     carrier doesn't merge these yet.",
                    multiple.len(),
                    constraints.join(", ")
                );
            }
        };
        let repo = pkg_repos
            .get(&name)
            .cloned()
            .unwrap_or_else(|| DEFAULT_CRAN_MIRROR.to_owned());
        packages.insert(name, ResolvedPackage { version_spec, repo });
    }

    let modules = mod_specs
        .into_keys()
        .map(|n| (n, "latest".to_owned()))
        .collect();

    Ok(ResolvedPlan { packages, modules })
}

/// Pretty-print the resolved plan to stdout.
pub fn print_plan(plan: &ResolvedPlan) {
    if plan.packages.is_empty() && plan.modules.is_empty() {
        println!("  No dependencies.");
        return;
    }
    if !plan.packages.is_empty() {
        println!("  R packages:");
        for (name, pkg) in &plan.packages {
            if pkg.repo == DEFAULT_CRAN_MIRROR {
                println!("    {} ({})", name, pkg.version_spec);
            } else {
                println!("    {} ({}) [{}]", name, pkg.version_spec, pkg.repo);
            }
        }
    }
    if !plan.modules.is_empty() {
        println!("  carrier modules:");
        for (name, ver) in &plan.modules {
            println!("    {} ({})", name, ver);
        }
    }
}

pub fn already_installed_module(name: &str) -> Result<bool> {
    let install_dir = resolve_install_dir()?;
    Ok(install_dir.join(name).is_dir())
}

/// Resolve a plan's R packages to exact versions and repos without
/// installing anything — what `carrier lock` calls. Module deps aren't
/// included: there's no automatic resolve+install path for them yet
/// (see the TODO in `resolve()` above), so there's nothing concrete to
/// pin for a module the same way there is for a package.
pub fn resolve_only(
    plan: &ResolvedPlan,
    lock: Option<&crate::lockfile::CarrierLock>,
) -> Result<std::collections::HashMap<String, (semver::Version, String)>> {
    if plan.packages.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    crate::cran::client::resolve_packages(&plan.packages, lock)
}

/// Runs the plan. On a real (non-dry-run) install, returns everything
/// that was resolved for R packages (direct and transitive) so the
/// caller can write it out as a fresh `carrier.lock` if asked to. A
/// dry run or a plan with no packages returns an empty map; there is
/// nothing yet to lock.
pub fn execute_plan(
    plan: &ResolvedPlan,
    dry_run: bool,
    lock: Option<&crate::lockfile::CarrierLock>,
) -> Result<std::collections::HashMap<String, (semver::Version, String)>> {
    let mut resolved = std::collections::HashMap::new();

    if !plan.packages.is_empty() {
        if dry_run {
            println!("  Would install R packages (pass --install-deps to proceed):");
            for (name, pkg) in &plan.packages {
                println!("    {} ({})", name, pkg.version_spec);
            }
        } else {
            let r_lib = resolve_r_lib_dir()?;
            println!("  Installing R packages into {}...", r_lib.display());
            resolved = crate::cran::client::install_packages(&plan.packages, &r_lib, lock)?;
        }
    }

    for (name, _spec) in &plan.modules {
        if already_installed_module(name)? {
            println!("  [ok] {} (already installed)", name);
        } else {
            println!(
                "  [missing] {} — install with: carrier install <path or gh:user/repo>",
                name
            );
        }
    }

    Ok(resolved)
}
