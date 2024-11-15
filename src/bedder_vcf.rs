#![allow(clippy::useless_conversion)] // these are needed to support e.g. smartstring
use crate::position::{Field, FieldError, Position, Positioned, Value};
use crate::string::String;
use noodles::core::Region;

pub use rust_htslib::bcf;
use std::io::{self, Read, Seek};
use std::iter::Iterator;
use std::result;
pub use xvcf;
use xvcf::Skip;

pub struct BedderVCF<'a> {
    reader: xvcf::Reader<'a>,
    record_number: u64,
    header: vcf::Header,
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
    // Try to get the info field by name
    match info
        .info(name.as_bytes())
        .map_err(|e| FieldError::InvalidFieldValue(e.to_string()))?
    {
        Some(value) => match value {
            bcf::record::Info::Integer(arr) => {
                Ok(Value::Ints(arr.into_iter().map(|v| v as i64).collect()))
            }
            bcf::record::Info::Float(arr) => {
                Ok(Value::Floats(arr.into_iter().map(|v| v as f64).collect()))
            }
            bcf::record::Info::String(arr) => Ok(Value::Strings(
                arr.into_iter()
                    .map(|s| String::from_utf8_lossy(s).into_owned().into())
                    .collect(),
            )),
            bcf::record::Info::Flag(true) => Ok(Value::Strings(vec![String::from("true")])),
            bcf::record::Info::Flag(false) => Ok(Value::Strings(vec![String::from("false")])),
        },
        None => Err(FieldError::InvalidFieldName(String::from(name))),
    }
}

pub fn match_value(
    record: &rust_htslib::bcf::Record,
    f: Field,
) -> result::Result<Value, FieldError> {
    match f {
        Field::String(s) => match s.as_str() {
            "chrom" => Ok(Value::Strings(vec![String::from(Positioned::chrom(
                record,
            ))])),
            "start" => Ok(Value::Ints(vec![Positioned::start(record) as i64])),
            "stop" => Ok(Value::Ints(vec![Positioned::stop(record) as i64])),
            "ID" => {
                let ids = record
                    .id()
                    .map_err(|e| FieldError::InvalidFieldValue(e.to_string()))?;
                Ok(Value::Strings(vec![String::from_utf8_lossy(ids)
                    .into_owned()
                    .into()]))
            }
            "FILTER" => {
                let filters = record
                    .filters()
                    .map_err(|e| FieldError::InvalidFieldValue(e.to_string()))?;
                Ok(Value::Strings(
                    filters
                        .iter()
                        .map(|s| String::from_utf8_lossy(s).into_owned().into())
                        .collect(),
                ))
            }
            "QUAL" => {
                let qual = record
                    .qual()
                    .map_err(|e| FieldError::InvalidFieldValue(e.to_string()))?;
                Ok(Value::Floats(vec![if qual.is_nan() {
                    -1.0
                } else {
                    qual as f64
                }]))
            }
            _ => {
                if s.len() > 5 && &s[0..5] == "INFO." {
                    match_info_value(record, &s[5..])
                } else {
                    // TODO: format
                    unimplemented!();
                }
            }
        },
        Field::Int(i) => Err(FieldError::InvalidFieldIndex(i)),
    }
}

impl Positioned for vcf::record::Record {
    #[inline]
    fn chrom(&self) -> &str {
        match self.chromosome() {
            Chromosome::Name(s) => s,
            Chromosome::Symbol(s) => s,
        }
    }

    #[inline]
    fn start(&self) -> u64 {
        usize::from(self.position()) as u64
    }

    #[inline]
    fn stop(&self) -> u64 {
        usize::from(self.end().expect("error getting end from vcf record")) as u64
    }

    fn set_start(&mut self, start: u64) {
        let p = self.position_mut();
        *p = vcf::record::Position::try_from((start + 1) as usize)
            .expect("error setting start position in vcf record");
    }

    fn set_stop(&mut self, _stop: u64) {
        // set_stop in vcf is currently a no-op
    }

    fn dup(&self) -> Box<dyn Positioned> {
        Box::new(self.clone())
    }
}

impl<R> crate::position::PositionedIterator for BedderVCF<R>
where
    R: Read + Seek + 'static,
{
    fn next_position(
        &mut self,
        q: Option<&crate::position::Position>,
    ) -> Option<std::result::Result<Position, std::io::Error>> {
        if let Some(q) = q {
            let s = noodles::core::Position::new(q.start() as usize + 1)?;
            let e = noodles::core::Position::new(q.stop() as usize + 1)?;
            let region = Region::new(q.chrom(), s..=e);
            match self.reader.skip_to(&self.header, &region) {
                Ok(_) => (),
                Err(e) => return Some(Err(e)),
            }
        }

        // take self.reader.variant if it's there
        if let Some(v) = self.reader.take() {
            self.record_number += 1;
            return Some(Ok(Position::Vcf(Box::new(v))));
        }

        let mut v = vcf::Record::default();

        match self.reader.next_record(&self.header, &mut v) {
            Ok(0) => None, // EOF
            Ok(_) => {
                self.record_number += 1;
                Some(Ok(Position::Vcf(Box::new(v))))
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
