use anyhow::Result;

pub struct InstallArgs {
    pub source: String,
    pub install_deps: bool,   // plain field, no #[arg]
}

pub fn run(args: InstallArgs) -> Result<()> {
    carrier_core::ops::install::run(&args.source, args.install_deps)
}
