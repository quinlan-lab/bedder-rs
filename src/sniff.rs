use std::io;
use std::path::Path;
pub(crate) use xvcf::rust_htslib::htslib as hts;

pub fn open_file(path: &Path, mode: &str) -> io::Result<hts::hFILE> {
    let cstr = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
    let mode_cstr = std::ffi::CString::new(mode).unwrap();
    let fh = unsafe { hts::hopen(cstr.as_ptr(), mode_cstr.as_ptr()) };
    let fh = unsafe { *fh };
    Ok(fh)
}

pub fn detect_file_format(hf: hts::hFILE) -> io::Result<hts::htsFormat> {
    let fmt: *mut hts::htsFormat = std::ptr::null_mut();
    let ret = unsafe { hts::hts_detect_format(&hf as *const _ as *mut _, fmt) };
    if ret != 0 {
        return Err(io::Error::last_os_error());
    }
    let fmt = unsafe { *fmt };
    Ok(fmt)
}
