use anyhow::Result;

pub struct LockArgs {
    pub path: String,
    pub update: bool,
}

pub fn run(args: LockArgs) -> Result<()> {
    carrier_core::ops::lock::run(&args.path, args.update)
}
