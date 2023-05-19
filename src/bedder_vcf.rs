use crate::position::{Field, FieldError, Result, Value};
use crate::string::String;
use noodles::bcf;
use noodles::core::{Position, Region};
use noodles::csi;
use noodles::vcf::{self, record::Chromosome};
use std::fs::File;
use std::io::{self, BufRead, BufReader};

pub trait VCFReader {
    fn read_header(&mut self) -> io::Result<vcf::Header>;
    fn read_record(&mut self, header: &vcf::Header, v: &mut vcf::Record) -> io::Result<usize>;

    // fn queryable
}

impl<R> VCFReader for vcf::Reader<R>
where
    R: BufRead,
{
    fn read_header(&mut self) -> io::Result<vcf::Header> {
        self.read_header()
    }

    #[inline]
    fn read_record(&mut self, header: &vcf::Header, v: &mut vcf::Record) -> io::Result<usize> {
        self.read_record(header, v)
    }
}

impl<R> VCFReader for vcf::indexed_reader::IndexedReader<R>
where
    R: BufRead,
{
    fn read_header(&mut self) -> io::Result<vcf::Header> {
        self.read_header()
    }

    #[inline]
    fn read_record(&mut self, header: &vcf::Header, v: &mut vcf::Record) -> io::Result<usize> {
        self.read_record(header, v)
    }
}

impl<R> VCFReader for bcf::Reader<R>
where
    R: BufRead,
{
    fn read_header(&mut self) -> io::Result<vcf::Header> {
        self.read_header()
    }

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
    pub fn new(r: Box<dyn VCFReader>, header: vcf::Header) -> io::Result<BedderVCF<'a>> {
        let v = BedderVCF {
            reader: r,
            header: header,
            record_number: 0,
        };
        Ok(v)
    }
}

impl crate::position::Positioned for vcf::record::Record {
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
