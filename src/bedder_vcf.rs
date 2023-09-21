use crate::position::{Field, FieldError, Position, Positioned, Value};
use crate::string::String;
use noodles::bcf;
use noodles::vcf::{self, record::Chromosome};
use std::io::{self, BufRead, Read};
use std::result;
use vcf::record::info::field;
use vcf::record::QualityScore;
pub use vcf::Record;
pub use xvcf;

pub struct BedderVCF<R> {
    reader: xvcf::Reader<R>,
    record_number: u64,
}

impl<R> BedderVCF<R>
where
    R: Read + 'static,
{
    pub fn new(r: xvcf::Reader<R>) -> io::Result<BedderVCF<R>> {
        let v = BedderVCF {
            reader: r,
            record_number: 0,
        };
        Ok(v)
    }
}

pub fn from_reader<R: Read>(r: Box<R>) -> io::Result<BedderVCF<R>> {
    let reader = xvcf::Reader::from_reader(r, None)?;
    BedderVCF::new(reader)
}

fn match_info_value(info: &vcf::record::Info, name: &str) -> result::Result<Value, FieldError> {
    //let info = record.info();
    let key: vcf::record::info::field::Key = name
        .parse()
        .map_err(|_| FieldError::InvalidFieldName(String::from(name.clone())))?;

    match info.get(&key) {
        Some(value) => match value {
            Some(field::Value::Integer(i)) => Ok(Value::Ints(vec![*i as i64])),
            Some(field::Value::Float(f)) => Ok(Value::Floats(vec![*f as f64])),
            Some(field::Value::String(s)) => Ok(Value::Strings(vec![String::from(s)])),
            Some(field::Value::Character(c)) => {
                Ok(Value::Strings(vec![String::from(c.to_string())]))
            }
            //Some(field::Value::Flag) => Ok(Value::Strings(vec![String::from("true")])),
            Some(field::Value::Array(arr)) => {
                match arr {
                    field::value::Array::Integer(arr) => Ok(Value::Ints(
                        arr.iter().flatten().map(|&v| v as i64).collect(),
                    )),
                    field::value::Array::Float(arr) => Ok(Value::Floats(
                        arr.iter().flatten().map(|&v| v as f64).collect(),
                    )),
                    field::value::Array::String(arr) => Ok(Value::Strings(
                        arr.iter().flatten().map(String::from).collect(),
                    )),
                    field::value::Array::Character(arr) => Ok(Value::Strings(
                        arr.iter().flatten().map(|v| v.to_string().into()).collect(),
                    )),
                    //field::Value::Flag => Ok(Value::Strings(vec![String::from("true")])),
                }
            }

            _ => Err(FieldError::InvalidFieldName(String::from(name))),
        },
        None => Err(FieldError::InvalidFieldName(String::from(name))),
    }
}

fn match_value(record: &vcf::record::Record, f: Field) -> result::Result<Value, FieldError> {
    match f {
        Field::String(s) => match s.as_str() {
            "chrom" => Ok(Value::Strings(vec![String::from(Positioned::chrom(
                record,
            ))])),
            "start" => Ok(Value::Ints(vec![Positioned::start(record) as i64])),
            "stop" => Ok(Value::Ints(vec![Positioned::stop(record) as i64])),
            "ID" => Ok(Value::Strings(
                record.ids().iter().map(|s| s.to_string().into()).collect(),
            )),
            "FILTER" => Ok(Value::Strings(
                record
                    .filters()
                    .iter()
                    .map(|s| String::from(s.to_string()))
                    .collect(),
            )),
            "QUAL" => Ok(Value::Floats(vec![f32::from(
                record
                    .quality_score()
                    .unwrap_or(QualityScore::try_from(0f32).expect("error getting quality score")),
            ) as f64])),
            _ => {
                if s.len() > 5 && &s[0..5] == "INFO." {
                    match_info_value(record.info(), &s[5..])
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

    fn value(&self, f: crate::position::Field) -> result::Result<Value, FieldError> {
        match_value(self, f)
    }
}

impl<R> crate::position::PositionedIterator for BedderVCF<R>
where
    R: Read + 'static,
{
    fn next_position(
        &mut self,
        _q: Option<&crate::position::Position>,
    ) -> Option<std::result::Result<Position, std::io::Error>> {
        let mut v = vcf::Record::default();

        match self.reader.next_record(&self.reader.header(), &mut v) {
            Ok(0) => None, // EOF
            Ok(_) => {
                self.record_number += 1;
                Some(Ok(Position::Vcf(Box::new(v))))
            }
            Err(e) => Some(Err(e)),
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
