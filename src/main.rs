extern crate bedder;
use bedder::intersections::{IntersectionMode, IntersectionPart};
use bedder::report::ReportFragment;
use clap::Parser;
use pyo3::prelude::*;
use std::env;
use std::fs::File;
use std::io::BufReader;
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
        default_value = "def default(report): return f'{report.a.chrom}\t{report.a.start}\t{report.a.stop}\t{len(report.b)}'"
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

    Python::with_gil(|py| {
        let mut compiled =
            bedder::py::CompiledFString::new(py, &args.f_string).expect("error compiling f-string");

        // iterate over the intersections
        ii.for_each(|intersection| {
            let intersection = intersection.expect("error getting intersection");
            let report = intersection.report(
                &IntersectionMode::Default,
                &IntersectionMode::Default,
                &IntersectionPart::Whole,
                &IntersectionPart::Whole,
                &bedder::intersections::OverlapAmount::Bases(1),
                &bedder::intersections::OverlapAmount::Bases(1),
            );
            if let Some(fragment) = report.into_iter().next() {
                let py_fragment = bedder::py::PyReportFragment::from(fragment.clone());
                match compiled.eval(py, py_fragment) {
                    Ok(result) => println!("{}", result),
                    Err(e) => eprintln!("Error formatting: {}", e),
                }
            }
        });
    });
    Ok(())
}
