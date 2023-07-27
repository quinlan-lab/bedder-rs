extern crate bedder;
use bedder::sniff;
use clap::Parser;
use std::env;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about=None)]
struct Args {
    #[arg(help = "input file", short = 'a')]
    query_path: PathBuf,
    #[arg(help = "other file(s)", short = 'b', required = true)]
    other_paths: Vec<PathBuf>,
    #[arg(
        help = "genome file for chromosome ordering",
        short = 'g',
        required = true
    )]
    genome_file: PathBuf,
}

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "bedder=info");
    }
    env_logger::init();
    log::info!("starting up");
    let args = Args::parse();

    let chrom_order =
        bedder::chrom_ordering::parse_genome(std::fs::File::open(&args.genome_file)?)?;

    let a_iter = sniff::open_file(&args.query_path)?;
    let b_iters: Vec<_> = args
        .other_paths
        .iter()
        .map(|p| sniff::open_file(p).expect("error opening file"))
        .collect();

    let iter = bedder::intersection::IntersectionIterator::new(a_iter, b_iters, &chrom_order)?;
    // iterate over the intersections
    iter.for_each(|intersection| {
        let intersection = intersection.expect("error getting intersection");
        println!("{:?}", intersection);
    });
    Ok(())
}
