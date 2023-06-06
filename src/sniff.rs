use flate2::read::GzDecoder;
use std::io::{BufRead, Read};
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, PartialEq)]
pub enum FileFormat {
    VCF,
    BCF,
    BAM,
    CRAM,
    SAM,
    BED,
    CSI,
    Unknown,
}

#[derive(Debug, PartialEq)]
pub enum Compression {
    None,
    GZ,
    BGZF,
    RAZF,
}

pub(crate) fn detect_file_format<R: BufRead, S: AsRef<Path>>(
    reader: &mut R,
    path: S,
) -> std::io::Result<(FileFormat, Compression)> {
    let buf = reader.fill_buf()?;
    let mut dec_buf = vec![0; buf.len()];

    let is_gzipped = &buf[0..2] == b"\x1f\x8b";
    let (compression, dec_buf) = if is_gzipped && buf[3] & 4 != 0 && buf.len() >= 18 {
        let c = match &buf[12..16] {
            // BGZF magic number
            b"BC\x02\x00" => Compression::BGZF,
            // RAZF magic number
            b"RAZF" => Compression::RAZF,
            _ => Compression::GZ,
        };

        let mut gz = GzDecoder::new(buf);
        gz.read_exact(&mut dec_buf)?;
        (c, dec_buf.as_slice())
    } else {
        (Compression::None, buf)
    };

    let format = if &dec_buf[0..4] == b"BAM\x01" {
        FileFormat::BAM
    } else if &dec_buf[0..3] == b"BCF" && (dec_buf[3] == 0x2 || dec_buf[3] == 0x4) {
        FileFormat::BCF
    } else if &dec_buf[0..16] == b"##fileformat=VCF" {
        FileFormat::VCF
    } else if &dec_buf[0..4] == b"CRAM" {
        FileFormat::CRAM
    } else if &dec_buf[0..3] == b"@HD\t"
        || &dec_buf[0..3] == b"@SQ\t"
        || &dec_buf[0..3] == b"@RG\t"
        || &dec_buf[0..3] == b"@PG\t"
        || &dec_buf[0..3] == b"@CO\t"
    {
        FileFormat::SAM
    } else {
        let p = path.as_ref();
        if p.ends_with(".bed") || p.ends_with(".bed.gz") || p.ends_with(".bed.bgz") {
            FileFormat::BED
        } else {
            if compression == Compression::BGZF {
                for ext in ["csi", "tbi"] {
                    let mut c: PathBuf = p.to_path_buf();
                    c.push(".");
                    c.push(ext);
                    if c.exists() {
                        return Ok((FileFormat::CSI, compression));
                    }

                }
                FileFormat::CSI
            } else {
                FileFormat::Unknown
            }
        }
    };

    Ok((format, compression))
}
