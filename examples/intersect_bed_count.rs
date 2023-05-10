use std::collections::HashMap;
use std::io;
use std::path::PathBuf;

use clap::Parser;
extern crate resort;
use crate::resort::position::Positioned;

#[derive(Parser, Debug)]
struct Args {
    a: PathBuf,
    b: PathBuf,

    fai: PathBuf,
}

fn main() -> io::Result<()> {
    let args = Args::parse();

    let a = std::io::BufReader::new(std::fs::File::open(args.a)?);
    let b = std::io::BufReader::new(std::fs::File::open(args.b)?);

    let ai = crate::resort::bedder_bed::BedderBed::new(a);
    let bi = crate::resort::bedder_bed::BedderBed::new(b);

    // TODO: parse chromosome order from fai
    let chrom_order = HashMap::from([(String::from("chr1"), 0), (String::from("chr2"), 1)]);

    let it = crate::resort::intersection::IntersectionIterator::new(ai, vec![bi], &chrom_order)?;

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
