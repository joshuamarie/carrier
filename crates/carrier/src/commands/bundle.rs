use anyhow::Result;

pub struct BundleArgs {
    pub path: String,
    pub rmbx: bool,
    pub binary: bool, 
    pub keep_source: bool, 
}

pub fn run(args: BundleArgs) -> Result<()> {
    carrier_core::ops::bundle::run(&args.path, args.rmbx, args.binary, args.keep_source)
}
