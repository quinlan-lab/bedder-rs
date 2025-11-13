extern crate bedder;
mod cli;
use clap::{Parser, Subcommand};
use std::env;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about=None, rename_all = "kebab-case", help_template = cli::shared::HELP_TEMPLATE, arg_required_else_help = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Full functionality with all options (almost never use this)
    Full(cli::full::FullCmdArgs),
    /// Intersection mode - hides closest options
    Intersect(cli::intersect::IntersectCmdArgs),
    /// Closest mode - hides overlap requirements
    Closest(cli::closest::ClosestCmdArgs),
}

#[cfg(feature = "mimalloc_allocator")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "bedder=warn");
    }
    env_logger::init();
    log::trace!("starting up");

    let cli = Cli::parse();

    match cli.command {
        Commands::Full(args) => cli::full::full_command(args),
        Commands::Intersect(args) => cli::intersect::intersect_command(args),
        Commands::Closest(args) => cli::closest::closest_command(args),
    }
}
