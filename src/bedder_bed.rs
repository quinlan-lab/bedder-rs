#![allow(clippy::useless_conversion)] // these are needed to support e.g. smartstring

use crate::position::{Position, Positioned};
use crate::string::String;
pub use simplebed;
use simplebed::{BedReader, BedRecord as SimpleBedRecord};
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};
#[derive(Debug, Clone)]
pub struct BedRecord(SimpleBedRecord);

impl crate::position::Positioned for BedRecord {
    #[inline]
    fn chrom(&self) -> &str {
        self.0.chrom()
    }

    #[inline]
    fn start(&self) -> u64 {
        self.0.start() as u64
    }

    #[inline]
    fn stop(&self) -> u64 {
        self.0.end() as u64
    }

    #[inline]
    fn set_start(&mut self, start: u64) {
        self.0.set_start(start);
    }

    #[inline]
    fn set_stop(&mut self, stop: u64) {
        self.0.set_end(stop);
    }

    fn clone_box(&self) -> Box<dyn Positioned> {
        Box::new(self.clone())
    }
}

impl BedRecord {
    pub fn inner(&self) -> &SimpleBedRecord {
        &self.0
    }
}

struct Last {
    chrom: String,
    start: u64,
    stop: u64,
}

pub struct BedderBed<R>
where
    R: BufRead,
{
    reader: BedReader<R>,
    last_record: Option<Last>,
    line_number: u64,
}

impl<R> BedderBed<R>
where
    R: BufRead,
{
    pub fn new<P: AsRef<Path>>(r: R, path: Option<P>) -> BedderBed<R> {
        let path: PathBuf = path
            .map(|p| p.as_ref().to_path_buf()) // Ensure it's a PathBuf
            .unwrap_or_else(|| PathBuf::from("memory"));
        BedderBed {
            reader: BedReader::new(r, path).expect("Failed to create BedReader"),
            last_record: None,
            line_number: 0,
        }
    }
}

impl<R> crate::position::PositionedIterator for BedderBed<R>
where
    R: BufRead,
{
    fn next_position(
        &mut self,
        _q: Option<&crate::position::Position>,
    ) -> Option<std::result::Result<Position, std::io::Error>> {
        loop {
            self.line_number += 1;
            match self.reader.read_record() {
                Ok(Some(record)) => {
                    match &mut self.last_record {
                        None => {
                            self.last_record = Some(Last {
                                chrom: String::from(record.chrom()),
                                start: record.start() as u64,
                                stop: record.end() as u64,
                            })
                        }
                        Some(r) => {
                            if r.chrom != record.chrom() {
                                r.chrom = String::from(record.chrom())
                            }
                            r.start = record.start() as u64;
                            r.stop = record.end() as u64;
                        }
                    }
                    return Some(Ok(Position::Bed(BedRecord(record))));
                }
                Ok(None) => return None,
                Err(e) => {
                    return Some(Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        e.to_string(),
                    )))
                }
            };
        }
    }

    fn name(&self) -> String {
        String::from(format!("bed:{}", self.line_number))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chrom_ordering::Chromosome;
    use crate::intersection::IntersectionIterator;
    use hashbrown::HashMap;
    use std::io::Cursor;

    #[test]
    fn test_bed_read() {
        // write a test for bed from a string using BufRead
        let ar = BedderBed::new(Cursor::new("chr1\t20\t30\nchr1\t21\t33"), None::<String>);
        let br = BedderBed::new(Cursor::new("chr1\t21\t30\nchr1\t22\t33"), None::<String>);

        let chrom_order = HashMap::from([
            (
                String::from("chr1"),
                Chromosome {
                    index: 0usize,
                    length: None,
                },
            ),
            (
                String::from("chr2"),
                Chromosome {
                    index: 1usize,
                    length: None,
                },
            ),
        ]);

        let it = IntersectionIterator::new(Box::new(ar), vec![Box::new(br)], &chrom_order)
            .expect("error creating iterator");

        let mut n = 0;
        it.for_each(|int| {
            let int = int.expect("error getting intersection");
            //dbg!(&int.overlapping);
            assert!(int.overlapping.len() == 2);
            n += 1;
        });
        assert!(n == 2);
    }
}
