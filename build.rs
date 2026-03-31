fn main() {
    // Compile the C trampoline for setjmp/longjmp error handling.
    // This is compiled into the miniR binary so package .so files
    // can resolve Rf_error, _minir_call_protected, etc. at load time.
    #[cfg(feature = "native")]
    {
        cc::Build::new()
            .file("csrc/native_trampoline.c")
            .warnings(false)
            .debug(true)
            .flag("-fno-omit-frame-pointer")
            .cargo_metadata(true) // need rerun-if-changed for build.rs
            .compile("native_trampoline");

        // Export symbols so dlopen'd .so files can resolve them.
        #[cfg(target_os = "linux")]
        println!("cargo:rustc-link-arg=-Wl,--export-dynamic");

        // On macOS, force export of all symbols from the binary.
        // Without this, dlopen'd libraries can't resolve our extern "C" functions.
        #[cfg(target_os = "macos")]
        println!("cargo:rustc-link-arg=-Wl,-export_dynamic");
    }
}
