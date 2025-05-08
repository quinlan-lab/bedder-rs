use crate::bedder_bed::BedderBed;
use crate::bedder_vcf::BedderVCF;
use crate::position::PositionedIterator;
use flate2::bufread::GzDecoder;
use log::info;
use std::io::{self, Read};
use std::path::Path;

#[derive(Debug)]
pub enum FileType {
    Bed,
    Vcf,
    Bcf,
}

#[derive(Debug)]
pub enum Compression {
    GZ,
    BGZF,
    RAZF,
    None,
}

pub enum BedderReader<R>
where
    R: io::BufRead + io::Seek + 'static,
{
    BedderBed(BedderBed<'static, R>),
    BedderVcf(BedderVCF),
}

impl<R> BedderReader<R>
where
    R: io::BufRead + io::Seek + 'static,
{
    pub fn new<P: AsRef<Path>>(reader: R, path: P) -> io::Result<Self> {
        open(reader, path)
    }

    pub fn into_positioned_iterator(self) -> Box<dyn PositionedIterator> {
        match self {
            BedderReader::BedderBed(rdr) => Box::new(rdr),
            BedderReader::BedderVcf(rdr) => Box::new(rdr),
        }
    }
}
// TODO: https://github.com/quinlan-lab/bedder-rs/blob/ffddd2b3a2075594a5375fb81b8672f4f5039acf/src/sniff.rs
pub fn open<P: AsRef<Path>, R: io::BufRead + io::Seek + 'static>(
    mut reader: R,
    p: P,
) -> io::Result<BedderReader<R>> {
    let (ft, c) =
        sniff(&mut reader).map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
    info!("sniffed file type: {:?}, compression: {:?}", ft, c);
    let rdr = match ft {
        FileType::Bed => BedderReader::BedderBed(BedderBed::new(reader, Some(p))),
        FileType::Vcf => {
            BedderReader::BedderVcf(BedderVCF::from_path(p.as_ref().to_str().unwrap())?)
        }
        _ => unimplemented!("Unsupported file type {:?}", ft),
    };
    Ok(rdr)
}

