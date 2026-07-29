use std::collections::HashMap;
use std::io::{BufRead, BufReader};

use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use semver::Version;

/// Packages that ship with R itself — never installable from CRAN.
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

/// Fetch and parse `PACKAGES.gz` from a CRAN-like repository URL.
pub fn fetch(repo_url: &str) -> Result<HashMap<String, PackageRecord>> {
    let url = format!(
        "{}/src/contrib/PACKAGES.gz",
        repo_url.trim_end_matches('/')
    );

    let mut last_err = None;
    for attempt in 1..=3 {
        match reqwest::blocking::get(&url) {
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
        std::thread::sleep(std::time::Duration::from_secs(2u64.pow(attempt)));
    }

    Err(last_err.unwrap())
}

fn parse_dcf(reader: impl BufRead) -> Result<HashMap<String, PackageRecord>> {
    let mut map = HashMap::new();

    let mut name: Option<String> = None;
    let mut version: Option<String> = None;
    let mut deps: Vec<String> = Vec::new();
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
        // Normalize dashes and truncate to 3 components for semver
        let v_norm = v.replace('-', ".");
        let v_truncated = v_norm
            .splitn(4, '.')
            .take(3)
            .collect::<Vec<_>>()
            .join(".");

        match Version::parse(&v_truncated) {
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
