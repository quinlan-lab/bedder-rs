use bedder::column::Column;
use bedder::hts_format::Format;
use bedder::report_options::{IntersectionMode, IntersectionPart, OverlapAmount, ReportOptions};
use bedder::writer::{InputHeader, Writer};
use clap::Parser;
use pyo3::prelude::*;
use std::ffi::CString;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::Arc;

pub const HELP_TEMPLATE: &str =
    "{name} v{version}\n{about}\n\n{usage-heading} {usage}\n\n{all-args}{after-help}";

#[derive(Parser, Debug)]
pub struct CommonArgs {
    #[arg(help = "input file", short = 'a')]
    pub query_path: PathBuf,

    #[arg(help = "other file", short = 'b', required = true)]
    pub other_paths: Vec<PathBuf>,

    #[arg(
        help = "genome file for chromosome ordering",
        short = 'g',
        long = "genome",
        required = true
    )]
    pub genome_file: PathBuf,

    #[arg(
        help = "columns to output (format: name:type:description:number:value_parser)",
        short = 'c',
        long = "columns"
    )]
    pub columns: Vec<String>,

    #[arg(
        help = "output file (default: stdout)",
        short = 'o',
        long = "output",
        default_value = "-"
    )]
    pub output_path: PathBuf,

    #[arg(
        help = "python file with functions to be used in columns",
        long = "python"
    )]
    pub python_file: Option<PathBuf>,

    #[arg(
        help = "optional filter expression (Python boolean expression; 'r' and 'fragment' are the current report fragment) indicates if the fragment should be included in the output",
        long = "filter",
        short = 'f'
    )]
    pub filter: Option<String>,

    #[arg(
        long = "dont-use-indexes",
        short = 'i',
        help = "don't use indexed query"
    )]
    pub dont_use_indexes: bool,
}

#[derive(Parser, Debug)]
pub struct OverlapArgs {
    #[arg(
        help = "intersection mode for a-file. this determines how the overlap requirements are accumulated.",
        short = 'm',
        long = "a-mode",
        default_value = "default"
    )]
    pub intersection_mode: IntersectionMode,

    #[arg(
        help = "intersection mode for b-file. this determines how the overlap requirements are accumulated.",
        short = 'M',
        long = "b-mode",
        default_value = "default"
    )]
    pub b_mode: IntersectionMode,

    #[arg(
        help = "the piece of the a intervals to report",
        short = 'p',
        long = "a-piece",
        default_value = "whole"
    )]
    pub a_piece: IntersectionPart,

    #[arg(
        help = "the piece of the b intervals to report",
        long = "b-piece",
        short = 'P',
        default_value = "whole"
    )]
    pub b_piece: IntersectionPart,

    #[arg(
        help = "a-requirements for overlap. A float value < 1 or a number ending with % will be the fraction (or %) of the interval. An integer will be the number of bases. Default is 1 unless n-closest is set.",
        short = 'r',
        long = "a-requirements"
    )]
    pub a_requirements: Option<OverlapAmount>,

    #[arg(
        help = "b-requirements for overlap. A float value < 1 or a number ending with % will be the fraction (or %) of the interval. An integer will be the number of bases. Default is 1 unless n-closest is set.",
        short = 'R',
        long = "b-requirements"
    )]
    pub b_requirements: Option<OverlapAmount>,
}

#[derive(Parser, Debug)]
pub struct ClosestArgs {
    #[arg(
        long = "n-closest",
        short = 'n',
        help = "report the n-closest intervals.
By default, all overlapping intervals are reported.
If n-closest is set, then the n closest intervals are reported, regardless of overlap.
When used, the default overlap requirement is set to 0, so that non-overlapping intervals can be reported."
    )]
    pub n_closest: Option<i64>,

    #[arg(
        long = "max-distance",
        short = 'd',
        help = "maximum distance to search for closest intervals.
By default, there is no distance limit.
When used, the default overlap requirement is set to 0, so that non-overlapping intervals can be reported."
    )]
    pub max_distance: Option<i64>,
}

