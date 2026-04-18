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
