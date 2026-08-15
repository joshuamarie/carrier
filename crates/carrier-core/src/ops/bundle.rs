use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

use crate::carrier_toml::{CarrierToml, DEFAULT_CRAN_MIRROR};
use crate::formats::{rmbx, tar};
use crate::manifest::{Dependencies, Manifest};

pub fn run(path: &str, use_rmbx: bool, binary: bool, keep_source: bool) -> Result<()> {
    if keep_source && !binary {
        bail!("--keep-source only applies together with --binary.");
    }

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

    let built = if binary {
        Some(crate::ops::build::run(&project_root)?)
    } else {
        None
    };

    let manifest = build_manifest(&toml, &project_root, &src_path, built.as_deref())?;

    // .lib/ is now dot-prefixed, so the archive writer's own
    // hidden-file filter excludes it from a plain bundle automatically
    // — no explicit exclusion needed there anymore. --binary needs the
    // opposite: force it back in despite the dot, since shipping it is
    // the whole point. --binary without --keep-source additionally
    // excludes native source; a mismatched/missing tag on install then
    // has nothing to fall back to and must error clearly (not yet
    // implemented on the install side — see the TODO on install.rs).
    let (exclude, force_include): (Vec<PathBuf>, Vec<PathBuf>) = if binary {
        let lib_dirs: Vec<PathBuf> = toml.resolve_native_dirs(&project_root)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|d| d.parent().map(|p| p.join(".lib")))
            .collect();
        let src_exclude = if keep_source {
            Vec::new()
        } else {
            toml.resolve_native_dirs(&project_root).unwrap_or_default()
        };
        (src_exclude, lib_dirs)
    } else {
        (Vec::new(), Vec::new())
    };

    let cwd = std::env::current_dir()
        .context("Failed to get current working directory")?;

    let ext = if use_rmbx { "rmbx" } else { "tar.gz" };
    let output_path = cwd.join(format!("{}_{}.{}", meta.name, meta.version, ext));

    if use_rmbx {
        rmbx::bundle(&src_path, &project_root, &output_path, &manifest, &exclude, &force_include)
            .with_context(|| format!("Failed to bundle: {}", src_path.display()))?;
    } else {
        tar::bundle(&src_path, &project_root, &output_path, &manifest, &exclude, &force_include)
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

    let manifest = build_manifest(&toml, project_root, &src_path, None)?;

    if use_rmbx {
        rmbx::bundle(&src_path, project_root, output_path, &manifest, &[], &[])
    } else {
        tar::bundle(&src_path, project_root, output_path, &manifest, &[], &[])
    }
}

fn build_manifest(
    toml: &CarrierToml,
    project_root: &Path,
    src_path: &Path,
    built: Option<&[crate::ops::build::BuiltArtifact]>,
) -> Result<Manifest> {
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

    let mut manifest = Manifest::new(
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
    );

    if let Some(artifacts) = built {
        let build_deps = toml.native.as_ref()
            .and_then(|n| n.build_deps.clone())
            .unwrap_or_default()
            .into_iter()
            .map(|(name, dep)| crate::manifest::PackageDepEntry {
                name,
                version: dep.version().to_owned(),
                repo: if dep.repo() == DEFAULT_CRAN_MIRROR { None } else { Some(dep.repo().to_owned()) },
            })
            .collect();

        // One archive can have multiple native dirs; use the first
        // build's source_hash as the manifest-level informational
        // hash, same as a single-native-dir module always would.
        let source_hash = artifacts.first()
            .map(|a| a.source_hash.clone())
            .unwrap_or_default();

        let native_artifacts = artifacts.iter().map(|a| {
            let rel = a.artifact_path.strip_prefix(src_path).unwrap_or(&a.artifact_path);
            crate::manifest::NativeArtifact {
                target_triple: a.target_triple.clone(),
                r_version: a.r_version.clone(),
                source_hash: a.source_hash.clone(),
                artifact: rel.to_string_lossy().replace('\\', "/"),
            }
        }).collect();

        manifest = manifest.with_native(crate::manifest::NativeManifest {
            build_deps,
            source_hash,
            artifacts: native_artifacts,
        });
    }

    Ok(manifest)
}
