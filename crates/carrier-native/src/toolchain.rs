use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

use crate::cache::{cache_dir, source_hash};

#[derive(Debug)]
pub struct BuildOutcome {
    /// Where the compiled artifact ended up —
    /// `<module_dir>/<module_name><dynlib_ext>`, next to `__init__.R`,
    /// since `box::file()` resolves relative to whichever module
    /// calls it, not wherever `[native].path` points. `module_dir`
    /// and `module_name` here mean whichever directory owns this
    /// particular native dir — a submodule's own dir and name when a
    /// module has several scattered compiled-code dirs, not
    /// necessarily the top-level module.
    pub artifact_path: PathBuf,
    pub source_hash: String,
    /// `R.version$platform`, e.g. `x86_64-pc-linux-gnu`. Not a real
    /// Rust target triple (R doesn't expose one directly) but it's
    /// exactly the granularity ABI compatibility actually depends on
    /// here, so it doubles as one for cache-keying purposes.
    pub target_triple: String,
    pub r_version: String,
    pub from_cache: bool,
}

/// Compile the sources under `native_dir` via `R CMD SHLIB`, mirroring
/// the same conventions worked out by hand earlier:
///   - output named `<module_name><dynlib_ext>` (not derived from the
///     source filenames. `R CMD SHLIB -o` requires an explicit name
///     whenever there's more than one source file)
///   - moved from `native_dir` up to `module_dir` after compiling
///   - `.o` object files discarded afterward, not needed at runtime
///
/// `native_dir` and `module_dir` are deliberately separate parameters.
/// `native_dir` is wherever a compiled-code dir resolves to. This
/// function has no opinion on what that directory is called or where
/// it sits relative to the module root, or which module it even
/// belongs to. A caller building several scattered native dirs for
/// one module passes a different `module_dir`/`module_name` pair per
/// dir, so each artifact lands next to its own owning submodule's
/// `__init__.R`, since that's where `box::file()` looks for it.
///
/// Checks the local build cache first, keyed by source hash + platform
/// + R version + `cache_key_name`. A hit just copies the cached
/// artifact into place and skips invoking the compiler entirely.
///
/// `cache_key_name` is deliberately a separate parameter from
/// `module_name`. `module_name` names the compiled artifact file
/// (`<module_name><dynlib_ext>`) and can be a native dir's own folder
/// name (e.g. `cpp`) when a module has several scattered native dirs.
/// `cache_key_name` should be the owning module's actual name, so two
/// unrelated modules that both happen to name their native folder
/// `cpp` don't share a cache bucket.
pub fn build(module_dir: &Path, native_dir: &Path, module_name: &str, cache_key_name: &str) -> Result<BuildOutcome> {
    if !crate::detect::has_native_src(native_dir) {
        bail!(
            "No Makevars or .c/.cpp/.cc/.cxx sources found in {} — nothing to compile.",
            native_dir.display()
        );
    }

    let ext = run_rscript_capture("cat(.Platform$dynlib.ext)")?;
    let target_triple = run_rscript_capture("cat(R.version$platform)")?;
    let r_version = run_rscript_capture(
        "cat(paste(R.version$major, strsplit(R.version$minor, '.', fixed = TRUE)[[1]][1], sep = '.'))",
    )?;
    let hash = source_hash(native_dir)?;

    let lib_name = format!("{module_name}{ext}");
    let lib_dir = module_dir.join("lib");
    std::fs::create_dir_all(&lib_dir)
        .with_context(|| format!("Failed to create {}", lib_dir.display()))?;
    let artifact_path = lib_dir.join(&lib_name);

    let cached_path = cache_dir()?
        .join(cache_key_name)
        .join(&target_triple)
        .join(&r_version)
        .join(&hash)
        .join(&lib_name);

    if cached_path.exists() {
        std::fs::copy(&cached_path, &artifact_path).with_context(|| {
            format!(
                "Failed to copy cached artifact from {} to {}",
                cached_path.display(),
                artifact_path.display()
            )
        })?;
        return Ok(BuildOutcome {
            artifact_path,
            source_hash: hash,
            target_triple,
            r_version,
            from_cache: true,
        });
    }

    let sources = native_sources(native_dir)?;
    if sources.is_empty() {
        bail!(
            "Makevars found but no .c/.cpp/.cc/.cxx sources in {}",
            native_dir.display()
        );
    }

    let status = Command::new(locate_binary("R"))
        .arg("CMD")
        .arg("SHLIB")
        .arg("-o")
        .arg(&lib_name)
        .args(&sources)
        .current_dir(native_dir)
        .status()
        .with_context(|| format!("Failed to run 'R CMD SHLIB' in {}", native_dir.display()))?;

    if !status.success() {
        bail!(
            "R CMD SHLIB failed for module '{module_name}' (exit code {:?}).\n\
             Check that a C/C++ toolchain is installed and on PATH:\n\
             \x20\x20Linux:   r-base-dev (or your distro's equivalent)\n\
             \x20\x20macOS:   Xcode Command Line Tools (xcode-select --install)\n\
             \x20\x20Windows: Rtools, matching your R version",
            status.code()
        );
    }

    let built_path = native_dir.join(&lib_name);
    std::fs::rename(&built_path, &artifact_path).with_context(|| {
        format!(
            "Failed to move built artifact from {} to {}",
            built_path.display(),
            artifact_path.display()
        )
    })?;

    for entry in walkdir::WalkDir::new(native_dir)
        .into_iter()
        .filter_entry(|e| e.file_name().to_str() != Some("target"))
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().and_then(|ext| ext.to_str()) == Some("o"))
    {
        std::fs::remove_file(entry.path())?;
    }

    if let Some(parent) = cached_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create cache directory {}", parent.display()))?;
    }
    std::fs::copy(&artifact_path, &cached_path).with_context(|| {
        format!("Failed to populate build cache at {}", cached_path.display())
    })?;

    Ok(BuildOutcome {
        artifact_path,
        source_hash: hash,
        target_triple,
        r_version,
        from_cache: false,
    })
}

