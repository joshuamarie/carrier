use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

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

// ---- CarrierToml (For metadata file) ----

#[derive(Debug, Serialize, Deserialize)]
pub struct CarrierToml {
    pub module: ModuleMeta,
    pub package_deps: Option<BTreeMap<String, PackageDep>>,
    pub module_deps: Option<BTreeMap<String, ModuleDep>>,
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

    /// Resolve the source directory for this module.
    ///
    /// Rules:
    ///   1. If `src` is set, use that directory — any name is fine,
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

        // Default — directory must be named after the module, no guessing
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

[test]
framework = "testthat"
dir = "tests"
"#
        )
    }
}
