use flate2::bufread::GzDecoder;
use std::io;
use std::io::Read;

#[derive(Debug)]
pub enum FileType {
    Bed,
    Vcf,
}

#[derive(Debug)]
pub enum Compression {
    GZ,
    BGZF,
    RAZF,
    None,
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
                return Err(e.into());
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
    let ft = if buf.starts_with(b"##fileformat=VCFv4") {
        FileType::Vcf
    } else {
        FileType::Bed
    };

    Ok((ft, c))
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{write::GzEncoder, GzBuilder};
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

    /*
    #[test]
    fn test_bgzip_bed() {
        let data = b"chr1\t1000\t2000\nchr2\t3000\t4000";
        let mut encoder = BgzfEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(data).unwrap();
        let compressed_data = encoder.finish().unwrap();

        let mut cursor = Cursor::new(compressed_data);
        let (ft, c) = sniff(&mut cursor).unwrap();
        assert!(matches!(ft, FileType::Bed));
        assert!(matches!(c, Compression::BGZF));
        let mut s = String::new();
        cursor.read_to_string(&mut s).unwrap();
        assert_eq!(s, "chr1\t1000\t2000\nchr2\t3000\t4000");
    }

    #[test]
    fn test_bgzip_vcf() {
        let data = b"##fileformat=VCFv4.3\n#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO\nchr1\t1000\t.\tA\tT\t.\t.\t.";
        let mut encoder = BgzfEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(data).unwrap();
        let compressed_data = encoder.finish().unwrap();

        let mut cursor = Cursor::new(compressed_data);
        let (ft, c) = sniff(&mut cursor).unwrap();
        assert!(matches!(ft, FileType::Vcf));
        assert!(matches!(c, Compression::BGZF));
        let mut s = String::new();
        cursor.read_to_string(&mut s).unwrap();
        assert_eq!(s, "##fileformat=VCFv4.3\n#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO\nchr1\t1000\t.\tA\tT\t.\t.\t.");
    }
    */
}
