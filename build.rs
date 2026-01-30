fn main() {
    // Use pyo3-build-config to get the Python interpreter's configuration,
    // then emit an rpath so the test/binary can find libpython at runtime on macOS.
    let config = pyo3_build_config::get();
    if let Some(lib_dir) = &config.lib_dir {
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_dir);
    }
}
