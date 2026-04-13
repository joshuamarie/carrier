use anyhow::{bail, Context, Result};
use std::fs;
use std::path::PathBuf;

use crate::carrier_toml::CarrierToml;

pub fn run(name: &str, dir_name: Option<&str>) -> Result<()> {
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
        CarrierToml::default_template(name),
    )
    .context("Failed to write carrier.toml")?;

    fs::write(
        project_root.join("README.md"),
        format!("# {}\n\nA box module.\n", name),
    )
    .context("Failed to write README.md")?;

    let src_dir = project_root.join(name);
    fs::create_dir_all(&src_dir)
        .with_context(|| format!("Failed to create source directory: {}", src_dir.display()))?;

    fs::write(
        src_dir.join("__init__.R"),
        "#' @export\nbox::use(./md)\n",
    )
    .context("Failed to write __init__.R")?;

    let md_dir = src_dir.join("md");
    fs::create_dir_all(&md_dir)
        .context("Failed to create md/ directory")?;

    fs::write(
        md_dir.join("__init__.R"),
        "#' @export\nbox::use(./hello)\n",
    )
    .context("Failed to write md/__init__.R")?;

    fs::write(
        md_dir.join("hello.R"),
        format!(
            "#' Say hello\n#'\n#' @export\nhello = function() {{\n  \"Hello from {}!\"\n}}\n",
            name
        ),
    )
    .context("Failed to write hello.R")?;

    let files = [
        "carrier.toml",
        "README.md",
        &format!("{name}/__init__.R"),
        &format!("{name}/md/__init__.R"),
        &format!("{name}/md/hello.R"),
    ];

    println!("Initialized module '{}' in '{}'", name, project_dir_name);
    for f in &files {
        println!("  {}/", f);
    }

    Ok(())
}
