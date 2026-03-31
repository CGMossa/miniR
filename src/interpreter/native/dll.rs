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

// Thread-local interpreter reference for callbacks from C code.
// Set before each .Call, cleared after.
thread_local! {
    static CURRENT_INTERP: std::cell::Cell<*const Interpreter> = const { std::cell::Cell::new(std::ptr::null()) };
}

fn callback_find_var(name: &str) -> Option<RValue> {
    CURRENT_INTERP.with(|cell| {
        let interp = cell.get();
        if interp.is_null() {
            return None;
        }
        let interp = unsafe { &*interp };
        interp.global_env.get(name)
    })
}

fn callback_define_var(name: &str, val: RValue) {
    CURRENT_INTERP.with(|cell| {
        let interp = cell.get();
        if interp.is_null() {
            return;
        }
        let interp = unsafe { &*interp };
        interp.global_env.set(name.to_string(), val);
    });
}

fn callback_eval_expr(expr: &RValue) -> Result<RValue, crate::interpreter::value::RError> {
    use crate::interpreter::value::RError;
    use crate::interpreter::CallFrame;
    CURRENT_INTERP.with(|cell| {
        let interp = cell.get();
        if interp.is_null() {
            return Err(RError::other(
                "no interpreter available for Rf_eval".to_string(),
            ));
        }
        let interp = unsafe { &*interp };

        // Push a native boundary marker so tracebacks show the C→R transition
        let boundary = CallFrame {
            call: None,
            function: RValue::Null,
            env: interp.global_env.clone(),
            formal_args: Default::default(),
            supplied_args: Default::default(),
            supplied_positional: Default::default(),
            supplied_named: Default::default(),
            supplied_arg_count: 0,
            is_native_boundary: true,
        };
        interp.call_stack.borrow_mut().push(boundary);

        let result = if let RValue::Language(ref lang) = expr {
            interp
                .eval_in(lang, &interp.global_env)
                .map_err(RError::from)
        } else if let Some(name) = expr.as_vector().and_then(|v| v.as_character_scalar()) {
            interp
                .global_env
                .get(&name)
                .ok_or_else(|| RError::other(format!("object '{name}' not found")))
        } else {
            Ok(expr.clone())
        };

        interp.call_stack.borrow_mut().pop();
        result
    })
}

fn callback_parse_text(text: &str) -> Result<RValue, crate::interpreter::value::RError> {
    use crate::interpreter::value::{Language, RError};
    // Parse the R source text into an AST
    let ast = crate::parser::parse_program(text)
        .map_err(|e| RError::other(format!("parse error: {e}")))?;
    Ok(RValue::Language(Language::new(ast)))
}

// region: C trampoline types

/// Signature of `R_init_<pkgname>(DllInfo*)` package init function.
type PkgInitFn = unsafe extern "C" fn(*mut u8);

// endregion

// region: CBuffer — C-compatible buffers for .C() calling convention

/// A C-compatible buffer for passing data to .C() functions.
///
/// .C() passes raw pointers to C functions:
/// - Double → `*mut f64`
/// - Integer → `*mut i32` (converted from miniR's i64)
/// - Logical → `*mut i32` (TRUE=1, FALSE=0, NA=NA_INTEGER)
/// - Character → `*mut *mut c_char` (array of null-terminated C strings)
/// - Raw → `*mut u8`
///
/// After the call, the C function may have modified the buffers in place.
/// `to_rvalue()` reads back the (possibly modified) data.
enum CBuffer {
    Double {
        data: Vec<f64>,
    },
    Integer {
        data: Vec<i32>,
    },
    Logical {
        data: Vec<i32>,
    },
    Character {
        /// Pointers to null-terminated C strings (owned by `_owned_strings`).
        ptrs: Vec<*mut c_char>,
        /// Backing storage for the C strings — kept alive for the call duration.
        /// Not read directly; exists to prevent deallocation while `ptrs` are live.
        _owned_strings: Vec<std::ffi::CString>,
    },
    Raw {
        data: Vec<u8>,
    },
}

