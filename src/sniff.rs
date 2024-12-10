use bio::io::bed;
use rust_htslib::bcf;
pub(crate) use rust_htslib::htslib as hts;
use std::fmt;
use std::io;
use std::mem;
use std::path::Path;
use std::rc::Rc;

use crate::bedder_vcf::BedderVCF;
use crate::position;

pub struct HtsFile {
    fh: hts::htsFile,
    kstr: hts::kstring_t,
    pos: usize,
}

impl io::Read for HtsFile {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.pos < self.kstr.l {
            let slice =
                unsafe { std::slice::from_raw_parts(self.kstr.s as *const u8, self.kstr.l) };
            let remaining = &slice[self.pos..];
            let mut copy_len = buf.len().min(remaining.len());
            buf[..copy_len].copy_from_slice(&remaining[..copy_len]);
            // if we read a full line, we must add '\n' to the buffer
            // we know we read a full line if we used all of the kstr
            self.pos += copy_len;
            if copy_len == remaining.len() {
                if copy_len < buf.len() {
                    buf[copy_len] = '\n' as u8;
                    copy_len += 1;
                } else {
                    // we must modify the kstr.s to contain only '\n'
                    unsafe {
                        *(self.kstr.s as *mut u8) = '\n' as u8;
                    }
                    self.kstr.l = 1;
                    self.kstr.m = 1;
                }
            }

            return Ok(copy_len);
        }

        self.pos = 0;
        let n = unsafe { hts::hts_getline(&mut self.fh, '\n' as i32, &mut self.kstr) };
        if n == -1 {
            return Ok(0);
        }
        if n < 0 {
            return Err(io::Error::last_os_error());
        }

        let slice = unsafe { std::slice::from_raw_parts(self.kstr.s as *const u8, self.kstr.l) };
        let mut copy_len = buf.len().min(slice.len());
        buf[..copy_len].copy_from_slice(&slice[..copy_len]);
        // if we read a full line, we must add '\n' to the buffer
        // we know we read a full line if we used all of the kstr
        self.pos += copy_len;
        if copy_len == slice.len() {
            if copy_len < buf.len() {
                buf[copy_len] = '\n' as u8;
                copy_len += 1;
            } else {
                // we must modify the kstr.s to contain only '\n'
                unsafe {
                    *(self.kstr.s as *mut u8) = '\n' as u8;
                }
                self.kstr.l = 1;
                self.kstr.m = 1;
            }
        }
        Ok(copy_len)
    }
}

// use this so we can open a bcf from an htsFile
struct BCFReader {
    _inner: *mut hts::htsFile,
    _header: Rc<bcf::header::HeaderView>,
    _index: Option<bcf::Index>,
    _itr: Option<*mut hts::hts_itr_t>,
    _kstring: hts::kstring_t,
}

// static assert that this Reader is the same as hts::bcf::Reader
const _: () = assert!(mem::size_of::<BCFReader>() == mem::size_of::<bcf::Reader>());

struct BCFIndexedReader {
    /// The synced VCF/BCF reader to use internally.
    inner: *mut hts::bcf_srs_t,
    /// The header.
    header: Rc<bcf::header::HeaderView>,

    /// The position of the previous fetch, if any.
    current_region: Option<(u32, u64, Option<u64>)>,
}
const _: () = assert!(mem::size_of::<BCFIndexedReader>() == mem::size_of::<bcf::IndexedReader>());

impl HtsFile {
    pub fn open_vcf(path: &Path) -> io::Result<Box<dyn position::PositionedIterator>> {
        let mut hf = open(path, "r")?;
        match hf.format()?.format().as_str() {
            "BCF" | "VCF" => {
                let mut vcf_reader = hf.vcf();
                BedderVCF::new(vcf_reader)
                    .map(|b| Box::new(b) as Box<dyn position::PositionedIterator>)
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
            }
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "unsupported format for vcf",
            )),
        }
    }

    pub fn vcf(mut self) -> bcf::Reader {
        let fmt_str = self.format().unwrap().format();
        assert!(
            fmt_str == "BCF" || fmt_str == "VCF",
            "unsupported format for bcf: {}",
            fmt_str
        );
        let hdr = unsafe { hts::bcf_hdr_read(&mut self.fh as *mut _) };
        let b = BCFReader {
            _inner: &mut self.fh as *mut _,
            _header: Rc::new(bcf::header::HeaderView::new(hdr)),
        };
        unsafe { mem::transmute(b) }
    }

    pub fn bed(self) -> bed::Reader<HtsFile> {
        let fmt_str = self.format().unwrap().format();
        assert!(fmt_str == "BED", "unsupported format for bed: {}", fmt_str);
        bed::Reader::new(self)
    }
}