pub fn process_bedder(
    common_args: CommonArgs,
    overlap_args: Option<OverlapArgs>,
    closest_args: Option<ClosestArgs>,
) -> Result<(), Box<dyn std::error::Error>> {
    let n_closest = closest_args.as_ref().and_then(|c| c.n_closest);
    let max_distance = closest_args.as_ref().and_then(|c| c.max_distance);
    let a_requirements = overlap_args.as_ref().and_then(|o| o.a_requirements.clone());
    let b_requirements = overlap_args.as_ref().and_then(|o| o.b_requirements.clone());
    let intersection_mode = overlap_args
        .as_ref()
        .map(|o| o.intersection_mode.clone())
        .unwrap_or_default();
    let b_mode = overlap_args
        .as_ref()
        .map(|o| o.b_mode.clone())
        .unwrap_or_default();
    let a_piece = overlap_args
        .as_ref()
        .map(|o| o.a_piece.clone())
        .unwrap_or_default();
    let mut b_piece = overlap_args
        .as_ref()
        .map(|o| o.b_piece.clone())
        .unwrap_or_default();

    if n_closest.is_some() {
        if a_requirements.is_some() || b_requirements.is_some() {
            log::error!("Cannot specify --n-closest with --a-requirements or --b-requirements. The 'closest' command is for finding nearest intervals, which may not overlap so overlap requirements are not applicable.");
            std::process::exit(1);
        }
        b_piece = IntersectionPart::Whole;
    }

    let chrom_order =
        bedder::chrom_ordering::parse_genome(std::fs::File::open(&common_args.genome_file)?)?;

    let afhile = BufReader::new(File::open(&common_args.query_path)?);

    let (a_bed_reader_obj, query_file_type) = bedder::sniff::open(afhile, &common_args.query_path)?;

    let input_header_for_writer: InputHeader = match query_file_type {
        bedder::sniff::FileType::Vcf | bedder::sniff::FileType::Bcf => match &a_bed_reader_obj {
            bedder::sniff::BedderReader::BedderVcf(vcf_reader) => {
                InputHeader::Vcf(vcf_reader.header.clone())
            }
            _ => {
                log::warn!(
                    "Query file type is {:?} but reader is not BedderVcf, cannot extract header.",
                    query_file_type
                );
                InputHeader::None
            }
        },
        bedder::sniff::FileType::Bed => InputHeader::None,
    };

    let a_iter = a_bed_reader_obj.into_positioned_iterator();

    let b_iters: Vec<_> = common_args
        .other_paths
        .iter()
        .map(|p| -> Result<_, Box<dyn std::error::Error>> {
            let fh = BufReader::new(File::open(p)?);
            let (b_reader, _) = bedder::sniff::open(fh, p)?;
            Ok(b_reader.into_positioned_iterator())
        })
        .collect::<Result<Vec<_>, _>>()?;

    let can_skip_ahead =
        !matches!(a_piece, IntersectionPart::Whole) && !common_args.dont_use_indexes;

    let ii = bedder::intersection::IntersectionIterator::new(
        a_iter,
        b_iters,
        &chrom_order,
        max_distance.unwrap_or(-1),
        n_closest.unwrap_or(-1),
        can_skip_ahead,
    )?;

    let mut output_format = match query_file_type {
        bedder::sniff::FileType::Bed => Format::Bed,
        bedder::sniff::FileType::Vcf => Format::Vcf,
        bedder::sniff::FileType::Bcf => Format::Bcf,
    };

    if output_format == Format::Vcf && common_args.output_path.to_str().unwrap().ends_with(".bcf") {
        output_format = Format::Bcf
    }

    let (a_reqs, b_reqs) = if n_closest.is_some() || max_distance.is_some() {
        (
            a_requirements.unwrap_or(OverlapAmount::Bases(0)),
            b_requirements.unwrap_or(OverlapAmount::Bases(0)),
        )
    } else {
        (
            a_requirements.unwrap_or(OverlapAmount::Bases(1)),
            b_requirements.unwrap_or(OverlapAmount::Bases(1)),
        )
    };

    let report_options = Arc::new(
        ReportOptions::builder()
            .a_mode(intersection_mode)
            .b_mode(b_mode)
            .a_piece(a_piece)
            .b_piece(b_piece)
            .a_requirements(a_reqs)
            .b_requirements(b_reqs)
            .build(),
    );

    Python::attach(|py| -> Result<(), Box<dyn std::error::Error>> {
        if let Some(python_file) = &common_args.python_file {
            let file = File::open(python_file)?;
            let code = std::io::read_to_string(file)?;
            let c_code = CString::new(code.as_str())?;
            py.run(&c_code, None, None)?;
        }

        let main_module = py.import("__main__")?;
        let globals_for_columns = main_module.dict();
        let functions_map = bedder::py::introspect_python_functions(py, globals_for_columns)?;

        let columns: Vec<Column<'_>> = common_args
            .columns
            .iter()
            .map(|c| {
                let (s, functions_map) = (c.as_str(), &functions_map);
                Column::try_from((s, functions_map))
            })
            .collect::<Result<Vec<_>, _>>()?;

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

        let compiled_filter = if let Some(filter_expr) = &common_args.filter {
            let compiled = bedder::py::CompiledExpr::new(py, filter_expr)?;
            Some(compiled)
        } else {
            None
        };

        let mut output = Writer::init(
            common_args.output_path.to_str().unwrap(),
            Some(output_format),
            None,
            input_header_for_writer,
            &py_columns,
        )?;
        bedder::py::initialize_python(py).expect("Failed to initialize Python environment");

        for intersection in ii {
            let mut intersection = intersection.expect("error getting intersection");
            output.write(
                &mut intersection,
                report_options.clone(),
                &py_columns,
                compiled_filter.as_ref(),
            )?;
        }
        Ok::<(), Box<dyn std::error::Error>>(())
    })
}
