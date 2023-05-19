use crate::position::{Field, FieldError, Positioned, Result, Value};
use noodles::core::{Position, Region};
use noodles::csi;
use noodles::vcf::{self, record::Chromosome};
use std::fs::File;
use std::io::{self, BufRead, BufReader};

pub struct BedderVCF<R>
where
    R: BufRead,
{
    reader: vcf::Reader<R>,
    header: vcf::Header,
    buf: std::string::String,
    record_number: u64,
}

impl<R> BedderVCF<R>
where
    R: BufRead,
{
    pub fn new(r: R) -> io::Result<BedderVCF<R>> {
        let mut v = BedderVCF {
            reader: vcf::Reader::new(r),
            header: vcf::Header::default(),
            buf: std::string::String::new(),
            record_number: 0,
        };
        v.header = v.reader.read_header()?;
        Ok(v)
    }
}

impl crate::position::Positioned for vcf::record::Record {
    #[inline]
    fn chrom(&self) -> &str {
        match self.chromosome() {
            Chromosome::Name(s) => &s,
            Chromosome::Symbol(s) => &s,
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
        match f {
            Field::String(s) => Ok(Value::Strings(vec![s])),
            Field::Int(i) => match i {
                0 => Ok(Value::Strings(vec![String::from(self.chrom())])),
                1 => Ok(Value::Ints(vec![self.start() as i64])),
                2 => Ok(Value::Ints(vec![self.stop() as i64])),
                _ => Err(FieldError::InvalidFieldIndex(i)),
            },
        }
    }
}

impl<R> crate::position::PositionedIterator for BedderVCF<R>
where
    R: BufRead,
{
    type Item = vcf::Record;

    fn next_position(
        &mut self,
        _q: Option<&dyn crate::position::Positioned>,
    ) -> Option<std::result::Result<Self::Item, std::io::Error>> {
        let mut record = vcf::Record::default();
        let result = self.reader.read_record(&self.header, &mut record);

        match result {
            Ok(0) => None,
            Ok(_) => {
                self.record_number += 1;
                Some(Ok(record))
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
    let index = csi::read(&index_path).unwrap();

    // Build an indexed VCF reader
    let mut reader = vcf::indexed_reader::Builder::default()
        .set_index(index)
        .build_from_reader(vcf_file_reader)
        .unwrap();

    let header = reader.read_header()?;

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
    Ok(())
}
