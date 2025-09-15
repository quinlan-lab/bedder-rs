extern crate bedder;
use bedder::column::Column;
use bedder::hts_format::Format;
use bedder::report_options::{IntersectionMode, IntersectionPart, OverlapAmount, ReportOptions};
use bedder::writer::{InputHeader, Writer};
use clap::Parser;
use pyo3::prelude::*;
use std::collections::HashMap;
use std::env;
use std::ffi::CString;
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
        long = "genome",
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
        help = "intersection mode for a-file. this determines how the overlap requirements are accumulated.",
        short = 'm',
        long = "a-mode",
        default_value = "default"
    )]
    intersection_mode: IntersectionMode,

    #[arg(
        help = "intersection mode for b-file. this determines how the overlap requirements are accumulated.",
        short = 'M',
        long = "b-mode",
        default_value = "default"
    )]
    b_mode: IntersectionMode,

    #[arg(
        help = "the piece of the a intervals to report",
        short = 'p',
        long = "a-piece",
        default_value = "whole"
    )]
    a_piece: IntersectionPart,

    #[arg(
        help = "the piece of the b intervals to report",
        long = "b-piece",
        short = 'P',
        default_value = "whole"
    )]
    b_piece: IntersectionPart,

    #[arg(
        help = "a-requirements for overlap. A float value < 1 or a number ending with % will be the fraction (or %) of the interval. An integer will be the number of bases. Default is 1 unless n-closest is set.",
        short = 'r',
        long = "a-requirements"
    )]
    a_requirements: Option<OverlapAmount>,

    #[arg(
        help = "b-requirements for overlap. A float value < 1 or a number ending with % will be the fraction (or %) of the interval. An integer will be the number of bases. Default is 1 unless n-closest is set.",
        short = 'R',
        long = "b-requirements"
    )]
    b_requirements: Option<OverlapAmount>,

    #[arg(
        help = "python file with functions to be used in columns",
        long = "python"
    )]
    python_file: Option<PathBuf>,

    #[arg(
        long = "n-closest",
        short = 'n',
        help = "report the n-closest intervals.
By default, all overlapping intervals are reported.
If n-closest is set, then the n closest intervals are reported, regardless of overlap.
When used, the default overlap requirement is set to 0, so that non-overlapping intervals can be reported.
This is mutually exclusive with --a-requirements and --b-requirements."
    )]
    n_closest: Option<i64>,

    #[arg(
        long = "max-distance",
        short = 'd',
        help = "maximum distance to search for closest intervals.
