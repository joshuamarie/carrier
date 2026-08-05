use anyhow::{Context, Result};
use std::path::Path;

use crate::carrier_toml::CarrierToml;
use crate::lockfile;
use crate::ops::resolve;

/// Resolve `path`'s R package dependencies to exact versions and repos,
/// then write `carrier.lock`. With `update: true`, any existing lock is
/// ignored and everything is re-resolved fresh; otherwise packages the
/// lock already pins are kept at their pinned version (see
/// `resolve_only` / `resolve_all`'s locked-package handling).
pub fn run(path: &str, update: bool) -> Result<()> {
    let project_root = Path::new(path);

    let toml_path = project_root.join("carrier.toml");
    let contents = std::fs::read_to_string(&toml_path)
        .with_context(|| format!("Failed to read {}", toml_path.display()))?;
    let toml: CarrierToml = ::toml::from_str(&contents)
        .with_context(|| format!("Failed to parse {}", toml_path.display()))?;

    let existing = if update { None } else { lockfile::read(project_root)? };

    let plan = resolve::resolve(&toml.package_deps, &toml.module_deps)?;
    let resolved = resolve::resolve_only(&plan, existing.as_ref())?;

    lockfile::write(project_root, &resolved.into_iter().collect())?;
    println!("Wrote {} ({} packages)", lockfile::LOCK_FILE_NAME, plan_len(&plan));

    Ok(())
}

fn plan_len(plan: &resolve::ResolvedPlan) -> usize {
    plan.packages.len()
}
