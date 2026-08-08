use anyhow::Result;

use carrier_native::NativeLang;

pub struct InstallArgs {
    pub source: String,
    pub install_deps: bool,
    pub repo: Option<String>,
    pub native: Option<String>,
}

pub fn run(args: InstallArgs) -> Result<()> {
    let native = args.native.as_deref().map(NativeLang::parse).transpose()?;
    carrier_core::ops::install::run(&args.source, args.install_deps, args.repo.as_deref(), native)
}
