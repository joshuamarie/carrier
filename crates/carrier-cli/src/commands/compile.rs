use anyhow::Result;
use std::path::PathBuf;

pub struct CompileArgs {
    pub path: String,
    pub clean: bool,
}

/// Thin CLI wrapper of `carrier_core::ops::compile()`
pub fn run(args: CompileArgs) -> Result<()> {
    let project_root = PathBuf::from(&args.path);

    if args.clean {
        println!("Clearing native build cache before compiling...");
    }

    let compiled = carrier_core::ops::compile::run(&project_root, args.clean)?;

    if compiled.is_empty() {
        println!("No native code to compile.");
        return Ok(());
    }

    for artifact in &compiled {
        println!(
            "Compiled {} -> {} ({})",
            artifact.native_dir.display(),
            artifact.artifact_path.display(),
            if artifact.from_cache { "cached" } else { "compiled" }
        );
    }
    println!();
    println!("Native code compiled in place. box::use() will pick it up on next load.");

    Ok(())
}
