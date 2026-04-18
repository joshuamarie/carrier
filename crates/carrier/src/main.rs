mod commands; 

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
#[command(version = env!("CARGO_PKG_VERSION"))]
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

        /// Override the project directory name.
        /// Defaults to <name>-proj if not specified.
        #[arg(long)]
        dir_name: Option<String>,
    },

    /// Bundle a module into <name>_<version>.tar.gz (default) or .rmbx
    Bundle {
        /// Path to the project root (e.g. `.` or `./my-project`)
        path: String,

        /// Bundle as .rmbx instead of the default .tar.gz
        #[arg(long)]
        rmbx: bool,
    },

    /// Install a module from a .tar.gz, .rmbx, or GitHub (gh:user/repo)
    Install {
        /// The module source
        source: String,
        #[arg(long, help = "Automatically install R package dependencies from CRAN")]
        install_deps: bool,
    },

    /// Remove an installed module
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
        Commands::Init { name, dir_name } => {
            commands::init::run(InitArgs { name, dir_name })
        }
        Commands::Bundle { path, rmbx } => {
            commands::bundle::run(BundleArgs { path, rmbx })
        }
        Commands::Install { source, install_deps } => {
            commands::install::run(InstallArgs { source, install_deps })
        }
        Commands::Remove { name, force } => {
            commands::remove::exec(RemoveArgs { name, force })
        }
    };

    if let Err(e) = result {
        eprintln!("error: {e:?}");
        std::process::exit(1);
    }
}
