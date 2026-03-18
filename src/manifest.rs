use chrono::Utc;
use serde::{Deserialize, Serialize};

/// Embedded inside every .rmbx archive as `manifest.json`.
/// Mirrors the fields from the module's `carrier.toml`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub name:         String,
    pub version:      String,
    pub description:  String,
    pub authors:      Vec<String>,
    pub license:      String,
    pub r_version:    String,
    pub dependencies: Dependencies,
    pub files:        Vec<String>,
    pub bundled_at:   String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Dependencies {
    /// R packages required (e.g. ["dplyr", "stringr"])
    pub packages: Vec<String>,
    /// Other carrier modules required (e.g. ["utils/helpers"])
    pub modules:  Vec<String>,
}

impl Manifest {
    pub fn new(
        name:         impl Into<String>,
        version:      impl Into<String>,
        description:  impl Into<String>,
        authors:      Vec<String>,
        license:      impl Into<String>,
        r_version:    impl Into<String>,
        dependencies: Dependencies,
        files:        Vec<String>,
    ) -> Self {
        Self {
            name:        name.into(),
            version:     version.into(),
            description: description.into(),
            authors,
            license:     license.into(),
            r_version:   r_version.into(),
            dependencies,
            files,
            bundled_at:  Utc::now().to_rfc3339(),
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
