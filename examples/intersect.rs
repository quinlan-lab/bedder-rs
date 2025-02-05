use std::fs;
use std::io::{self, BufReader, BufWriter, Write};
use std::path::PathBuf;

use bedder::hts_format::Format;
use bedder::writer::InputHeader;
use clap::Parser;
extern crate bedder;
use crate::bedder::chrom_ordering::parse_genome;
use crate::bedder::intersection::IntersectionIterator;
use crate::bedder::intersections::{IntersectionMode, IntersectionPart, OverlapAmount};
use crate::bedder::writer;

#[derive(Parser, Debug)]
struct Args {
    a: PathBuf,
    b: Vec<PathBuf>,

    #[clap(long, default_value = "", help = "intersection mode for A")]
    a_mode: IntersectionMode,

    #[clap(long, default_value = "", help = "intersection mode for Bs")]
    b_mode: IntersectionMode,

    #[clap(long, default_value = "whole")]
    a_part: IntersectionPart,

    #[clap(long, default_value = "whole")]
    b_part: IntersectionPart,

    #[clap(
        long,
        default_value = "1",
        help = "overlap requirements. Specify as integer bases or a percent, e.g. '50%' where '%' is required."
    )]
    a_requirements: OverlapAmount,

    #[clap(
        long,
        default_value = "1",
        help = "overlap requirements. Specify as integer bases or a percent, e.g. '50%' where '%' is required."
    )]
    b_requirements: OverlapAmount,

    #[clap(
        long,
        short,
        help = "fai or genome file that dictates chromosome order"
    )]
    fai: PathBuf,

    #[clap(long, short, help = "count the number of intersections")]
    count: bool,

    #[clap(long, short = 'b', help = "count the bases of overlaps")]
    count_base: bool,
}

fn main() -> io::Result<()> {
    let args = Args::parse();

    let a = BufReader::new(fs::File::open(&args.a)?);
    let ai = bedder::sniff::open(a, &args.a)?;

    //let ai = sniff::open_file(&args.a)?;
    let bis = args
        .b
        .iter()
        .map(|b| {
            let f = BufReader::new(fs::File::open(b)?);
            let b = bedder::sniff::open(f, b)?;
            Ok(b.into_positioned_iterator())
        })
        .collect::<io::Result<Vec<Box<dyn bedder::position::PositionedIterator>>>>()?;

    // bedder always requires a hashmap that indicates the chromosome order

    // bedder always requires a hashmap that indicates the chromosome order
    let fh = BufReader::new(fs::File::open(&args.fai)?);
    let h = parse_genome(fh)?;
    let format = Format::Bed;

    let mut wtr = match writer::Writer::init(
        "output.bed",
        Some(format),
        None, // Some(hts::htsCompression_bgzf),
        InputHeader::None,
    ) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("error: {:?}", e);
            std::process::exit(1);
        }
    };

    // get the bedderbed since the query in IntersectionIterator is not a box<dyn>.
    let ai = match ai {
        bedder::sniff::BedderReader::BedderBed(rdr) => rdr,
    };

    // we can have any number of b (other_iterators).
    let it = IntersectionIterator::new(ai, bis, &h)?;

    // we need to use buffered stdout or performance is determined by
    // file IO
    let mut stdout = BufWriter::new(io::stdout());

    for intersection in it {
        let intersection = intersection?;
        let report = intersection.report(
            &args.a_mode,
            &args.b_mode,
            &args.a_part,
            &args.b_part,
            &args.a_requirements,
            &args.b_requirements,
        );
        //eprintln!("{:?} {:?}", report.len(), report);
        //eprintln!("a reqs: {:?}", args.a_requirements);

        if args.count {
            writeln!(
                &mut stdout,
                "{}\t{}\t{}\t{}",
                intersection.base_interval.chrom(),
                intersection.base_interval.start(),
                intersection.base_interval.stop(),
                report
                    .count_overlaps_by_id()
                    .iter()
                    .map(|x| x.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            )?;
            continue;
        }
        if args.count_base {
            writeln!(
                &mut stdout,
                "{}\t{}\t{}\t{}",
                intersection.base_interval.chrom(),
                intersection.base_interval.start(),
                intersection.base_interval.stop(),
                report
                    .count_bases_by_id()
                    .iter()
                    .map(|x| x.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            )?;
            continue;
        }
        //eprintln!("report: {:?}", report);
        //eprintln!("args: {:?}", &args);
        //let c = ColumnReporter::new();
        let v = vec![];

        wtr.write(&report, &v)?;
    }

    Ok(())
}
