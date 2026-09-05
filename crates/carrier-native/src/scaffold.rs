use std::path::Path;

use anyhow::{Context, Result};

use crate::lang::{Backend, NativeLang};
use crate::templates::{c, cpp, r_glue};

struct Template {
    hook: &'static str,
    hello: &'static str,
    add: &'static str,
    hello_r: &'static str,
    add_r: &'static str,
    makevars: &'static str,
    src_ext: &'static str,
}

fn template_for(lang: NativeLang, backend: Option<Backend>) -> Result<Template> {
    match lang {
        NativeLang::C => Ok(Template {
            hook: r_glue::HOOK,
            hello: c::HELLO,
            add: c::ADD,
            hello_r: r_glue::HELLO,
            add_r: r_glue::ADD,
            makevars: c::MAKEVARS,
            src_ext: c::SRC_EXT,
        }),
        NativeLang::Cpp => Ok(match backend.unwrap_or_default() {
            Backend::Rcpp => Template {
                hook: r_glue::HOOK_RCPP,
                hello: cpp::rcpp::HELLO,
                add: cpp::rcpp::ADD,
                hello_r: r_glue::HELLO,
                add_r: r_glue::ADD,
                makevars: cpp::rcpp::MAKEVARS,
                src_ext: cpp::SRC_EXT,
            },
            Backend::Cpp11 => Template {
                hook: r_glue::HOOK,
                hello: cpp::cpp11::HELLO,
                add: cpp::cpp11::ADD,
                hello_r: r_glue::HELLO,
                add_r: r_glue::ADD,
                makevars: cpp::cpp11::MAKEVARS,
                src_ext: cpp::SRC_EXT,
            },
        }),
        NativeLang::Fortran => anyhow::bail!(
            "Fortran scaffolding isn't supported yet, as the build pipeline doesn't compile `*.f` sources as of current version."
        ),
    }
}

/// Scaffold a module's R-only example code — the same hello/add shape
/// `scaffold()` writes for native modules, minus anything compiled: no
/// native dir, no hook.r, no dyn.load. Function bodies live entirely
/// in R. Used by `carrier init` when `--native` isn't passed, so a
/// fresh module starts from working, runnable examples instead of an
/// empty `box::use()`.
pub fn scaffold_pure_r(module_dir: &Path) -> Result<Vec<String>> {
    std::fs::write(module_dir.join("hello.r"), r_glue::HELLO_PURE)
        .context("Failed to write hello.r")?;
    std::fs::write(module_dir.join("add.r"), r_glue::ADD_PURE)
        .context("Failed to write add.r")?;
    std::fs::write(module_dir.join("__init__.r"), r_glue::INIT)
        .context("Failed to write __init__.r")?;

    Ok(vec![
        "hello.r".to_string(),
        "add.r".to_string(),
        "__init__.r".to_string(),
    ])
}

/// Folder name for a module's native source. Always `src/` — matching
/// R's own convention, and matching the one folder name that
/// `carrier-core`'s `artifact_name()` maps to the module's own name
/// rather than the folder's. That pairing is deliberate: a scaffolded
/// module's calling code can reference `dlls$<module_name>` as a fixed
/// literal because the folder that produces it is guaranteed to be
/// named `src`, not a guess.
pub fn native_dir_name(_lang: NativeLang) -> &'static str {
    "src"
}

/// Scaffold a module's native code and R glue:
///   `<module_dir>/<lang>/{hello,add}.<ext>` + `Makevars`
///   `<module_dir>/{hook,hello,add,__init__}.r`
/// `hook.r` is placed in `module_dir`, not the native subdir, deliberately.
/// `toolchain::build()` moves the compiled artifact up to `module_dir` so
/// `box::file()` (called from `hook.r`) resolves next to it.
/// Returns the paths written, relative to `module_dir`, for the caller
/// to report.
pub fn scaffold(
    module_dir: &Path,
    module_name: &str,
    lang: NativeLang,
    backend: Option<Backend>,
) -> Result<Vec<String>> {
    let template = template_for(lang, backend)?;
    let dir_name = native_dir_name(lang);
    let native_dir = module_dir.join(dir_name);

    std::fs::create_dir_all(&native_dir)
        .with_context(|| format!("Failed to create native directory: {}", native_dir.display()))?;

    std::fs::write(native_dir.join(format!("hello.{}", template.src_ext)), template.hello)
        .context("Failed to write hello example")?;
    std::fs::write(native_dir.join(format!("add.{}", template.src_ext)), template.add)
        .context("Failed to write add example")?;
    std::fs::write(native_dir.join("Makevars"), template.makevars)
        .context("Failed to write Makevars")?;

    std::fs::write(module_dir.join("hook.r"), template.hook).context("Failed to write hook.r")?;
    let hello_r = template.hello_r.replace("{{module_name}}", module_name);
    let add_r = template.add_r.replace("{{module_name}}", module_name);
    std::fs::write(module_dir.join("hello.r"), hello_r).context("Failed to write hello.r")?;
    std::fs::write(module_dir.join("add.r"), add_r).context("Failed to write add.r")?;
    std::fs::write(module_dir.join("__init__.r"), r_glue::INIT).context("Failed to write __init__.r")?;

    Ok(vec![
        format!("{dir_name}/hello.{}", template.src_ext),
        format!("{dir_name}/add.{}", template.src_ext),
        format!("{dir_name}/Makevars"),
        "hook.r".to_string(),
        "hello.r".to_string(),
        "add.r".to_string(),
        "__init__.r".to_string(),
    ])
}
