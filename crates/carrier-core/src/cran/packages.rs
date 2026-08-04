use std::collections::HashMap;
use std::io::{BufRead, BufReader};

use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use semver::Version;

/// Packages that ship with R itself (they never installable from CRAN).
const BASE_PACKAGES: &[&str] = &[
    // Native packages (base)
    "R", "base", "compiler", "datasets", "graphics", "grDevices",
    "grid", "methods", "parallel", "splines", "stats", "stats4",
    "tcltk", "tools", "utils",
    // recommended packages: ship with R, not on CRAN src/contrib
    "boot", "class", "cluster", "codetools", "foreign", "KernSmooth",
    "lattice", "MASS", "Matrix", "mgcv", "nlme", "nnet", "rpart",
    "spatial", "survival",
];

#[derive(Debug, Clone)]
pub struct PackageRecord {
    pub version: Version,
    pub deps: Vec<(String, String)>,
}

/// Shared client for every request this module makes. A bare
/// `reqwest::blocking::get` has no timeout at all. A stalled connection
/// (mirror hung, network partition mid-download) would block the whole
/// install indefinitely instead of failing with a message pointing at
/// the actual cause.
fn http_client() -> Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .context("Failed to build HTTP client")
}

/// Fetch and parse `PACKAGES.gz` from a CRAN-like repository URL.
pub fn fetch(repo_url: &str) -> Result<HashMap<String, PackageRecord>> {
    let url = format!(
        "{}/src/contrib/PACKAGES.gz",
        repo_url.trim_end_matches('/')
    );
    let client = http_client()?;

    let mut last_err = None;
    for attempt in 1..=3 {
        match client.get(&url).send() {
            Ok(response) if response.status().is_success() => {
                let bytes = response.bytes().context("Failed to read PACKAGES.gz bytes")?;
                let gz = GzDecoder::new(bytes.as_ref());
                return parse_dcf(BufReader::new(gz));
            }
            Ok(response) => {
                let status = response.status();
                eprintln!("  [warn] attempt {}/3: HTTP {} fetching index, retrying...", attempt, status);
                last_err = Some(anyhow::anyhow!("HTTP {} fetching package index: {}", status, url));
            }
            Err(e) => {
                eprintln!("  [warn] attempt {}/3: request failed, retrying...", attempt);
                last_err = Some(anyhow::anyhow!("Failed to fetch package index: {}: {}", url, e));
            }
        }
        if attempt < 3 {
            std::thread::sleep(std::time::Duration::from_secs(2u64.pow(attempt)));
        }
    }

    Err(last_err.unwrap())
}

/// This fetches all archived versions of a package from CRAN's Archive/ HTML index.
/// Returns versions sorted newest-first. An empty result (rather than an
/// error) means the package simply has no archive (not every package does).
pub fn fetch_archive_versions(repo_url: &str, pkg: &str) -> Result<Vec<Version>> {
    let url = format!(
        "{}/src/contrib/Archive/{}/",
        repo_url.trim_end_matches('/'),
        pkg
    );

    let response = http_client()?
        .get(&url)
        .send()
        .with_context(|| format!("Failed to fetch archive listing: {}", url))?;

    if !response.status().is_success() {
        return Ok(Vec::new());
    }

    let body = response.text()
        .with_context(|| format!("Failed to read archive listing body: {}", url))?;

    Ok(parse_archive_listing(&body, pkg))
}

/// Parse an Apache-style HTML directory listing for `{pkg}_{version}.tar.gz`
/// links. Deliberately a simple substring scan rather than a full HTML
/// parser. CRAN's Archive listings are consistently plain `<a href="...">`
/// tags, so this avoids pulling in an HTML parsing dependency for one job.
fn parse_archive_listing(html: &str, pkg: &str) -> Vec<Version> {
    let prefix = format!("{}_", pkg);
    let mut versions = Vec::new();

    for part in html.split("href=\"") {
        let Some(end) = part.find('"') else { continue };
        let href = &part[..end];

        let Some(name) = href.strip_suffix(".tar.gz") else { continue };
        let Some(ver_str) = name.strip_prefix(&prefix) else { continue };

        if let Ok(v) = Version::parse(&normalize_r_version(ver_str)) {
            versions.push(v);
        }
    }

    versions.sort_by(|a, b| b.cmp(a));
    versions
}