impl CBuffer {
    /// Convert an RValue to a C-compatible buffer.
    fn from_rvalue(val: &RValue) -> Result<Self, String> {
        match val {
            RValue::Vector(rv) => match &rv.inner {
                Vector::Double(d) => {
                    let data: Vec<f64> = d.iter_opt().map(|v| v.unwrap_or(sexp::NA_REAL)).collect();
                    Ok(CBuffer::Double { data })
                }
                Vector::Integer(int) => {
                    let data: Vec<i32> = int
                        .iter_opt()
                        .map(|v| match v {
                            Some(i) => i32::try_from(i).unwrap_or(sexp::NA_INTEGER),
                            None => sexp::NA_INTEGER,
                        })
                        .collect();
                    Ok(CBuffer::Integer { data })
                }
                Vector::Logical(l) => {
                    let data: Vec<i32> = (0..l.len())
                        .map(|i| match l[i] {
                            Some(true) => 1i32,
                            Some(false) => 0i32,
                            None => sexp::NA_LOGICAL,
                        })
                        .collect();
                    Ok(CBuffer::Logical { data })
                }
                Vector::Character(c) => {
                    let mut owned_strings = Vec::with_capacity(c.len());
                    let mut ptrs = Vec::with_capacity(c.len());
                    for i in 0..c.len() {
                        let cstr = match &c[i] {
                            Some(s) => std::ffi::CString::new(s.as_str()).unwrap_or_else(|_| {
                                std::ffi::CString::new("").expect("empty CString")
                            }),
                            None => std::ffi::CString::new("NA").expect("NA CString"),
                        };
                        owned_strings.push(cstr);
                    }
                    // Build pointer array after all CStrings are in the Vec
                    // (so they don't move).
                    for cstr in &owned_strings {
                        ptrs.push(cstr.as_ptr() as *mut c_char);
                    }
                    Ok(CBuffer::Character {
                        ptrs,
                        _owned_strings: owned_strings,
                    })
                }
                Vector::Raw(r) => Ok(CBuffer::Raw { data: r.clone() }),
                Vector::Complex(_) => Err(
                    "complex vectors are not supported by .C() — use .Call() instead".to_string(),
                ),
            },
            RValue::Null => {
                // NULL is valid in .C — pass as empty double buffer
                Ok(CBuffer::Double { data: Vec::new() })
            }
            _ => Err(format!(
                "unsupported argument type for .C(): {}",
                val.type_name()
            )),
        }
    }

    /// Get a void pointer to the buffer data.
    fn as_void_ptr(&mut self) -> *mut u8 {
        match self {
            CBuffer::Double { data } => data.as_mut_ptr() as *mut u8,
            CBuffer::Integer { data } => data.as_mut_ptr() as *mut u8,
            CBuffer::Logical { data } => data.as_mut_ptr() as *mut u8,
            CBuffer::Character { ptrs, .. } => ptrs.as_mut_ptr() as *mut u8,
            CBuffer::Raw { data } => data.as_mut_ptr(),
        }
    }

