use std::fmt;
use std::io;
use std::path::Path;
pub(crate) use xvcf::rust_htslib::htslib as hts;

pub struct HtsFile {
    fh: hts::htsFile,
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
        let descp = unsafe { hts::hts_format_description(&self.fmt) };
        if descp.is_null() {
            write!(f, "unknown-format")
        } else {
            let descp = unsafe { std::ffi::CStr::from_ptr(descp).to_str().unwrap() };
            // split on space and take the first element
            let descp = descp.split(' ').next().unwrap();
            write!(f, "{}", descp)
        }
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
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_open() {
        let path = Path::new("tests/test.bam");
        let mode = "r";
        let result = open(&path, mode);
        assert!(result.is_ok(), "Failed to open file: {:?}", result.err());
        let file = result.unwrap();
        eprintln!("{:?}", file);
    }

    #[test]
    fn test_repr() {
        let path = Path::new("tests/test.bam");
        let mode = "r";
        let result = open(&path, mode);
        let file = result.expect("Failed to open file");
        assert_eq!(format!("{:?}", file), r#"HtsFile("tests/test.bam", "BAM")"#);
    }
}
