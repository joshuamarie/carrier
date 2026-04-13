use anyhow::{Context, Result};
use std::path::PathBuf;

const CARRIER_LIB_ENV: &str = "CARRIER_LIB";
const CARRIER_DIR: &str = ".carrier";
const MODULES_DIR: &str = "modules";

/// Resolves the carrier install directory.
///
/// Priority:
///   1. `CARRIER_LIB` environment variable — explicit override
///      Works with any isolation tool (renv, pak, groundhog, etc.)
///   2. `~/.carrier/modules/` — global fallback
///
/// Only resolves the path — does NOT create directories.
/// Callers are responsible for creating the directory if needed.
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
