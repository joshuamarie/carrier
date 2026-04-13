use anyhow::Result;

pub struct RemoveArgs {
    pub name: String,
    pub force: bool,
}

pub fn exec(args: RemoveArgs) -> Result<()> {
    carrier_core::ops::remove::run(&args.name, args.force)
}
