use crate::cli::shared::{process_bedder, ClosestArgs, CommonArgs};
use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about=None, rename_all = "kebab-case", help_template = crate::cli::shared::HELP_TEMPLATE, arg_required_else_help = true)]
pub struct ClosestCmdArgs {
    #[command(flatten)]
    pub common: CommonArgs,

    #[command(flatten)]
    pub closest: ClosestArgs,
}

pub fn closest_command(args: ClosestCmdArgs) -> Result<(), Box<dyn std::error::Error>> {
    process_bedder(args.common, None, Some(args.closest))
}