pub fn sniff<R: io::BufRead>(
    rdr: &mut R,
) -> Result<(FileType, Compression), Box<dyn std::error::Error>> {
    let buf = rdr.fill_buf()?;
    let mut dec_buf = vec![0u8; buf.len()];

    let is_gzipped = &buf[0..2] == b"\x1f\x8b";
    let mut c = Compression::None;

    if is_gzipped && buf[3] & 4 != 0 && buf.len() >= 18 {
        c = match &buf[12..16] {
            b"BC\x02\x00" => Compression::BGZF,
            b"RAZF" => Compression::RAZF,
            _ => Compression::GZ,
        };
        let mut gz = GzDecoder::new(buf);
        let res = gz.read_exact(&mut dec_buf);
        if matches!(c, Compression::BGZF) {
            if let Err(e) = res {
                if e.kind() != io::ErrorKind::UnexpectedEof {
                    return Err(e.into());
                }
            }
        }
    } else if is_gzipped {
        c = Compression::GZ;
        let mut gz = GzDecoder::new(buf);
        _ = gz.read_exact(&mut dec_buf);
    }
    let buf = match c {
        Compression::None => {
            buf
            // read buf to get first bytes
        }
        _ => &dec_buf,
    };
    // now we guess filel type based on whats in buf
    let ft = if buf.starts_with(b"##fileformat=VCF") {
        FileType::Vcf
    } else if buf.starts_with(b"BCF") && (buf[3] == 0x2 || buf[3] == 0x4) {
        FileType::Bcf
    } else {
        FileType::Bed
    };

    Ok((ft, c))
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{write::GzEncoder, GzBuilder};
    use noodles::bgzf;
    use std::io::Cursor;
    use std::io::Write;

    #[test]
    fn test_uncompressed_bed() {
        let data = b"chr1\t1000\t2000\nchr2\t3000\t4000";
        let mut cursor = Cursor::new(data);
        let (ft, c) = sniff(&mut cursor).unwrap();
        assert!(matches!(ft, FileType::Bed));
        assert!(matches!(c, Compression::None));
        let mut s = String::new();
        cursor.read_to_string(&mut s).unwrap();
        assert_eq!(s, "chr1\t1000\t2000\nchr2\t3000\t4000");
    }

    #[test]
    fn test_uncompressed_vcf() {
        let data = b"##fileformat=VCFv4.3\n#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO\nchr1\t1000\t.\tA\tT\t.\t.\t.";
        let mut cursor = Cursor::new(data);
        let (ft, c) = sniff(&mut cursor).unwrap();
        assert!(matches!(ft, FileType::Vcf));
        assert!(matches!(c, Compression::None));
        let mut s = String::new();
        cursor.read_to_string(&mut s).unwrap();
        assert_eq!(s, "##fileformat=VCFv4.3\n#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO\nchr1\t1000\t.\tA\tT\t.\t.\t.");
    }

    #[test]
    fn test_gzip_bed() {
        let data = b"chr1\t1000\t2000\nchr2\t3000\t4000";
        let mut buf = Vec::new();
        let mut gz = GzBuilder::new().write(&mut buf, flate2::Compression::default());
        gz.write_all(data).unwrap();
        let compressed_data = gz.finish().unwrap();

        let mut cursor = Cursor::new(compressed_data);
        let (ft, c) = sniff(&mut cursor).unwrap();
        assert!(matches!(ft, FileType::Bed));
        assert!(matches!(c, Compression::GZ));
    }

    #[test]
    fn test_gzip_vcf() {
        let data = b"##fileformat=VCFv4.3\n#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO\nchr1\t1000\t.\tA\tT\t.\t.\t.";
        let mut encoder = GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(data).unwrap();
        let compressed_data = encoder.finish().unwrap();

        let mut cursor = Cursor::new(compressed_data);
        let (ft, c) = sniff(&mut cursor).unwrap();
        assert!(matches!(c, Compression::GZ));
        assert!(matches!(ft, FileType::Vcf));
    }

    #[test]
    fn test_bgzip_bed() {
        let data = b"chr1\t1000\t2000\nchr2\t3000\t4000";

        let mut buf = Vec::new();
        let mut encoder = bgzf::Writer::new(&mut buf);
        encoder.write_all(data).unwrap();
        let res = encoder.finish().unwrap();

        let mut cursor = Cursor::new(&res);
        let (ft, c) = sniff(&mut cursor).unwrap();
        assert!(matches!(ft, FileType::Bed));
        assert!(matches!(c, Compression::BGZF));

        let mut cursor = Cursor::new(&res);
        let mut rdr = bgzf::Reader::new(&mut cursor);
        let mut s = String::new();
        rdr.read_to_string(&mut s).unwrap();
        assert_eq!(s, "chr1\t1000\t2000\nchr2\t3000\t4000");
    }

    #[test]
    fn test_bgzip_vcf() {
        let data = b"##fileformat=VCFv4.3\n#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO\nchr1\t1000\t.\tA\tT\t.\t.\t.";
        let mut encoder = bgzf::Writer::new(Vec::new());
        encoder.write_all(data).unwrap();
        let compressed_data = encoder.finish().unwrap();

        let mut cursor = Cursor::new(&compressed_data);
        let (ft, c) = sniff(&mut cursor).unwrap();
        assert!(matches!(ft, FileType::Vcf));
        assert!(matches!(c, Compression::BGZF));

        let mut cursor = Cursor::new(&compressed_data);
        let mut rdr = bgzf::Reader::new(&mut cursor);
        let mut s = String::new();
        rdr.read_to_string(&mut s).unwrap();
        assert_eq!(s, "##fileformat=VCFv4.3\n#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO\nchr1\t1000\t.\tA\tT\t.\t.\t.");
    }
}
