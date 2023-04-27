use crate::intersection;
use crate::position::{Position, Positioned, PositionedIterator};
use smartstring::alias::String;
use std::io;
use std::io::{BufRead, BufReader};

pub struct BedFile {
    fh: BufReader<std::fs::File>,
    chroms: Vec<String>,
}

pub struct BedInterval {
    chromosome: String,
    start: u64,
    stop: u64,
}

impl Positioned for BedInterval {
    fn position(&self) -> Position {
        Position::new(self.chromosome, self.start, self.stop)
    }
}

// the new method on BedFile opens the file and returns a BedFile
impl BedFile {
    pub fn new(path: &str) -> io::Result<Self> {
        let fh = std::fs::File::open(path)?;
        let br = BufReader::new(fh);
        Ok(BedFile {
            fh: br,
            chroms: vec![],
        })
    }
}

impl PositionedIterator for BedFile {
    type Item = BedInterval;

    fn next(&mut self) -> Option<Self::Item> {
        // read a line from fh

        let mut line = std::string::String::new();
        let mut toks = match self.fh.read_line(&mut line) {
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

        let chrom = self.chroms[self.chroms.len() - 1];
        // parse the line into a Position
        let b: Option<BedInterval> = Some(BedInterval {
            chromosome: String::from(chrom),
            start: toks.next()?.parse().ok()?,
            stop: toks.next()?.parse().ok()?,
        });
        b
    }
}
