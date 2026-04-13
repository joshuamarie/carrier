use anyhow::Result;

pub struct BundleArgs {
    pub path: String,
    pub rmbx: bool,
}

pub fn run(args: BundleArgs) -> Result<()> {
    carrier_core::ops::bundle::run(&args.path, args.rmbx)
}
