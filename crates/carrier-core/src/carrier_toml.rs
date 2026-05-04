use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const DEFAULT_CRAN_MIRROR: &str = "https://cloud.r-project.org";

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

#[derive(Debug, Serialize, Deserialize)]
pub struct CarrierToml {
    pub module: ModuleMeta,
    pub package_deps: Option<BTreeMap<String, PackageDep>>,
    pub module_deps: Option<BTreeMap<String, String>>,
    pub test: Option<TestConfig>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ModuleMeta {
    pub name: String,
    pub version: String,
    pub description: String,
    pub authors: Vec<String>,
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
        let candidates: &[PathBuf] = &[
            self.module.src.as_deref()
                .map(|s| project_root.join(s))
                .unwrap_or_else(|| project_root.join(&self.module.name)),
            project_root.to_path_buf(),
        ];

        for path in candidates {
            if path.is_dir() {
                let has_r_files = walkdir::WalkDir::new(path)
                    .into_iter()
                    .filter_map(|e| e.ok())
                    .any(|e| e.path().extension().and_then(|x| x.to_str()) == Some("R"));
                if has_r_files {
                    return Ok(path.clone());
                }
            }
        }

        bail!(
            "No R source files found in '{}' or its '{}' subdirectory.",
            project_root.display(),
            self.module.name,
        );
    }

    pub fn default_template(name: &str) -> String {
        format!(
            r#"[module]
name = "{name}"
version = "0.1.0"
description = ""
authors = []
license = "Unknown"
r_version = "4.0.0"

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
