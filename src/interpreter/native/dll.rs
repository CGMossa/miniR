//! Dynamic library loading — dyn.load(), dyn.unload(), symbol lookup.
//!
//! Uses `libloading` to load shared libraries (.so on Linux, .dylib on macOS)
//! and resolve function symbols for `.Call()` dispatch.
//!
//! The actual native function call goes through a C trampoline
//! (`_minir_call_protected` in `minir_runtime.c`) which sets up `setjmp`
//! so that `Rf_error()` in C code safely longjmps back instead of crashing.
//! The trampoline also handles variable argument counts (up to 16 SEXP args).

use std::collections::HashMap;
use std::ffi::CStr;
use std::os::raw::c_char;
use std::path::{Path, PathBuf};

use libloading::{Library, Symbol};

use super::convert;
use super::sexp::{self, Sexp};
use crate::interpreter::value::*;
use crate::interpreter::Interpreter;

// region: C trampoline types

/// Signature of `R_init_<pkgname>(DllInfo*)` package init function.
type PkgInitFn = unsafe extern "C" fn(*mut u8);

// endregion

// region: LoadedDll

/// A loaded dynamic library and its resolved symbols.
pub struct LoadedDll {
    /// Path to the .so/.dylib file.
    pub path: PathBuf,
    /// Short name (e.g. "myPkg" from "myPkg.so").
    pub name: String,
    /// The underlying library handle.
    lib: Library,
    /// Cached symbol addresses: function name → raw pointer.
    symbols: HashMap<String, *const ()>,
    /// Symbols registered via R_registerRoutines during R_init_<pkg>.
    pub registered_calls: HashMap<String, *const ()>,
}

// Safety: LoadedDll is only used from a single interpreter thread.
// The Library handle and symbol pointers are stable once loaded.
unsafe impl Send for LoadedDll {}

impl LoadedDll {
    /// Load a shared library from the given path, then call R_init_<pkgname>
    /// if it exists (to register routines).
    pub fn load(path: &Path) -> Result<Self, String> {
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        // Safety: loading a shared library can execute arbitrary code (init functions).
        // We trust that package .so files are safe — they were compiled from the
        // package's own C source code.
        let lib = unsafe { Library::new(path) }
            .map_err(|e| format!("dyn.load(\"{}\") failed: {e}", path.display()))?;

        let mut dll = LoadedDll {
            path: path.to_path_buf(),
            name: name.clone(),
            lib,
            symbols: HashMap::new(),
            registered_calls: HashMap::new(),
        };

        // Call R_init_<pkgname> if it exists — this triggers R_registerRoutines
        dll.call_pkg_init(&name);

        Ok(dll)
    }

    /// Call the package init function `R_init_<name>(DllInfo*)` if present.
    fn call_pkg_init(&mut self, pkg_name: &str) {
        let init_name = format!("R_init_{pkg_name}");
        if let Ok(ptr) = self.get_symbol(&init_name) {
            unsafe {
                let init: PkgInitFn = std::mem::transmute(ptr);
                // Pass a null DllInfo* — our runtime ignores it
                init(std::ptr::null_mut());
            }
            // After init, collect registered .Call methods
            self.collect_registered_calls();
        }
    }

    /// Read registered .Call methods from the Rust runtime's registry.
    fn collect_registered_calls(&mut self) {
        // R_registerRoutines (called by R_init_<pkg>) stores registrations
        // in the Rust runtime's shared registry.
        for (name, ptr) in super::runtime::REGISTERED_CALLS
            .lock()
            .expect("lock registered calls")
            .iter()
        {
            self.registered_calls.insert(name.clone(), ptr.0);
        }
    }

    /// Look up a function symbol by name. Checks registered routines first,
    /// then falls back to dynamic symbol lookup. Caches the result.
    pub fn get_symbol(&mut self, name: &str) -> Result<*const (), String> {
        // Check registered .Call methods first
        if let Some(&ptr) = self.registered_calls.get(name) {
            return Ok(ptr);
        }

        // Check cache
        if let Some(&ptr) = self.symbols.get(name) {
            return Ok(ptr);
        }

        let c_name =
            std::ffi::CString::new(name).map_err(|_| format!("invalid symbol name: {name}"))?;

        // Safety: we trust the symbol exists and is a valid function pointer
        let sym: Symbol<*const ()> = unsafe {
            self.lib
                .get(c_name.as_bytes_with_nul())
                .map_err(|e| format!("symbol '{name}' not found in {}: {e}", self.path.display()))?
        };

        let ptr = *sym;
        self.symbols.insert(name.to_string(), ptr);
        Ok(ptr)
    }
}

