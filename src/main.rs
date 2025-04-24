extern crate bedder;
use bedder::column::Column;
use bedder::hts_format::Format;
use bedder::report_options::{IntersectionMode, IntersectionPart, OverlapAmount, ReportOptions};
use bedder::writer::{InputHeader, Writer};
use clap::Parser;
use pyo3::prelude::*;
use std::env;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about=None, rename_all = "kebab-case")]
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
        long = "columns"
    )]
    columns: Vec<String>,

    #[arg(
        help = "output file (default: stdout)",
        short = 'o',
        long = "output",
        default_value = "-"
    )]
    output_path: PathBuf,

    #[arg(
        help = "intersection mode for a-file",
        short = 'm',
        default_value = "d"
    )]
    intersection_mode: IntersectionMode,

    #[arg(help = "a-part", short = 'p', default_value = "whole")]
    a_part: IntersectionPart,

    #[arg(help = "b-part", default_value = "whole")]
    b_part: IntersectionPart,

    #[arg(
        help = "a-requirements for overlap. A float value < 1 or a number ending with % will be the fraction (or %) of the interval. An integer will be the number of bases.",
        short = 'r',
        default_value = "1"
    )]
    a_requirements: OverlapAmount,
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

    /*
    let mut output: Box<dyn Write> = if args.output_path.to_str().unwrap() == "-" {
        Box::new(BufWriter::new(std::io::stdout().lock()))
    } else {
        Box::new(BufWriter::new(File::create(&args.output_path)?))
    };
    */

    let mut output = Writer::init(
        args.output_path.to_str().unwrap(),
        Some(Format::Bed),
        None,
        InputHeader::None,
    )?;

    // Parse columns
    let columns: Vec<Column> = args
        .columns
        .iter()
        .map(|c| Column::try_from(c.as_str()))
        .collect::<Result<Vec<_>, _>>()?;

    // Print header
    /*
    writeln!(
        output,
        "#{}",
        columns
            .iter()
            .map(|c| c.name())
            .collect::<Vec<_>>()
            .join("\t")
    )?;
    */

    let report_options = Arc::new(
        ReportOptions::builder()
            .a_mode(args.intersection_mode)
            .a_part(args.a_part)
            .b_part(args.b_part)
            .a_requirements(args.a_requirements)
            .build(),
    );
    // Use Python for columns that need it
    Python::with_gil(|py| {
        // Initialize Python expressions in columns if needed
        let py_columns: Vec<Column<'_>> = columns
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
        //log::info!("py_columns: {:?}", py_columns);
        bedder::py::initialize_python(py).expect("Failed to initialize Python environment");

        // Process intersections with columns
        for intersection in ii {
            // TODO: brent start here. need to auto call report when needed.
            let mut intersection = intersection.expect("error getting intersection");

            output.write(&mut intersection, report_options.clone(), &py_columns)?;
            /*
            let values: Vec<String> = py_columns
                .iter()
                .map(
                    |col| match col.value(&intersection, report_options.clone()) {
                        Ok(val) => val.to_string(),
                        Err(e) => panic!("Error getting column value: {:?}", e),
                    },
                )
                .collect();
            if values.iter().any(|v| !v.is_empty()) {
                match writeln!(output, "{}", values.join("\t")) {
                    Ok(_) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => {
                        std::process::exit(0);
                    }
                    Err(e) => {
                        panic!("Error writing to output({:?}): {}", args.output_path, e);
                    }
                }
            }
            */
        }
        Ok::<(), Box<dyn std::error::Error>>(())
    })
}
