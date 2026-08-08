use anyhow::Result;

use carrier_native::NativeLang;

pub struct InitArgs {
    pub name: String,
    pub dir_name: Option<String>,
    pub native: Option<String>,
}

pub fn run(args: InitArgs) -> Result<()> {
    let native = args.native.as_deref().map(NativeLang::parse).transpose()?;
    carrier_core::ops::init::run(&args.name, args.dir_name.as_deref(), native)
}
