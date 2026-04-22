use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

use crate::carrier_toml::CarrierToml;
use crate::formats::{rmbx, tar};
use crate::manifest::{Dependencies, Manifest};

pub fn run(path: &str, use_rmbx: bool) -> Result<()> {
    let project_root = PathBuf::from(path);

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

    let cwd = std::env::current_dir()
        .context("Failed to get current working directory")?;

    let ext = if use_rmbx { "rmbx" } else { "tar.gz" };
    let output_path = cwd.join(format!("{}_{}.{}", meta.name, meta.version, ext));

    if use_rmbx {
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
pub fn bundle_to(project_root: &Path, output_path: &Path, use_rmbx: bool) -> Result<()> {
    let toml = CarrierToml::from_dir(project_root)?;
    let src_path = toml.resolve_src_dir(project_root)?;

    let manifest = build_manifest(&toml, &src_path)?;

    if use_rmbx {
        rmbx::bundle(&src_path, project_root, output_path, &manifest)
    } else {
        tar::bundle(&src_path, project_root, output_path, &manifest)
    }
}

fn build_manifest(toml: &CarrierToml, src_path: &Path) -> Result<Manifest> {
    let meta = &toml.module;

    // let files = crate::formats::rmbx::collect_files(src_path)
    //     .context("Failed to collect source files")?;
    
    let files = tar::collect_files(src_path)  
        .context("Failed to collect source files")?;

    if files.is_empty() {
        bail!("No files found in: {}", src_path.display());
    }

    let deps = toml.dependencies.clone().unwrap_or_default();
    let dependencies = Dependencies {
        packages: deps.packages.unwrap_or_default().into_keys().collect(),
        modules: deps.modules.unwrap_or_default().into_keys().collect(),
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