impl std::fmt::Debug for LoadedDll {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoadedDll")
            .field("name", &self.name)
            .field("path", &self.path)
            .field("symbols", &self.symbols.keys().collect::<Vec<_>>())
            .field(
                "registered_calls",
                &self.registered_calls.keys().collect::<Vec<_>>(),
            )
            .finish()
    }
}

// endregion

// region: Interpreter DLL state

impl Interpreter {
    /// Load a shared library and register it. Returns the DLL name.
    pub(crate) fn dyn_load(&self, path: &Path) -> Result<String, RError> {
        let dll = LoadedDll::load(path).map_err(|e| RError::new(RErrorKind::Other, e))?;
        let name = dll.name.clone();
        self.loaded_dlls.borrow_mut().push(dll);
        Ok(name)
    }

    /// Unload a shared library by name.
    pub(crate) fn dyn_unload(&self, name: &str) -> Result<(), RError> {
        let mut dlls = self.loaded_dlls.borrow_mut();
        let pos = dlls.iter().position(|d| d.name == name);
        match pos {
            Some(i) => {
                dlls.remove(i);
                Ok(())
            }
            None => Err(RError::new(
                RErrorKind::Other,
                format!("shared object '{name}' was not loaded"),
            )),
        }
    }

    /// Check if a symbol is loaded in any DLL.
    pub(crate) fn is_symbol_loaded(&self, name: &str) -> bool {
        let mut dlls = self.loaded_dlls.borrow_mut();
        dlls.iter_mut().any(|dll| dll.get_symbol(name).is_ok())
    }

    /// Look up a symbol across all loaded DLLs. Returns the function pointer
    /// and the index of the DLL that contains it.
    fn find_native_symbol_with_dll(&self, name: &str) -> Result<(*const (), usize), RError> {
        let mut dlls = self.loaded_dlls.borrow_mut();
        for (i, dll) in dlls.iter_mut().enumerate().rev() {
            if let Ok(ptr) = dll.get_symbol(name) {
                return Ok((ptr, i));
            }
        }
        Err(RError::new(
            RErrorKind::Other,
            format!(".Call symbol '{name}' not found in any loaded DLL"),
        ))
    }

    /// Look up a symbol across all loaded DLLs. Returns the function pointer.
    pub(crate) fn find_native_symbol(&self, name: &str) -> Result<*const (), RError> {
        self.find_native_symbol_with_dll(name).map(|(ptr, _)| ptr)
    }

    /// Execute a `.Call()` invocation using the C trampoline for error safety.
    ///
    /// Flow:
    /// 1. Convert RValue args → SEXP (using C allocator)
    /// 2. Look up the native function symbol
    /// 3. Look up the C trampoline `_minir_call_protected` in the same DLL
    /// 4. Call the trampoline (which does setjmp + dispatch)
    /// 5. If the C function called Rf_error(), the trampoline returns 1 and
    ///    we convert the error message to an RError
    /// 6. Convert the result SEXP → RValue
    /// 7. Free all C-side allocations via `_minir_free_allocs`
    /// 8. Free Rust-side input SEXPs
    pub(crate) fn dot_call(&self, symbol_name: &str, args: &[RValue]) -> Result<RValue, RError> {
        let fn_ptr = self.find_native_symbol(symbol_name)?;

        // Convert RValue args to SEXPs (allocated with C allocator)
        let sexp_args: Vec<Sexp> = args.iter().map(convert::rvalue_to_sexp).collect();

        // The trampoline and error accessors are now in the binary
        // (compiled from csrc/native_trampoline.c via build.rs).
        extern "C" {
            fn _minir_call_protected(
                fn_ptr: *const (),
                args: *const Sexp,
                nargs: i32,
                result: *mut Sexp,
            ) -> i32;
            fn _minir_get_error_msg() -> *const c_char;
        }

        let mut result_sexp: Sexp = sexp::R_NIL_VALUE;
        let nargs = i32::try_from(sexp_args.len()).unwrap_or(0);
        let error_code = unsafe {
            _minir_call_protected(
                fn_ptr,
                sexp_args.as_ptr(),
                nargs,
                &mut result_sexp as *mut Sexp,
            )
        };

        // Check if Rf_error was called
        if error_code != 0 {
            let error_msg = unsafe {
                let msg_ptr = _minir_get_error_msg();
                if msg_ptr.is_null() {
                    "unknown error in native code".to_string()
                } else {
                    CStr::from_ptr(msg_ptr)
                        .to_str()
                        .unwrap_or("unknown error")
                        .to_string()
                }
            };

            // Clean up before returning error
            super::runtime::free_allocs();
            unsafe {
                for s in sexp_args {
                    sexp::free_sexp(s);
                }
            }

            return Err(RError::new(RErrorKind::Other, error_msg));
        }

        // Convert result to RValue (copies all data)
        let result = unsafe { convert::sexp_to_rvalue(result_sexp) };

        // Free runtime allocations (result SEXP + any intermediates)
        super::runtime::free_allocs();

        // Free Rust-allocated input SEXPs. External pointers (wrapped as
        // lists with .sexp_ptr attr) pass the raw SEXP directly — skip freeing those.
        unsafe {
            for (s, arg) in sexp_args.into_iter().zip(args.iter()) {
                if let RValue::List(list) = arg {
                    if list
                        .attrs
                        .as_ref()
                        .is_some_and(|a| a.contains_key(".sexp_ptr"))
                    {
                        continue; // external pointer — owned by C
                    }
                }
                sexp::free_sexp(s);
            }
        }

        Ok(result)
    }

