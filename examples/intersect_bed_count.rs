use std::collections::HashMap;
use std::fs;
use std::io::{self, BufRead, BufReader};
use std::path::PathBuf;

use bedder::sniff;
use clap::Parser;
extern crate bedder;
use crate::bedder::genome_file::parse_genome;
use crate::bedder::intersection::IntersectionIterator;
use crate::bedder::position::Positioned;
use crate::bedder::string::String;

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
        // here we have the Positioned intersections (below we use the first() one)
        // NOTE that as implemented, we'd have to have some other way to know
        // that a given file is a VCF and has an AD field.
        // see issue #15 for more discussion into using an enum.
        // alternatively, we *can* know the file type from the sniff operation when opening.
        // but this places more burden on the user of the API to track file-types.
        intersection.overlapping.first().map(|p| {
            let value = p
                .interval
                .value(bedder::position::Field::String(String::from("AD")))
                .expect("hard coded AD field expecting VCF");
            // extract the integer value
            match value {
                bedder::position::Value::Ints(i) => println!("AD: {:?}", i),
                _ => panic!("expected integer"),
            }
        });
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