impl fmt::Debug for HtsFile {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let fname = self.fh.fn_;
        let fname = if fname.is_null() {
            ""
        } else {
            unsafe { std::ffi::CStr::from_ptr(fname).to_str().unwrap() }
        };
        let format = self
            .format()
            .map(|f| format!("{}", f))
            .unwrap_or_else(|e| e.to_string());
        write!(f, r#"HtsFile("{}", "{}")"#, fname, format)
    }
}

pub struct HtsFormat {
    fmt: hts::htsFormat,
}

impl fmt::Display for HtsFormat {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let fmt = self.format();
        write!(f, "{}", fmt)
    }
}

impl fmt::Debug for HtsFormat {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let descp = unsafe { hts::hts_format_description(&self.fmt) };
        if descp.is_null() {
            write!(f, "unknown-format")
        } else {
            let descp = unsafe { std::ffi::CStr::from_ptr(descp).to_str().unwrap() };
            write!(f, "{}", descp)
        }
    }
}

impl HtsFormat {
    pub fn format(&self) -> String {
        let desc = unsafe { hts::hts_format_description(&self.fmt) };
        if desc.is_null() {
            "unknown-format".to_string()
        } else {
            let fmt = unsafe { std::ffi::CStr::from_ptr(desc).to_str().unwrap() };
            fmt.split(' ')
                .next()
                .unwrap_or("unknown-format")
                .to_string()
        }
    }
}

impl HtsFile {
    pub fn new(path: &Path, mode: &str) -> io::Result<Self> {
        open(path, mode)
    }

    pub fn format(&self) -> io::Result<HtsFormat> {
        let fmt = unsafe { hts::hts_get_format(&self.fh as *const _ as *mut _) };
        if fmt.is_null() {
            return Err(io::Error::last_os_error());
        }
        Ok(HtsFormat {
            fmt: unsafe { *fmt },
        })
    }
}

