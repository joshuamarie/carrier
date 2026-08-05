use anyhow::Result;

pub struct InstallArgs {
    pub source: String,
    pub install_deps: bool,
    pub repo: Option<String>,
}

pub fn run(args: InstallArgs) -> Result<()> {
    carrier_core::ops::install::run(&args.source, args.install_deps, args.repo.as_deref())
}