By default, there is no distance limit.
When used, the default overlap requirement is set to 0, so that non-overlapping intervals can be reported.
This can be overridden by setting a-requirements and b-requirements."
    )]
    max_distance: Option<i64>,

    #[arg(
        long = "dont-use-indexes",
        short = 'i',
        help = "don't use indexed query"
    )]
    dont_use_indexes: bool,
}

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "bedder=warn");
    }
    env_logger::init();
    log::info!("starting up");
    let mut args = Args::parse();

    if args.n_closest.is_some() {
        if args.a_requirements.is_some() || args.b_requirements.is_some() {
            log::error!("Cannot specify --n-closest with --a-requirements or --b-requirements. The 'closest' command is for finding nearest intervals, which may not overlap so overlap requirements are not applicable.");
            std::process::exit(1);
        }
        args.b_piece = IntersectionPart::Whole;
    }

    let chrom_order =
        bedder::chrom_ordering::parse_genome(std::fs::File::open(&args.genome_file)?)?;

    let afhile = BufReader::new(File::open(&args.query_path)?);

    let (a_bed_reader_obj, query_file_type) = bedder::sniff::open(afhile, &args.query_path)?;

    let input_header_for_writer: InputHeader = match query_file_type {
        bedder::sniff::FileType::Vcf | bedder::sniff::FileType::Bcf => {
            // a_bed_reader_obj is of type bedder::sniff::BedderReader
            // We need to match it to get to the underlying BedderVCF if present.
            match &a_bed_reader_obj {
                // Take a reference for inspection
                bedder::sniff::BedderReader::BedderVcf(vcf_reader) => {
                    InputHeader::Vcf(vcf_reader.header.clone())
                }
                _ => {
                    // This case implies that sniff::open returned a FileType (Vcf/Bcf)
                    // but the BedderReader enum variant doesn't match BedderVcf.
                    // This shouldn't happen if sniff::open is consistent.
                    log::warn!(
                        "Query file type is {:?} but reader is not BedderVcf, cannot extract header.",
                        query_file_type
                    );
                    InputHeader::None
                }
            }
        }
        bedder::sniff::FileType::Bed => {
            // BED files don't have a structured header in the way VCF/BCF do.
            InputHeader::None
        } // Other file types like SAM/BAM would be handled here if supported and they have headers.
    };

    let a_iter = a_bed_reader_obj.into_positioned_iterator();

    let b_iters: Vec<_> = args
        .other_paths
        .iter()
        .map(|p| -> Result<_, Box<dyn std::error::Error>> {
            let fh = BufReader::new(File::open(p)?);
            let (b_reader, _) = bedder::sniff::open(fh, p)?;
            Ok(b_reader.into_positioned_iterator())
        })
        .collect::<Result<Vec<_>, _>>()?;

    // We can skip ahead when we don't need to report all query intervals.
    // This is true when:
    // - a_piece is None (don't report query intervals at all - PERFECT for skipping)
    // - a_piece is Piece (only report non-overlapping parts - can skip when no overlaps exist far ahead)
    // We cannot skip when a_piece is Whole (always report entire query intervals).
    let can_skip_ahead = !matches!(args.a_piece, IntersectionPart::Whole) && !args.dont_use_indexes;

    let ii = bedder::intersection::IntersectionIterator::new(
        a_iter,
        b_iters,
        &chrom_order,
        args.max_distance.unwrap_or(-1),
        args.n_closest.unwrap_or(-1),
        can_skip_ahead,
    )?;

    // Convert sniff::FileType to hts_format::Format
    let mut output_format = match query_file_type {
        bedder::sniff::FileType::Bed => Format::Bed,
        bedder::sniff::FileType::Vcf => Format::Vcf,
        bedder::sniff::FileType::Bcf => Format::Bcf,
    };

    // if input is vcf and output ends with .bcf, then set output to bcf
    if output_format == Format::Vcf && args.output_path.to_str().unwrap().ends_with(".bcf") {
        output_format = Format::Bcf
    }

    let a_reqs = args.a_requirements.clone();
    let b_reqs = args.b_requirements.clone();

    let (a_requirements, b_requirements) =
        if args.n_closest.is_some() || args.max_distance.is_some() {
            (
                a_reqs.unwrap_or(OverlapAmount::Bases(0)),
                b_reqs.unwrap_or(OverlapAmount::Bases(0)),
            )
        } else {
            (
                a_reqs.unwrap_or(OverlapAmount::Bases(1)),
                b_reqs.unwrap_or(OverlapAmount::Bases(1)),
            )
        };

    let report_options = Arc::new(
        ReportOptions::builder()
            .a_mode(args.intersection_mode)
            .a_piece(args.a_piece)
            .b_piece(args.b_piece)
            .a_requirements(a_requirements)
            .b_requirements(b_requirements)
            .build(),
    );
    log::info!("report_options: {:?}", report_options);
    // Use Python for columns that need it
    Python::attach(|py| {
        // Initialize Python expressions in columns if needed

        let mut functions_map = HashMap::new();

        if let Some(python_file) = &args.python_file {
            let file = File::open(python_file)?;
            let code = std::io::read_to_string(file)?;
            let c_code = CString::new(code.as_str())?;
            py.run(&c_code, None, None)?;

            // Introspect loaded functions
            log::info!("Introspecting functions loaded from Python file:");
            let main_module = py.import("__main__")?;
            let globals = main_module.dict();

            functions_map = bedder::py::introspect_python_functions(py, globals)?;
            log::info!("python functions map: {:?}", &functions_map);
        }

        let columns: Vec<Column> = args
            .columns
            .iter()
            .map(|c| {
                let (s, functions_map) = (c.as_str(), &functions_map);
                Column::try_from((s, functions_map))
            })
            .collect::<Result<Vec<_>, _>>()?;

        let mut output = Writer::init(
            args.output_path.to_str().unwrap(),
            Some(output_format),
            None,
            input_header_for_writer,
            &columns,
        )?;

        let py_columns: Vec<Column<'_>> = columns
            .into_iter()
            .map(|mut col| {
                if let Some(bedder::column::ValueParser::PythonExpression(function_name)) =
                    &col.value_parser
                {
                    let compiled =
                        bedder::py::CompiledPython::new(py, function_name, &functions_map)
                            .expect("error compiling Python expression");
                    eprintln!("compiled: {:?}", compiled);
                    col.py = Some(compiled);
                }
                col
            })
            .collect();
        log::info!("py_columns: {:?}", py_columns);
        bedder::py::initialize_python(py).expect("Failed to initialize Python environment");

        // Process intersections with columns
        for intersection in ii {
            let mut intersection = intersection.expect("error getting intersection");
            output.write(&mut intersection, report_options.clone(), &py_columns)?;
        }
        Ok::<(), Box<dyn std::error::Error>>(())
    })
}
