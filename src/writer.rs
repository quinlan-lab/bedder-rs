use crate::position::Position;
use crate::report::{Report, ReportFragment};
use noodles::bam;
use noodles::sam;
use std::result::Result;
use std::string::String;
use xvcf::rust_htslib::bcf::header::HeaderView;
use xvcf::rust_htslib::htslib as hts;

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
    Sam(sam::Header),
    None,
}

pub struct Writer {
    format: hts::htsExactFormat,
    compression: hts::htsCompression,
    writer: GenomicWriter,
    header: Option<InputHeader>,
}

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

        let writer = match format {
            hts::htsExactFormat_vcf => {
                GenomicWriter::Vcf(vcf::Writer::new(Box::new(HFile::new(path, "w")?)))
            }
            hts::htsExactFormat_bcf => {
                GenomicWriter::Bcf(bcf::Writer::new(Box::new(HFile::new(path, "wb")?)))
            }
            hts::htsExactFormat_bam => {
                GenomicWriter::Bam(bam::Writer::new(Box::new(HFile::new(path, "wb")?)))
            }
            hts::htsExactFormat_bed => GenomicWriter::Bed(Box::new(HFile::new(path, "w")?)),
            _ => return Err(FormatConversionError::UnsupportedFormat(format)),
        };

        let header = match input_header {
            InputHeader::Vcf(h) => Some(InputHeader::Vcf(h)),
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

    pub fn write_vcf_header(&mut self, header: &HeaderView) -> Result<(), std::io::Error> {
        match &mut self.writer {
            GenomicWriter::Vcf(vcf_writer) => {
                vcf_writer.write_header(header)?;
            }
            GenomicWriter::Bcf(bcf_writer) => {
                bcf_writer.write_header(header)?;
            }
            _ => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Cannot write VCF header for non-VCF/BCF formats",
                ));
            }
        }
        self.header = Some(InputHeader::Vcf(header.clone()));
        Ok(())
    }

    pub fn write(
        &mut self,
        report: &Report,
        crs: &[Box<dyn ColumnReporter>],
    ) -> Result<(), std::io::Error> {
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
                    let mut record = match &fragment.a {
                        Some(position) => match position {
                            Position::Vcf(record) => record.clone(),
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
                    let mut vcf_record = record.record;

                    for cr in crs {
                        if let Ok(value) = cr.value(fragment) {
                            let key = cr.name().as_bytes();
                            match value {
                                Value::Int(i) => {
                                    let vals = vec![i];
                                    vcf_record.push_info_integer(key, &vals);
                                }
                                Value::Float(f) => {
                                    let vals = vec![f];
                                    vcf_record.push_info_float(key, &vals);
                                }
                                Value::String(s) => {
                                    let byte_slice = vec![s.as_bytes()];
                                    vcf_record.push_info_string(key, &byte_slice);
                                }
                                Value::Flag(b) => {
                                    if b {
                                        vcf_record.push_info_flag(key);
                                    } else {
                                        vcf_record.clear_info_flag(key);
                                    }
                                }
                                Value::VecInt(v) => {
                                    vcf_record.push_info_integer(key, &v);
                                }
                                Value::VecFloat(v) => {
                                    vcf_record.push_info_float(key, &v);
                                }
                                Value::VecString(v) => {
                                    let byte_slices: Vec<&[u8]> =
                                        v.iter().map(|s| s.as_bytes()).collect();
                                    vcf_record.push_info_string(key, &byte_slices);
                                }
                                _ => {
                                    return Err(std::io::Error::new(
                                        std::io::ErrorKind::InvalidData,
                                        "Unsupported value type",
                                    ));
                                }
                            }
                        }
                    }

                    vcf_writer.write_record(&self.header, &record)?;
                }
            }
            hts::htsExactFormat_bed => {
                for fragment in report {
                    // return an error if fragment.a is None
                    let frag_a = fragment.a.as_ref().ok_or_else(|| {
                        std::io::Error::new(std::io::ErrorKind::InvalidData, "missing chromosome")
                    })?;

                    write!(
                        self.writer,
                        "{}\t{}\t{}",
                        frag_a.chrom(),
                        frag_a.start(),
                        frag_a.stop(),
                    )?;
                    for cr in crs {
                        if let Ok(value) = cr.value(fragment) {
                            write!(self.writer, "\t{}", self.format_value(&value))?;
                        } else {
                            write!(self.writer, "\t.")?;
                        }
                    }
                    writeln!(self.writer)?;
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

pub enum GenomicWriter {
    Vcf(vcf::Writer<HFile>),
    Bcf(bcf::Writer<HFile>),
    Bam(bam::Writer<HFile>),
    Bed(HFile),
    Gff(gff::Writer<HFile>),
}

impl GenomicWriterTrait for GenomicWriter {
    fn write_header(&mut self, header: &Header) -> Result<(), std::io::Error> {
        match self {
            GenomicWriter::Vcf(writer) => writer.write_header(header),
            GenomicWriter::Bcf(writer) => writer.write_header(header),
            GenomicWriter::Bam(writer) => writer.write_header(header),
            GenomicWriter::Bed(_) => Ok(()), // BED doesn't have a header
            GenomicWriter::Gff(writer) => writer.write_header(header),
            // Handle other formats
        }
    }

    fn write_record(&mut self, record: &Record) -> Result<(), std::io::Error> {
        match self {
            GenomicWriter::Vcf(writer) => writer.write_record(record),
            GenomicWriter::Bcf(writer) => writer.write_record(record),
            GenomicWriter::Bam(writer) => writer.write_record(record),
            GenomicWriter::Bed(writer) => {
                // Implement BED record writing
                writeln!(
                    writer,
                    "{}\t{}\t{}",
                    record.chrom(),
                    record.start(),
                    record.end()
                )
            }
            GenomicWriter::Gff(writer) => writer.write_record(record),
            // Handle other formats
        }
    }
}

pub trait GenomicWriterTrait {
    fn write_header(&mut self, header: &Header) -> Result<(), std::io::Error>;
    fn write_record(&mut self, record: &Record) -> Result<(), std::io::Error>;
    // Add other common methods as needed
}
