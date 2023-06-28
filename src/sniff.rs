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
    let (format, compression) = detect_file_format(&mut reader, &path)?;
    log::info!(
        "path: {:?}, format: {:?} compression: {:?}",
        path.as_ref(),
        format,
        compression
    );
    let br: Box<dyn BufRead> = match compression {
        Compression::None => Box::new(reader),
        Compression::GZ => Box::new(std::io::BufReader::new(GzDecoder::new(reader))),
        Compression::BGZF => match format {
            // BCF|BAM will appear as bgzf so we don't want to do this outside
            FileFormat::BCF | FileFormat::BAM => Box::new(reader),
            _ => Box::new(bgzf::Reader::new(reader)),
        },
        Compression::RAZF => unimplemented!(),
    };
    match format {
        FileFormat::VCF => {
            let mut vcf = vcf::reader::Builder.build_from_reader(br)?;
            let hdr = vcf.read_header()?;
            let bed_vcf = BedderVCF::new(Box::new(vcf), hdr)?;
            Ok(Box::new(bed_vcf))
        }
        FileFormat::BCF => {
            let mut bcf = noodles::bcf::Reader::new(br);
            let hdr = bcf.read_header()?;
            let bed_vcf = BedderVCF::new(Box::new(bcf), hdr)?;
            Ok(Box::new(bed_vcf))
        }

        FileFormat::BED => {
            let reader = BedderBed::new(br);
            Ok(Box::new(reader))
        }
        _ => unimplemented!("{format:?} not yet supported"),
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

    let format = if dec_buf.starts_with(b"BAM\x01") {
        FileFormat::BAM
    } else if &dec_buf[0..3] == b"BCF" && (dec_buf[3] == 0x2 || dec_buf[3] == 0x4) {
        FileFormat::BCF
    } else if dec_buf.starts_with(b"##fileformat=VCF") {
        FileFormat::VCF
    } else if dec_buf.starts_with(b"CRAM") {
        FileFormat::CRAM
    } else if dec_buf.len() > 3
        && (&dec_buf[0..4] == b"@HD\t"
            || &dec_buf[0..4] == b"@SQ\t"
            || &dec_buf[0..4] == b"@RG\t"
            || &dec_buf[0..4] == b"@PG\t"
            || &dec_buf[0..4] == b"@CO\t")
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

    if matches!(format, FileFormat::Unknown) {
        let s = String::from_utf8_lossy(dec_buf);
        let mut lines = s
            .lines()
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .collect::<Vec<_>>();
        if lines
            .last()
            .map(|l| !l.ends_with('\n') && l.split('\t').collect::<Vec<_>>().len() < 3)
            .unwrap_or(false)
        {
            // drop the final incomplete line
            lines.pop();
        }

        if !lines.is_empty() && lines.iter().all(|&line| is_bed_line(line)) {
            return Ok((FileFormat::BED, compression));
        }
    }

    Ok((format, compression))
}

fn is_bed_line(s: &str) -> bool {
    if s.starts_with('#') {
        return true;
    }
    let cols: Vec<_> = s.split('\t').collect();
    if cols.len() < 3 {
        return false;
    }
    // check that 2nd and 3rd cols are integers
    cols[1].parse::<i32>().is_ok() && cols[2].parse::<i32>().is_ok()
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

    #[test]
    fn test_is_bed_line() {
        // Test valid BED line
        let valid_bed_line = "chr1\t100\t200\tname\t0\t+\t50\t150\t0\t2\t10,20\t0,80";
        assert!(is_bed_line(valid_bed_line));

        // Test invalid BED line with missing columns
        let invalid_bed_line = "chr1\t100";
        assert!(!is_bed_line(invalid_bed_line));

        // Test invalid BED line with non-integer columns
        let invalid_bed_line = "chr1\ta\tb\tname\t0\t+\t50\t150\t0\t2\t10,20\t0,80";
        assert!(!is_bed_line(invalid_bed_line));

        // Test comment line
        let comment_line = "# This is a comment";
        assert!(is_bed_line(comment_line));

        // single interval with no newline.
        let valid_bed_line = "chr1\t100\t200";
        assert!(is_bed_line(valid_bed_line));
    }
}
