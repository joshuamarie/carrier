use std::path::Path;

/// Whether `native_dir` has compiled code to build. A `Makevars` or
/// `Makevars.win` file is enough on its own, that's still the
/// deliberate signal for "this dir wants custom compile flags." But a
/// directory with no Makevars still counts if it has actual
/// `.c`/`.cpp`/`.cc`/`.cxx` sources: `R CMD SHLIB` compiles those fine
/// with default flags, and a hand-authored native dir shouldn't be
/// invisible to carrier just for skipping a file it doesn't strictly
/// need. Deliberately filesystem-only, no dependency on
/// `carrier-core`'s TOML types, so this crate can be driven directly
/// against a bare directory, by tests, or by a future `carrier compile`
/// run outside a full resolve.
///
/// Nothing here assumes a folder is named `src/`, or that it sits
/// inside a module directory at all. `native_dir` is whatever path
/// `carrier-core` already resolved from `[native].path`.
pub fn has_native_src(native_dir: &Path) -> bool {
    if native_dir.join("Makevars").exists() || native_dir.join("Makevars.win").exists() {
        return true;
    }
    std::fs::read_dir(native_dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .any(|e| {
            matches!(
                e.path().extension().and_then(|ext| ext.to_str()),
                Some("c") | Some("cpp") | Some("cc") | Some("cxx")
                    | Some("f") | Some("f90") | Some("f95") | Some("f03")
            )
        })
}

/// Every directory under `root` that itself qualifies via
/// `has_native_src`. New patches involving compiled code no longer has
/// to live in one blessed location. A nested directory with its own
/// Makevars or native sources is its own independent compilation unit
/// — but only when it isn't already inside one: once a directory
/// qualifies, its own subdirectories are part of that same
/// compilation unit (`R CMD SHLIB` recurses into them on its own) and
/// are never inspected as candidates in their own right. `target/` is
/// skipped so a Rust-mixed module's `cargo build` output is never
/// mistaken for a native dir. `WalkDir` visits a directory before its
/// children, so `skip_current_dir` is enough to prune both cases.
/// Sorted for deterministic build order.
pub fn find_native_dirs(root: &Path) -> Vec<std::path::PathBuf> {
    let mut dirs = Vec::new();
    let mut walker = walkdir::WalkDir::new(root).into_iter();

    loop {
        let entry = match walker.next() {
            Some(Ok(e)) => e,
            Some(Err(_)) => continue,
            None => break,
        };

        if entry.file_name().to_str() == Some("target") {
            walker.skip_current_dir();
            continue;
        }

        if !entry.file_type().is_dir() {
            continue;
        }

        if has_native_src(entry.path()) {
            dirs.push(entry.path().to_owned());
            walker.skip_current_dir();
        }
    }

    dirs.sort();
    dirs
}