pub fn open(path: &Path, mode: &str) -> io::Result<HtsFile> {
    let cstr = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
    let mode_cstr = std::ffi::CString::new(mode).unwrap();
    let hts_file = unsafe { hts::hts_open(cstr.as_ptr(), mode_cstr.as_ptr()) };
    if hts_file.is_null() {
        Err(io::Error::last_os_error())
    } else {
        Ok(HtsFile {
            fh: unsafe { *hts_file },
            kstr: hts::kstring_t {
                s: std::ptr::null_mut(),
                l: 0,
                m: 0,
            },
            pos: 0,
        })
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use std::io::{BufRead, Read};
    use std::path::Path;

    #[test]
    fn test_open() {
        let path = Path::new("tests/test.bam");
        let mode = "r";
        let result = open(&path, mode);
        assert!(result.is_ok(), "Failed to open file: {:?}", result.err());
        let file = result.unwrap();
    }

    #[test]
    fn test_repr() {
        let path = Path::new("tests/test.bam");
        let mode = "r";
        let result = open(&path, mode);
        let file = result.expect("Failed to open file");
        assert_eq!(format!("{:?}", file), r#"HtsFile("tests/test.bam", "BAM")"#);
    }

    #[test]
    fn test_bam_fmt() {
        let path = Path::new("tests/test.bam");
        let mode = "r";
        let result = open(&path, mode);
        let file = result.expect("Failed to open file");
        assert_eq!(file.format().unwrap().format(), "BAM");
    }

    #[test]
    fn test_read_small_chunks() {
        let path = Path::new("tests/test.bam");
        let mut file = open(&path, "r").unwrap();
        let mut buf = [0u8; 4]; // Small buffer to force multiple reads
        let mut result = Vec::new();

        loop {
            match file.read(&mut buf) {
                Ok(0) => break, // EOF
                Ok(n) => result.extend_from_slice(&buf[..n]),
                Err(e) => panic!("Read error: {}", e),
            }
        }

        assert!(!result.is_empty(), "Should have read some data");
        assert!(result.len() > 4, "Should have read multiple chunks");
    }

    #[test]
    fn test_read_exact() {
        let path = Path::new("tests/test.bam");
        let mut file = open(&path, "r").unwrap();
        let mut buf = [0u8; 4];

        // Should be able to read exactly 4 bytes
        file.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"BAM\x01", "First 4 bytes should be BAM header");
    }

    #[test]
    fn test_read_bed_file() {
        let path = Path::new("tests/test.bed");
        let mut file = open(&path, "r").unwrap();
        let mut content = String::new();
        file.read_to_string(&mut content).unwrap();

        // Verify content
        assert!(content.starts_with("chr1\t1\t21\tAAAAA\n"));

        // Split into lines and verify we got all 7 entries
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 7, "Should have 7 lines");

        // Verify last line
        assert_eq!(lines.last().unwrap(), &"chr1\t7\t27\tGGGGG");
    }

    #[test]
    fn test_read_bed_chunks() {
        let path = Path::new("tests/test.bed");
        let mut file = open(&path, "r").unwrap();
        let mut buf = [0u8; 10]; // Small buffer to test chunked reading

        // Read first chunk
        let n = file.read(&mut buf).unwrap();
        assert_eq!(n, 10);
        assert_eq!(&buf, b"chr1\t1\t21\t");

        // Read next chunk
        let n = file.read(&mut buf).unwrap();
        assert_eq!(n, 6);
        assert_eq!(&buf[..n], b"AAAAA\n");

        // read then next line
        let n = file.read(&mut buf).unwrap();
        assert_eq!(n, 10);
        assert_eq!(&buf[..n], b"chr1\t2\t22\t");

        // Read next chunk
        let n = file.read(&mut buf).unwrap();
        assert_eq!(n, 6);
        assert_eq!(&buf[..n], b"BBBBB\n");
    }

    #[test]
    fn test_read_bed_large_buffer() {
        let path = Path::new("tests/test.bed");
        let mut file = io::BufReader::new(open(&path, "r").unwrap());
        let mut buf = [0u8; 100]; // Buffer larger than any line (lines are ~20 bytes)

        // Read first line
        let n = file.read(&mut buf).unwrap();
        assert_eq!(n, 16); // "chr1\t1\t21\tAAAAA\n" is 21 bytes
        assert_eq!(&buf[..n], b"chr1\t1\t21\tAAAAA\n");

        // Read second line
        let n = file.read(&mut buf).unwrap();
        assert_eq!(n, 16); // "chr1\t2\t22\tBBBBB\n" is 21 bytes
        assert_eq!(&buf[..n], b"chr1\t2\t22\tBBBBB\n");

        // Verify we can read all lines
        let mut line_count = 2; // We already read 2 lines
        let mut buf = String::new();
        while file.read_line(&mut buf).unwrap() > 0 {
            eprint!("line {}", buf);
            line_count += 1;
            if line_count < 8 {
                buf.clear();
            }
        }
        // last line should end with \n
        assert!(buf.ends_with("Z\n"), "Last line should end with Z\\n");
        assert_eq!(line_count, 8, "Should have read all 8 lines");
    }

    #[test]
    fn test_read_single_chars() {
        use std::io::Write;
        // Create a temporary file with 10 single-character lines
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        let mut f = temp_file.as_file();
        for c in ('A'..='J').into_iter() {
            writeln!(f, "{}", c).unwrap();
        }
        f.flush().unwrap();

        // Read the file using HtsFile
        let mut file = open(temp_file.path(), "r").unwrap();
        let mut content = String::new();
        file.read_to_string(&mut content).unwrap();

        // Verify content
        assert_eq!(content, "A\nB\nC\nD\nE\nF\nG\nH\nI\nJ\n");

        // Test reading in small chunks
        let mut file = open(temp_file.path(), "r").unwrap();
        let mut buf = [0u8; 2];
        let mut result = Vec::new();

        loop {
            match file.read(&mut buf) {
                Ok(0) => break, // EOF
                Ok(2) => result.extend_from_slice(&buf[..2]),
                Ok(_) => {
                    panic!("Read more than 2 bytes but only expecting the character and newline")
                }
                Err(e) => panic!("Read error: {}", e),
            }
        }

        assert_eq!(
            String::from_utf8(result).unwrap(),
            "A\nB\nC\nD\nE\nF\nG\nH\nI\nJ\n"
        );
    }
}
