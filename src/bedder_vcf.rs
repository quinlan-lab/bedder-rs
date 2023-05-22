use crate::position::{Field, FieldError, Positioned, Result, Value};
use crate::string::String;
use noodles::bcf;
use noodles::core::{Position, Region};
use noodles::csi;
use noodles::vcf::{self, record::Chromosome};
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use vcf::record::info::field;
use vcf::record::QualityScore;

pub trait VCFReader {
    fn read_record(&mut self, header: &vcf::Header, v: &mut vcf::Record) -> io::Result<usize>;
    // fn queryable
}

impl<R> VCFReader for vcf::Reader<R>
where
    R: BufRead,
{
    #[inline]
    fn read_record(&mut self, header: &vcf::Header, v: &mut vcf::Record) -> io::Result<usize> {
        self.read_record(header, v)
    }
}

impl<R> VCFReader for vcf::indexed_reader::IndexedReader<R>
where
    R: BufRead,
{
    #[inline]
    fn read_record(&mut self, header: &vcf::Header, v: &mut vcf::Record) -> io::Result<usize> {
        self.read_record(header, v)
    }
}

impl<R> VCFReader for bcf::Reader<R>
where
    R: BufRead,
{
    #[inline]
    fn read_record(&mut self, header: &vcf::Header, v: &mut vcf::Record) -> io::Result<usize> {
        self.read_record(header, v)
    }
}

pub struct BedderVCF<'a> {
    reader: Box<dyn VCFReader + 'a>,
    header: vcf::Header,
    record_number: u64,
}

impl<'a> BedderVCF<'a> {
    pub fn new(r: Box<dyn VCFReader + 'a>, header: vcf::Header) -> io::Result<BedderVCF<'a>> {
        let v = BedderVCF {
            reader: r,
            header: header,
            record_number: 0,
        };
        Ok(v)
    }
}

fn match_info_value(info: &vcf::record::Info, name: &str) -> Result {
    //let info = record.info();
    let key: vcf::record::info::field::Key = name
        .parse()
        .map_err(|_| FieldError::InvalidFieldName(String::from(name)))?;

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
                        arr.iter().flatten().map(|v| String::from(v)).collect(),
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

fn match_value(record: &vcf::record::Record, f: Field) -> Result {
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
            _ => Err(FieldError::InvalidFieldName(s)),
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

    fn value(&self, f: crate::position::Field) -> Result {
        // TODO: implement this!
        match_value(self, f)
    }
}

impl<'a> crate::position::PositionedIterator for BedderVCF<'a> {
    type Item = vcf::Record;

    fn next_position(
        &mut self,
        _q: Option<&dyn crate::position::Positioned>,
    ) -> Option<std::result::Result<Self::Item, std::io::Error>> {
        let mut v = vcf::Record::default();

        match self.reader.read_record(&self.header, &mut v) {
            Ok(0) => None, // EOF
            Ok(_) => {
                self.record_number += 1;
                Some(Ok(v))
            }
            Err(e) => Some(Err(e)),
        }
    }
    fn name(&self) -> String {
        String::from("vcf")
    }
}

pub fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let vcf_path = "sample.vcf.gz";
    let index_path = "sample.vcf.gz.csi";

    // Open the VCF file
    let vcf_file = File::open(vcf_path).unwrap();
    let vcf_file_reader = BufReader::new(vcf_file);
    //let mut vcf_reader = vcf::Reader::new(vcf_file_reader);

    // Read the VCF header
    //let header = vcf_reader.read_header().unwrap();

    // Open the index
    let index = csi::read(index_path).unwrap();

    // Build an indexed VCF reader
    let mut reader = vcf::indexed_reader::Builder::default()
        .set_index(index)
        .build_from_reader(vcf_file_reader)
        .unwrap();

    let header = reader.read_header()?;
    let b = Box::new(reader);
    let br = BedderVCF::new(b, header.clone())?;

    /*
    // Define the region to query
    let start = Position::try_from(1)?;
    let stop = Position::try_from(1_000_000)?;
    let region = Region::new("chr1", start..=stop);

    // Query the region
    let query = reader.query(&header, &region)?;

    // Iterate over variants in the region
    for result in query {
        let record = result.unwrap();
        println!("{:?}", record);
    }
    */
    Ok(())
}

// tests
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_match_info() {
        let key: field::Key = "AAA".parse().expect("error parsing key");

        let info: vcf::record::Info = [(key.clone(), Some(field::Value::Integer(1)))]
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
            key.clone(),
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
            assert!(false);
        }
    }
}
