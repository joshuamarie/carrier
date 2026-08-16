use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{bail, Context, Result};
use semver::Version;
use serde::{Deserialize, Serialize};

pub const LOCK_FILE_NAME: &str = "carrier.lock";
const LOCK_FORMAT_VERSION: u32 = 1;

/// One package pinned to the exact version and repo carrier resolved it
/// to the last time the lock was written. R packages only for now —
/// module deps have no automatic resolve+install path yet (execute_plan
/// only reports whether a module is already installed), so there is
/// nothing concrete to pin for them.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LockedPackage {
    pub name: String,
    pub version: String,
    pub repo: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct CarrierLock {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub r_version: Option<String>,
    #[serde(default, rename = "package")]
    pub packages: Vec<LockedPackage>,
}

fn default_version() -> u32 {
    LOCK_FORMAT_VERSION
}

impl CarrierLock {
    /// Build a lock from an already-resolved package set — used when
    /// reconstructing a lock from a bundle's embedded manifest, where
    /// there is no carrier.lock file on disk to read.
    pub fn from_packages(packages: Vec<LockedPackage>) -> Self {
        Self { version: LOCK_FORMAT_VERSION, r_version: None, packages }
    }

    /// Look up a locked package by name and return its exact version,
    /// already parsed. A malformed entry is a hard error rather than a
    /// silent fall-through to fresh resolution — a lock the tool can't
    /// trust is worse than no lock at all, since it would look
    /// authoritative while quietly not being enforced.
    pub fn locked_version(&self, name: &str) -> Result<Option<(Version, String)>> {
        let Some(p) = self.packages.iter().find(|p| p.name == name) else {
            return Ok(None);
        };
        let v = Version::parse(&p.version).with_context(|| {
            format!(
                "carrier.lock has an invalid version for '{}': '{}'",
                name, p.version
            )
        })?;
        Ok(Some((v, p.repo.clone())))
    }
}

/// Read `carrier.lock` from `project_root`. `Ok(None)` means the file
/// doesn't exist — its presence is the only opt-in switch, so a missing
/// lock is not an error, it just means carrier resolves fresh the way
/// it always has. An existing-but-unparseable lock IS an error: silently
/// ignoring a broken lock would defeat the reason it exists.
pub fn read(project_root: &Path) -> Result<Option<CarrierLock>> {
    let path = project_root.join(LOCK_FILE_NAME);
    if !path.exists() {
        return Ok(None);
    }
    let contents = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    let lock: CarrierLock = toml::from_str(&contents)
        .with_context(|| format!("Failed to parse {}", path.display()))?;

    if lock.version != LOCK_FORMAT_VERSION {
        bail!(
            "{} declares lock format version {}, but this build of carrier \
             only understands version {}. Regenerate it with `carrier lock`.",
            path.display(),
            lock.version,
            LOCK_FORMAT_VERSION
        );
    }

    Ok(Some(lock))
}

/// Write `carrier.lock` from a resolved set of `{name: (version, repo)}`,
/// sorted by package name for a stable, diffable file — an unsorted lock
/// would produce noisy diffs on every write even when nothing about the
/// resolved graph actually changed.
pub fn write(
    project_root: &Path,
    resolved: &BTreeMap<String, (Version, String)>,
    r_version: &str,
) -> Result<()> {
    let mut packages: Vec<LockedPackage> = resolved
        .iter()
        .map(|(name, (version, repo))| LockedPackage {
            name: name.clone(),
            version: version.to_string(),
            repo: repo.clone(),
        })
        .collect();
    packages.sort_by(|a, b| a.name.cmp(&b.name));

    let lock = CarrierLock {
        version: LOCK_FORMAT_VERSION,
        r_version: Some(r_version.to_owned()),
        packages,
    };
    let contents = toml::to_string_pretty(&lock).context("Failed to serialize carrier.lock")?;

    let path = project_root.join(LOCK_FILE_NAME);
    let tmp_path = path.with_extension("lock.tmp");
    std::fs::write(&tmp_path, &contents)
        .with_context(|| format!("Failed to write {}", tmp_path.display()))?;
    std::fs::rename(&tmp_path, &path)
        .with_context(|| format!("Failed to finalize {}", path.display()))?;
    Ok(())
}
