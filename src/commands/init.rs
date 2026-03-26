use anyhow::{bail, Context, Result};
use std::fs;
use std::path::PathBuf;

use crate::carrier_toml::CarrierToml;

pub struct InitArgs {
    pub name: String,
    pub dir_name: Option<String>,
}

pub fn run(args: InitArgs) -> Result<()> {
    // Project root is either --dir-name or <name>-proj by default
    let default_dir = format!("{}-proj", args.name);
    let project_dir_name = args.dir_name
        .as_deref()
        .unwrap_or(&default_dir);

    let project_root = PathBuf::from(project_dir_name);

    if project_root.exists() {
        bail!("'{}' already exists.", project_root.display());
    }

    // --- Project root ---
    fs::create_dir_all(&project_root)
        .with_context(|| format!("Failed to create directory: {}", project_root.display()))?;

    // carrier.toml at project root
    fs::write(
        project_root.join("carrier.toml"),
        CarrierToml::default_template(&args.name),
    )
    .context("Failed to write carrier.toml")?;

    // README.md at project root
    fs::write(
        project_root.join("README.md"),
        format!("# {}\n\nA box module.\n", args.name),
    )
    .context("Failed to write README.md")?;

    // --- Source folder: <project_root>/<name>/ ---
    let src_dir = project_root.join(&args.name);
    fs::create_dir_all(&src_dir)
        .with_context(|| format!("Failed to create source directory: {}", src_dir.display()))?;

    // <name>/__init__.R — module entry point
    fs::write(
        src_dir.join("__init__.R"),
        "#' @export\nbox::use(./md)\n",
    )
    .context("Failed to write __init__.R")?;

    // --- <name>/md/ submodule ---
    let md_dir = src_dir.join("md");
    fs::create_dir_all(&md_dir)
        .context("Failed to create md/ directory")?;

    // <name>/md/__init__.R
    fs::write(
        md_dir.join("__init__.R"),
        "#' @export\nbox::use(./hello)\n",
    )
    .context("Failed to write md/__init__.R")?;

    // <name>/md/hello.R — stub implementation
    fs::write(
        md_dir.join("hello.R"),
        format!(
            "#' Say hello\n#'\n#' @export\nhello = function() {{\n  \"Hello from {}!\"\n}}\n",
            args.name
        ),
    )
    .context("Failed to write hello.R")?;

    let files = [
        "carrier.toml",
        "README.md",
        &format!("{n}/__init__.R", n = args.name),
        &format!("{n}/md/__init__.R", n = args.name),
        &format!("{n}/md/hello.R", n = args.name),
    ];

    println!("Initialized module '{}' in '{}'", args.name, project_dir_name);
    for f in &files {
        println!("  {}/", f);
    }

    Ok(())
}
