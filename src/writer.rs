use crate::hts_format::{Compression, Format};
use crate::position::Position;
use crate::report::{Report, ReportFragment};
//use crate::sniff::HtsFile;
use rust_htslib::bam;
use rust_htslib::bcf::{self, header::HeaderView};
use rust_htslib::htslib as hts;
use simplebed::{self, BedValue};
use std::mem;
use std::rc::Rc;
use std::result::Result;
use std::string::String;

pub enum Type {
    Integer,
    Float,
    Character,
    String,
    Flag,
}

/// The number of Values to expect (similar to Number attribute in VCF INFO/FMT fields)
pub enum Number {
    Not,
    One,
    R,
    A,
    Dot,
}

pub enum Value {
    Int(i32),
    Float(f32),
    String(String),
    Flag(bool),
    VecInt(Vec<i32>),
    VecFloat(Vec<f32>),
    VecString(Vec<String>),
}

pub enum ColumnError {
    InvalidValue(String),
}

/// A ColumnReporter tells bedder how to report a column in the output.
pub trait ColumnReporter {
    /// report the name, e.g. `count` for the INFO field of the VCF
    fn name(&self) -> String;
    /// report the type, for the INFO field of the VCF
    fn ftype(&self) -> Type; // Type is some enum from noodles or here that limits to relevant types
    fn description(&self) -> String;
    fn number(&self) -> Number;

    fn value(&self, r: &ReportFragment) -> Result<Value, ColumnError>; // Value probably something from noodles that encapsulates Float/Int/Vec<Float>/String/...
}

#[derive(Debug)]
pub enum FormatConversionError {
    HtslibError(String),
    UnsupportedFormat(hts::htsExactFormat),
    IoError(std::io::Error),
}

impl From<std::io::Error> for FormatConversionError {
    fn from(error: std::io::Error) -> Self {
        FormatConversionError::IoError(error)
    }
}

pub enum InputHeader {
    Vcf(HeaderView),
    Sam(bam::Header),
    None,
}

/// A writer for the possible genomic formats
pub enum GenomicWriter {
    Vcf(bcf::Writer),
    Bcf(bcf::Writer),
    //Bam(bam::Writer),
    //Sam(bam::Writer),
    Bed(simplebed::BedWriter),
    //Gff(gff::Writer<HFile>),
}

#[allow(dead_code)]
pub struct Writer {
    format: Format,
    compression: Compression,
    writer: GenomicWriter,
    header: Option<InputHeader>,
}

#[allow(dead_code)]
struct BCFWriter {
    _inner: *mut hts::htsFile,
    _header: Rc<HeaderView>,
    _subset: Option<bcf::header::SampleSubset>,
}
const _: () = assert!(mem::size_of::<BCFWriter>() == mem::size_of::<bcf::Writer>());

impl Writer {
    pub fn init(
        path: &str,
        format: Option<Format>,
        compression: Option<Compression>,
        input_header: InputHeader,
    ) -> Result<Self, FormatConversionError> {
        // Detect format if not specified
        let format = match format {
            Some(f) => f,
            None => unimplemented!("format must be specified"),
        };

        // Use default compression if not specified
        let compression = compression.unwrap_or(Compression::None);
        // TODO: set compression in htslib.

        let writer = match format {
            Format::Vcf | Format::Bcf => {
                /*
                let write_mode = match format {
                    Format::Vcf => "wz",
                    Format::Bcf => "wb",
                    _ => unreachable!(),
                };
                */
                unimplemented!("VCF/BCF writing not yet implemented");
            }
            Format::Bam => {
                unimplemented!("BAM writing not yet implemented");
            }
            Format::Bed => {
                let bed_writer = simplebed::BedWriter::new(path)
                    .map_err(|e| FormatConversionError::HtslibError(e.to_string()))?;
                GenomicWriter::Bed(bed_writer)
            }
            _ => return Err(FormatConversionError::UnsupportedFormat(format.into())),
        };

        let header = match &input_header {
            InputHeader::Vcf(h) => Some(InputHeader::Vcf(h.clone())),
            InputHeader::Sam(_) => None,
            InputHeader::None => None,
        };

        Ok(Self {
            format,
            compression,
            writer,
            header,
        })
    }

