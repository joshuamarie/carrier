mod commands; 

use anyhow::Result;
use clap::{Parser, Subcommand};

use commands::{
    build::BuildArgs,
    bundle::BundleArgs,
    init::InitArgs,
    install::InstallArgs,
    lock::LockArgs,
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

        /// Scaffold compiled-code support: c or cpp
        #[arg(long)]
        native: Option<String>,

        /// Binding library for --native (cpp: rcpp (default) or cpp11)
        #[arg(long)]
        backend: Option<String>,
    },

    /// Bundle a module into <name>_<version>.tar.gz (default) or .rmbx
    Bundle {
        /// Path to the project root (e.g. `.` or `./my-project`)
        path: String,

        /// Bundle as .rmbx instead of the default .tar.gz
        #[arg(long)]
        rmbx: bool,

        /// Also compile native code in place and include the tagged
        /// binary in the archive alongside source. A mismatched or
        /// missing tag on install falls back to compiling from the
        /// source that ships in the same archive either way.
        #[arg(long)]
        binary: bool,
    },

    /// Compile a module's native code in place for local dev/testing.
    /// Does not delete the native source, unlike the build step that
    /// runs during `install`.
    Build {
        /// Path to the project root (e.g. `.` or `./my-project`)
        path: String,
    },

    /// Install a module from a .tar.gz, .rmbx, GitHub (gh:user/repo), or
    /// a module registry (bare name + --repo; registries aren't
    /// implemented yet)
    Install {
        /// The module source
        source: String,
        #[arg(long, help = "Automatically install R package dependencies from CRAN")]
        install_deps: bool,
        #[arg(long, help = "Registry URL to install SOURCE from (registries aren't implemented yet)")]
        repo: Option<String>,
        // /// Build compiled code if present: c, rcpp, or rust
        // #[arg(long)]
        // native: Option<String>,
    },

    /// Resolve R package dependencies and write carrier.lock, without
    /// installing anything
    Lock {
        /// Path to the project root (e.g. `.` or `./my-project`)
        path: String,

        /// Ignore any existing carrier.lock and re-resolve everything
        /// fresh instead of reusing its pins
        #[arg(long)]
        update: bool,
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
        Commands::Init { name, dir_name, native, backend } => {
            commands::init::run(InitArgs { name, dir_name, native, backend })
        }
        Commands::Build { path } => {
            commands::build::run(BuildArgs { path })
        }
        Commands::Bundle { path, rmbx, binary } => {
            commands::bundle::run(BundleArgs { path, rmbx, binary })
        }
        // Commands::Install { source, install_deps, repo, native } => {
        //     commands::install::run(InstallArgs { source, install_deps, repo, native })
        // }
        Commands::Install { source, install_deps, repo } => {
            commands::install::run(InstallArgs { source, install_deps, repo })
        }
        Commands::Lock { path, update } => {
            commands::lock::run(LockArgs { path, update })
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
