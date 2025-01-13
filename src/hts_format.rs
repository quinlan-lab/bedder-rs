use crate::writer::FormatConversionError;
use rust_htslib::htslib as hts;

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u32)]
pub enum Format {
    Unknown = hts::htsExactFormat_unknown_format,
    Binary = hts::htsExactFormat_binary_format,
    Text = hts::htsExactFormat_text_format,
    Sam = hts::htsExactFormat_sam,
    Bam = hts::htsExactFormat_bam,
    Bai = hts::htsExactFormat_bai,
    Cram = hts::htsExactFormat_cram,
    Crai = hts::htsExactFormat_crai,
    Vcf = hts::htsExactFormat_vcf,
    Bcf = hts::htsExactFormat_bcf,
    Csi = hts::htsExactFormat_csi,
    Gzi = hts::htsExactFormat_gzi,
    Tbi = hts::htsExactFormat_tbi,
    Bed = hts::htsExactFormat_bed,
    Htsget = hts::htsExactFormat_htsget,
    //Json = hts::htsExactFormat_json,
    Empty = hts::htsExactFormat_empty_format,
    Fasta = hts::htsExactFormat_fasta_format,
    Fastq = hts::htsExactFormat_fastq_format,
    Fai = hts::htsExactFormat_fai_format,
    Fqi = hts::htsExactFormat_fqi_format,
    HtsCrypt4gh = hts::htsExactFormat_hts_crypt4gh_format,
    D4 = hts::htsExactFormat_d4_format,
}

impl From<Format> for hts::htsExactFormat {
    fn from(format: Format) -> Self {
        format as hts::htsExactFormat
    }
}

impl TryFrom<hts::htsExactFormat> for Format {
    type Error = FormatConversionError;

    fn try_from(format: hts::htsExactFormat) -> Result<Self, Self::Error> {
        // Safety: Format is #[repr(u32)] and contains all valid htsExactFormat values
        let result = unsafe { std::mem::transmute::<hts::htsExactFormat, Format>(format) };
        if matches!(
            result,
            Format::Unknown
                | Format::Binary
                | Format::Text
                | Format::Sam
                | Format::Bam
                | Format::Bai
                | Format::Cram
                | Format::Crai
                | Format::Vcf
                | Format::Bcf
                | Format::Csi
                | Format::Gzi
                | Format::Tbi
                | Format::Bed
                | Format::Htsget
                //| Format::Json
                | Format::Empty
                | Format::Fasta
                | Format::Fastq
                | Format::Fai
                | Format::Fqi
                | Format::HtsCrypt4gh
                | Format::D4
        ) {
            Ok(result)
        } else {
            Err(FormatConversionError::UnsupportedFormat(format))
        }
    }
}

impl From<FormatConversionError> for std::io::Error {
    fn from(error: FormatConversionError) -> Self {
        match error {
            FormatConversionError::HtslibError(msg) => {
                std::io::Error::new(std::io::ErrorKind::Other, msg)
            }
            FormatConversionError::UnsupportedFormat(fmt) => std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                format!("Unsupported format: {:?}", fmt),
            ),
            FormatConversionError::IoError(e) => e,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u32)]
pub enum Compression {
    None = hts::htsCompression_no_compression,
    Gzip = hts::htsCompression_gzip,
    Bgzf = hts::htsCompression_bgzf,
    Custom = hts::htsCompression_custom,
    Bzip2 = hts::htsCompression_bzip2_compression,
    Razf = hts::htsCompression_razf_compression,
    Xz = hts::htsCompression_xz_compression,
    Zstd = hts::htsCompression_zstd_compression,
    Maximum = hts::htsCompression_compression_maximum,
}

impl From<Compression> for hts::htsCompression {
    fn from(compression: Compression) -> Self {
        compression as hts::htsCompression
    }
}

impl TryFrom<hts::htsCompression> for Compression {
    type Error = FormatConversionError;

    fn try_from(compression: hts::htsCompression) -> Result<Self, Self::Error> {
        // Safety: Compression is #[repr(u32)] and contains all valid htsCompression values
        let result =
            unsafe { std::mem::transmute::<hts::htsCompression, Compression>(compression) };
        if matches!(
            result,
            Compression::None
                | Compression::Gzip
                | Compression::Bgzf
                | Compression::Custom
                | Compression::Bzip2
                | Compression::Razf
                | Compression::Xz
                | Compression::Zstd
                | Compression::Maximum
        ) {
            Ok(result)
        } else {
            Err(FormatConversionError::UnsupportedFormat(
                compression as hts::htsExactFormat,
            ))
        }
    }
}