    fn add_info_field_to_vcf_record(
        record: &mut bcf::Record,
        key: String,
        value: Value,
    ) -> Result<(), std::io::Error> {
        let key_bytes = key.as_bytes();
        match value {
            Value::Int(i) => {
                let vals = vec![i];
                record.push_info_integer(key_bytes, &vals)
            }
            Value::Float(f) => {
                let vals = vec![f];
                record.push_info_float(key_bytes, &vals)
            }
            Value::String(s) => {
                let byte_slice = vec![s.as_bytes()];
                record.push_info_string(key_bytes, &byte_slice)
            }
            Value::Flag(b) => {
                if b {
                    record.push_info_flag(key_bytes)
                } else {
                    record.clear_info_flag(key_bytes)
                }
            }
            Value::VecInt(v) => record.push_info_integer(key_bytes, &v),
            Value::VecFloat(v) => record.push_info_float(key_bytes, &v),
            Value::VecString(v) => {
                let byte_slices: Vec<&[u8]> = v.iter().map(|s| s.as_bytes()).collect();
                record.push_info_string(key_bytes, &byte_slices)
            }
        }
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))
    }

    fn update(
        format: Format,
        fragment: &mut ReportFragment,
        crs: &[Box<dyn ColumnReporter>],
    ) -> Result<(), std::io::Error> {
        match format {
            Format::Vcf => {
                // Get mutable reference to the VCF record
                /*
                let record = match &mut fragment.a {
                    Some(Position::Vcf(record)) => &mut record.record,
                    Some(_) => {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "Position is not a VCF record",
                        ))
                    }
                    None => {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "missing position",
                        ))
                    }
                };

                for cr in crs {
                    if let Ok(value) = cr.value(fragment) {
                        Self::add_info_field_to_vcf_record(record, cr.name(), value)?;
                    }
                }
                */
                unimplemented!("VCF writing not yet implemented");
            }
            Format::Bed => {
                let mut values = Vec::with_capacity(crs.len());

                for cr in crs.into_iter() {
                    if let Ok(value) = cr.value(fragment) {
                        values.push(value);
                    }
                }
                let bed_record = match &mut fragment.a {
                    Some(Position::Bed(ref mut interval)) => interval,
                    Some(_) => {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "Position is not a BED interval",
                        ))
                    }
                    None => {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "no a interval",
                        ))
                    }
                };

                for value in values {
                    match value {
                        Value::Int(i) => bed_record
                            .inner_mut()
                            .push_field(BedValue::Integer(i as i64)),
                        Value::Float(f) => {
                            bed_record.inner_mut().push_field(BedValue::Float(f as f64))
                        }
                        Value::String(s) => bed_record.inner_mut().push_field(BedValue::String(s)),
                        Value::Flag(b) => bed_record
                            .inner_mut()
                            .push_field(BedValue::Integer(if b { 1 } else { 0 })),
                        Value::VecInt(v) => {
                            v.iter().for_each(|i| {
                                bed_record
                                    .inner_mut()
                                    .push_field(BedValue::Integer(*i as i64));
                            });
                        }
                        Value::VecFloat(v) => {
                            v.iter().for_each(|f| {
                                bed_record
                                    .inner_mut()
                                    .push_field(BedValue::Float(*f as f64));
                            });
                        }
                        Value::VecString(v) => {
                            v.iter().for_each(|s| {
                                bed_record
                                    .inner_mut()
                                    .push_field(BedValue::String(s.clone()));
                            });
                        }
                    }
                }
            }
            Format::Sam => unimplemented!("SAM writing not yet implemented"),
            _ => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    format!("Unsupported output format: {:?}", format),
                ))
            }
        }
        Ok(())
    }

    pub fn write(
        &mut self,
        report: &mut Report,
        crs: &[Box<dyn ColumnReporter>],
    ) -> Result<(), std::io::Error> {
        if report.is_empty() {
            return Ok(());
        }

        match self.format {
            Format::Vcf => {
                let vcf_writer = match &mut self.writer {
                    GenomicWriter::Vcf(writer) => writer,
                    _ => {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            "Expected VCF writer, but found a different format",
                        ))
                    }
                };

                for fragment in report.iter_mut() {
                    Self::update(self.format, fragment, crs)?;
                    if let Some(Position::Vcf(record)) = &fragment.a {
                        vcf_writer.write(&record.record).map_err(|e| {
                            std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())
                        })?;
                    }
                }
            }
            Format::Bed => {
                let bed_writer = match &mut self.writer {
                    GenomicWriter::Bed(writer) => writer,
                    _ => {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            "Expected BED writer, but found a different format",
                        ))
                    }
                };

                for fragment in report {
                    Self::update(self.format, fragment, crs)?;
                    if let Some(Position::Bed(interval)) = &fragment.a {
                        bed_writer.write_record(interval.inner()).map_err(|e| {
                            std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())
                        })?;
                    }
                }
            }
            Format::Sam => unimplemented!("SAM writing not yet implemented"),
            _ => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    format!("Unsupported output format: {:?}", self.format),
                ))
            }
        }
        Ok(())
    }

    // TODO: use serde?
    #[allow(unused)]
    fn format_value(&self, value: &Value) -> String {
        match value {
            Value::Int(i) => i.to_string(),
            Value::Float(f) => f.to_string(),
            Value::String(s) => s.clone(),
            Value::Flag(b) => if *b { "1" } else { "0" }.to_string(),
            Value::VecInt(v) => v
                .iter()
                .map(|i| i.to_string())
                .collect::<Vec<String>>()
                .join(","),
            Value::VecFloat(v) => v
                .iter()
                .map(|f| f.to_string())
                .collect::<Vec<String>>()
                .join(","),
            Value::VecString(v) => v.join(","),
        }
    }
}
