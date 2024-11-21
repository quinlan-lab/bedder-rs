#![allow(clippy::useless_conversion)] // these are needed to support e.g. smartstring
use crate::position::{Field, FieldError, Position, Positioned, Value};
use crate::string::String;

use std::io;
use std::result;
pub use xvcf;
use xvcf::rust_htslib::{self, bcf};
use xvcf::Skip;

pub struct BedderVCF<'a> {
    reader: xvcf::Reader<'a>,
    record_number: u64,
    header: bcf::header::HeaderView,
}

impl<'a> BedderVCF<'a> {
    pub fn new(r: xvcf::Reader<'a>) -> io::Result<BedderVCF<'a>> {
        let h = r.header().clone();
        let v = BedderVCF {
            reader: r,
            record_number: 0,
            header: h,
        };
        Ok(v)
    }
}

pub fn match_info_value(
    _info: &rust_htslib::bcf::Record,
    _name: &str,
) -> result::Result<Value, FieldError> {
    unimplemented!()
}

pub fn match_value(
    _record: &rust_htslib::bcf::Record,
    _f: Field,
) -> result::Result<Value, FieldError> {
    unimplemented!()
}

#[derive(Debug)]
pub struct BedderRecord {
    pub record: bcf::Record,
}

impl Clone for BedderRecord {
    fn clone(&self) -> Self {
        Self {
            record: self.record.clone(),
        }
    }
}

impl BedderRecord {
    pub fn new(record: bcf::Record) -> Self {
        Self { record }
    }
}

impl Positioned for BedderRecord {
    #[inline]
    fn chrom(&self) -> &str {
        let h = self.record.header();
        let rid = self.record.rid();
        if let Some(rid) = rid {
            let name = h.rid2name(rid).expect("error getting chromosome name");
            std::str::from_utf8(name).expect("invalid UTF-8 in chromosome name")
        } else {
            ""
        }
    }

    #[inline]
    fn start(&self) -> u64 {
        self.record.pos() as u64
    }

    #[inline]
    fn stop(&self) -> u64 {
        self.record.end() as u64
    }

    fn set_start(&mut self, start: u64) {
        self.record.set_pos(start as i64);
    }

    fn set_stop(&mut self, _stop: u64) {
        // set_stop in vcf is currently a no-op
    }

    fn clone_box(&self) -> Box<dyn Positioned> {
        Box::new(Self {
            record: self.record.clone(),
        })
    }
}

impl<'a> crate::position::PositionedIterator for BedderVCF<'a> {
    fn next_position(
        &mut self,
        q: Option<&crate::position::Position>,
    ) -> Option<std::result::Result<Position, std::io::Error>> {
        if let Some(q) = q {
            match self.reader.skip_to(q.chrom(), q.start() - 1 as u64) {
                Ok(_) => (),
                Err(e) => return Some(Err(e)),
            }
        }

        if let Some(v) = self.reader.take() {
            self.record_number += 1;
            return Some(Ok(Position::Vcf(Box::new(BedderRecord::new(v)))));
        }

        match self.reader.next_record() {
            Ok(None) => None, // EOF
            Ok(Some(v)) => {
                self.record_number += 1;
                Some(Ok(Position::Vcf(Box::new(BedderRecord::new(v)))))
            }
            Err(e) => {
                eprintln!(
                    "error reading vcf record: {} at line number: {}",
                    e, self.record_number
                );
                Some(Err(e))
            }
        }
    }
    fn name(&self) -> String {
        String::from("vcf line number:".to_owned() + self.record_number.to_string().as_str())
    }
}

// tests
#[cfg(test)]
mod tests {
    use super::*;
}
