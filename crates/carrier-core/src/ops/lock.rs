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
///
/// `with_r_version` controls whether the detected R version is recorded
/// in the lock as provenance (see `lockfile::write`'s doc comment). This is
/// off by default so routine re-locks across contributors on different
/// R installs stay diff-quiet.
///
/// `remove: true` safely deletes `carrier.lock` instead of writing one, and
/// returns before any resolution happens. This is always safe: a
/// missing lock is not an error state anywhere in carrier. Every
/// caller (`ops::install`, `ops::bundle`) already treats "no lock" as
/// "resolve fresh", exactly as if `carrier lock` had never been run.
pub fn run(path: &str, update: bool, with_r_version: bool, remove: bool) -> Result<()> {
    let project_root = Path::new(path);

    if remove {
        let lock_path = project_root.join(lockfile::LOCK_FILE_NAME);
        if lock_path.exists() {
            std::fs::remove_file(&lock_path)
                .with_context(|| format!("Failed to remove {}", lock_path.display()))?;
            println!("Removed {}", lockfile::LOCK_FILE_NAME);
        } else {
            println!("No {} present, nothing to remove.", lockfile::LOCK_FILE_NAME);
        }
        return Ok(());
    }

    let toml_path = project_root.join("carrier.toml");
    let contents = std::fs::read_to_string(&toml_path)
        .with_context(|| format!("Failed to read {}", toml_path.display()))?;
    let toml: CarrierToml = ::toml::from_str(&contents)
        .with_context(|| format!("Failed to parse {}", toml_path.display()))?;

    let r_spec = toml.module.r_version_spec()?;
    crate::version::check_r_version(&r_spec)?;
    let detected = crate::paths::detect_r_version()?;

    let existing = if update { None } else { lockfile::read(project_root)? };

    let plan = resolve::resolve(&toml.package_deps, &toml.module_deps)?;
    let resolved = resolve::resolve_only(&plan, existing.as_ref())?;

    let r_version = if with_r_version { Some(detected.to_string()) } else { None };
    lockfile::write(project_root, &resolved.into_iter().collect(), r_version.as_deref())?;
    println!("Wrote {} ({} packages)", lockfile::LOCK_FILE_NAME, plan_len(&plan));

    Ok(())
}

fn plan_len(plan: &resolve::ResolvedPlan) -> usize {
    plan.packages.len()
}
