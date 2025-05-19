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
        help = "intersection mode for a-file",
        short = 'm',
        long = "a-mode",
        default_value = "default"
    )]
    intersection_mode: IntersectionMode,

    #[arg(
        help = "intersection mode for b-file",
        short = 'M',
        long = "b-mode",
        default_value = "default"
    )]
    b_mode: IntersectionMode,

    #[arg(help = "a-part", short = 'p', long = "a-part", default_value = "whole")]
    a_part: IntersectionPart,

    #[arg(help = "b-part", long = "b-part", default_value = "whole")]
    b_part: IntersectionPart,

    #[arg(
        help = "a-requirements for overlap. A float value < 1 or a number ending with % will be the fraction (or %) of the interval. An integer will be the number of bases.",
        short = 'r',
        long = "a-requirements",
        default_value = "1"
    )]
    a_requirements: OverlapAmount,

    #[arg(
        help = "b-requirements for overlap. A float value < 1 or a number ending with % will be the fraction (or %) of the interval. An integer will be the number of bases.",
        short = 'R',
        long = "b-requirements",
        default_value = "1"
    )]
    b_requirements: OverlapAmount,

    #[arg(
        help = "python file with functions to be used in columns",
        short = 'P',
        long = "python"
    )]
    python_file: Option<PathBuf>,
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

    let ii = bedder::intersection::IntersectionIterator::new(a_iter, b_iters, &chrom_order)?;

    // Convert sniff::FileType to hts_format::Format
    let mut output_format = match query_file_type {
        bedder::sniff::FileType::Bed => Format::Bed,
        bedder::sniff::FileType::Vcf => Format::Vcf,
        bedder::sniff::FileType::Bcf => Format::Bcf,
    };
    if output_format == Format::Vcf && args.output_path.to_str().unwrap().ends_with(".bcf") {
        output_format = Format::Bcf
    }

    let columns: Vec<Column> = args
        .columns
        .iter()
        .map(|c| Column::try_from(c.as_str()))
        .collect::<Result<Vec<_>, _>>()?;

    let mut output = Writer::init(
        args.output_path.to_str().unwrap(),
        Some(output_format),
        None,
        input_header_for_writer,
        &columns,
    )?;

    let report_options = Arc::new(
        ReportOptions::builder()
            .a_mode(args.intersection_mode)
            .a_part(args.a_part)
            .b_part(args.b_part)
            .a_requirements(args.a_requirements)
            .b_requirements(args.b_requirements)
            .build(),
    );
    log::info!("report_options: {:?}", report_options);
    // Use Python for columns that need it
    Python::with_gil(|py| {
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

        let py_columns: Vec<Column<'_>> = columns
            .into_iter()
            .map(|mut col| {
                if let Some(bedder::column::ValueParser::PythonExpression(function_name)) =
                    &col.value_parser
                {
                    let compiled =
                        bedder::py::CompiledPython::new(py, function_name, &functions_map)
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
            let mut intersection = intersection.expect("error getting intersection");
            output.write(&mut intersection, report_options.clone(), &py_columns)?;
        }
        Ok::<(), Box<dyn std::error::Error>>(())
    })
}
