use crate::column::{Column, ColumnReporter, Value};
use crate::hts_format::{Compression, Format};
use crate::intersection::Intersections;
use crate::position::Position;
use crate::report::Report;
use crate::report_options::ReportOptions;
use rust_htslib::bam;
use rust_htslib::bcf::{self, header::HeaderView};
use rust_htslib::htslib as hts;
use simplebed::{self, BedValue};
use std::fmt;
use std::mem;
use std::rc::Rc;
use std::result::Result;
use std::string::String;
use std::sync::Arc;

#[derive(Debug)]
pub enum FormatConversionError {
    HtslibError(String),
    UnsupportedFormat(hts::htsExactFormat),
    IoError(std::io::Error),
}

impl fmt::Display for FormatConversionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FormatConversionError::HtslibError(msg) => write!(f, "HTSlib error: {}", msg),
            FormatConversionError::UnsupportedFormat(format) => {
                write!(f, "Unsupported format: {:?}", format)
            }
            FormatConversionError::IoError(e) => write!(f, "IO error: {}", e),
        }
    }
}

impl std::error::Error for FormatConversionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            FormatConversionError::IoError(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for FormatConversionError {
    fn from(error: std::io::Error) -> Self {
        FormatConversionError::IoError(error)
    }
}

#[derive(Clone, Debug)]
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

impl fmt::Debug for GenomicWriter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GenomicWriter::Vcf(_) => write!(f, "GenomicWriter::Vcf"),
            GenomicWriter::Bcf(_) => write!(f, "GenomicWriter::Bcf"),
            GenomicWriter::Bed(_) => write!(f, "GenomicWriter::Bed"),
        }
    }
}

#[allow(dead_code)]
pub struct Writer {
    format: Format,
    compression: Compression,
    writer: GenomicWriter,
    header: InputHeader,
}

#[allow(dead_code)]
struct BCFWriter {
    _inner: *mut hts::htsFile,
    _header: Rc<HeaderView>,
    _subset: Option<bcf::header::SampleSubset>,
}
const _: () = assert!(mem::size_of::<BCFWriter>() == mem::size_of::<bcf::Writer>());

// This helper function converts a given `Value` into one or more `BedValue`s and
// pushes them onto the provided mutable bed record.
fn push_value_to_bed_record(bed_record: &mut crate::bedder_bed::BedRecord, value: Value) {
    match value {
        Value::Int(i) => {
            bed_record
                .inner_mut()
                .push_field(BedValue::Integer(i as i64));
        }
        Value::Float(f) => {
            bed_record.inner_mut().push_field(BedValue::Float(f as f64));
        }
        Value::String(s) => {
            bed_record.inner_mut().push_field(BedValue::String(s));
        }
        Value::Flag(b) => {
            bed_record
                .inner_mut()
                .push_field(BedValue::Integer(if b { 1 } else { 0 }));
        }
        Value::VecInt(v) => {
            v.into_iter().for_each(|i| {
                bed_record
                    .inner_mut()
                    .push_field(BedValue::Integer(i as i64));
            });
        }
        Value::VecFloat(v) => {
            v.into_iter().for_each(|f| {
                bed_record.inner_mut().push_field(BedValue::Float(f as f64));
            });
        }
        Value::VecString(v) => {
            v.into_iter().for_each(|s| {
                bed_record.inner_mut().push_field(BedValue::String(s));
            });
        }
    }
}

fn update_header(header: &mut bcf::Header, columns: &[Column]) {
    for column in columns {
        let name = column.name();
        // INFO=<ID=ID,Number=number,Type=type,Description="description",Source="source",Version="version">
        // We'll use Number="." for unknown number of values, Type="String" as a general type for now.
        let info_line = format!(
            "##INFO=<ID={},Number={},Type={},Description=\"{}\">",
            name,
            column.number(),
            column.ftype(),
            column.description(),
        );
        header.push_record(info_line.as_bytes());
    }
}

