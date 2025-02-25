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
        help = "python f-string expression",
        short = 'f',
        default_value = "def main(intersection): return f'{intersection.base_interval.chrom}\t{intersection.base_interval.start}\t{intersection.base_interval.stop}\t{len(intersection.overlapping)}'"
    )]
    f_string: String,
    #[arg(
        help = "use Lua instead of Python for expression",
        short = 'l',
        long = "lua"
    )]
    use_lua: bool,
    #[arg(
        help = "columns to output (format: name:type:description:number:value_parser)",
        short = 'c',
        long = "columns"
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

    // Check if we're using columns or legacy mode
    let use_columns = !args.columns.is_empty();

    if use_columns {
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

        // Process with columns
        if args.use_lua {
            // Initialize Lua expressions in columns if needed
            // (This would require implementing Lua support for columns)

            // Process intersections with columns
            for intersection in ii {
                let intersection = intersection.expect("error getting intersection");
                let values: Vec<String> = columns
                    .iter()
                    .map(|col| match col.value(&intersection) {
                        Ok(val) => val.to_string(),
                        Err(e) => panic!("Error getting column value: {:?}", e),
                    })
                    .collect();

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
        } else {
            // Use Python for columns that need it
            Python::with_gil(|py| {
                // Initialize Python expressions in columns if needed
                let mut py_columns: Vec<Column> = columns
                    .into_iter()
                    .map(|mut col| {
                        if let Some(bedder::column::ValueParser::PythonExpression(expr)) =
                            &col.value_parser
                        {
                            let compiled = bedder::py::CompiledPython::new(py, expr)
                                .expect("error compiling Python expression");
                            col.py = Some(compiled);
                        }
                        col
                    })
                    .collect();
                log::info!("py_columns: {:?}", py_columns);

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
            });
        }
    } else {
        // Legacy mode with f-string
        if args.use_lua {
            let compiled = bedder::lua_wrapper::CompiledLua::new(&args.f_string)
                .expect("error compiling Lua expression");

            // iterate over the intersections
            ii.for_each(|intersection| {
                let intersection = intersection.expect("error getting intersection");
                match compiled.eval(intersection) {
                    Ok(result) => writeln!(stdout, "{}", result).expect("error writing to stdout"),
                    Err(e) => {
                        panic!("Error formatting: {}", e);
                    }
                }
            });
        } else {
            Python::with_gil(|py| {
                let compiled = bedder::py::CompiledPython::new(py, &args.f_string)
                    .expect("error compiling f-string");

                // iterate over the intersections
                for intersection in ii {
                    let intersection = intersection.expect("error getting intersection");
                    let py_intersection = bedder::py::PyIntersections::from(intersection);
                    match compiled.eval(py_intersection) {
                        Ok(result) => match writeln!(stdout, "{}", result) {
                            Ok(_) => {}
                            Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => {
                                std::process::exit(0);
                            }
                            Err(e) => {
                                panic!("Error formatting: {}", e);
                            }
                        },
                        Err(e) => {
                            panic!("Error formatting: {}", e);
                        }
                    }
                }
            });
        }
    }

    Ok(())
}
