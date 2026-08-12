use std::path::Path;

use anyhow::{Context, Result};

use crate::lang::{Backend, NativeLang};
use crate::templates::{c, cpp, r_glue};

struct Template {
    hook: &'static str,
    hello: &'static str,
    add: &'static str,
    makevars: &'static str,
    src_ext: &'static str,
}

fn template_for(lang: NativeLang, backend: Option<Backend>) -> Result<Template> {
    match lang {
        NativeLang::C => Ok(Template {
            hook: r_glue::HOOK,
            hello: c::HELLO,
            add: c::ADD,
            makevars: c::MAKEVARS,
            src_ext: c::SRC_EXT,
        }),
        NativeLang::Cpp => Ok(match backend.unwrap_or_default() {
            Backend::Rcpp => Template {
                hook: r_glue::HOOK_RCPP,
                hello: cpp::rcpp::HELLO,
                add: cpp::rcpp::ADD,
                makevars: cpp::rcpp::MAKEVARS,
                src_ext: cpp::SRC_EXT,
            },
            Backend::Cpp11 => Template {
                hook: r_glue::HOOK,
                hello: cpp::cpp11::HELLO,
                add: cpp::cpp11::ADD,
                makevars: cpp::cpp11::MAKEVARS,
                src_ext: cpp::SRC_EXT,
            },
        }),
        NativeLang::Fortran => anyhow::bail!(
            "Fortran scaffolding isn't supported yet — the build pipeline doesn't compile .f90 sources."
        ),
    }
}

/// Folder name for a module's native source. The language's own name,
/// not a generic `src/` — `find_native_dirs` detects by `Makevars`
/// presence, not by folder name, so this is free to be descriptive.
pub fn native_dir_name(lang: NativeLang) -> &'static str {
    match lang {
        NativeLang::C => "c",
        NativeLang::Cpp => "cpp",
        NativeLang::Fortran => "fortran",
    }
}

/// Scaffold a module's native code and R glue:
///   `<module_dir>/<lang>/{hello,add}.<ext>` + `Makevars`
///   `<module_dir>/{hook,hello,add,__init__}.r`
/// `hook.r` is placed in `module_dir`, not the native subdir, deliberately —
/// `toolchain::build()` moves the compiled artifact up to `module_dir` so
/// `box::file()` (called from `hook.r`) resolves next to it.
/// Returns the paths written, relative to `module_dir`, for the caller
/// to report.
pub fn scaffold(
    module_dir: &Path,
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

    let hook = template.hook.replace("{{native_dir}}", dir_name);
    std::fs::write(module_dir.join("hook.r"), hook).context("Failed to write hook.r")?;
    std::fs::write(module_dir.join("hello.r"), r_glue::HELLO).context("Failed to write hello.r")?;
    std::fs::write(module_dir.join("add.r"), r_glue::ADD).context("Failed to write add.r")?;
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
