use std::collections::{BTreeMap, VecDeque};
use anyhow::Result;
// use semver::Version;

use crate::carrier_toml::TomlDependencies;
// use crate::version::{check_conflicts, VersionSpec};
use crate::version::VersionSpec;
use crate::paths::{resolve_install_dir, resolve_r_lib_dir};

pub struct ResolvedPlan {
    pub packages: BTreeMap<String, String>,
    pub modules: BTreeMap<String, String>,
    pub cran_url: String,
}

/// Walk the dependency graph breadth-first, collecting all version specs
/// for each package/module across the full graph, then resolve to one
/// version per dep.
///
/// STUB: transitive module resolution is not yet implemented.
/// Only direct deps of the root are resolved. See the TODO below.
pub fn resolve(root_deps: &TomlDependencies, cran_url: &str) -> Result<ResolvedPlan> {
    let mut pkg_specs: BTreeMap<String, Vec<VersionSpec>> = BTreeMap::new();
    let mut mod_specs: BTreeMap<String, Vec<VersionSpec>> = BTreeMap::new();
    let mut queue: VecDeque<TomlDependencies> = VecDeque::new();

    queue.push_back(root_deps.clone());

    while let Some(deps) = queue.pop_front() {
        for (name, spec_str) in deps.packages.unwrap_or_default() {
            let spec = VersionSpec::parse(&spec_str)?;
            pkg_specs.entry(name).or_default().push(spec);
        }

        for (name, spec_str) in deps.modules.unwrap_or_default() {
            let spec = VersionSpec::parse(&spec_str)?;
            mod_specs.entry(name).or_default().push(spec);

            // TODO: look up `name` in the local install dir or a registry cache,
            // parse its carrier.toml, and push its [dependencies] onto the queue.
            // Without this, transitive deps of carrier modules are silently ignored.
        }
    }

    // No registry yet — conflict checking requires real candidate version lists.
    // Uncomment and adapt once CRAN/registry index fetching is in place:
    //
    // for (name, specs) in &pkg_specs {
    //     let candidates = registry::fetch_versions(name)?;
    //     check_conflicts(name, &specs, &candidates)?;
    // }

    // let packages = pkg_specs.into_keys().map(|n| (n, "latest".into())).collect();
    let packages = pkg_specs
        .into_iter()
        .map(|(name, specs)| {
            let spec_str = specs.first()
                .map(|s| format!("{}", s))
                .unwrap_or_else(|| "*".to_owned());
            (name, spec_str)
        })
        .collect();
    let modules = mod_specs.into_keys().map(|n| (n, "latest".into())).collect();

    Ok(ResolvedPlan { packages, modules, cran_url: cran_url.to_owned() })
}

/// Pretty-print the resolved plan to stdout.
pub fn print_plan(plan: &ResolvedPlan) {
    if plan.packages.is_empty() && plan.modules.is_empty() {
        println!("  No dependencies.");
        return;
    }
    if !plan.packages.is_empty() {
        println!("  R packages:");
        for (name, ver) in &plan.packages {
            println!("    {} ({})", name, ver);
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

pub fn execute_plan(plan: &ResolvedPlan, dry_run: bool) -> Result<()> {
    // --- R packages ---
    if !plan.packages.is_empty() {
        if dry_run {
            println!("  Would install R packages (pass --install-deps to proceed):");
            for (name, spec) in &plan.packages {
                println!("    {} ({})", name, spec);
            }
        } else {
            let r_lib = resolve_r_lib_dir()?;
            println!("  Installing R packages into {}...", r_lib.display());
            crate::cran::client::install_packages(&plan.packages, &plan.cran_url, &r_lib)?;
        }
    }

    // --- carrier modules ---
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
    Ok(())
}
