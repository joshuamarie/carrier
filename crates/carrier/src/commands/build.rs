use anyhow::Result;
use std::path::PathBuf;

pub struct BuildArgs {
    pub path: String,
}

/// Thin CLI wrapper of `carrier_core::ops::build()`
pub fn run(args: BuildArgs) -> Result<()> {
    let project_root = PathBuf::from(&args.path);

    let built = carrier_core::ops::build::run(&project_root)?;

    if built.is_empty() {
        println!("No native code to build.");
        return Ok(());
    }

    for artifact in &built {
        println!(
            "Built {} -> {} ({})",
            artifact.native_dir.display(),
            artifact.artifact_path.display(),
            if artifact.from_cache { "cached" } else { "compiled" }
        );
    }
    println!();
    println!("Native code built in place. box::use() will pick it up on next load.");

    Ok(())
}
