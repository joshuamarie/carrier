use std::path::PathBuf;

use anyhow::{bail, Result};

use crate::carrier_toml::CarrierToml;
use crate::ops::resolve;

/// Resolve a project's `package_deps` to exact versions and write
/// `carrier.lock`, without installing anything.
///
/// By default, reuses an existing lock's pins where they're still valid
/// (via the same lock-aware path `resolve_packages` already uses for
/// `carrier install --write-lock`), so re-running this with no real
/// change to `carrier.toml` produces the same file. Pass `update = true`
/// to ignore the existing lock entirely and re-resolve everything fresh
/// — the equivalent of `poetry lock --no-cache` / `cargo update`.
pub fn run(path: &str, update: bool) -> Result<()> {
    let project_root = PathBuf::from(path);
    if !project_root.join("carrier.toml").exists() {
        bail!(
            "No carrier.toml found in {}. Is this a carrier module project?",
            project_root.display()
        );
    }

    let toml = CarrierToml::from_dir(&project_root)?;
    let plan = resolve::resolve(&toml.package_deps, &toml.module_deps)?;

    if plan.packages.is_empty() {
        println!("  No R package dependencies to lock.");
        return Ok(());
    }

    let existing_lock = if update {
        None
    } else {
        crate::lockfile::read(&project_root)?
    };

    let resolved = resolve::resolve_only(&plan, existing_lock.as_ref())?;

    if resolved.is_empty() {
        println!("  Nothing resolved — {} not written.", crate::lockfile::LOCK_FILE_NAME);
        return Ok(());
    }

    let count = resolved.len();
    let locked: std::collections::BTreeMap<_, _> = resolved.into_iter().collect();
    crate::lockfile::write(&project_root, &locked)?;
    println!(
        "  Wrote {} ({} package{})",
        crate::lockfile::LOCK_FILE_NAME,
        count,
        if count == 1 { "" } else { "s" }
    );

    Ok(())
}
