#![allow(clippy::useless_conversion)] // these are needed to support e.g. smartstring

use crate::position::{Position, Positioned};
use crate::string::String;
pub use simplebed;
pub use simplebed::{BedError, BedReader, BedRecord as SimpleBedRecord, BedValue};
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};
#[derive(Debug, Clone)]
pub struct BedRecord(pub SimpleBedRecord);

impl BedRecord {
    #[allow(dead_code)]
    pub(crate) fn new(
        chrom: &str,
        start: u64,
        end: u64,
        name: Option<&str>,
        score: Option<f64>,
        other_fields: Vec<String>,
    ) -> Self {
        let record = SimpleBedRecord::new(
            chrom.to_string(),
            start,
            end,
            name.map(|s| s.to_string()),
            score,
            other_fields
                .into_iter()
                .map(|s| BedValue::String(s.to_string()))
                .collect(),
        );
        Self(record)
    }

    pub fn push_field(&mut self, field: BedValue) {
        self.0.push_field(field);
    }
}

impl crate::position::Positioned for BedRecord {
    #[inline]
    fn chrom(&self) -> &str {
        self.0.chrom()
    }

    #[inline]
    fn start(&self) -> u64 {
        self.0.start()
    }

    #[inline]
    fn stop(&self) -> u64 {
        self.0.end()
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

    pub fn inner_mut(&mut self) -> &mut SimpleBedRecord {
        &mut self.0
    }
}

struct Last {
    chrom: String,
    start: u64,
    stop: u64,
}

pub struct BedderBed<'a, R>
where
    R: BufRead + 'a,
{
    reader: BedReader<R>,
    last_record: Option<Last>,
    line_number: u64,
    query_iter: Option<Box<dyn Iterator<Item = Result<SimpleBedRecord, BedError>> + 'a>>,
}

impl<'a, R> BedderBed<'a, R>
where
    R: BufRead,
{
    pub fn new<P: AsRef<Path>>(r: R, path: Option<P>) -> BedderBed<'a, R> {
        let path: PathBuf = path
            .map(|p| p.as_ref().to_path_buf()) // Ensure it's a PathBuf
            .unwrap_or_else(|| PathBuf::from("memory"));
        BedderBed {
            reader: BedReader::new(r, path).expect("Failed to create BedReader"),
            last_record: None,
            line_number: 0,
            query_iter: None,
        }
    }
}

impl<'a, R> crate::position::PositionedIterator for BedderBed<'a, R>
where
    R: BufRead + std::io::Seek + 'a,
{
    fn next_position(
        &mut self,
        _query: Option<&crate::position::Position>,
    ) -> Option<std::result::Result<Position, std::io::Error>> {
        // If we have a query, set up the query iterator
        if self.query_iter.is_some() {
            self.query_iter = None;
        }
        /*
        if let Some(query) = query {
            log::trace!("querying: {:?}", query);
            let q = self.reader.query(
                query.chrom(),
                (query.start() as usize) + 1,
                query.stop() as usize,
            );
            match q {
                Ok(iter) => {
                    let iter: Box<dyn Iterator<Item = Result<SimpleBedRecord, BedError>> + 'a> = unsafe {
                        std::mem::transmute(Box::new(iter)
                            as Box<dyn Iterator<Item = Result<SimpleBedRecord, BedError>> + '_>)
                    };
                    self.query_iter = Some(iter);
                }
                Err(BedError::NoChromosomeOrder) => {
                    log::trace!("no chromosome order");
                    self.query_iter = None;
                }
                Err(e) => {
                    log::info!("error querying: {}. it's possible that there was no index or the chromosome was not found", e);
                    self.query_iter = None;
                }
            }
        }
        */

        // If we have an active query iterator, use it
        if let Some(iter) = &mut self.query_iter {
            match iter.next() {
                Some(Ok(record)) => return Some(Ok(Position::Bed(BedRecord(record)))),
                Some(Err(e)) => {
                    return Some(Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        e.to_string(),
                    )))
                }
                None => {
                    self.query_iter = None;
                    return None;
                }
            }
        }

        // No query iterator, read next record from the reader
        self.line_number += 1;
        match self.reader.read_record() {
            Ok(Some(record)) => {
                match &mut self.last_record {
                    None => {
                        self.last_record = Some(Last {
                            chrom: String::from(record.chrom()),
                            start: record.start(),
                            stop: record.end(),
                        })
                    }
                    Some(r) => {
                        if r.chrom != record.chrom() {
                            r.chrom = String::from(record.chrom())
                        }
                        r.start = record.start();
                        r.stop = record.end();
                    }
                }
                Some(Ok(Position::Bed(BedRecord(record))))
            }
            Ok(None) => None,
            Err(e) => Some(Err(io::Error::new(
                io::ErrorKind::InvalidData,
                e.to_string(),
            ))),
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
    use crate::string::String;
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

        let it = IntersectionIterator::new(Box::new(ar), vec![Box::new(br)], &chrom_order, 0, 0)
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
