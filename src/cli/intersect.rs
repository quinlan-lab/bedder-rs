use crate::cli::shared::{process_bedder, CommonArgs, OverlapArgs};
use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about=None, rename_all = "kebab-case", help_template = crate::cli::shared::HELP_TEMPLATE, arg_required_else_help = true)]
pub struct IntersectCmdArgs {
    #[command(flatten)]
    pub common: CommonArgs,

    #[command(flatten)]
    pub overlap: OverlapArgs,
}

pub fn intersect_command(args: IntersectCmdArgs) -> Result<(), Box<dyn std::error::Error>> {
    process_bedder(args.common, Some(args.overlap), None)
}
