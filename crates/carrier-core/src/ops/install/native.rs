use anyhow::{bail, Context, Result};
use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::carrier_toml::PackageDep;
use crate::ops::resolve;

/// Compiles a module's native code, if it has any, right after
/// unpacking. `module_path` is the module's own flat installed
/// directory (`<install_dir>/<n>`). Native code isn't assumed to
/// live in one blessed spot, `find_native_dirs` walks the whole
/// installed tree, so a module can have several compiled-code dirs
/// nested under different submodules.
///
/// Detection is purely filesystem-based (`has_native_src` via
/// `find_native_dirs`), not keyed off the manifest's `native` field.
///
/// Gated behind `install_deps`: `[native].build_deps` are resolved
/// and installed here, separately from `[package_deps]` and skipping
/// `carrier.lock` since they're compile-time-only, not a runtime
/// contract.
pub(super) fn build_native_if_present(
    module_path: &PathBuf,
    name: &str,
    install_deps: bool,
    native: Option<&crate::manifest::NativeManifest>,
) -> Result<()> {
    let native_dirs: Vec<PathBuf> = match native.map(|n| n.declared_dirs.as_slice()) {
        Some(paths) if !paths.is_empty() => {
            let mut dirs = Vec::with_capacity(paths.len());
            for p in paths {
                let dir = module_path.join(p);
                if !dir.is_dir() {
                    bail!(
                        "Manifest declares native path '{}' for '{}', but it does not exist after unpacking. \
                         The archive may be corrupted or out of date with its own manifest.",
                        p, name
                    );
                }
                dirs.push(dir);
            }
            dirs
        }
        _ => carrier_native::detect::find_native_dirs(module_path),
    };

    if native_dirs.is_empty() {
        return Ok(());
    }

    if !install_deps {
        println!(
            " [native] {} has compiled code, build with: carrier install --install-deps",
            name
        );
        return Ok(());
    }

    let build_deps: Option<BTreeMap<String, PackageDep>> = native
        .map(|n| &n.build_deps)
        .filter(|deps| !deps.is_empty())
        .map(|deps| {
            deps.iter()
                .map(|entry| {
                    let dep = match &entry.repo {
                        Some(repo) => PackageDep::Extended { version: entry.version.clone(), repo: Some(repo.clone()) },
                        None => PackageDep::Simple(entry.version.clone()),
                    };
                    (entry.name.clone(), dep)
                })
                .collect()
        });

    if let Some(deps) = build_deps {
        println!("  Installing native build deps for '{}'...", name);
        let plan = resolve::resolve(&Some(deps), &None)?;
        resolve::print_plan(&plan);
        resolve::execute_plan(&plan, false, None)?;
    }

    let mut cleared_lib_dirs: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();

    for native_dir in &native_dirs {
        let target_dir = native_dir.parent().unwrap_or(module_path.as_path());
        let lib_dir = target_dir.join(".lib");
        if cleared_lib_dirs.insert(lib_dir.clone()) && lib_dir.exists() {
            std::fs::remove_dir_all(&lib_dir)
                .with_context(|| format!("Failed to clear {}", lib_dir.display()))?;
        }

        let binary_name = crate::ops::compile::binary_name(native_dir, name);

        println!("Building native code for '{}' ({})...", name, native_dir.display());
        let outcome = carrier_native::build(target_dir, native_dir, binary_name, name)
            .with_context(|| format!("Failed to build native code for '{}' at {}", name, native_dir.display()))?;

        println!(
            " built: {} ({})",
            outcome.artifact_path.display(),
            if outcome.from_cache { "cached" } else { "compiled" }
        );

        std::fs::remove_dir_all(native_dir)
            .with_context(|| format!("Failed to remove native source at {}", native_dir.display()))?;
    }

    Ok(())
}
