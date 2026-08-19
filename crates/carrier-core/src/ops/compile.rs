use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

use crate::carrier_toml::CarrierToml;

/// Compile a module's native code in place, directly into its own
/// source directory. This tries to mirror `devtools::load_all()`'s
/// convention of building `src/*.so` right where the source lives, for fast
/// dev-loop iteration against a running R session.
///
/// Unlike `install`'s `build_native_if_present`, this never deletes
/// the native source dir afterward: the whole point of `carrier
/// compile` is to keep iterating on that source. Each artifact lands at
/// `<native_dir_parent>/lib/<native_dir_name><dynlib_ext>`, exactly
/// where `box::file()` in the scaffolded hook already expects it, so
/// no change to `r_glue.rs` is required.
///
/// Uses `resolve_native_dirs()`, not a raw scan, so an explicit
/// `[native].path`/`paths` override in carrier.toml is respected the
/// same way `install` and `bundle` already respect it.
///
/// Excluding this dev-built `lib/` from a plain source bundle is a
/// separate, still-open concern in `formats/tar.rs` and `formats/rmbx.rs`
/// (not handled here).
pub fn run(project_root: &Path) -> Result<Vec<CompiledArtifact>> {
    if !project_root.join("carrier.toml").exists() {
        bail!(
            "No carrier.toml found in {}. Is this a carrier module project?",
            project_root.display()
        );
    }

    let toml = CarrierToml::from_dir(project_root)?;
    let name = toml.module.name.clone();
    let native_dirs = toml.resolve_native_dirs(project_root)?;

    if native_dirs.is_empty() {
        return Ok(Vec::new());
    }

    let mut compiled = Vec::new();
    for native_dir in &native_dirs {
        if !native_dir.is_dir() {
            bail!(
                "Configured native dir '{}' does not exist.",
                native_dir.display()
            );
        }

        let target_dir = native_dir.parent().unwrap_or(project_root);
        let artifact_name = native_dir
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or(&name);

        let outcome = carrier_native::build(target_dir, native_dir, artifact_name, &name)
            .with_context(|| format!("Failed to compile native code for '{}' at {}", name, native_dir.display()))?;

        compiled.push(CompiledArtifact {
            native_dir: native_dir.clone(),
            artifact_path: outcome.artifact_path,
            target_triple: outcome.target_triple,
            r_version: outcome.r_version,
            source_hash: outcome.source_hash,
            from_cache: outcome.from_cache,
        });
    }

    Ok(compiled)
}

pub struct CompiledArtifact {
    pub native_dir: PathBuf,
    pub artifact_path: PathBuf,
    pub target_triple: String,
    pub r_version: String,
    pub source_hash: String,
    pub from_cache: bool,
}
