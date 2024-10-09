use crate::position::Position;
use crate::report::{Report, ReportFragment};
use crate::sniff::Compression;
use crate::sniff::FileFormat;
use noodles::bam;
use noodles::bcf;
use noodles::bgzf;
use noodles::sam;
use noodles::vcf::{self, Header, Record};
use std::io::Write;
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
    IncompatibleFormats(FileFormat, FileFormat, String),
    UnsupportedFormat(FileFormat),
}

pub enum InputHeader {
    Vcf(vcf::Header),
    Sam(sam::Header),
    None,
}

pub struct Writer {
    in_fmt: FileFormat,
    out_fmt: FileFormat,
    compression: Compression,
    writer: GenomicWriter,
    header: Option<Header>,
}

impl Writer {
    pub fn init(
        in_fmt: FileFormat,
        out_fmt: Option<FileFormat>,
        compression: Compression,
        writer: Box<dyn Write>,
        input_header: InputHeader,
    ) -> Result<Self, FormatConversionError> {
        let out_fmt = match out_fmt {
            Some(f) => f,
            None => match in_fmt {
                FileFormat::BAM | FileFormat::CRAM => FileFormat::SAM,
                FileFormat::BCF => FileFormat::VCF,
                _ => in_fmt.clone(),
            },
        };

        let header = match input_header {
            InputHeader::Vcf(h) => Some(h),
            InputHeader::Sam(_) => None, // We'll need to convert SAM header to VCF header if needed
            InputHeader::None => None,
        };

        let genomic_writer = match out_fmt {
            FileFormat::VCF => GenomicWriter::Vcf(vcf::Writer::new(writer)),
            FileFormat::BCF => GenomicWriter::Bcf(bcf::Writer::new(writer)),
            FileFormat::BAM => GenomicWriter::Bam(bam::Writer::new(writer)),
            FileFormat::BED => GenomicWriter::Bed(writer),
            // Handle other formats
            _ => return Err(FormatConversionError::UnsupportedFormat(out_fmt)),
        };

        Ok(Self {
            in_fmt: in_fmt.clone(),
            out_fmt,
            compression,
            writer: genomic_writer,
            header,
        })
    }

    pub fn write_vcf_header(&mut self, header: &Header) -> Result<(), std::io::Error> {
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
        self.header = Some(header.clone());
        Ok(())
    }

    pub fn write(
        &mut self,
        report: &Report,
        crs: &[Box<dyn ColumnReporter>],
    ) -> Result<(), std::io::Error> {
        match self.out_fmt {
            FileFormat::VCF => {
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
                    for cr in crs {
                        if let Ok(value) = cr.value(fragment) {
                            let key = cr.name().parse().map_err(|e| {
                                std::io::Error::new(std::io::ErrorKind::InvalidData, e)
                            })?;
                            let field_value = self.convert_to_vcf_value(&value)?;
                            record.info_mut().insert(key, Some(field_value));
                        }
                    }

                    vcf_writer.write_record(&self.header, &record)?;
                }
            }
            FileFormat::BED => {
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
            FileFormat::SAM => {
                // Implement SAM writing logic
                unimplemented!("SAM writing not yet implemented");
            }
            _ => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    format!("Unsupported output format: {:?}", self.out_fmt),
                ));
            }
        }
        Ok(())
    }

    fn convert_to_vcf_value(
        &self,
        value: &Value,
    ) -> Result<vcf::record::info::field::Value, std::io::Error> {
        match value {
            Value::Int(i) => Ok(vcf::record::info::field::Value::Integer(*i)),
            Value::Float(f) => Ok(vcf::record::info::field::Value::Float(*f)),
            Value::String(s) => Ok(vcf::record::info::field::Value::String(s.clone())),
            Value::Flag(_b) => Ok(vcf::record::info::field::Value::Flag),
            Value::VecInt(v) => Ok(vcf::record::info::field::Value::Array(
                vcf::record::info::field::value::Array::Integer(
                    v.iter().map(|&x| Some(x)).collect(),
                ),
            )),
            Value::VecFloat(v) => Ok(vcf::record::info::field::Value::Array(
                vcf::record::info::field::value::Array::Float(v.iter().map(|&x| Some(x)).collect()),
            )),
            Value::VecString(v) => Ok(vcf::record::info::field::Value::Array(
                vcf::record::info::field::value::Array::String(
                    v.iter().map(|x| Some(x.to_string())).collect(),
                ),
            )),
        }
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
    Vcf(vcf::Writer<Box<dyn Write>>),
    Bcf(bcf::Writer<bgzf::Writer<Box<dyn Write>>>),
    Bam(bam::Writer<bgzf::Writer<Box<dyn Write>>>),
    Bed(Box<dyn Write>),
    Gff(gff::Writer<Box<dyn Write>>),
    // Add other formats as needed
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
