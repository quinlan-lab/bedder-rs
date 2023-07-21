use std::fs;
use std::io::{self, BufReader};
use std::path::PathBuf;

use bedder::sniff;
use clap::Parser;
extern crate bedder;
use crate::bedder::chrom_ordering::parse_genome;
use crate::bedder::intersection::IntersectionIterator;
use crate::bedder::position::Positioned;

#[derive(Parser, Debug)]
struct Args {
    a: PathBuf,
    b: PathBuf,

    fai: PathBuf,
}

fn main() -> io::Result<()> {
    let args = Args::parse();

    // sniff determines the file type (bam/cram/bcf/vcf/bed/gff/gtf)
    // and returns a PositionIterator
    let ai = sniff::open_file(&args.a)?;
    let bi = sniff::open_file(&args.b)?;

    // bedder always requires a hashmap that indicates the chromosome order
    let fh = BufReader::new(fs::File::open(args.fai)?);
    let h = parse_genome(fh)?;

    // we can have any number of b (other_iterators).
    let it = IntersectionIterator::new(ai, vec![bi], &h)?;

    for intersection in it {
        let intersection = intersection?;
        println!(
            "{}\t{}\t{}\t{}",
            intersection.base_interval.chrom(),
            intersection.base_interval.start(),
            intersection.base_interval.stop(),
            intersection.overlapping.len()
        );
    }

    Ok(())
}
