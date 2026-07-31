use anyhow::{Context, Result};
use std::path::PathBuf;

const CARRIER_LIB_ENV: &str = "CARRIER_LIB";
const CARRIER_R_LIB_ENV: &str = "CARRIER_R_LIB";
const CARRIER_DIR: &str = ".carrier";
const MODULES_DIR: &str = "modules";

pub fn resolve_install_dir() -> Result<PathBuf> {
    if let Ok(lib) = std::env::var(CARRIER_LIB_ENV) {
        if !lib.is_empty() {
            return Ok(PathBuf::from(lib));
        }
    }
    let global = dirs::home_dir()
        .context("Cannot find home directory")?
        .join(CARRIER_DIR)
        .join(MODULES_DIR);
    Ok(global)
}

pub struct RPlatform {
    pub os: RPlatformOs,
    pub r_version_short: String,
}

pub enum RPlatformOs {
    Windows,
    MacOs,
    Other,
}

pub fn detect_r_platform() -> Result<RPlatform> {
    let os = if cfg!(target_os = "windows") {
        RPlatformOs::Windows
    } else if cfg!(target_os = "macos") {
        RPlatformOs::MacOs
    } else {
        RPlatformOs::Other
    };

    let output = std::process::Command::new("Rscript")
        .args(["-e", "cat(paste(R.version$major, strsplit(R.version$minor, '.', fixed=TRUE)[[1]][1], sep='.'))"])
        .output()
        .context("Failed to run Rscript to detect R version — is R installed and on PATH?")?;

    let r_version_short = String::from_utf8(output.stdout)
        .context("Rscript output was not valid UTF-8")?
        .trim()
        .to_owned();

    if r_version_short.is_empty() {
        anyhow::bail!("Could not determine R version from Rscript output");
    }

    Ok(RPlatform { os, r_version_short })
}

/// Resolves the R user library path where R packages should be installed.
///
/// Priority:
///   1. `CARRIER_R_LIB` — explicit override (useful for renv/rv projects)
///   2. `R_LIBS_USER`   — R's own user library variable, set by R at startup
///   3. Subprocess fallback: `Rscript -e "cat(.libPaths()[1])"`
///
/// Callers are responsible for creating the directory if needed.
pub fn resolve_r_lib_dir() -> Result<PathBuf> {
    if let Ok(lib) = std::env::var(CARRIER_R_LIB_ENV) {
        if !lib.is_empty() {
            return Ok(PathBuf::from(lib));
        }
    }

    if let Ok(lib) = std::env::var("R_LIBS_USER") {
        if !lib.is_empty() {
            return Ok(PathBuf::from(lib));
        }
    }

    // Last resort: ask R directly
    let output = std::process::Command::new("Rscript")
        .args(["-e", "cat(.libPaths()[1])"])
        .output()
        .context("Failed to run Rscript — is R installed and on PATH?")?;

    let path_str = String::from_utf8(output.stdout)
        .context("Rscript output was not valid UTF-8")?;
    let path_str = path_str.trim();

    if path_str.is_empty() {
        anyhow::bail!(
            "Could not determine R library path. \
             Set CARRIER_R_LIB to the path of your R library."
        );
    }

    Ok(PathBuf::from(path_str))
}
