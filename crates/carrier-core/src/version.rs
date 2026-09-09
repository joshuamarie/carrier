use anyhow::{bail, Result};
use semver::{Version, VersionReq};

/// A parsed version requirement, wrapping semver's `VersionReq`.
///
/// Supported syntax:
///   `"*"`                 — any version
///   `">=1.0.0"`           — lower bound only
///   `">=1.0.0, <2.0.0"`  — range (comma-separated bounds)
///   `"^1.2.0"`            — semver-compatible with 1.2.0
///   `"=1.2.3"`            — exact version pin
///
/// Note: R version strings sometimes use dashes (e.g. `"4.0-3"`).
/// Normalise those to dots before calling `parse` if reading from R metadata.
#[derive(Debug, Clone)]
pub struct VersionSpec(VersionReq);

impl VersionSpec {
    pub fn parse(s: &str) -> Result<Self> {
        let s = s.trim();
        let req_str = if s == "*" { "*" } else { s };
        VersionReq::parse(req_str)
            .map(VersionSpec)
            .map_err(|e| anyhow::anyhow!("Invalid version spec {:?}: {}", s, e))
    }

    /// semver excludes pre-release versions from a bare `*` by design.
    /// normalize_r_version's dash-as-pre-release encoding makes CRAN
    /// versions like some older packages such as `{BayesFactor}`. 
    /// Retry with it stripped if the strict match fails because of that.
    pub fn matches(&self, v: &Version) -> bool {
        if self.0.matches(v) {
            return true;
        }
        if !v.pre.is_empty() {
            let mut stripped = v.clone();
            stripped.pre = semver::Prerelease::EMPTY;
            return self.0.matches(&stripped);
        }
        false
    }

    /// Given a list of specs and candidate versions (sorted newest-first),
    /// return the best version satisfying ALL specs simultaneously.
    pub fn resolve<'a>(specs: &[VersionSpec], candidates: &'a [Version]) -> Option<&'a Version> {
        candidates.iter().find(|v| specs.iter().all(|s| s.matches(v)))
    }
}

/// Verify that at least one candidate version satisfies all collected specs.
/// Call this during resolution once real candidate lists are available.
pub fn check_conflicts(name: &str, specs: &[VersionSpec], candidates: &[Version]) -> Result<()> {
    if VersionSpec::resolve(specs, candidates).is_none() {
        bail!(
            "Version conflict for '{}': no version satisfies all constraints.\n\
             Constraints: {}",
            name,
            specs.iter().map(|s| format!("{:?}", s.0)).collect::<Vec<_>>().join(", ")
        );
    }
    Ok(())
}

/// Verify the R currently on PATH satisfies `spec`. Called at install
/// and lock time — this is the one place a version mismatch has to
/// fail loudly instead of letting a module load under an R it was
/// never declared compatible with.
pub fn check_r_version(spec: &VersionSpec) -> Result<()> {
    let detected = crate::paths::detect_r_version()?;
    if !spec.matches(&detected) {
        bail!(
            "Installed R is {detected}, but this module requires r_version = \"{spec}\"."
        );
    }
    Ok(())
}

impl std::fmt::Display for VersionSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
