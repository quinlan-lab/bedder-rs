extern crate bedder;
use bedder::sniff::detect_file_format;
use clap::Parser;
use std::env;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about=None)]
struct Args {
    #[arg(help = "input file", short = 'a')]
    query_path: PathBuf,
    #[arg(help = "other file", short = 'b')]
    other_path: PathBuf,
}

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "bedder=info");
    }
    env_logger::init();
    log::info!("starting up");
    let args = Args::parse();
    let mut a_reader = std::io::BufReader::new(std::fs::File::open(&args.query_path)?);
    let (a_format, a_compression) = detect_file_format(&mut a_reader, &args.query_path)?;
    log::info!(
        "-a: format: {:?} compression: {:?}",
        a_format,
        a_compression
    );
    let mut b_reader = std::io::BufReader::new(std::fs::File::open(&args.other_path)?);
    let (b_format, b_compression) = detect_file_format(&mut b_reader, &args.other_path)?;
    log::info!(
        "-b: format: {:?} compression: {:?}",
        b_format,
        b_compression
    );
    Ok(())
}
