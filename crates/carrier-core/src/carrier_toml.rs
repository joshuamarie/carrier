use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use carrier_native::detect::find_native_dirs;
use carrier_native::{Backend, NativeLang};

use crate::version::VersionSpec;

pub const DEFAULT_CRAN_MIRROR: &str = "https://cloud.r-project.org";

// ---- Author ----

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum Author {
    Simple(String),
    Extended {
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        email: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        url: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        orcid: Option<String>,
    },
}

impl Author {
    pub fn name(&self) -> &str {
        match self {
            Author::Simple(n) => n,
            Author::Extended { name, .. } => name,
        }
    }

    pub fn email(&self) -> Option<&str> {
        match self {
            Author::Simple(_) => None,
            Author::Extended { email, .. } => email.as_deref(),
        }
    }

    pub fn url(&self) -> Option<&str> {
        match self {
            Author::Simple(_) => None,
            Author::Extended { url, .. } => url.as_deref(),
        }
    }

    pub fn orcid(&self) -> Option<&str> {
        match self {
            Author::Simple(_) => None,
            Author::Extended { orcid, .. } => orcid.as_deref(),
        }
    }
}

impl std::fmt::Display for Author {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Author::Simple(n) => write!(f, "{}", n),
            Author::Extended { name, email, url, .. } => {
                write!(f, "{}", name)?;
                if let Some(e) = email { write!(f, " <{}>", e)?; }
                if let Some(u) = url { write!(f, " ({})", u)?; }
                Ok(())
            }
        }
    }
}

// ---- PackageDep ----

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum PackageDep {
    Simple(String),
    Extended { version: String, repo: Option<String> },
}

impl PackageDep {
    pub fn version(&self) -> &str {
        match self {
            PackageDep::Simple(v) => v,
            PackageDep::Extended { version, .. } => version,
        }
    }

    pub fn repo(&self) -> &str {
        match self {
            PackageDep::Simple(_) => DEFAULT_CRAN_MIRROR,
            PackageDep::Extended { repo, .. } => {
                repo.as_deref().unwrap_or(DEFAULT_CRAN_MIRROR)
            }
        }
    }
}

// ---- ModuleDep ----

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum ModuleDep {
    Simple(String),
    Extended { version: String, source: Option<String> },
}

impl ModuleDep {
    pub fn version(&self) -> &str {
        match self {
            ModuleDep::Simple(v) => v,
            ModuleDep::Extended { version, .. } => version,
        }
    }

    pub fn source(&self) -> Option<&str> {
        match self {
            ModuleDep::Simple(_) => None,
            ModuleDep::Extended { source, .. } => source.as_deref(),
        }
    }
}

// ---- NativePath ----

/// `[native].path` accepts either a single string or an array, so a
/// module with one compiled-code dir doesn't have to write
/// `path = ["cpp/"]` just to satisfy a Vec-only field, and a module
/// with several doesn't have to pick one arbitrarily.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum NativePath {
    Single(String),
    Multiple(Vec<String>),
}

impl NativePath {
    pub fn as_paths(&self) -> Vec<&str> {
        match self {
            NativePath::Single(p) => vec![p.as_str()],
            NativePath::Multiple(ps) => ps.iter().map(String::as_str).collect(),
        }
    }
}

// ---- NativeConfig ----
/// Declares where a module's compiled code lives, and its build-time-
/// only R package deps (e.g. `Rcpp`), whose headers a `Makevars` needs
/// to find via `system.file()` before `R CMD SHLIB` can run.
///
/// `path` is relative to the module's own source directory (whatever
/// `resolve_src_dir()` resolves to) — the same base `src` in
/// `[module]` already uses, not the project root `carrier.toml` lives
/// in. `path = ["cpp", "extra/src"]` in a module's own `carrier.toml`
/// means exactly what it looks like: two dirs nested under that
/// module's source tree.
///
/// `path` is optional and exists purely as an override. When omitted,
/// `resolve_native_dirs()` scans the module's whole source tree for
/// compiled-code dirs instead of assuming one is where it must live.
#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct NativeConfig {
    pub path: Option<NativePath>,
    pub build_deps: Option<BTreeMap<String, PackageDep>>,
}

