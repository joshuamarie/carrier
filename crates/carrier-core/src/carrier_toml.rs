use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use carrier_native::detect::find_native_dirs;
use carrier_native::{Backend, NativeLang};

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
                if let Some(u) = url  { write!(f, " ({})", u)?; }
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

// ---- NativeConfig ----
/// Declares where a module's compiled code lives, and its build-time-
/// only R package deps (e.g. `Rcpp`), whose headers a `Makevars` needs
/// to find via `system.file()` before `R CMD SHLIB` can run.
///
/// `path`/`paths` are relative to the module's own source directory
/// (whatever `resolve_src_dir()` resolves to) — the same base `src`
/// in `[module]` already uses, not the project root `carrier.toml`
/// lives in. `paths = ["cpp", "extra/src"]` in a module's own
/// `carrier.toml` means exactly what it looks like: two dirs nested
/// under that module's source tree.
///
/// `path` is optional and exists purely as an override. When omitted,
/// `resolve_native_dirs()` scans the module's whole source tree for
/// compiled-code dirs instead of assuming one is where it must live.
/// `paths` is the same idea for more than one location. `path` and
/// `paths` are mutually exclusive; if both are set, `paths` wins.
#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct NativeConfig {
    pub path: Option<String>,
    pub paths: Option<Vec<String>>,
    pub build_deps: Option<BTreeMap<String, PackageDep>>,
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

#[derive(Debug, Serialize, Deserialize)]
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
            if !dir.join("__init__.R").exists() {
                bail!(
                    "No `__init__.R` found in `src` directory '{}'.\n\
                     `__init__.R` is required as the module entry point.",
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
        if !dir.join("__init__.R").exists() {
            bail!(
                "No `__init__.R` found in '{}'.\n\
                 `__init__.R` is required as the module entry point, \
                 similar to NAMESPACE in R packages.",
                dir.display()
            );
        }
        Ok(dir)
    }

    /// Resolve the directory containing this module's compiled code.
    /// `[native].path`, when set, is relative to the module's own
    /// source directory — same base as `resolve_src_dir()` — not the
    /// project root.
    pub fn resolve_native_dir(&self, project_root: &Path) -> Result<PathBuf> {
        let src_dir = self.resolve_src_dir(project_root)?;
        match self.native.as_ref().and_then(|n| n.path.as_deref()) {
            Some(path) => Ok(src_dir.join(path)),
            None => Ok(src_dir.join("src")),
        }
    }

    /// Every native code location this module actually has.
    /// `[native].paths`/`path`, when set, are resolved relative to the
    /// module's own source directory, not the project root — a module
    /// can write `paths = ["cpp", "extra/src"]` meaning exactly those
    /// two subdirectories of its own source tree. Without either, this
    /// scans the whole module source tree for compiled-code dirs.
    pub fn resolve_native_dirs(&self, project_root: &Path) -> Result<Vec<PathBuf>> {
        let native = self.native.as_ref();
        let src_dir = self.resolve_src_dir(project_root)?;

        if let Some(paths) = native.and_then(|n| n.paths.as_ref()) {
            return Ok(paths.iter().map(|p| src_dir.join(p)).collect());
        }
        if let Some(path) = native.and_then(|n| n.path.as_deref()) {
            return Ok(vec![src_dir.join(path)]);
        }

        Ok(find_native_dirs(&src_dir))
    }

    pub fn has_native_code(&self, project_root: &Path) -> Result<bool> {
        Ok(!self.resolve_native_dirs(project_root)?.is_empty())
    }

    /// `native` is `Some((lang, backend))` when `carrier init` was run
    /// with `--native`. `path` is written relative to the module's own
    /// source directory — matching how `resolve_native_dir()` now
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
path = "{dir_name}/"            # override (defaults to src/ if omitted)
# paths = ["{dir_name}/"]  # If there are multiple folders containing the native code
{build_deps_line}
# resolved and installed before compiling.
# Does not imply a runtime dependency; list in
# [package_deps] too if the compiled code also
# needs it loaded at runtime"#
                )
            }
            None => r#"[native]
# Only needed if native code doesn't live in the default location
# (src/ under this module's source dir), or if `src/Makevars`
# references headers from another R package (e.g. Rcpp).
# path = "native/"            # override (defaults to src/ if omitted)
# build_deps = { Rcpp = "*" }
                    # resolved and installed before compiling.
                    # Does not imply a runtime dependency; list in
                    # [package_deps] too if the compiled code also
                    # needs it loaded at runtime"#
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
r_version = "4.0.0"
# src = "{name}"    # path to the source directory containing __init__.R
                    # defaults to a directory named after the module

[package_deps]
# dplyr = "*"
# ggplot2 = ">=3.4.0"
# fable = {{ version = "*", repo = "https://tidyverts.r-universe.dev/" }}

[module_deps]
# other_module = "*"

{native_block}

[test]
framework = "testthat"
dir = "tests"
"#
        )
    }
}
