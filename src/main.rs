extern crate bedder;
use bedder::column::{Column, ColumnReporter};
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
        help = "columns to output (format: name:type:description:number:value_parser)",
        short = 'c',
        long = "columns",
        required = true
    )]
    columns: Vec<String>,
}

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

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

    // Parse columns
    let columns: Vec<Column> = args
        .columns
        .iter()
        .map(|c| Column::try_from(c.as_str()))
        .collect::<Result<Vec<_>, _>>()?;

    // Print header
    writeln!(
        stdout,
        "#{}",
        columns
            .iter()
            .map(|c| c.name())
            .collect::<Vec<_>>()
            .join("\t")
    )?;

    // Use Python for columns that need it
    Python::with_gil(|py| {
        // Initialize Python expressions in columns if needed
        let py_columns: Vec<Column> = columns
            .into_iter()
            .map(|mut col| {
                if let Some(bedder::column::ValueParser::PythonExpression(expr)) = &col.value_parser
                {
                    let compiled = bedder::py::CompiledPython::new(py, expr, false)
                        .expect("error compiling Python expression");
                    col.py = Some(compiled);
                }
                col
            })
            .collect();
        log::info!("py_columns: {:?}", py_columns);
        bedder::py::initialize_python(py).expect("Failed to initialize Python environment");

        // Process intersections with columns
        for intersection in ii {
            let intersection = intersection.expect("error getting intersection");
            let values: Vec<String> = py_columns
                .iter()
                .map(|col| match col.value(&intersection) {
                    Ok(val) => val.to_string(),
                    Err(e) => panic!("Error getting column value: {:?}", e),
                })
                .collect();
            if values.iter().any(|v| !v.is_empty()) {
                match writeln!(stdout, "{}", values.join("\t")) {
                    Ok(_) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => {
                        std::process::exit(0);
                    }
                    Err(e) => {
                        panic!("Error writing to stdout: {}", e);
                    }
                }
            }
        }
    });

    Ok(())
}
