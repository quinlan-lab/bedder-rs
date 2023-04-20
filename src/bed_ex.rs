use crate::intersection;
use crate::position::{Position, Positioned, PositionedIterator};
use std::io;
use std::io::{BufRead, BufReader};

pub struct BedFile<'a> {
    fh: BufReader<std::fs::File>,
    chroms: Vec<String>,
    // use phantom data to indicate that the lifetime of the BedFile is the same as the lifetime of the
    // strings in chroms
    _phantom: std::marker::PhantomData<&'a ()>,
}

pub struct BedInterval<'a> {
    chromosome: &'a str,
    start: u64,
    stop: u64,
}

impl<'a> Positioned<'a> for BedInterval<'a> {
    fn position(&self) -> Position<'a> {
        Position::new(self.chromosome, self.start, self.stop)
    }
}

// the new method on BedFile opens the file and returns a BedFile
impl<'a> BedFile<'a> {
    pub fn new(path: &str) -> io::Result<Self> {
        let fh = std::fs::File::open(path)?;
        let br = BufReader::new(fh);
        Ok(BedFile {
            fh: br,
            chroms: vec![],
            _phantom: std::marker::PhantomData,
        })
    }
}

impl<'a, 'b> PositionedIterator<'a, 'b> for BedFile<'a> {
    type Item = BedInterval<'a>;

    fn next(&'b mut self) -> Option<Self::Item> {
        // read a line from fh

        let mut line = String::new();
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
            self.chroms.push(chromosome.to_string());
        }

        let chrom: &'a str = &self.chroms[self.chroms.len() - 1];
        // parse the line into a Position
        let b: Option<BedInterval> = Some(BedInterval {
            chromosome: chrom,
            start: toks.next()?.parse().ok()?,
            stop: toks.next()?.parse().ok()?,
        });
        b
    }
}
