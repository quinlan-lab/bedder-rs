use std::fs;
use std::io::{self, BufReader, BufWriter, Write};
use std::path::PathBuf;

use bedder::sniff;
use clap::Parser;
extern crate bedder;
use crate::bedder::chrom_ordering::parse_genome;
use crate::bedder::intersection::IntersectionIterator;
use crate::bedder::intersections::{IntersectionMode, IntersectionPart, OverlapAmount};

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
}

fn main() -> io::Result<()> {
    let args = Args::parse();

    // sniff determines the file type (bam/cram/bcf/vcf/bed/gff/gtf)
    // and returns a PositionIterator
    let ai = sniff::open_file(&args.a)?;
    let bis = args
        .b
        .iter()
        .map(|b| sniff::open_file(b))
        .collect::<io::Result<Vec<_>>>()?;

    // bedder always requires a hashmap that indicates the chromosome order
    let fh = BufReader::new(fs::File::open(args.fai)?);
    let h = parse_genome(fh)?;

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
        eprintln!("{:?}", report);
        eprintln!("a reqs: {:?}", args.a_requirements);

        writeln!(
            &mut stdout,
            "{}\t{}\t{}\t{}",
            intersection.base_interval.chrom(),
            intersection.base_interval.start(),
            intersection.base_interval.stop(),
            intersection.overlapping.len()
        )?;
    }

    Ok(())
}
