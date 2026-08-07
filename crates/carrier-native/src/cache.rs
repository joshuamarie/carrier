use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

/// Hash the contents of `native_dir`
/// Every file's path relative to it, plus its bytes, in sorted order 
/// for determinism. This is the cache key that decides whether a 
/// previously-built artifact can be reused instead of recompiling: 
/// same source bytes, same platform, same R version => same output, 
/// so `R CMD SHLIB` never has to run twice for the same combination.
///
/// `native_dir` is the module's already-resolved `[native].path`.
/// This function has no opinion on what that directory is named or
/// where it lives relative to the module root.
///
/// Excludes any directory named `target` 
/// Cargo's own build output directory, for modules mixing in Rust 
/// (see e.g. rextendr's pattern: a Makevars-triggered `cargo build` 
/// alongside R CMD SHLIB compiling the rest). `target/`'s contents 
/// aren't deterministic between builds even with identical source, 
/// so hashing it would change the cache key on every build regardless 
/// of whether anything real changed (defeating the cache silently).
pub fn source_hash(native_dir: &Path) -> Result<String> {
    let mut entries: Vec<PathBuf> = walkdir::WalkDir::new(native_dir)
        .into_iter()
        .filter_entry(|e| e.file_name().to_str() != Some("target"))
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.path().to_path_buf())
        .collect();
    entries.sort();

    let mut hasher = Sha256::new();
    for path in entries {
        let rel = path.strip_prefix(native_dir).unwrap_or(&path);
        hasher.update(rel.to_string_lossy().as_bytes());
        let bytes = std::fs::read(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        hasher.update(&bytes);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// Root of the local native build cache: `~/.carrier/native-cache/`.
/// Artifacts are stored under
/// `<module_name>/<target_triple>/<r_version>/<source_hash>/`, so a
/// lookup is a pure path check.
/// No index file to keep in sync.
pub fn cache_dir() -> Result<PathBuf> {
    let home = dirs::home_dir()
        .context("Could not determine home directory for the native build cache")?;
    Ok(home.join(".carrier").join("native-cache"))
}
