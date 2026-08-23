use anyhow::{bail, Context, Result};
use std::fs;
use std::path::PathBuf;

use carrier_native::{Backend, NativeLang};

use crate::carrier_toml::CarrierToml;

pub fn run(
    name: &str,
    dir_name: Option<&str>,
    native: Option<NativeLang>,
    backend: Option<Backend>,
) -> Result<()> {
    let default_dir = format!("{}-proj", name);
    let project_dir_name = dir_name.unwrap_or(&default_dir);
    let project_root = PathBuf::from(project_dir_name);
    
    if project_root.exists() {
        bail!("'{}' already exists.", project_root.display());
    }
    
    fs::create_dir_all(&project_root)
    .with_context(|| format!("Failed to create directory: {}", project_root.display()))?;
    
    fs::write(
        project_root.join("carrier.toml"),
        CarrierToml::default_template(name, native.map(|lang| (lang, backend))),
    )
    .context("Failed to write carrier.toml")?;
    
    fs::write(
        project_root.join("README.md"),
        format!("# {}\n\nA box module.\n", name),
    )
    .context("Failed to write README.md")?;
    
    // The default convention: Source directory named after the module 
    let src_dir = project_root.join(name);
    fs::create_dir_all(&src_dir)
    .with_context(|| format!("Failed to create source directory: {}", src_dir.display()))?;
    
    let mut files = vec![
        "carrier.toml".to_string(),
        "README.md".to_string(),
    ];
    
    // Scaffolds the native dir with real, buildable example code, and
    // default_template() already writes a [native] block with
    // build_deps. path is deliberately omitted there — resolve_native_dirs()
    // auto-detects src/ (and any other native dir) on its own.
    if let Some(lang) = native {
        let scaffolded = carrier_native::scaffold::scaffold(&src_dir, name, lang, backend)
            .with_context(|| format!("Failed to scaffold native code in {}", src_dir.display()))?;
        for f in scaffolded {
            files.push(format!("{}/{}", name, f));
        }
    } else {
        let scaffolded = carrier_native::scaffold::scaffold_pure_r(&src_dir)
            .with_context(|| format!("Failed to scaffold R-only example code in {}", src_dir.display()))?;
        for f in scaffolded {
            files.push(format!("{}/{}", name, f));
        }
    }
    
    println!("Initialized module '{}' in '{}'", name, project_dir_name);
    for f in &files {
        println!("  {}", f);
    }
    println!();
    println!(
        "Source directory: '{}/'\n\
         Rename it and set `src` in carrier.toml if you prefer a different name.",
        name
    );
    
    Ok(())
}
