use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

use crate::carrier_toml::CarrierToml;
use crate::formats::rmbx;
use crate::manifest::{Dependencies, Manifest};

pub struct BundleArgs {
    pub path: String,
    pub force: bool,
}

pub fn run(args: BundleArgs) -> Result<()> {
    let module_path = PathBuf::from(&args.path);

    if !module_path.exists() {
        bail!("Path does not exist: {}", module_path.display());
    }
    if !module_path.is_dir() {
        bail!("Path is not a directory: {}", module_path.display());
    }

    let toml = CarrierToml::from_dir(&module_path)?;
    let meta = &toml.module;

    let files = rmbx::collect_r_files(&module_path)
        .context("Failed to collect R files")?;

    if files.is_empty() {
        bail!("No files found in: {}", module_path.display());
    }

    let deps = toml.dependencies.unwrap_or_default();
    let dependencies = Dependencies {
        packages: deps.packages.unwrap_or_default(),
        modules: deps.modules.unwrap_or_default(),
    };

    let manifest = Manifest::new(
        &meta.name,
        &meta.version,
        &meta.description,
        meta.authors.clone(),
        &meta.license,
        &meta.r_version,
        dependencies,
        files,
    );

    // Output sits next to the module directory itself
    // e.g. ./mymodule -> ./mymodule.rmbx
    let output_path = module_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join(format!("{}.rmbx", meta.name));

    if output_path.exists() && !args.force {
        bail!(
            "{} already exists. Use --force to overwrite.",
            output_path.display()
        );
    }

    rmbx::bundle(&module_path, &output_path, &manifest)
        .with_context(|| format!("Failed to bundle: {}", module_path.display()))?;

    println!("Bundled '{}' -> {}", meta.name, output_path.display());

    Ok(())
}

/// Used by `install` when bundling a GitHub-downloaded module
/// to a specific path rather than the project's .mod/ directory.
pub fn bundle_to(module_path: &Path, output_path: &Path) -> Result<()> {
    let toml = CarrierToml::from_dir(module_path)?;
    let meta = &toml.module;

    let files = rmbx::collect_r_files(module_path)
        .context("Failed to collect R files")?;

    if files.is_empty() {
        bail!("No files found in: {}", module_path.display());
    }

    let deps = toml.dependencies.unwrap_or_default();
    let dependencies = Dependencies {
        packages: deps.packages.unwrap_or_default(),
        modules:  deps.modules.unwrap_or_default(),
    };

    let manifest = Manifest::new(
        &meta.name,
        &meta.version,
        &meta.description,
        meta.authors.clone(),
        &meta.license,
        &meta.r_version,
        dependencies,
        files,
    );

    rmbx::bundle(module_path, output_path, &manifest)
}