impl Writer {
    pub fn init(
        path: &str,
        format: Option<Format>,
        compression: Option<Compression>,
        input_header: InputHeader,
        columns: &[Column],
    ) -> Result<Self, FormatConversionError> {
        // Detect format if not specified
        let format = match format {
            Some(f) => f,
            None => unimplemented!("format must be specified"),
        };
        let path = if path == "-" { "/dev/stdout" } else { path };

        // Use default compression if not specified
        let compression = compression.unwrap_or(Compression::None);
        // TODO: set compression in htslib.
        eprintln!("in writer.init, format: {:?}", format);

        let writer = match format {
            Format::Vcf | Format::Bcf => {
                eprintln!("in writer.init, input_header: {:?}", input_header);
                let mut header = match &input_header {
                    InputHeader::Vcf(h) => bcf::Header::from_template(h),
                    InputHeader::Sam(_) => {
                        return Err(FormatConversionError::UnsupportedFormat(format.into()))
                    }
                    InputHeader::None => {
                        // TODO: create a minimal header if none is provided.
                        // For now, error out.
                        return Err(FormatConversionError::UnsupportedFormat(format.into()));
                    }
                };
                eprintln!("header before: {:?}", header);
                update_header(&mut header, columns);
                eprintln!("header: {:?}", header);

                let writer = bcf::Writer::from_path(
                    path,
                    &header,
                    compression == Compression::None,
                    if format == Format::Vcf {
                        bcf::Format::Vcf
                    } else {
                        bcf::Format::Bcf
                    },
                )
                .map_err(|e| FormatConversionError::HtslibError(e.to_string()))?;
                GenomicWriter::Bcf(writer)
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

        Ok(Self {
            format,
            compression,
            writer,
            header: input_header.clone(),
        })
    }

    #[allow(dead_code)]
    fn add_info_field_to_vcf_record(
        record: &mut bcf::Record,
        key: &str,
        value: &Value,
    ) -> Result<(), std::io::Error> {
        let key_bytes = key.as_bytes();
        match value {
            Value::Int(i) => {
                let vals = vec![*i];
                record.push_info_integer(key_bytes, &vals)
            }
            Value::Float(f) => {
                let vals = vec![*f];
                record.push_info_float(key_bytes, &vals)
            }
            Value::String(s) => {
                let byte_slice = vec![s.as_bytes()];
                record.push_info_string(key_bytes, &byte_slice)
            }
            Value::Flag(b) => {
                if *b {
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

    fn apply_report<T: ColumnReporter>(
        format: Format,
        intersections: &mut Intersections,
        report_options: Arc<ReportOptions>,
        crs: &[T],
    ) -> Result<Arc<Report>, std::io::Error> {
        let report = intersections.report(&report_options);

        match format {
            Format::Vcf => {
                for frag in report.iter() {
                    let mut record = frag
                        .a
                        .as_ref()
                        .expect("Fragment Position is not a VCF record")
                        .try_lock()
                        .expect("Failed to lock VCF Position");
                    match *record {
                        Position::Vcf(ref mut record) => {
                            for cr in crs.iter() {
                                if let Ok(value) = cr.value(frag) {
                                    Self::add_info_field_to_vcf_record(
                                        &mut record.record,
                                        cr.name(),
                                        &value,
                                    )?;
                                }
                            }
                        }
                        _ => {
                            return Err(std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                "Position is not a VCF record",
                            ))
                        }
                    };
                }
            }
            Format::Bed => {
                for frag in report.iter() {
                    let mut values = Vec::with_capacity(crs.len());
                    for b in frag.b.iter() {
                        let b = b.try_lock().expect("Failed to lock b-Position");
                        values.push(Value::String(b.chrom().to_string()));
                        values.push(Value::Int(b.start() as i32));
                        values.push(Value::Int(b.stop() as i32));
                    }
                    for cr in crs.iter() {
                        match cr.value(frag) {
                            Ok(value) => values.push(value),
                            Err(e) => {
                                return Err(std::io::Error::new(
                                    std::io::ErrorKind::InvalidData,
                                    format!(
                                        "Error getting value for column: {}. Error: {}",
                                        cr.name(),
                                        e
                                    ),
                                ));
                            }
                        }
                    }
                    log::info!("frag.a: {:?}", frag.a);
                    let mut a_lock = frag
                        .a
                        .as_ref()
                        .expect("Fragment Position is not a BED interval")
                        .try_lock()
                        .expect("Failed to lock BED Position");
                    match *a_lock {
                        Position::Bed(ref mut bed_record) => {
                            // Add all values to our clone
                            for value in values {
                                push_value_to_bed_record(bed_record, value);
                            }
                        }
                        Position::Vcf(ref mut record) => {
                            for (i, value) in values.iter().enumerate() {
                                Self::add_info_field_to_vcf_record(
                                    &mut record.record,
                                    crs[i].name(),
                                    value,
                                )?;
                            }
                        }
                        _ => {
                            unimplemented!("Interval writing not yet implemented");
                        }
                    }
                }
            }
            _ => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    format!("Currently unsupported output format: {:?}", format),
                ))
            }
        }
        Ok(report)
    }

    pub fn write<T: ColumnReporter>(
        &mut self,
        intersections: &mut Intersections,
        report_options: Arc<ReportOptions>,
        crs: &[T],
    ) -> Result<(), std::io::Error> {
        log::info!("got writer: {:?}", self.writer);
        match self.format {
            Format::Vcf => {
                let vcf_writer = match &mut self.writer {
                    GenomicWriter::Vcf(writer) | GenomicWriter::Bcf(writer) => writer,
                    _ => {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            "Expected VCF writer, but found a different format",
                        ));
                    }
                };

                let report = Self::apply_report(self.format, intersections, report_options, crs)?;

                for fragment in report.iter() {
                    if let Position::Vcf(ref record) = *fragment
                        .a
                        .as_ref()
                        .expect("Fragment Position is not a VCF record")
                        .try_lock()
                        .expect("Failed to lock VCF Position")
                    {
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

                let report = Self::apply_report(self.format, intersections, report_options, crs)?;

                for frag in report.iter() {
                    if let Position::Bed(ref bed_record) = *frag
                        .a
                        .as_ref()
                        .expect("Position is not a BED interval")
                        .try_lock()
                        .expect("Failed to lock Position")
                    {
                        bed_writer.write_record(bed_record.inner()).map_err(|e| {
                            std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())
                        })?;
                    } else {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "Position is not a BED interval",
                        ));
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
