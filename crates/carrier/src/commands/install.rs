use anyhow::Result;

pub struct InstallArgs {
    pub source: String,
}

pub fn run(args: InstallArgs) -> Result<()> {
    carrier_core::ops::install::run(&args.source)
}
