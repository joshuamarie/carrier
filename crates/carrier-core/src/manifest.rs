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
    #[serde(default)]
    pub native: Option<NativeManifest>,
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

/// Present only when the bundled module has compiled code (mirrors
/// `carrier_toml::NativeConfig`, plus a source hash computed at bundle
/// time). Carries the module's build-time deps forward into the
/// archive so `carrier install` can resolve+install them on the
/// installing machine before compiling. The same reason `Dependencies`
/// gets embedded here instead of re-read from a `carrier.toml` that
/// may not travel with every install path (e.g. `.rmbx`).
///
/// `artifacts` is empty unless the bundle was made with `--binary`.
/// Distributing prebuilts beyond one machine's own tagged output is
/// still a registry-level concern that doesn't exist yet.
/// A single tagged, precompiled binary attached to a bundle by
/// `carrier bundle --binary`. `target_triple`/`r_version` are the
/// exact two axes ABI compatibility depends on for R native code (see
/// `carrier_native::toolchain::BuildOutcome`). Install-only trusts
/// this artifact when both match the installing machine AND
/// `source_hash` matches the unpacked source's own recomputed hash.
/// Any mismatch on any of the three falls back to compiling from
/// source, same as today. This can only make an install faster, never
/// wrong.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NativeArtifact {
    pub target_triple: String,
    pub r_version: String,
    pub source_hash: String,
    /// Path to the compiled file inside the archive, e.g. "lib/cpp.dll".
    pub artifact: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct NativeManifest {
    pub build_deps: Vec<PackageDepEntry>,
    /// Hash of the module's native-code directory contents at bundle
    /// time (whatever `CarrierToml::resolve_native_dir()` resolved to
    /// — see `carrier_native::source_hash`). This is informational for
    /// now (lets an installer log "source changed since this was
    /// published"); the installing machine always recomputes its own
    /// hash for cache lookups rather than trusting this one, since
    /// it's describing the bundler's directory, not necessarily
    /// byte-identical to what ends up on disk after unpacking.
    pub source_hash: String,
    /// Precompiled binaries attached via `carrier bundle --binary`.
    /// Empty for a plain source bundle. `#[serde(default)]` so a
    /// manifest.json from before this field existed still parses.
    #[serde(default)]
    pub artifacts: Vec<NativeArtifact>,
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
            native: None,
            files,
            bundled_at: Utc::now().to_rfc3339(),
            locked_packages,
            test,
        }
    }

    /// Attaches native build info to a manifest already built via
    /// `new()`. Kept as a separate fluent setter rather than an
    /// argument on `new()` so existing call sites for non-native
    /// modules (the overwhelming majority) don't all need updating.
    pub fn with_native(mut self, native: NativeManifest) -> Self {
        self.native = Some(native);
        self
    }

    pub fn to_json(&self) -> anyhow::Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    #[allow(dead_code)]
    pub fn from_json(s: &str) -> anyhow::Result<Self> {
        Ok(serde_json::from_str(s)?)
    }
}
