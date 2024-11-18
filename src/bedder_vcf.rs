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
    info: &rust_htslib::bcf::Record,
    name: &str,
) -> result::Result<Value, FieldError> {
    unimplemented!()
}

pub fn match_value(
    record: &rust_htslib::bcf::Record,
    f: Field,
) -> result::Result<Value, FieldError> {
    unimplemented!()
}

#[derive(Debug)]
pub struct BedderRecord {
    pub record: bcf::Record,
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

        let header = self.reader.header();
        let mut v = header.empty_record();

        match self.reader.next_record(&mut v) {
            Ok(0) => None, // EOF
            Ok(_) => {
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

    #[test]
    fn test_match_info() {
        let key: field::Key = "AAA".parse().expect("error parsing key");

        let info: vcf::record::Info = [(key, Some(field::Value::Integer(1)))]
            .into_iter()
            .collect();

        // write a test to extract the value using match_info_value
        let value = match_info_value(&info, "AAA").unwrap();
        assert!(matches!(value, Value::Ints(_)));
    }

    #[test]
    fn test_match_info_vector() {
        let key: field::Key = "AAA".parse().expect("error parsing key");

        let info: vcf::record::Info = [(
            key,
            Some(field::Value::Array(field::value::Array::Integer(vec![
                Some(-1),
                Some(2),
                Some(3),
                None,
                Some(496),
            ]))),
        )]
        .into_iter()
        .collect();

        // write a test to extract the value using match_info_value
        let value = match_info_value(&info, "AAA").unwrap();
        assert!(matches!(value, Value::Ints(_)));

        if let Value::Ints(v) = value {
            assert_eq!(v.len(), 4);
            assert_eq!(v[0], -1);
            assert_eq!(v[1], 2);
            assert_eq!(v[2], 3);
            assert_eq!(v[3], 496);
        } else {
            panic!("error getting value");
        }
    }
}