/// Normalize an R-style version string into something `semver::Version`
/// can parse. R versions use dash or dot separators and often carry a 4th
/// component CRAN doesn't (`1.2.3.9000` for a dev build) (semver only
/// has room for three). The previous approach truncated anything past the
/// 3rd component, which meant `1.2.3.9000` and `1.2.3.1` both normalized
/// to `1.2.3` and compared equal, silently skipping a reinstall that
/// should have happened. Anything past the 3rd component now becomes a
/// semver pre-release identifier instead of being discarded, so distinct
/// versions stay distinct.
///
/// This does not give correct *ordering* for 4-component versions, so
/// semver treats a pre-release as lower precedence than the plain
/// release (`1.2.3-9000 < 1.2.3`), while R's own convention treats a
/// higher trailing number as newer. Equality is now reliable; ordering
/// across a 4-component and a 3-component version of the same package is
/// still not something to trust.
fn normalize_r_version(raw: &str) -> String {
    let dashed = raw.replace('-', ".");
    let mut parts = dashed.splitn(4, '.');
    let major = parts.next().unwrap_or("0");
    let minor = parts.next().unwrap_or("0");
    let patch = parts.next().unwrap_or("0");
    match parts.next() {
        Some(extra) if !extra.is_empty() => format!("{major}.{minor}.{patch}-{extra}"),
        _ => format!("{major}.{minor}.{patch}"),
    }
}

fn parse_dcf(reader: impl BufRead) -> Result<HashMap<String, PackageRecord>> {
    let mut map = HashMap::new();

    let mut name: Option<String> = None;
    let mut version: Option<String> = None;
    let mut deps: Vec<(String, String)> = Vec::new();
    let mut in_dep_field = false;

    for line in reader.lines() {
        let line = line.context("Failed to read line from PACKAGES")?;

        if line.is_empty() {
            flush(&mut map, &mut name, &mut version, &mut deps);
            in_dep_field = false;
            continue;
        }

        // Continuation line (starts with whitespace)
        if line.starts_with(' ') || line.starts_with('\t') {
            if in_dep_field {
                deps.extend(parse_dep_field(line.trim()));
            }
            continue;
        }

        // New field
        in_dep_field = false;
        if let Some((key, val)) = line.split_once(": ") {
            match key {
                "Package" => name = Some(val.to_owned()),
                "Version" => version = Some(val.to_owned()),
                "Imports" | "Depends" => {
                    deps.extend(parse_dep_field(val));
                    in_dep_field = true;
                }
                _ => {}
            }
        }
    }

    // Final block — file may not end with a blank line
    flush(&mut map, &mut name, &mut version, &mut deps);

    Ok(map)
}

fn flush(
    map: &mut HashMap<String, PackageRecord>,
    name: &mut Option<String>,
    version: &mut Option<String>,
    deps: &mut Vec<(String, String)>,
) {
    if let (Some(n), Some(v)) = (name.take(), version.take()) {
        match Version::parse(&normalize_r_version(&v)) {
            Ok(parsed) => {
                map.insert(n, PackageRecord {
                    version: parsed,
                    deps: std::mem::take(deps),
                });
            }
            Err(_) => {
                deps.clear();
            }
        }
    } else {
        deps.clear();
    }
}

/// Parse a comma-separated dep field into bare package names.
/// Handles both `"rlang (>= 1.0.0)"` and `"rlang(>=1.0.0)"` (no space).
/// Filters out base and recommended packages.
fn parse_dep_field(s: &str) -> Vec<(String, String)> {
    s.split(',')
        .filter_map(|entry| {
            let trimmed = entry.trim();
            let (bare, constraint) = match trimmed.split_once('(') {
                Some((name, rest)) => (name.trim(), rest.trim_end_matches(')').trim().to_owned()),
                None => {
                    let bare = trimmed.split_once(' ').map(|(n, _)| n).unwrap_or(trimmed);
                    (bare.trim(), "*".to_owned())
                }
            };
            if bare.is_empty() || BASE_PACKAGES.contains(&bare) {
                None
            } else {
                Some((bare.to_owned(), constraint))
            }
        })
        .collect()
}
