use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const DEFAULT_CRAN_MIRROR: &str = "https://cloud.r-project.org";

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct TomlRepositories {
    pub cran: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CarrierToml {
    pub module: ModuleMeta,
    pub dependencies: Option<TomlDependencies>,
    pub test: Option<TestConfig>,
    pub repositories: Option<TomlRepositories>, 
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ModuleMeta {
    pub name: String,
    pub version: String,
    pub description: String,
    pub authors: Vec<String>,
    pub license: String,
    pub r_version: String,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct TomlDependencies {
    pub packages: Option<BTreeMap<String, String>>,
    pub modules: Option<BTreeMap<String, String>>,
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
        let src_path = project_root.join(&self.module.name);
        if !src_path.exists() {
            bail!(
                "Source directory '{}' not found in {}.\n\
                 Expected a folder named '{}' next to carrier.toml.",
                self.module.name,
                project_root.display(),
                self.module.name,
            );
        }
        if !src_path.is_dir() {
            bail!("'{}' exists but is not a directory.", src_path.display());
        }
        Ok(src_path)
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

[dependencies.packages]
# dplyr = "*"
# ggplot2 = ">=3.5.0, <=4.1.0"

[dependencies.modules]
# utils/core = "*"

[repositories]
# cran = "https://cloud.r-project.org"

[test]
framework = "testthat"
dir = "tests"
"#
        )
    }
    
    pub fn cran_url(&self) -> &str {
        self.repositories
            .as_ref()
            .and_then(|r| r.cran.as_deref())
            .unwrap_or(DEFAULT_CRAN_MIRROR)
    }
}
