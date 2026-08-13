use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

use crate::carrier_toml::{CarrierToml, DEFAULT_CRAN_MIRROR};
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
    let manifest = build_manifest(&toml, &project_root, &src_path)?;

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

    let manifest = build_manifest(&toml, project_root, &src_path)?;

    if use_rmbx {
        rmbx::bundle(&src_path, project_root, output_path, &manifest)
    } else {
        tar::bundle(&src_path, project_root, output_path, &manifest)
    }
}

fn build_manifest(toml: &CarrierToml, project_root: &Path, src_path: &Path) -> Result<Manifest> {
    let meta = &toml.module;

    // let files = crate::formats::rmbx::collect_files(src_path)
    //     .context("Failed to collect source files")?;
    
    let files = tar::collect_files(src_path)  
        .context("Failed to collect source files")?;

    if files.is_empty() {
        bail!("No files found in: {}", src_path.display());
    }

    let dependencies = Dependencies {
        packages: toml.package_deps
            .clone()
            .unwrap_or_default()
            .into_iter()
            .map(|(name, dep)| {
                let repo = dep.repo();
                crate::manifest::PackageDepEntry {
                    name,
                    version: dep.version().to_owned(),
                    repo: if repo == DEFAULT_CRAN_MIRROR { None } else { Some(repo.to_owned()) },
                }
            })
            .collect(),
        modules: toml.module_deps
            .clone()
            .unwrap_or_default()
            .into_iter()
            .map(|(name, dep)| crate::manifest::ModuleDepEntry {
                name,
                version: dep.version().to_owned(),
                source: dep.source().map(str::to_owned),
            })
            .collect(),
    };

    let lock = crate::lockfile::read(project_root)
        .with_context(|| format!("Failed to read carrier.lock in {}", project_root.display()))?;

    Ok(Manifest::new(
        &meta.name,
        &meta.version,
        &meta.description,
        meta.authors.clone(),
        &meta.license,
        &meta.r_version,
        dependencies,
        files,
        lock.map(|l| l.packages),
        toml.test.clone(),
    ))
}