/// `box` accepts either case for a module's `.r`/`.R` extension, so
/// carrier's own entry-point check shouldn't hardcode one — a module
/// scaffolded with either convention, or hand-authored either way,
/// must resolve the same regardless of which case the author used or
/// which filesystem carrier itself happens to be running on.
fn find_init_file(dir: &Path) -> Option<PathBuf> {
    for name in ["__init__.r", "__init__.R"] {
        let candidate = dir.join(name);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

// ---- CarrierToml (For metadata file) ----

#[derive(Debug, Serialize, Deserialize)]
pub struct CarrierToml {
    pub module: ModuleMeta,
    pub package_deps: Option<BTreeMap<String, PackageDep>>,
    pub module_deps: Option<BTreeMap<String, ModuleDep>>,
    pub native: Option<NativeConfig>,
    pub test: Option<TestConfig>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ModuleMeta {
    pub name: String,
    pub version: String,
    pub description: String,
    pub authors: Vec<Author>,
    pub license: String,
    pub r_version: String,
    pub src: Option<String>,
}

impl ModuleMeta {
    pub fn r_version_spec(&self) -> Result<VersionSpec> {
        VersionSpec::parse(&self.r_version)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestConfig {
    pub framework: String,
    pub dir: Option<String>,
}

impl CarrierToml {
    pub fn from_dir(project_root: &Path) -> Result<Self> {
        let toml_path = project_root.join("carrier.toml");
        let contents = std::fs::read_to_string(&toml_path)
            .with_context(|| format!(
                "Could not read carrier.toml at {}. \
                 Run `carrier init` to create one.",
                toml_path.display()
            ))?;
        toml::from_str(&contents)
            .with_context(|| format!("Failed to parse carrier.toml at {}", toml_path.display()))
    }

    pub fn resolve_src_dir(&self, project_root: &Path) -> Result<PathBuf> {
        if let Some(src) = &self.module.src {
            let dir = project_root.join(src);
            if !dir.is_dir() {
                bail!("`src` path '{}' is not a directory.", dir.display());
            }
            if find_init_file(&dir).is_none() {
                bail!(
                    "No `__init__.r` (or `__init__.R`) found in `src` directory '{}'.\n\
                     `__init__.r` is required as the module entry point.",
                    dir.display()
                );
            }
            return Ok(dir);
        }

        let dir = project_root.join(&self.module.name);
        if !dir.is_dir() {
            bail!(
                "Source directory '{}' not found in '{}'.\n\
                 The source directory must match the module name '{}', \
                 or set `src` in carrier.toml to point to the correct directory.",
                self.module.name,
                project_root.display(),
                self.module.name,
            );
        }
        if find_init_file(&dir).is_none() {
            bail!(
                "No `__init__.r` (or `__init__.R`) found in '{}'.\n\
                 `__init__.r` is required as the module entry point, \
                 similar to NAMESPACE in R packages.",
                dir.display()
            );
        }
        Ok(dir)
    }

    /// Every native code location this module actually has.
    /// `[native].path`, when set, is resolved relative to the module's
    /// own source directory, not the project root — a module can write
    /// `path = "cpp"` for one location or `path = ["cpp", "extra/src"]`
    /// for several, both resolved against that module's own source
    /// tree. Without it, this scans the whole module source tree for
    /// compiled-code dirs.
    pub fn resolve_native_dirs(&self, project_root: &Path) -> Result<Vec<PathBuf>> {
        let native = self.native.as_ref();
        let src_dir = self.resolve_src_dir(project_root)?;

        if let Some(path) = native.and_then(|n| n.path.as_ref()) {
            return Ok(path.as_paths().into_iter().map(|p| src_dir.join(p)).collect());
        }

        Ok(find_native_dirs(&src_dir))
    }

    pub fn has_native_code(&self, project_root: &Path) -> Result<bool> {
        Ok(!self.resolve_native_dirs(project_root)?.is_empty())
    }

    /// `native` is `Some((lang, backend))` when `carrier init` was run
    /// with `--native`. `path` is written relative to the module's own
    /// source directory — matching how `resolve_native_dirs()` now
    /// resolves it — not the project root.
    pub fn default_template(name: &str, native: Option<(NativeLang, Option<Backend>)>) -> String {
        let native_block = match native {
            Some((lang, backend)) => {
                let dir_name = carrier_native::scaffold::native_dir_name(lang);
                let build_deps_line = match lang {
                    NativeLang::Cpp => match backend.unwrap_or_default() {
                        Backend::Rcpp => "build_deps = { Rcpp = \"*\" }".to_string(),
                        Backend::Cpp11 => "build_deps = { cpp11 = \"*\" }".to_string(),
                    },
                    _ => "# build_deps = { Rcpp = \"*\" }".to_string(),
                };
                format!(
                    r#"[native]
# Only needed if native code doesn't live in the default location
# (src/ under this module's source dir), or if `src/Makevars`
# references headers from another R package (e.g. Rcpp).
path = "{dir_name}/"
# path can also be an array: path = ["{dir_name}/", "extra/src"]
{build_deps_line}
# build_deps is resolved and installed before compiling.
# Does not imply a runtime dependency; list in [package_deps]
# too if the compiled code also needs it loaded at runtime"#
                )
            }
            None => r#"[native]
# Only needed if native code doesn't live in the default location
# (src/ under this module's source dir), or if `src/Makevars`
# references headers from another R package (e.g. Rcpp).
# path = "native/"
# path can also be an array: path = ["native/", "extra/src"]
# build_deps = { Rcpp = "*" }
# build_deps is resolved and installed before compiling.
# Does not imply a runtime dependency; list in [package_deps]
# too if the compiled code also needs it loaded at runtime"#
                .to_string(),
        };

        format!(
            r#"[module]
name = "{name}"
version = "0.1.0"
description = ""
authors = [
    {{ name = "Your Name", email = "you@example.com" }},
]
license = "Unknown"
r_version = ">=4.0.0"
# src = "{name}"    # path to the source directory containing __init__.R
                    # defaults to a directory named after the module

[package_deps]
# dplyr = "*"
# ggplot2 = ">=3.4.0"
# fable = {{ version = "*", repo = "https://tidyverts.r-universe.dev/" }}

[module_deps]
# other_module = "*"

{native_block}

# [test]
# framework = "testthat"
# dir = "tests"
"#
        )
    }
}
