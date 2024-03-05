use crate::sniff::Compression;
use crate::sniff::FileFormat;
use crate::report::Report;
use std::string::String;

pub struct Writer {
    in_fmt: FileFormat,
    out_fmt: FileFormat,
    compression: Compression,
}

pub enum Type {
    Integer,
    Float,
    Character,
    String,
    Flag,
}

pub enum Number {
    Not,
    One,
    R,
    A,
    Dot,
}

pub trait ColumnReporter {
   /// report the name, e.g. `count` for the INFO field of the VCF 
   fn name(&self) -> String;
   /// report the type, for the INFO field of the VCF
   fn ftype(&self) -> Type; // Type is some enum from noodles or here that limits to relevant types
   fn description(&self) -> String;
   fn number(&self) -> Number;

  fn value(&self, r: &Report) -> Value // Value probably something from noodles that encapsulates Float/Int/Vec<Float>/String/...
}

pub enum FormatConversionError {
    IncompatibleFormats(FileFormat, FileFormat, String),
}

impl Writer {
    pub fn init(
        in_fmt: FileFormat,
        out_fmt: Option<FileFormat>,
        compression: Compression,
    ) -> Result<Self, FormatConversionError> {
        let out_fmt = match out_fmt {
            Some(f) => f,
            // TODO: may want, e.g. BAM/CRAM to default to SAM
            // and BCF to default to VCF.
            None => in_fmt,
        };

        // if out_fmt is the same as in_fmt, then we can just pass through
        if in_fmt == out_fmt {
            return Ok(Self {
                in_fmt,
                out_fmt,
                compression,
            });
        }

        // if out_fmt is different from in_fmt, then we need to convert
        match (in_fmt, out_fmt) {
            (FileFormat::VCF, FileFormat::BED) => {
                // convert vcf to bed
            }
            (FileFormat::BED, FileFormat::VCF) => {
                // convert bed to vcf
            }
            _ => Err(FormatConversionError::IncompatibleFormats(
                in_fmt,
                out_fmt,
                String::from("No conversion yet available. Please report"),
            )),
        }
    }
}