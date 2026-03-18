use anyhow::{bail, Context, Result};
use std::fs;
use std::path::PathBuf;

use crate::carrier_toml::CarrierToml;

pub struct InitArgs {
    pub name: String,
}

pub fn run(args: InitArgs) -> Result<()> {
    let module_path = PathBuf::from(&args.name);

    if module_path.exists() {
        bail!("'{}' already exists.", module_path.display());
    }

    // carrier.toml
    fs::create_dir_all(&module_path)
        .with_context(|| format!("Failed to create directory: {}", module_path.display()))?;
    fs::write(
        module_path.join("carrier.toml"),
        CarrierToml::default_template(&args.name),
    )
    .context("Failed to write carrier.toml")?;

    // __init__.R — module entry point
    // explicitly re-exports from the md/ submodule
    fs::write(
        module_path.join("__init__.R"),
        "#' @export\nbox::use(./md)\n",
    )
    .context("Failed to write __init__.R")?;

    // md/__init__.R — submodule entry point
    let md_dir = module_path.join("md");
    fs::create_dir_all(&md_dir)
        .context("Failed to create md/ directory")?;
    fs::write(
        md_dir.join("__init__.R"),
        format!(
            "#' @export\nbox::use(./hello)\n",
        ),
    )
    .context("Failed to write md/__init__.R")?;

    // md/hello.R — stub implementation
    fs::write(
        md_dir.join("hello.R"),
        format!(
            "#' Say hello\n#'\n#' @export\nhello = function() {{\n  \"Hello from {}!\"\n}}\n",
            args.name
        ),
    )
    .context("Failed to write md/hello.R")?;

    // README.md
    fs::write(
        module_path.join("README.md"),
        format!("# {}\n\nA box module.\n", args.name),
    )
    .context("Failed to write README.md")?;

    println!("Initialized module '{}'", args.name);
    println!("  {}/carrier.toml", args.name);
    println!("  {}/__init__.R", args.name);
    println!("  {}/md/__init__.R", args.name);
    println!("  {}/md/hello.R", args.name);
    println!("  {}/README.md", args.name);

    Ok(())
}
