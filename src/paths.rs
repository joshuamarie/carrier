use anyhow::{Context, Result};
use std::path::PathBuf;

const RENV_DIR: &str = "renv";
const RENV_CARRIER_DIR: &str = "carrier";
const CARRIER_DIR: &str = ".carrier";
const MODULES_DIR: &str = "modules";

/// Resolves the carrier install directory based on the current environment.
///
/// Priority:
///   1. `renv/carrier/` — if renv is active in CWD
///   2. `~/.carrier/modules/` — global fallback
///
/// Only resolves the path — does NOT create directories.
/// Callers are responsible for creating the directory if needed.
pub fn resolve_install_dir() -> Result<PathBuf> {
    let cwd = std::env::current_dir()
        .context("Failed to get current working directory")?;

    let renv_dir = cwd.join(RENV_DIR);
    if renv_dir.exists() {
        return Ok(renv_dir.join(RENV_CARRIER_DIR));
    }

    let global = dirs::home_dir()
        .context("Cannot find home directory")?
        .join(CARRIER_DIR)
        .join(MODULES_DIR);

    Ok(global)
}
