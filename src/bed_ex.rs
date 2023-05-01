use crate::intersection;
use crate::position::{Positioned, PositionedIterator};
use smartstring::alias::String;
use std::io;
use std::io::BufRead;

pub struct BedFile<R> {
    inner: R,
    chroms: Vec<String>,
    name: String,
}

pub struct BedInterval {
    chromosome: String,
    start: u64,
    stop: u64,
}

impl Positioned for BedInterval {
    fn chrom(&self) -> &str {
        &self.chromosome
    }
    fn start(&self) -> u64 {
        self.start
    }
    fn stop(&self) -> u64 {
        self.stop
    }
}

// the new method on BedFile opens the file and returns a BedFile
impl<R> BedFile<R>
where
    R: BufRead,
{
    pub fn new(inner: R, name: String) -> Self {
        Self {
            inner,
            chroms: vec![],
            name,
        }
    }
}

impl<R> PositionedIterator for BedFile<R>
where
    R: BufRead,
{
    type Item = BedInterval;

    fn name(&self) -> String {
        self.name.clone()
    }

    fn next(&mut self) -> Option<Self::Item> {
        // read a line from fh

        let mut line = std::string::String::new();
        let mut toks = match self.inner.read_line(&mut line) {
            Ok(_) => line.trim().split('\t'),
            Err(e) => {
                // check if e is EOF error:
                if e.kind() == io::ErrorKind::UnexpectedEof {
                    return None;
                } else {
                    panic!("Error reading file: {}", e);
                }
            }
        };
        let chromosome = toks.next()?;
        // TODO: check if we've seen this chrom before and error.
        if self.chroms.is_empty() || self.chroms[self.chroms.len() - 1] != chromosome {
            self.chroms.push(String::from(chromosome));
        }

        // todo, evaluate compact_str which has O(1) clone.
        let chrom = self.chroms[self.chroms.len() - 1].clone();
        // parse the line into a Position
        let b: Option<BedInterval> = Some(BedInterval {
            chromosome: chrom,
            start: toks.next()?.parse().ok()?,
            stop: toks.next()?.parse().ok()?,
        });
        b
    }
}
