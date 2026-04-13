use anyhow::Result;

pub struct InitArgs {
    pub name: String,
    pub dir_name: Option<String>,
}

pub fn run(args: InitArgs) -> Result<()> {
    carrier_core::ops::init::run(&args.name, args.dir_name.as_deref())
}
