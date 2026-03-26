use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

use crate::carrier_toml::CarrierToml;
use crate::formats::{rmbx, tar};
use crate::manifest::{Dependencies, Manifest};

pub struct BundleArgs {
    pub path: String,
    pub rmbx: bool,
}

pub fn run(args: BundleArgs) -> Result<()> {
    let project_root = PathBuf::from(&args.path);

    if !project_root.exists() {
        bail!("Path does not exist: {}", project_root.display());
    }
    if !project_root.is_dir() {
        bail!("Path is not a directory: {}", project_root.display());
    }

    let toml = CarrierToml::from_dir(&project_root)?;
    let src_path = toml.resolve_src_dir(&project_root)?;
    let meta = &toml.module;

    let manifest = build_manifest(&toml, &src_path)?;

    // Output goes to CWD, named <name>_<version>.<ext>
    let cwd = std::env::current_dir()
        .context("Failed to get current working directory")?;

    let ext = if args.rmbx { "rmbx" } else { "tar.gz" };
    let output_path = cwd.join(format!("{}_{}.{}", meta.name, meta.version, ext));

    if args.rmbx {
        rmbx::bundle(&src_path, &project_root, &output_path, &manifest)
            .with_context(|| format!("Failed to bundle: {}", src_path.display()))?;
    } else {
        tar::bundle(&src_path, &project_root, &output_path, &manifest)
            .with_context(|| format!("Failed to bundle: {}", src_path.display()))?;
    }

    println!(
        "Bundled '{}' ({}) -> {}",
        meta.name,
        meta.version,
        output_path.display()
    );

    Ok(())
}

/// Used by `install` when bundling a GitHub-downloaded module.
pub fn bundle_to(project_root: &Path, output_path: &Path, rmbx: bool) -> Result<()> {
    let toml = CarrierToml::from_dir(project_root)?;
    let src_path = toml.resolve_src_dir(project_root)?;

    let manifest = build_manifest(&toml, &src_path)?;

    if rmbx {
        rmbx::bundle(&src_path, project_root, output_path, &manifest)
    } else {
        tar::bundle(&src_path, project_root, output_path, &manifest)
    }
}

/// Shared manifest construction from a parsed `carrier.toml` and resolved `src` path.
fn build_manifest(toml: &CarrierToml, src_path: &Path) -> Result<Manifest> {
    let meta = &toml.module;

    let files = crate::formats::rmbx::collect_files(src_path)
        .context("Failed to collect source files")?;

    if files.is_empty() {
        bail!("No files found in: {}", src_path.display());
    }

    let deps = toml.dependencies.clone().unwrap_or_default();
    let dependencies = Dependencies {
        packages: deps.packages.unwrap_or_default(),
        modules: deps.modules.unwrap_or_default(),
    };

    Ok(Manifest::new(
        &meta.name,
        &meta.version,
        &meta.description,
        meta.authors.clone(),
        &meta.license,
        &meta.r_version,
        dependencies,
        files,
    ))
}