    /// Convert the (possibly modified) buffer back to an RValue.
    fn to_rvalue(&self) -> RValue {
        match self {
            CBuffer::Double { data } => {
                let vals: Vec<Option<f64>> = data
                    .iter()
                    .map(|&v| if sexp::is_na_real(v) { None } else { Some(v) })
                    .collect();
                RValue::vec(Vector::Double(vals.into()))
            }
            CBuffer::Integer { data } => {
                let vals: Vec<Option<i64>> = data
                    .iter()
                    .map(|&v| {
                        if v == sexp::NA_INTEGER {
                            None
                        } else {
                            Some(i64::from(v))
                        }
                    })
                    .collect();
                RValue::vec(Vector::Integer(vals.into()))
            }
            CBuffer::Logical { data } => {
                let vals: Vec<Option<bool>> = data
                    .iter()
                    .map(|&v| {
                        if v == sexp::NA_LOGICAL {
                            None
                        } else {
                            Some(v != 0)
                        }
                    })
                    .collect();
                RValue::vec(Vector::Logical(vals.into()))
            }
            CBuffer::Character { ptrs, .. } => {
                let vals: Vec<Option<String>> = ptrs
                    .iter()
                    .map(|&p| {
                        if p.is_null() {
                            None
                        } else {
                            // Safety: the C function may have modified the string
                            // but we still own the buffer. Read it back.
                            let cstr = unsafe { CStr::from_ptr(p) };
                            Some(cstr.to_str().unwrap_or("").to_string())
                        }
                    })
                    .collect();
                RValue::vec(Vector::Character(vals.into()))
            }
            CBuffer::Raw { data } => RValue::vec(Vector::Raw(data.clone())),
        }
    }
}

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
    /// .Call methods registered via R_registerRoutines during R_init_<pkg>.
    pub registered_calls: HashMap<String, *const ()>,
    /// .C methods registered via R_registerRoutines during R_init_<pkg>.
    pub registered_c_methods: HashMap<String, *const ()>,
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
            registered_c_methods: HashMap::new(),
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

    /// Read registered .Call and .C methods from the Rust runtime's registry.
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
        for (name, ptr) in super::runtime::REGISTERED_C_METHODS
            .lock()
            .expect("lock registered C methods")
            .iter()
        {
            self.registered_c_methods.insert(name.clone(), ptr.0);
        }
    }

    /// Look up a .C method symbol by name. Checks registered .C methods first,
    /// then falls back to dynamic symbol lookup.
    pub fn get_c_symbol(&mut self, name: &str) -> Result<*const (), String> {
        if let Some(&ptr) = self.registered_c_methods.get(name) {
            return Ok(ptr);
        }
        // Fall back to dlsym — many packages don't register .C methods
        self.get_symbol(name)
    }

    /// Look up a function symbol by name. Checks registered .Call routines first,
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
            .field(
                "registered_c_methods",
                &self.registered_c_methods.keys().collect::<Vec<_>>(),
            )
            .finish()
    }
}

// endregion

// region: Interpreter DLL state

