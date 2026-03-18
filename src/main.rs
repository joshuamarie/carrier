mod carrier_toml;
mod commands;
mod formats;
mod manifest;

use anyhow::Result;
use clap::{Parser, Subcommand};

use commands::{
    bundle::BundleArgs,
    init::InitArgs,
    install::InstallArgs,
    remove::RemoveArgs,
};

#[derive(Parser)]
#[command(name = "carrier")]
#[command(version = "0.1.0")]
#[command(about = "A bundler and package manager for box modules")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Scaffold a new box module
    Init {
        /// Name of the module to create
        name: String,
    },

    /// Bundle a module directory into .mod/<name>.rmbx
    Bundle {
        /// Path to the module directory (e.g. ./play or play)
        path: String,

        /// Overwrite existing bundle
        #[arg(long)]
        force: bool,
    },

    /// Install a module from a local path or GitHub (gh:user/repo)
    Install {
        /// Module source: a local path or gh:username/repo
        source: String,

        /// Overwrite if already installed
        #[arg(long)]
        force: bool,

        /// Install to ~/.carrier/modules/ instead of project .mod/
        #[arg(long)]
        global: bool,
    },

    /// Remove an installed module from .mod/
    Remove {
        /// Name of the module to remove
        name: String,

        /// Skip confirmation prompt
        #[arg(long)]
        force: bool,
    },
}

fn main() {
    let cli = Cli::parse();

    let result: Result<()> = match cli.command {
        Commands::Init { name } => {
            commands::init::run(InitArgs { name })
        }
        Commands::Bundle { path, force } => {
            commands::bundle::run(BundleArgs { path, force })
        }
        Commands::Install { source, force, global } => {
            commands::install::run(InstallArgs { source, force, global })
        }
        Commands::Remove { name, force } => {
            commands::remove::run(RemoveArgs { name, force })
        }
    };

    if let Err(e) = result {
        eprintln!("error: {e:?}");
        std::process::exit(1);
    }
}
