use std::path::Path;

/// Whether `native_dir`, a module's declared `[native].path`, already
/// resolved to a concrete directory, has compiled code to build.
/// Detected the same way R packages signal it: a `Makevars` or
/// `Makevars.win` file directly inside it. Deliberately filesystem-
/// only, no dependency on `carrier-core`'s TOML types, so this crate
/// can be driven directly against a bare directory, by tests, or by
/// a future `carrier build` run outside a full resolve.
///
/// Nothing here assumes a folder is named `src/`, or that it sits
/// inside a module directory at all. `native_dir` is whatever path
/// `carrier-core` already resolved from `[native].path`.
pub fn has_native_src(native_dir: &Path) -> bool {
    native_dir.join("Makevars").exists() || native_dir.join("Makevars.win").exists()
}

/// Every directory under `root` that itself qualifies via
/// `has_native_src`. New patches involving compiled code no longer has 
/// to live in one blessed location. Any nested directory with its own
/// `Makevars`/`Makevars.win` is its own independent compilation unit.
/// `target/` is skipped so a Rust-mixed module's `cargo build` output
/// is never mistaken for a second native dir. Sorted for deterministic
/// build order.
pub fn find_native_dirs(root: &Path) -> Vec<std::path::PathBuf> {
    let mut dirs: Vec<std::path::PathBuf> = walkdir::WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| e.file_name().to_str() != Some("target"))
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_dir())
        .map(|e| e.path().to_owned())
        .filter(|d| has_native_src(d))
        .collect();
    dirs.sort();
    dirs
}
