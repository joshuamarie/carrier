use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use carrier_native::detect::find_native_dirs;

pub const DEFAULT_CRAN_MIRROR: &str = "https://cloud.r-project.org";

// ---- Author ----

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum Author {
    /// Simple string, e.g. ["John Doe"]
    Simple(String),
    /// Inline table, e.g. { name = "John Doe", email = "doe.john@example.com" }
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
/// `Simple(String)` mirrors `PackageDep::Simple` structurally, but there
/// is no CRAN-equivalent default registry for modules. A `Simple` dep
/// has a version constraint and nowhere to resolve it from. Resolution
/// must treat a missing `source` as a hard error, not a fallback.

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
/// `path` is optional and exists purely as an override. When omitted,
/// `resolve_native_dirs()` scans the module's whole source tree for
/// compiled-code dirs instead of assuming one is where it must live —
/// so whether a module HAS native code at all is still a filesystem
/// fact for the common case, not something that requires a `[native]`
/// block to discover, the same way `__init__.R`'s presence (not a
/// TOML field) is what makes a directory a module. `path` is for
/// naming exactly one location by hand instead of relying on that
/// scan — e.g. compiled code living outside the module's own source
/// directory entirely, where auto-discovery wouldn't look.
///
/// `paths` is the same idea for more than one location: naming
/// several compiled-code dirs explicitly rather than trusting
/// auto-discovery to find them all. Auto-discovery already finds
/// multiple scattered native dirs on its own regardless of what
/// they're named — `paths` exists for pinning specific ones by hand,
/// same motivation as `path`, just plural. `path` and `paths` are
/// mutually exclusive; if both are set, `paths` wins.
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
    /// Structured author entries. Each entry can be a plain string or an
    /// inline table with optional `email`, `url`, and `orcid` fields.
    ///
    /// ``` toml
    /// # Simple
    /// authors = ["Joshua Marie"]
    ///
    /// # Extended
    /// authors = [
    ///     { name = "Joshua Marie", email = "joshua.marie.k@gmail.com" },
    /// ]
    ///
    /// # Mixed
    /// authors = [
    ///     "Jane Doe",
    ///     { name = "Joshua Marie", email = "joshua.marie.k@gmail.com" },
    /// ]
    /// ```
    pub authors: Vec<Author>,
    pub license: String,
    pub r_version: String,
    /// Optional path to the source directory.
    /// If omitted, carrier looks for a directory named after the module.
    /// Must contain `__init__.R`.
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

    /// Resolve the source directory for this module.
    ///
    /// Rules:
    ///   1. If `src` is set, use that directory, any name is fine,
    ///      but it must exist and contain `__init__.R`.
    ///   2. If `src` is omitted, the directory must be named exactly
    ///      after the module. No guessing, no fallbacks.
    pub fn resolve_src_dir(&self, project_root: &Path) -> Result<PathBuf> {
        if let Some(src) = &self.module.src {
            let dir = project_root.join(src);
            if !dir.is_dir() {
                bail!(
                    "`src` path '{}' is not a directory.",
                    dir.display()
                );
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

        // Default 
        // Directory must be named after the module, no guessing
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

    /// Resolve the directory containing this module's compiled code,
    /// independent of `resolve_src_dir()`. A module's R source (`src`
    /// in `[module]`) and its native code (`path` in `[native]`) are
    /// unrelated locations, but neither is derived from the other. A
    /// module could have `src = "R/"` and `[native] path = "cpp/"`
    /// with no naming relationship between them at all.
    ///
    /// Falls back to `<module_src_dir>/src/` when `[native].path` is
    /// omitted — see `NativeConfig`'s doc comment for why that default
    /// exists and what it preserves.
    pub fn resolve_native_dir(&self, project_root: &Path) -> Result<PathBuf> {
        match self.native.as_ref().and_then(|n| n.path.as_deref()) {
            Some(path) => Ok(project_root.join(path)),
            None => Ok(self.resolve_src_dir(project_root)?.join("src")),
        }
    }

    /// Every native code location this module actually has, not just
    /// the one `[native].path` can override. `[native].paths` (plural)
    /// wins if set — several deliberately-named locations. Otherwise
    /// `[native].path` (singular) is one deliberate override — same
    /// meaning as `resolve_native_dir()`, kept for that case. Without
    /// either, this scans the whole module source tree for
    /// compiled-code dirs instead of assuming `src/` is the only place
    /// they can live — a module can have several, nested under
    /// different submodules (`mass/src/`, `temp/src/`, ...).
    pub fn resolve_native_dirs(&self, project_root: &Path) -> Result<Vec<PathBuf>> {
        let native = self.native.as_ref();

        if let Some(paths) = native.and_then(|n| n.paths.as_ref()) {
            return Ok(paths.iter().map(|p| project_root.join(p)).collect());
        }
        if let Some(path) = native.and_then(|n| n.path.as_deref()) {
            return Ok(vec![project_root.join(path)]);
        }

        let src_dir = self.resolve_src_dir(project_root)?;
        Ok(find_native_dirs(&src_dir))
    }

    /// Whether this module has compiled code that needs building via
    /// `R CMD SHLIB` before it can be loaded. Purely a filesystem
    /// check against whatever `resolve_native_dir()` resolves to. A
    /// `Makevars` or `Makevars.win` there is what makes a module
    /// "native," not `[native]`'s presence in the TOML (an author can
    /// still use `[native]` for `build_deps` alone without implying
    /// compiled code exists).
    pub fn has_native_code(&self, project_root: &Path) -> Result<bool> {
        Ok(!self.resolve_native_dirs(project_root)?.is_empty())
    }

    pub fn default_template(name: &str) -> String {
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

[native]
# Only needed if native code doesn't live in the default location
# (src/ under this module's source dir), or if `src/Makevars`
# references headers from another R package (e.g. Rcpp).
# path = "native/"            # override (defaults to src/ if omitted)
# build_deps = {{ Rcpp = "*" }}
                    # resolved and installed before compiling.
                    # Does not imply a runtime dependency; list in
                    # [package_deps] too if the compiled code also
                    # needs it loaded at runtime

[test]
framework = "testthat"
dir = "tests"
"#
        )
    }
}