impl Interpreter {
    /// Load a shared library and register it. Returns the DLL name.
    pub(crate) fn dyn_load(&self, path: &Path) -> Result<String, RError> {
        // Set interpreter callbacks before loading — R_init_<pkg> may call
        // Rf_eval, R_ParseVector, etc. during initialization.
        CURRENT_INTERP.with(|cell| cell.set(self as *const Interpreter));
        super::runtime::set_callbacks(super::runtime::InterpreterCallbacks {
            find_var: Some(callback_find_var),
            define_var: Some(callback_define_var),
            eval_expr: Some(callback_eval_expr),
            parse_text: Some(callback_parse_text),
        });

        let dll = LoadedDll::load(path).map_err(|e| RError::new(RErrorKind::Other, e))?;
        let name = dll.name.clone();
        self.loaded_dlls.borrow_mut().push(dll);

        // DON'T clear callbacks — .onLoad may call .Call which needs them.
        // Callbacks are cleared after .Call returns in dot_call().

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
            format!("symbol '{name}' not found in any loaded DLL"),
        ))
    }

    /// Look up a symbol across all loaded DLLs. Returns the function pointer.
    pub(crate) fn find_native_symbol(&self, name: &str) -> Result<*const (), RError> {
        self.find_native_symbol_with_dll(name).map(|(ptr, _)| ptr)
    }

    /// Look up a .C symbol across all loaded DLLs — checks registered .C methods first.
    fn find_c_symbol(&self, name: &str) -> Result<*const (), RError> {
        let mut dlls = self.loaded_dlls.borrow_mut();
        for dll in dlls.iter_mut().rev() {
            if let Ok(ptr) = dll.get_c_symbol(name) {
                return Ok(ptr);
            }
        }
        Err(RError::new(
            RErrorKind::Other,
            format!("symbol '{name}' not found in any loaded DLL"),
        ))
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

        // Set interpreter callbacks so C code can call back for Rf_findVar, etc.
        CURRENT_INTERP.with(|cell| cell.set(self as *const Interpreter));
        super::runtime::set_callbacks(super::runtime::InterpreterCallbacks {
            find_var: Some(callback_find_var),
            define_var: Some(callback_define_var),
            eval_expr: Some(callback_eval_expr),
            parse_text: Some(callback_parse_text),
        });

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
            fn _minir_bt_count() -> i32;
            fn _minir_bt_frames() -> *const *const std::ffi::c_void;
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

            // Capture native backtrace from the C trampoline
            let bt_count = unsafe { _minir_bt_count() } as usize;
            if bt_count > 0 {
                let bt_raw = unsafe { std::slice::from_raw_parts(_minir_bt_frames(), bt_count) };
                let native_bt = crate::interpreter::NativeBacktrace {
                    frames: bt_raw.iter().map(|p| *p as usize).collect(),
                };
                *self.pending_native_backtrace.borrow_mut() = Some(native_bt);
            }

            // Clean up before returning error
            super::runtime::clear_callbacks();
            CURRENT_INTERP.with(|cell| cell.set(std::ptr::null()));
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

        // Clear interpreter callbacks
        super::runtime::clear_callbacks();
        CURRENT_INTERP.with(|cell| cell.set(std::ptr::null()));

        // Free runtime allocations (result SEXP + any intermediates)
        super::runtime::free_allocs();

        // Free Rust-allocated input SEXPs. Skip external pointers and environments.
        unsafe {
            for (s, arg) in sexp_args.into_iter().zip(args.iter()) {
                match arg {
                    RValue::List(list)
                        if list
                            .attrs
                            .as_ref()
                            .is_some_and(|a| a.contains_key(".sexp_ptr")) =>
                    {
                        continue; // external pointer — owned by C
                    }
                    RValue::Environment(_) => {
                        // Don't free the Environment box — it's shared via Rc
                        // Just free the SexpRec shell
                        (*s).data = std::ptr::null_mut();
                        sexp::free_sexp(s);
                    }
                    _ => sexp::free_sexp(s),
                }
            }
        }

        Ok(result)
    }

    /// Execute a `.External()` invocation.
    ///
    /// `.External()` passes a single SEXP pairlist to the C function.
    /// The pairlist's first CAR is the function symbol; CDR is the argument chain.
    pub(crate) fn dot_external(
        &self,
        symbol_name: &str,
        args: &[RValue],
    ) -> Result<RValue, RError> {
        let fn_ptr = self.find_native_symbol(symbol_name)?;

        // Build a pairlist: first node is a symbol for the function name,
        // then each argument is a node.
        let func_sym = super::runtime::Rf_install(
            std::ffi::CString::new(symbol_name)
                .unwrap_or_default()
                .as_ptr(),
        );
        let pairlist = super::runtime::Rf_cons(func_sym, unsafe { super::runtime::R_NilValue });

        // Append arguments in reverse order
        let mut tail = pairlist;
        for arg in args {
            let sexp_arg = convert::rvalue_to_sexp(arg);
            let node = super::runtime::Rf_cons(sexp_arg, unsafe { super::runtime::R_NilValue });
            // Set CDR of tail to node
            unsafe {
                let pd = (*tail).data as *mut sexp::PairlistData;
                if !pd.is_null() {
                    (*pd).cdr = node;
                }
            }
            tail = node;
        }

        // Set interpreter callbacks
        CURRENT_INTERP.with(|cell| cell.set(self as *const Interpreter));
        super::runtime::set_callbacks(super::runtime::InterpreterCallbacks {
            find_var: Some(callback_find_var),
            define_var: Some(callback_define_var),
            eval_expr: Some(callback_eval_expr),
            parse_text: Some(callback_parse_text),
        });

        extern "C" {
            fn _minir_call_protected(
                fn_ptr: *const (),
                args: *const Sexp,
                nargs: i32,
                result: *mut Sexp,
            ) -> i32;
            fn _minir_get_error_msg() -> *const c_char;
            fn _minir_bt_count() -> i32;
            fn _minir_bt_frames() -> *const *const std::ffi::c_void;
        }

        let mut result_sexp: Sexp = sexp::R_NIL_VALUE;
        let sexp_args = [pairlist];
        let error_code =
            unsafe { _minir_call_protected(fn_ptr, sexp_args.as_ptr(), 1, &mut result_sexp) };

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

            // Capture native backtrace from the C trampoline
            let bt_count = unsafe { _minir_bt_count() } as usize;
            if bt_count > 0 {
                let bt_raw = unsafe { std::slice::from_raw_parts(_minir_bt_frames(), bt_count) };
                let native_bt = crate::interpreter::NativeBacktrace {
                    frames: bt_raw.iter().map(|p| *p as usize).collect(),
                };
                *self.pending_native_backtrace.borrow_mut() = Some(native_bt);
            }

            super::runtime::clear_callbacks();
            CURRENT_INTERP.with(|cell| cell.set(std::ptr::null()));
            super::runtime::free_allocs();
            return Err(RError::new(RErrorKind::Other, error_msg));
        }

        let result = unsafe { convert::sexp_to_rvalue(result_sexp) };
        super::runtime::clear_callbacks();
        CURRENT_INTERP.with(|cell| cell.set(std::ptr::null()));
        super::runtime::free_allocs();

        Ok(result)
    }

    /// Execute a `.C()` invocation using the C trampoline for error safety.
    ///
    /// `.C()` is simpler than `.Call()` — it passes pointers to raw data
    /// buffers directly to C functions. The C function receives `double*`,
    /// `int*`, `char**`, etc. and modifies them in place. After the call,
    /// the modified buffers are read back into R values.
    ///
    /// Flow:
    /// 1. Convert each RValue arg to a C-compatible buffer
    /// 2. Look up the native function symbol
    /// 3. Call the trampoline with void* pointers
    /// 4. Read back modified buffers into R values
    /// 5. Return a named list of the (possibly modified) arguments
    pub(crate) fn dot_c(
        &self,
        symbol_name: &str,
        args: &[RValue],
        arg_names: &[Option<String>],
    ) -> Result<RValue, RError> {
        let fn_ptr = self.find_c_symbol(symbol_name)?;

        // Convert each arg to a C-compatible buffer and collect void* pointers.
        // Each CBuffer owns the memory; we read it back after the call.
        let mut buffers: Vec<CBuffer> = Vec::with_capacity(args.len());
        for (i, arg) in args.iter().enumerate() {
            buffers.push(CBuffer::from_rvalue(arg).map_err(|e| {
                RError::new(RErrorKind::Argument, format!(".C: argument {}: {e}", i + 1))
            })?);
        }

        let mut ptrs: Vec<*mut u8> = buffers.iter_mut().map(|b| b.as_void_ptr()).collect();

        // Set interpreter callbacks so C code can call back for Rf_findVar, etc.
        CURRENT_INTERP.with(|cell| cell.set(self as *const Interpreter));
        super::runtime::set_callbacks(super::runtime::InterpreterCallbacks {
            find_var: Some(callback_find_var),
            define_var: Some(callback_define_var),
            eval_expr: Some(callback_eval_expr),
            parse_text: Some(callback_parse_text),
        });

        extern "C" {
            fn _minir_dotC_call_protected(fn_ptr: *const (), args: *mut *mut u8, nargs: i32)
                -> i32;
            fn _minir_get_error_msg() -> *const c_char;
            fn _minir_bt_count() -> i32;
            fn _minir_bt_frames() -> *const *const std::ffi::c_void;
        }

        let nargs = i32::try_from(ptrs.len()).unwrap_or(0);
        let error_code = unsafe { _minir_dotC_call_protected(fn_ptr, ptrs.as_mut_ptr(), nargs) };

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

            // Capture native backtrace from the C trampoline
            let bt_count = unsafe { _minir_bt_count() } as usize;
            if bt_count > 0 {
                let bt_raw = unsafe { std::slice::from_raw_parts(_minir_bt_frames(), bt_count) };
                let native_bt = crate::interpreter::NativeBacktrace {
                    frames: bt_raw.iter().map(|p| *p as usize).collect(),
                };
                *self.pending_native_backtrace.borrow_mut() = Some(native_bt);
            }

            super::runtime::clear_callbacks();
            CURRENT_INTERP.with(|cell| cell.set(std::ptr::null()));
            super::runtime::free_allocs();

            return Err(RError::new(RErrorKind::Other, error_msg));
        }

        // Read back modified buffers into R values
        let mut result_values: Vec<(Option<String>, RValue)> = Vec::with_capacity(buffers.len());
        for (i, buf) in buffers.iter().enumerate() {
            let name = arg_names.get(i).and_then(|n| n.clone());
            result_values.push((name, buf.to_rvalue()));
        }

        // Clear interpreter callbacks
        super::runtime::clear_callbacks();
        CURRENT_INTERP.with(|cell| cell.set(std::ptr::null()));
        super::runtime::free_allocs();

        Ok(RValue::List(RList::new(result_values)))
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

            // Resolve LinkingTo include paths from DESCRIPTION
            let linking_to_includes = self.resolve_linking_to_includes(pkg_dir);

            // Compile into a temporary output directory under the package
            let output_dir = pkg_dir.join("libs");
            let compile = |out_dir: &std::path::Path| {
                super::compile::compile_package_with_deps(
                    &src_dir,
                    lib_name,
                    out_dir,
                    &include_dir,
                    &linking_to_includes,
                )
                .map_err(|e| {
                    RError::other(format!(
                        "compilation of native code for '{pkg_name}' failed: {e}"
                    ))
                })
            };

            if std::fs::create_dir_all(&output_dir).is_err() {
                // If we can't write to pkg_dir/libs, use temp dir
                let output_dir = self.temp_dir.path().join(format!("native-{pkg_name}"));
                std::fs::create_dir_all(&output_dir)
                    .map_err(|e| RError::other(format!("cannot create output directory: {e}")))?;
                let lib_path = compile(&output_dir)?;
                self.dyn_load(&lib_path)?;
                continue;
            }

            let lib_path = compile(&output_dir)?;
            self.dyn_load(&lib_path)?;
        }

        Ok(())
    }

    /// Resolve include paths for LinkingTo dependencies.
    ///
    /// Reads the package's DESCRIPTION, finds LinkingTo packages, and returns
    /// their `inst/include` directories (or `include/` at package root).
    fn resolve_linking_to_includes(&self, pkg_dir: &std::path::Path) -> Vec<std::path::PathBuf> {
        let desc_path = pkg_dir.join("DESCRIPTION");
        let desc_text = match std::fs::read_to_string(&desc_path) {
            Ok(t) => t,
            Err(_) => return Vec::new(),
        };
        let desc = match crate::interpreter::packages::description::PackageDescription::parse(
            &desc_text,
        ) {
            Ok(d) => d,
            Err(_) => return Vec::new(),
        };

        let mut includes = Vec::new();
        for dep in &desc.linking_to {
            // Find the dependency's package directory
            if let Some(dep_dir) = self.find_package_dir(&dep.package) {
                // R packages export headers from inst/include/ (installed) or include/ (source)
                let inst_include = dep_dir.join("inst").join("include");
                if inst_include.is_dir() {
                    includes.push(inst_include);
                } else {
                    let include = dep_dir.join("include");
                    if include.is_dir() {
                        includes.push(include);
                    }
                }
            }
        }
        includes
    }
}

// endregion
