use anyhow::{bail, Context, Result};
use std::io::{self, Write};

pub struct RemoveArgs {
    pub name: String,
    pub force: bool,
}

pub fn exec(args: RemoveArgs) -> Result<()> {
    let mod_dir = std::env::current_dir()
        .context("Failed to get current working directory")?
        .join(".mod");

    // .mod/<name>/ — the unpacked module folder
    let module_path = mod_dir.join(&args.name);

    if !module_path.exists() {
        bail!(
            "Module '{}' is not installed (.mod/{}/ not found).",
            args.name, args.name
        );
    }

    if !args.force {
        let confirmed = prompt_confirm(&format!(
            "Remove '{}'? This cannot be undone. [y/N] ",
            args.name
        ))?;

        if !confirmed {
            println!("Aborted.");
            return Ok(());
        }
    }

    std::fs::remove_dir_all(&module_path)
        .with_context(|| format!("Failed to remove: {}", module_path.display()))?;

    println!("Removed '{}'.", args.name);

    Ok(())
}

fn prompt_confirm(message: &str) -> Result<bool> {
    print!("{}", message);
    io::stdout().flush().context("Failed to flush stdout")?;

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .context("Failed to read input")?;

    Ok(matches!(input.trim().to_lowercase().as_str(), "y" | "yes"))
}
