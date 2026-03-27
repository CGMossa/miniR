//! Dynamic library loading — dyn.load(), dyn.unload(), symbol lookup.
//!
//! Uses `libloading` to load shared libraries (.so on Linux, .dylib on macOS)
//! and resolve function symbols for `.Call()` dispatch.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use libloading::{Library, Symbol};

use super::convert;
use super::sexp::{self, Sexp};
use crate::interpreter::value::*;
use crate::interpreter::Interpreter;

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
}

// Safety: LoadedDll is only used from a single interpreter thread.
// The Library handle and symbol pointers are stable once loaded.
unsafe impl Send for LoadedDll {}

impl LoadedDll {
    /// Load a shared library from the given path.
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

        Ok(LoadedDll {
            path: path.to_path_buf(),
            name,
            lib,
            symbols: HashMap::new(),
        })
    }

    /// Look up a function symbol by name. Caches the result.
    pub fn get_symbol(&mut self, name: &str) -> Result<*const (), String> {
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
            .finish()
    }
}

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

    /// Look up a symbol across all loaded DLLs. Returns the function pointer.
    pub(crate) fn find_native_symbol(&self, name: &str) -> Result<*const (), RError> {
        let mut dlls = self.loaded_dlls.borrow_mut();
        for dll in dlls.iter_mut().rev() {
            if let Ok(ptr) = dll.get_symbol(name) {
                return Ok(ptr);
            }
        }
        Err(RError::new(
            RErrorKind::Other,
            format!(".Call symbol '{name}' not found in any loaded DLL"),
        ))
    }

    /// Execute a `.Call()` invocation: look up symbol, convert args, call, convert result.
    pub(crate) fn dot_call(&self, symbol_name: &str, args: &[RValue]) -> Result<RValue, RError> {
        let fn_ptr = self.find_native_symbol(symbol_name)?;

        // Convert RValue args to SEXPs (allocated with C allocator)
        let sexp_args: Vec<Sexp> = args.iter().map(convert::rvalue_to_sexp).collect();

        // Call the native function with the right number of args.
        // .Call functions always take and return SEXP.
        let result_sexp = unsafe { call_native(fn_ptr, &sexp_args) }?;

        // Convert result back to RValue (copies all data)
        let result = unsafe { convert::sexp_to_rvalue(result_sexp) };

        // Free C-side allocations via the .so's cleanup function.
        // The C runtime (Rinternals.h) tracks all allocations made during the call
        // in _minir_alloc_list. Call _minir_free_allocs to free them all.
        // This frees both the result SEXP and any intermediate allocations.
        self.call_cleanup_fn();

        // Free input SEXPs that Rust allocated (these are NOT tracked by the .so's
        // alloc list because they were allocated by Rust's sexp module, not by
        // Rinternals.h's Rf_allocVector).
        unsafe {
            for s in sexp_args {
                sexp::free_sexp(s);
            }
        }

        Ok(result)
    }

    /// Call _minir_free_allocs in the most recently loaded DLL (if available).
    fn call_cleanup_fn(&self) {
        type CleanupFn = unsafe extern "C" fn();
        let mut dlls = self.loaded_dlls.borrow_mut();
        for dll in dlls.iter_mut().rev() {
            if let Ok(ptr) = dll.get_symbol("_minir_free_allocs") {
                unsafe {
                    let cleanup: CleanupFn = std::mem::transmute(ptr);
                    cleanup();
                }
                return;
            }
        }
    }
}

// endregion

// region: Native call dispatch

/// Type aliases for .Call function signatures (up to 16 args).
type NativeFn0 = unsafe extern "C" fn() -> Sexp;
type NativeFn1 = unsafe extern "C" fn(Sexp) -> Sexp;
type NativeFn2 = unsafe extern "C" fn(Sexp, Sexp) -> Sexp;
type NativeFn3 = unsafe extern "C" fn(Sexp, Sexp, Sexp) -> Sexp;
type NativeFn4 = unsafe extern "C" fn(Sexp, Sexp, Sexp, Sexp) -> Sexp;
type NativeFn5 = unsafe extern "C" fn(Sexp, Sexp, Sexp, Sexp, Sexp) -> Sexp;
type NativeFn6 = unsafe extern "C" fn(Sexp, Sexp, Sexp, Sexp, Sexp, Sexp) -> Sexp;
type NativeFn7 = unsafe extern "C" fn(Sexp, Sexp, Sexp, Sexp, Sexp, Sexp, Sexp) -> Sexp;
type NativeFn8 = unsafe extern "C" fn(Sexp, Sexp, Sexp, Sexp, Sexp, Sexp, Sexp, Sexp) -> Sexp;

/// Call a native function pointer with the given SEXP arguments.
///
/// # Safety
/// `fn_ptr` must point to a valid C function with the correct number of SEXP args.
unsafe fn call_native(fn_ptr: *const (), args: &[Sexp]) -> Result<Sexp, RError> {
    let result = match args.len() {
        0 => {
            let f: NativeFn0 = std::mem::transmute(fn_ptr);
            f()
        }
        1 => {
            let f: NativeFn1 = std::mem::transmute(fn_ptr);
            f(args[0])
        }
        2 => {
            let f: NativeFn2 = std::mem::transmute(fn_ptr);
            f(args[0], args[1])
        }
        3 => {
            let f: NativeFn3 = std::mem::transmute(fn_ptr);
            f(args[0], args[1], args[2])
        }
        4 => {
            let f: NativeFn4 = std::mem::transmute(fn_ptr);
            f(args[0], args[1], args[2], args[3])
        }
        5 => {
            let f: NativeFn5 = std::mem::transmute(fn_ptr);
            f(args[0], args[1], args[2], args[3], args[4])
        }
        6 => {
            let f: NativeFn6 = std::mem::transmute(fn_ptr);
            f(args[0], args[1], args[2], args[3], args[4], args[5])
        }
        7 => {
            let f: NativeFn7 = std::mem::transmute(fn_ptr);
            f(
                args[0], args[1], args[2], args[3], args[4], args[5], args[6],
            )
        }
        8 => {
            let f: NativeFn8 = std::mem::transmute(fn_ptr);
            f(
                args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7],
            )
        }
        n => {
            return Err(RError::new(
                RErrorKind::Other,
                format!(".Call with {n} arguments is not supported (max 8)"),
            ));
        }
    };
    Ok(result)
}

// endregion
