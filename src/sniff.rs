use flate2::read::GzDecoder;
use std::io::{BufRead, Read};
use std::path::Path;

use crate::bedder_bed::BedderBed;
use crate::bedder_vcf::BedderVCF;
use crate::position::{Positioned, PositionedIterator};
use noodles::bgzf;
use noodles::vcf;

/// File formats supported by this file detector.
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

/// Possible Compression formats.
#[derive(Debug, PartialEq)]
pub enum Compression {
    None,
    GZ,
    BGZF,
    RAZF,
}

pub fn open_file<P>(
    path: P,
) -> std::io::Result<Box<dyn PositionedIterator<Item = Box<dyn Positioned>>>>
where
    P: AsRef<Path>,
{
    let file = std::fs::File::open(&path)?;
    let r = open_reader(file, path);
    r
}

pub fn open_reader<R, P>(
    reader: R,
    path: P,
) -> std::io::Result<Box<dyn PositionedIterator<Item = Box<dyn Positioned>>>>
where
    R: Read + 'static,
    P: AsRef<Path>,
{
    let mut reader = std::io::BufReader::new(reader);
    let (format, compression) = detect_file_format(&mut reader, path)?;
    match format {
        FileFormat::VCF => {
            let br = Box::new(reader);
            let mut vcf = vcf::reader::Builder::default()
                //.set_index(index)
                .build_from_reader(br)?;
            let hdr = vcf.read_header()?;
            let bed_vcf = BedderVCF::new(Box::new(vcf), hdr)?;
            Ok(Box::new(bed_vcf))
        }
        FileFormat::BED => {
            let reader: Box<dyn BufRead> = match compression {
                Compression::None => Box::new(reader),
                Compression::GZ => Box::new(std::io::BufReader::new(GzDecoder::new(reader))),
                Compression::BGZF => Box::new(std::io::BufReader::new(bgzf::Reader::new(reader))),
                Compression::RAZF => unimplemented!(),
            };
            let reader = BedderBed::new(reader);
            Ok(Box::new(reader))
        }
        _ => unimplemented!(),
    }
}

/// detect the file format of a reader.
pub fn detect_file_format<R: BufRead, P: AsRef<Path>>(
    reader: &mut R,
    path: P,
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
        // it's ok if we have an unexepected EOF here
        match gz.read_exact(&mut dec_buf) {
            Ok(_) => {}
            Err(e) => {
                if e.kind() != std::io::ErrorKind::UnexpectedEof {
                    return Err(e);
                }
            }
        }
        (c, dec_buf.as_slice())
    } else {
        (
            if is_gzipped {
                Compression::GZ
            } else {
                Compression::None
            },
            buf,
        )
    };

    let format = if &dec_buf[0..4] == b"BAM\x01" {
        FileFormat::BAM
    } else if &dec_buf[0..3] == b"BCF" && (dec_buf[3] == 0x2 || dec_buf[3] == 0x4) {
        FileFormat::BCF
    } else if &dec_buf[0..16] == b"##fileformat=VCF" {
        FileFormat::VCF
    } else if &dec_buf[0..4] == b"CRAM" {
        FileFormat::CRAM
    } else if &dec_buf[0..4] == b"@HD\t"
        || &dec_buf[0..4] == b"@SQ\t"
        || &dec_buf[0..4] == b"@RG\t"
        || &dec_buf[0..4] == b"@PG\t"
        || &dec_buf[0..4] == b"@CO\t"
    {
        FileFormat::SAM
    } else {
        let p = path.as_ref();
        if p.ends_with(".bed") || p.ends_with(".bed.gz") || p.ends_with(".bed.bgz") {
            FileFormat::BED
        } else {
            FileFormat::Unknown
        }
    };

    Ok((format, compression))
}

#[cfg(test)]
mod tests {

    use super::*;
    use noodles::bam;
    use noodles::sam;

    #[test]
    fn test_detect_format_bam() {
        let file_path = "tests/test.bam";
        let mut fs = std::fs::File::open(file_path).unwrap();
        let mut rdr = std::io::BufReader::new(&mut fs);
        let (format, compression) = detect_file_format(&mut rdr, file_path).unwrap();
        assert_eq!(compression, Compression::BGZF);
        assert_eq!(format, FileFormat::BAM);

        let mut b = bam::reader::Reader::new(&mut rdr);
        let h = b.read_header().expect("eror reading header");
        for r in b.records(&h) {
            let r = r.expect("error reading record");
            eprintln!("{:?}", r);
        }
    }

    #[test]
    fn test_detect_format_sam() {
        let file_path = "tests/test.sam";
        let mut fs = std::fs::File::open(file_path).unwrap();
        let mut rdr = std::io::BufReader::new(&mut fs);
        let (format, compression) = detect_file_format(&mut rdr, file_path).unwrap();
        assert_eq!(compression, Compression::None);
        assert_eq!(format, FileFormat::SAM);

        let mut b = sam::reader::Reader::new(&mut rdr);
        let h = b.read_header().expect("eror reading header");
        for r in b.records(&h) {
            let r = r.expect("error reading record");
            eprintln!("{:?}", r);
        }
    }
}
