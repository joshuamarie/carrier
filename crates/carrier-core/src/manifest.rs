use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::carrier_toml::{Author, TestConfig};
use crate::lockfile::LockedPackage;

/// Embedded inside every .rmbx/.tar.gz archive as `manifest.json`.
/// Mirrors `carrier.toml`, minus `module.src` — bundle() already
/// flattens the source tree relative to that directory, so by install
/// time there's nothing left for `src` to point at.
#[derive(Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub name: String,
    pub version: String,
    pub description: String,
    pub authors: Vec<Author>,
    pub license: String,
    pub r_version: String,
    pub dependencies: Dependencies,
    pub files: Vec<String>,
    pub bundled_at: String,
    /// The resolved package set from `carrier.lock` at bundle time, if
    /// one existed. `dependencies.packages` only carries the constraint
    /// strings declared in `carrier.toml`. This is what lets a
    /// standalone archive reproduce the exact install a lock would have
    /// given, without the original project directory around to read
    /// `carrier.lock` from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locked_packages: Option<Vec<LockedPackage>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test: Option<TestConfig>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PackageDepEntry {
    pub name: String,
    pub version: String,
    /// None means the default CRAN mirror.
    pub repo: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModuleDepEntry {
    pub name: String,
    pub version: String,
    /// Unlike `PackageDepEntry.repo`, `None` here is not a default —
    /// there is no default module registry. `None` means the source
    /// carrier.toml declared none, which resolution treats as an error.
    pub source: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Dependencies {
    /// Every 'box' modules uses R packages
    /// Write R package deps through e.g. ["dplyr", "stringr"])
    pub packages: Vec<PackageDepEntry>,
    /// Then other carrier modules required (e.g. ["utils/helpers"])
    pub modules: Vec<ModuleDepEntry>,
}

impl Manifest {
    pub fn new(
        name: impl Into<String>,
        version: impl Into<String>,
        description: impl Into<String>,
        authors: Vec<Author>,
        license: impl Into<String>,
        r_version: impl Into<String>,
        dependencies: Dependencies,
        files: Vec<String>,
        locked_packages: Option<Vec<LockedPackage>>,
        test: Option<TestConfig>,
    ) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            description: description.into(),
            authors,
            license: license.into(),
            r_version: r_version.into(),
            dependencies,
            files,
            bundled_at: Utc::now().to_rfc3339(),
            locked_packages,
            test,
        }
    }

    pub fn to_json(&self) -> anyhow::Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    #[allow(dead_code)]
    pub fn from_json(s: &str) -> anyhow::Result<Self> {
        Ok(serde_json::from_str(s)?)
    }
}
