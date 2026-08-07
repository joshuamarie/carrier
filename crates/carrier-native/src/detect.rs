use std::path::Path;

/// Whether `native_dir`, a module's declared `[native].path`, already
/// resolved to a concrete directory, has compiled code to build.
/// Detected the same way R packages signal it: a `Makevars` or
/// `Makevars.win` file directly inside it. Deliberately filesystem-
/// only, no dependency on `carrier-core`'s TOML types, so this crate
/// can be driven directly against a bare directory — by tests, or by
/// a future `carrier build` run outside a full resolve.
///
/// Nothing here assumes a folder is named `src/`, or that it sits
/// inside a module directory at all. `native_dir` is whatever path
/// `carrier-core` already resolved from `[native].path`.
pub fn has_native_src(native_dir: &Path) -> bool {
    native_dir.join("Makevars").exists() || native_dir.join("Makevars.win").exists()
}
