extern crate bedder;
use bedder::sniff::detect_file_format;
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about=None)]
struct Args {
    #[arg(help = "input file")]
    query_path: PathBuf,
}

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let mut reader = std::io::BufReader::new(std::fs::File::open(&args.query_path)?);
    let (format, compression) = detect_file_format(&mut reader, &args.query_path)?;
    println!("format: {:?}", format);
    println!("compression: {:?}", compression);
    Ok(())
}
