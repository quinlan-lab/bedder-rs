#![allow(clippy::useless_conversion)] // these are needed to support e.g. smartstring
use crate::position::{Field, FieldError, Position, Positioned, Value};
use crate::skip::Skip;
use crate::string::String;

use rust_htslib::{self, bcf, bcf::Read};
use std::io;
use std::result;

pub struct BedderVCF {
    reader: bcf::Reader,
    record_number: u64,
    #[allow(unused)]
    pub header: bcf::header::HeaderView,
    last_record: Option<bcf::Record>,
    path: String,
}

impl BedderVCF {
    pub fn new(r: bcf::Reader, path: String) -> io::Result<BedderVCF> {
        let h = r.header().clone();
        let v = BedderVCF {
            reader: r,
            record_number: 0,
            header: h,
            last_record: None,
            path: path,
        };
        Ok(v)
    }

    pub fn from_path(p: &str) -> io::Result<BedderVCF> {
        if p == "-" || p == "stdin" || p == "/dev/stdin" {
            let r =
                bcf::Reader::from_stdin().map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            BedderVCF::new(r, String::from("stdin"))
        } else {
            let r =
                bcf::Reader::from_path(p).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            BedderVCF::new(r, String::from(p))
        }
    }
}

use rust_htslib::errors::Error;

impl Skip for BedderVCF {
    fn skip_to(&mut self, chrom: &str, pos0: u64) -> io::Result<()> {
        let rid = self.reader.header().name2rid(chrom.as_bytes()).unwrap();

        match self.reader.fetch(rid, pos0, None) {
            Ok(()) => Ok(()),
            Err(Error::FileNotFound { .. }) => {
                // iterate over the vcf until we get to the chrom, pos0
                // and then fetch the record
                for r in self.reader.records() {
                    let r = r.unwrap();
                    if r.rid().unwrap_or(u32::MAX) > rid {
                        self.last_record = Some(r);
                        break;
                    }
                    if r.rid().unwrap_or(u32::MAX) < rid || (r.pos() as u64) < pos0 {
                        continue;
                    }
                    self.last_record = Some(r);
                    break;
                }
                Ok(())
            }
            Err(e) => Err(io::Error::new(io::ErrorKind::Other, e)),
        }
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
    pub chrom: Option<String>,
}

impl Clone for BedderRecord {
    fn clone(&self) -> Self {
        Self {
            record: self.record.clone(),
            chrom: self.chrom.clone(),
        }
    }
}

impl BedderRecord {
    pub fn new(record: bcf::Record) -> Self {
        let chrom_name = record.header().rid2name(record.rid().unwrap()).unwrap();
        let chrom = unsafe { String::from_utf8_unchecked(chrom_name.to_vec()) };
        Self {
            record,
            chrom: Some(chrom),
        }
    }
}

impl Positioned for BedderRecord {
    #[inline]
    fn chrom(&self) -> &str {
        self.chrom.as_ref().unwrap()
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
        log::info!("vcf clone box");
        Box::new(Self {
            record: self.record.clone(),
            chrom: self.chrom.clone(),
        })
    }
}

fn debug_record(r: &bcf::Record) -> String {
    let chrom = String::from_utf8_lossy(r.header().rid2name(r.rid().unwrap()).unwrap());
    let ref_allele = String::from_utf8_lossy(r.alleles()[0]);
    let alt_alleles = if r.alleles().len() > 1 {
        r.alleles()[1..]
            .iter()
            .map(|a| String::from_utf8_lossy(a).to_string())
            .collect::<Vec<_>>()
            .join(",")
    } else {
        "".to_string()
    };
    format!(
        "{}:{:?}-{:?}({}/{})",
        chrom,
        r.pos(),
        r.end(),
        ref_allele,
        alt_alleles
    )
}

impl crate::position::PositionedIterator for BedderVCF {
    fn next_position(
        &mut self,
        q: Option<&crate::position::Position>,
    ) -> Option<std::result::Result<Position, std::io::Error>> {
        if let Some(q) = q {
            match self.skip_to(q.chrom(), q.start() - 1_u64) {
                Ok(_) => (),
                Err(e) => return Some(Err(e)),
            }
        }

        if let Some(v) = self.last_record.take() {
            self.record_number += 1;
            return Some(Ok(Position::Vcf(Box::new(BedderRecord::new(v)))));
        }

        let mut r = self.reader.empty_record();

        match self.reader.read(&mut r) {
            None => None, // EOF
            Some(Ok(())) => {
                self.record_number += 1;
                log::trace!(
                    "read vcf record: {:?} from file: {}",
                    debug_record(&r),
                    self.path
                );
                Some(Ok(Position::Vcf(Box::new(BedderRecord::new(r)))))
            }
            Some(Err(e)) => {
                log::error!(
                    "error reading vcf record: {} at line number: {}",
                    e,
                    self.record_number
                );
                Some(Err(io::Error::new(io::ErrorKind::Other, e)))
            }
        }
    }
    fn name(&self) -> String {
        String::from(format!("VCF|{}:record #:{}", self.path, self.record_number))
    }
}

// tests
#[cfg(test)]
mod tests {}