/// Sources under `native_dir`, recursively. This matches `source_hash()`'s
/// own recursion exactly. These two walking the tree differently was
/// a real bug: the cache key could change for a nested source file
/// that R CMD SHLIB was never actually told to compile, leaving the
/// cache and the compiled artifact silently out of sync.
///
/// `target/` excluded Cargo's own build directory for modules mixing
/// in Rust, same as s`ource_hash()`. No C/C++ source would ever
/// legitimately live there, so recursing into it is pure waste at
/// best and a correctness risk at worst.
fn native_sources(native_dir: &Path) -> Result<Vec<String>> {
    let mut entries: Vec<PathBuf> = walkdir::WalkDir::new(native_dir)
        .into_iter()
        .filter_entry(|e| e.file_name().to_str() != Some("target"))
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.path().to_path_buf())
        .collect();
    entries.sort();

    let mut out = Vec::new();
    for path in entries {
        if matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("c") | Some("cpp") | Some("cc") | Some("cxx")
        ) {
            let rel = path.strip_prefix(native_dir).unwrap_or(&path);
            out.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(out)
}

/// Prefer `$R_HOME/bin/<name>` when `R_HOME` is set (matches how R
/// itself locates its own satellite binaries); otherwise fall back to
/// bare `<name>` and let `Command` resolve it via `PATH`. No early
/// existence probe beyond the `R_HOME` case, letting the actual
/// `Command::status()`/`output()` call fail is simpler than adding a
/// platform-specific `which`/`where` dependency just to check first.
fn locate_binary(name: &str) -> String {
    if let Ok(r_home) = std::env::var("R_HOME") {
        let candidate = Path::new(&r_home).join("bin").join(name);
        if candidate.exists() {
            return candidate.to_string_lossy().into_owned();
        }
    }
    name.to_owned()
}

fn run_rscript_capture(expr: &str) -> Result<String> {
    let rscript = locate_binary("Rscript");
    let output = Command::new(&rscript)
        .arg("-e")
        .arg(expr)
        .output()
        .with_context(|| format!("Failed to run '{rscript} -e \"{expr}\"' - is R installed and on PATH?"))?;

    if !output.status.success() {
        bail!(
            "'{}' failed: {}",
            rscript,
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}
