use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// `carrier.toml` file inside a module directory acts 
/// like R's DESCRIPTION or Python's pyproject.toml
#[derive(Debug, Serialize, Deserialize)]
pub struct CarrierToml {
    pub module: ModuleMeta,
    pub dependencies: Option<TomlDependencies>,
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
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct TomlDependencies {
    pub packages: Option<Vec<String>>,
    pub modules: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TestConfig {
    pub framework: String,
    pub dir: Option<String>,
}

/// Every modules to be installed with `carrier` requires `carrier.toml` metadata
/// Use `from_dir` to read and parse a `carrier.toml` from a module directory.
/// We have `default_template` to auto-generate the default `carrier.toml` content for a new module.
impl CarrierToml {
    
    pub fn from_dir(module_path: &Path) -> Result<Self> {
        let toml_path = module_path.join("carrier.toml");
        let contents = std::fs::read_to_string(&toml_path)
            .with_context(|| format!(
                "Could not read carrier.toml at {}. \
                 Run `carrier init` to create one.",
                toml_path.display()
            ))?;
        toml::from_str(&contents)
            .with_context(|| format!("Failed to parse carrier.toml at {}", toml_path.display()))
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

[dependencies]
packages = []
modules = []

[test]
framework = "testthat"
dir = "tests"
"#
        )
    }
}