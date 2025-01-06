use crate::position::Position;
use crate::report::{Report, ReportFragment};
use crate::sniff::HtsFile;
use bio::io::bed;
use rust_htslib::bam;
use rust_htslib::bcf::{self, header::HeaderView};
use rust_htslib::htslib as hts;
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
    Bam(bam::Writer),
    Sam(bam::Writer),
    Bed(bed::Writer<HtsFile>),
    //Gff(gff::Writer<HFile>),
}

pub struct Writer {
    format: hts::htsExactFormat,
    compression: hts::htsCompression,
    writer: GenomicWriter,
    header: Option<InputHeader>,
}

pub struct BCFWriter {
    _inner: *mut hts::htsFile,
    _header: Rc<HeaderView>,
    _subset: Option<bcf::header::SampleSubset>,
}
const _: () = assert!(mem::size_of::<BCFWriter>() == mem::size_of::<bcf::Writer>());

impl Writer {
    pub fn init(
        path: &str,
        format: Option<hts::htsExactFormat>,
        compression: Option<hts::htsCompression>,
        input_header: InputHeader,
    ) -> Result<Self, FormatConversionError> {
        // Detect format if not specified
        let format = match format {
            Some(f) => f,
            None => unimplemented!("format must be specified"),
        };

        // Use default compression if not specified
        let compression = compression.unwrap_or(hts::htsCompression_no_compression);
        // TODO: set compression in htslib.

        let writer = match format {
            hts::htsExactFormat_vcf | hts::htsExactFormat_bcf => {
                let write_mode = match format {
                    hts::htsExactFormat_vcf => "wz",
                    hts::htsExactFormat_bcf => "wb",
                    _ => unreachable!(),
                };
                let mut hf = HtsFile::new(path.as_ref(), write_mode)
                    .map_err(|e| FormatConversionError::HtslibError(e.to_string()))?;

                let bwtr = BCFWriter {
                    // TODO: does this cause problems. Since hf will be dropped.
                    _inner: hf.htsfile(),
                    _header: match &input_header {
                        InputHeader::Vcf(header) => Rc::new(header.clone()),
                        _ => return Err(FormatConversionError::UnsupportedFormat(format)),
                    },
                    _subset: None,
                };
                let vcf_writer = unsafe { std::mem::transmute(bwtr) };
                match format {
                    hts::htsExactFormat_vcf => GenomicWriter::Vcf(vcf_writer),
                    hts::htsExactFormat_bcf => GenomicWriter::Bcf(vcf_writer),
                    _ => unreachable!(),
                }
            }
            hts::htsExactFormat_bam => {
                unimplemented!("BAM writing not yet implemented");
            }
            hts::htsExactFormat_bed => {
                let write_mode = match compression {
                    hts::htsCompression_bgzf => "wz",
                    _ => "w",
                };
                eprintln!("open file: {:?} with mode: {}", path, write_mode);
                let hf = HtsFile::new(path.as_ref(), write_mode)
                    .map_err(|e| FormatConversionError::HtslibError(e.to_string()))?;
                let bed_writer = bio::io::bed::Writer::new(hf);
                GenomicWriter::Bed(bed_writer)
            }
            _ => return Err(FormatConversionError::UnsupportedFormat(format)),
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

    pub fn write(
        &mut self,
        report: &Report,
        crs: &[Box<dyn ColumnReporter>],
    ) -> Result<(), std::io::Error> {
        if report.len() == 0 {
            return Ok(());
        }
        match self.format {
            hts::htsExactFormat_vcf => {
                let vcf_writer = match &mut self.writer {
                    GenomicWriter::Vcf(writer) => writer,
                    _ => {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            "Expected VCF writer, but found a different format",
                        ))
                    }
                };

                for fragment in report {
                    // Extract the VCF record from the position using matches!
                    let record = match &fragment.a {
                        Some(position) => match position {
                            Position::Vcf(record) => record, /*.clone() */
                            _ => {
                                return Err(std::io::Error::new(
                                    std::io::ErrorKind::InvalidData,
                                    "Position is not a VCF record",
                                ))
                            }
                        },
                        None => {
                            return Err(std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                "missing position",
                            ))
                        }
                    };

                    // Add INFO fields
                    if !matches!(fragment.a, Some(Position::Vcf(_))) {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "Fragment.a is not a VCF record",
                        ));
                    }
                    let mut vcf_record = record.record.clone();

                    for cr in crs {
                        if let Ok(value) = cr.value(fragment) {
                            Self::add_info_field_to_vcf_record(&mut vcf_record, cr.name(), value)?;
                        }
                    }
                    let vcf_record = &record.record;

                    vcf_writer.write(&vcf_record).map_err(|e| {
                        std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())
                    })?;
                }
            }
            hts::htsExactFormat_bed => {
                eprintln!("report: {:?}", report);
                for fragment in report {
                    // return an error if fragment.a is None
                    let frag_a = fragment.a.as_ref().ok_or_else(|| {
                        std::io::Error::new(std::io::ErrorKind::InvalidData, "missing chromosome")
                    })?;
                    let mut br = bio::io::bed::Record::new();
                    br.set_chrom(frag_a.chrom());
                    br.set_start(frag_a.start());
                    br.set_end(frag_a.stop());
                    // TODO: br.set_name(), etc.
                    for cr in crs {
                        if let Ok(value) = cr.value(fragment) {
                            br.push_aux(self.format_value(&value).as_str());
                        }
                    }

                    if let GenomicWriter::Bed(ref mut writer) = self.writer {
                        writer.write(&br).map_err(|e| {
                            std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())
                        })?;
                    } else {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            "Expected BED writer, but found a different format",
                        ));
                    }
                }
            }
            hts::htsExactFormat_sam => {
                // Implement SAM writing logic
                unimplemented!("SAM writing not yet implemented");
            }
            _ => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    format!("Unsupported output format: {:?}", self.format),
                ));
            }
        }
        Ok(())
    }

    // TODO: use serde?
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
