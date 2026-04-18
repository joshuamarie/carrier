use std::collections::HashMap;
use std::io::{BufRead, BufReader};

use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use semver::Version;

/// Packages that ship with R itself — never installable from CRAN.
const BASE_PACKAGES: &[&str] = &[
    "R", "base", "compiler", "datasets", "graphics", "grDevices",
    "grid", "methods", "parallel", "splines", "stats", "stats4",
    "tcltk", "tools", "utils",
];

#[derive(Debug, Clone)]
pub struct PackageRecord {
    pub version: Version,
    pub deps: Vec<String>,
}

/// Fetch and parse `PACKAGES.gz` from a CRAN-like repository URL.
/// Returns a map of package name → [`PackageRecord`].
pub fn fetch(repo_url: &str) -> Result<HashMap<String, PackageRecord>> {
    let url = format!(
        "{}/src/contrib/PACKAGES.gz",
        repo_url.trim_end_matches('/')
    );

    let response = reqwest::blocking::get(&url)
        .with_context(|| format!("Failed to fetch package index: {}", url))?;

    if !response.status().is_success() {
        anyhow::bail!("HTTP {} fetching package index: {}", response.status(), url);
    }

    let bytes = response.bytes().context("Failed to read PACKAGES.gz bytes")?;
    let gz = GzDecoder::new(bytes.as_ref());
    parse_dcf(BufReader::new(gz))
}

fn parse_dcf(reader: impl BufRead) -> Result<HashMap<String, PackageRecord>> {
    let mut map = HashMap::new();

    let mut name: Option<String> = None;
    let mut version: Option<String> = None;
    let mut deps: Vec<String> = Vec::new();
    // Track whether the previous field was a dep field so continuation
    // lines are appended correctly.
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
    deps: &mut Vec<String>,
) {
    if let (Some(n), Some(v)) = (name.take(), version.take()) {
        // R version strings can be "4.1-2" — normalise dashes to dots
        let v_norm = v.replace('-', ".");
        match Version::parse(&v_norm) {
            Ok(parsed) => {
                map.insert(n, PackageRecord {
                    version: parsed,
                    deps: std::mem::take(deps),
                });
            }
            Err(_) => {
                // Unparseable version (shouldn't happen for well-formed CRAN
                // packages, but skip rather than abort the whole index).
                deps.clear();
            }
        }
    } else {
        deps.clear();
    }
}

/// Parse a comma-separated dep field value into bare package names.
/// Strips version constraints — `"rlang (>= 1.0.0)"` → `"rlang"`.
/// Filters out base/recommended packages that are never on CRAN.
fn parse_dep_field(s: &str) -> Vec<String> {
    s.split(',')
        .filter_map(|entry| {
            let bare = entry.trim().split_once(' ')
                .map(|(name, _)| name)
                .unwrap_or(entry.trim());
            if bare.is_empty() || BASE_PACKAGES.contains(&bare) {
                None
            } else {
                Some(bare.to_owned())
            }
        })
        .collect()
}
