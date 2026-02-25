fn main() {
    println!("cargo:rerun-if-env-changed=BEDDER_ALLOW_BUILD_PYTHON_RPATH");

    // Use pyo3-build-config to get the Python interpreter's configuration,
    // optionally emit an rpath so local test/binaries can find libpython at runtime.
    //
    // By default this is enabled for non-release profiles to keep local
    // dev/tests working, and disabled for release builds to avoid shipping
    // builder-specific absolute paths.
    let allow_build_python_rpath_env = std::env::var_os("BEDDER_ALLOW_BUILD_PYTHON_RPATH")
        .and_then(|v| v.into_string().ok())
        .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false);
    let profile = std::env::var("PROFILE").unwrap_or_default();
    let allow_build_python_rpath = allow_build_python_rpath_env || profile != "release";

    if allow_build_python_rpath {
        let config = pyo3_build_config::get();
        if let Some(lib_dir) = &config.lib_dir {
            println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_dir);
        }
    }
}