    /// Find miniR's `include/` directory containing Rinternals.h.
    ///
    /// Search order:
    /// 1. `MINIR_INCLUDE` environment variable
    /// 2. `<exe_dir>/../include` (installed layout)
    /// 3. `<working_dir>/include` (development layout)
    pub(crate) fn find_include_dir(&self) -> Option<std::path::PathBuf> {
        // Check env var first
        if let Some(dir) = self.get_env_var("MINIR_INCLUDE") {
            let p = std::path::PathBuf::from(dir);
            if p.join("miniR").join("Rinternals.h").is_file() {
                return Some(p);
            }
        }

        // Check relative to executable
        if let Ok(exe) = std::env::current_exe() {
            if let Some(exe_dir) = exe.parent() {
                let p = exe_dir.join("../include");
                if p.join("miniR").join("Rinternals.h").is_file() {
                    return Some(p);
                }
            }
        }

        // Check working directory (development layout)
        let wd = self.get_working_dir();
        let p = wd.join("include");
        if p.join("miniR").join("Rinternals.h").is_file() {
            return Some(p);
        }

        // Check CARGO_MANIFEST_DIR for test/dev builds
        let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let p = manifest.join("include");
        if p.join("miniR").join("Rinternals.h").is_file() {
            return Some(p);
        }

        None
    }

    /// Load native code for a package based on its useDynLib directives.
    ///
    /// For each useDynLib directive in the NAMESPACE:
    /// 1. Look for a pre-compiled .so/.dylib in `<pkg_dir>/libs/`
    /// 2. If not found, compile `<pkg_dir>/src/*.c` on demand
    /// 3. Load the shared library via dyn_load
    pub(crate) fn load_package_native_code(
        &self,
        pkg_name: &str,
        pkg_dir: &std::path::Path,
        dyn_libs: &[crate::interpreter::packages::namespace::DynLibDirective],
    ) -> Result<(), RError> {
        if dyn_libs.is_empty() {
            return Ok(());
        }

        let ext = if cfg!(target_os = "macos") {
            "dylib"
        } else {
            "so"
        };

        for directive in dyn_libs {
            let lib_name = &directive.library;

            // 1. Check for pre-compiled library in libs/
            let precompiled = pkg_dir.join("libs").join(format!("{lib_name}.{ext}"));
            if precompiled.is_file() {
                self.dyn_load(&precompiled)?;
                continue;
            }

            // 2. Compile from src/ on demand
            let src_dir = pkg_dir.join("src");
            if !src_dir.is_dir() {
                tracing::warn!(
                    "useDynLib({lib_name}): no precompiled library and no src/ directory in {}",
                    pkg_dir.display()
                );
                continue;
            }

            let include_dir = self.find_include_dir().ok_or_else(|| {
                RError::other(format!(
                    "cannot compile native code for '{pkg_name}': \
                     miniR include directory not found (set MINIR_INCLUDE env var)"
                ))
            })?;

            // Compile into a temporary output directory under the package
            let output_dir = pkg_dir.join("libs");
            if std::fs::create_dir_all(&output_dir).is_err() {
                // If we can't write to pkg_dir/libs, use temp dir
                let output_dir = self.temp_dir.path().join(format!("native-{pkg_name}"));
                std::fs::create_dir_all(&output_dir)
                    .map_err(|e| RError::other(format!("cannot create output directory: {e}")))?;
                let lib_path =
                    super::compile::compile_package(&src_dir, lib_name, &output_dir, &include_dir)
                        .map_err(|e| {
                            RError::other(format!(
                                "compilation of native code for '{pkg_name}' failed: {e}"
                            ))
                        })?;
                self.dyn_load(&lib_path)?;
                continue;
            }

            let lib_path =
                super::compile::compile_package(&src_dir, lib_name, &output_dir, &include_dir)
                    .map_err(|e| {
                        RError::other(format!(
                            "compilation of native code for '{pkg_name}' failed: {e}"
                        ))
                    })?;
            self.dyn_load(&lib_path)?;
        }

        Ok(())
    }
}

// endregion
