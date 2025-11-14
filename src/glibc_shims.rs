#![allow(dead_code)]

use libc::{c_char, c_int, dev_t, mode_t};

#[inline(always)]
unsafe fn read_dev(dev: *const dev_t) -> dev_t {
    if dev.is_null() {
        0
    } else {
        *dev
    }
}

#[no_mangle]
pub unsafe extern "C" fn __xmknod(
    _ver: c_int,
    path: *const c_char,
    mode: mode_t,
    dev: *const dev_t,
) -> c_int {
    libc::mknod(path, mode, read_dev(dev))
}

#[no_mangle]
pub unsafe extern "C" fn __xmknodat(
    _ver: c_int,
    dirfd: c_int,
    path: *const c_char,
    mode: mode_t,
    dev: *const dev_t,
) -> c_int {
    libc::mknodat(dirfd, path, mode, read_dev(dev))
}
