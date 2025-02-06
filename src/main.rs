extern crate bedder;
use clap::Parser;
use pyo3::prelude::*;
use std::env;
use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about=None)]
struct Args {
    #[arg(help = "input file", short = 'a')]
    query_path: PathBuf,
    #[arg(help = "other file", short = 'b', required = true)]
    other_paths: Vec<PathBuf>,
    #[arg(
        help = "genome file for chromosome ordering",
        short = 'g',
        required = true
    )]
    genome_file: PathBuf,
    #[arg(
        help = "python f-string expression",
        short = 'f',
        default_value = "def main(intersection): return f'{intersection.base_interval.chrom}\t{intersection.base_interval.start}\t{intersection.base_interval.stop}\t{len(intersection.overlapping)}'"
    )]
    f_string: String,
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

    let afhile = BufReader::new(File::open(&args.query_path)?);

    let a_iter = bedder::sniff::open(afhile, &args.query_path)?;
    let b_iters: Vec<_> = args
        .other_paths
        .iter()
        .map(|p| -> Result<_, Box<dyn std::error::Error>> {
            let fh = BufReader::new(File::open(p)?);
            Ok(bedder::sniff::open(fh, p)?.into_positioned_iterator())
        })
        .collect::<Result<Vec<_>, _>>()?;

    let bedder::sniff::BedderReader::BedderBed(aiter) = a_iter;

    let ii = bedder::intersection::IntersectionIterator::new(aiter, b_iters, &chrom_order)?;

    let mut stdout = BufWriter::new(std::io::stdout().lock());

    Python::with_gil(|py| {
        let compiled =
            bedder::py::CompiledPython::new(py, &args.f_string).expect("error compiling f-string");

        // iterate over the intersections
        ii.for_each(|intersection| {
            let intersection = intersection.expect("error getting intersection");
            let py_intersection = bedder::py::PyIntersections::from(intersection);
            match compiled.eval(py_intersection) {
                Ok(result) => writeln!(stdout, "{}", result).expect("error writing to stdout"),
                Err(e) => eprintln!("Error formatting: {}", e),
            }
        });
    });
    Ok(())
}
