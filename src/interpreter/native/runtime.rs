//! Rust-native R C API runtime.
//!
//! These functions deref raw SEXP pointers from C code — this is inherently
//! unsafe but required for C API compatibility. We suppress the clippy lint
//! at module level since every function in this module works with raw pointers.
#![allow(clippy::not_unsafe_ptr_arg_deref)]
//!
//! Implements the R C API functions (`Rf_allocVector`, `Rf_protect`, etc.)
//! as `extern "C"` Rust functions that are compiled into the miniR binary.
//! Package `.so` files resolve these symbols at load time.
//!
//! setjmp/longjmp-based functions (`Rf_error`, `_minir_call_protected`)
//! are in `csrc/native_trampoline.c` (compiled via build.rs) because
//! longjmp is not safely callable from Rust.

use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_void};
use std::ptr;

use super::sexp::{self, PairlistData, Sexp, SexpRec};

// region: allocation tracking

/// Linked list node for tracking allocations.
struct AllocNode {
    sexp: Sexp,
    next: *mut AllocNode,
}

/// Result type for interpreter callbacks.
type CbResult = Result<crate::interpreter::value::RValue, crate::interpreter::value::RError>;

/// Callback function pointers set by the Rust interpreter before each .Call.
#[derive(Default)]
pub struct InterpreterCallbacks {
    pub find_var: Option<fn(&str) -> Option<crate::interpreter::value::RValue>>,
    pub define_var: Option<fn(&str, crate::interpreter::value::RValue)>,
    pub eval_expr: Option<fn(&crate::interpreter::value::RValue) -> CbResult>,
    pub parse_text: Option<fn(&str) -> CbResult>,
}

/// Thread-local allocation state for the current .Call invocation.
/// This is in the binary (shared by all packages), not per-.so.
struct RuntimeState {
    alloc_head: *mut AllocNode,
    protect_stack: Vec<Sexp>,
    callbacks: InterpreterCallbacks,
}

thread_local! {
    static STATE: std::cell::RefCell<RuntimeState> = std::cell::RefCell::new(RuntimeState {
        alloc_head: ptr::null_mut(),
        protect_stack: Vec::with_capacity(128),
        callbacks: InterpreterCallbacks::default(),
    });
}

/// Set interpreter callbacks for the current .Call invocation.
pub fn set_callbacks(callbacks: InterpreterCallbacks) {
    STATE.with(|state| {
        state.borrow_mut().callbacks = callbacks;
    });
}

/// Clear interpreter callbacks after .Call returns.
pub fn clear_callbacks() {
    STATE.with(|state| {
        let mut st = state.borrow_mut();
        st.callbacks = InterpreterCallbacks::default();
    });
}

fn track(s: Sexp) {
    let node = Box::into_raw(Box::new(AllocNode {
        sexp: s,
        next: ptr::null_mut(),
    }));
    STATE.with(|state| {
        let mut st = state.borrow_mut();
        unsafe {
            (*node).next = st.alloc_head;
        }
        st.alloc_head = node;
    });
}

// endregion

// region: sentinel globals

// Safety: These globals are initialized once by `init_globals()` and never
// written again. Multiple reader threads are safe. The `static mut` is used
// because `#[no_mangle]` extern statics must be `static mut` for C ABI compat.
// The `unsafe` blocks in init_globals are the only writes.

static mut NIL_REC: SexpRec = SexpRec {
    stype: sexp::NILSXP,
    flags: 0,
    padding: 0,
    length: 0,
    data: ptr::null_mut(),
    attrib: ptr::null_mut(),
};

/// R_NilValue — exported to C code.
#[no_mangle]
pub static mut R_NilValue: Sexp = ptr::null_mut();

#[no_mangle]
pub static mut R_NaString: Sexp = ptr::null_mut();

#[no_mangle]
pub static mut R_BlankString: Sexp = ptr::null_mut();

#[no_mangle]
pub static mut R_GlobalEnv: Sexp = ptr::null_mut();

#[no_mangle]
pub static mut R_BaseEnv: Sexp = ptr::null_mut();

#[no_mangle]
pub static mut R_UnboundValue: Sexp = ptr::null_mut();

// Well-known symbols
static mut SYM_NAMES: SexpRec = SexpRec {
    stype: sexp::SYMSXP,
    flags: 0,
    padding: 0,
    length: 5,
    data: ptr::null_mut(),
    attrib: ptr::null_mut(),
};
static mut SYM_DIM: SexpRec = SexpRec {
    stype: sexp::SYMSXP,
    flags: 0,
    padding: 0,
    length: 3,
    data: ptr::null_mut(),
    attrib: ptr::null_mut(),
};
static mut SYM_DIMNAMES: SexpRec = SexpRec {
    stype: sexp::SYMSXP,
    flags: 0,
    padding: 0,
    length: 8,
    data: ptr::null_mut(),
    attrib: ptr::null_mut(),
};
static mut SYM_CLASS: SexpRec = SexpRec {
    stype: sexp::SYMSXP,
    flags: 0,
    padding: 0,
    length: 5,
    data: ptr::null_mut(),
    attrib: ptr::null_mut(),
};
static mut SYM_ROWNAMES: SexpRec = SexpRec {
    stype: sexp::SYMSXP,
    flags: 0,
    padding: 0,
    length: 10,
    data: ptr::null_mut(),
    attrib: ptr::null_mut(),
};
static mut SYM_LEVELS: SexpRec = SexpRec {
    stype: sexp::SYMSXP,
    flags: 0,
    padding: 0,
    length: 6,
    data: ptr::null_mut(),
    attrib: ptr::null_mut(),
};

#[no_mangle]
pub static mut R_NamesSymbol: Sexp = ptr::null_mut();
#[no_mangle]
pub static mut R_DimSymbol: Sexp = ptr::null_mut();
#[no_mangle]
pub static mut R_DimNamesSymbol: Sexp = ptr::null_mut();
#[no_mangle]
pub static mut R_ClassSymbol: Sexp = ptr::null_mut();
#[no_mangle]
pub static mut R_RowNamesSymbol: Sexp = ptr::null_mut();
#[no_mangle]
pub static mut R_LevelsSymbol: Sexp = ptr::null_mut();

static mut SYM_DOTS: SexpRec = SexpRec {
    stype: sexp::SYMSXP,
    flags: 0,
    padding: 0,
    length: 3,
    data: ptr::null_mut(),
    attrib: ptr::null_mut(),
};
#[no_mangle]
pub static mut R_DotsSymbol: Sexp = ptr::null_mut();

#[no_mangle]
pub static mut _minir_current_dll_info: *mut c_void = ptr::null_mut();

/// Initialize global sentinels. Called once at interpreter startup.
pub fn init_globals() {
    unsafe {
        // Static string data for sentinels
        static NA_STR: &[u8] = b"NA\0";
        static BLANK_STR: &[u8] = b"\0";
        static NAMES_STR: &[u8] = b"names\0";
        static DIM_STR: &[u8] = b"dim\0";
        static DIMNAMES_STR: &[u8] = b"dimnames\0";
        static CLASS_STR: &[u8] = b"class\0";
        static ROWNAMES_STR: &[u8] = b"row.names\0";
        static LEVELS_STR: &[u8] = b"levels\0";
        static DOTS_STR: &[u8] = b"...\0";

        R_NilValue = &raw mut NIL_REC;

        // R_NaString
        static mut NA_STRING_REC: SexpRec = SexpRec {
            stype: sexp::CHARSXP,
            flags: 0,
            padding: 0,
            length: 2,
            data: ptr::null_mut(),
            attrib: ptr::null_mut(),
        };
        NA_STRING_REC.data = NA_STR.as_ptr() as *mut u8;
        NA_STRING_REC.attrib = R_NilValue;
        R_NaString = &raw mut NA_STRING_REC;

        // R_BlankString
        static mut BLANK_STRING_REC: SexpRec = SexpRec {
            stype: sexp::CHARSXP,
            flags: 0,
            padding: 0,
            length: 0,
            data: ptr::null_mut(),
            attrib: ptr::null_mut(),
        };
        BLANK_STRING_REC.data = BLANK_STR.as_ptr() as *mut u8;
        BLANK_STRING_REC.attrib = R_NilValue;
        R_BlankString = &raw mut BLANK_STRING_REC;

        R_GlobalEnv = R_NilValue;
        R_BaseEnv = R_NilValue;
        R_UnboundValue = R_NilValue;
        R_EmptyEnv = R_NilValue;
        R_MissingArg = R_NilValue;
        R_NamespaceRegistry = R_NilValue;
        R_Srcref = R_NilValue;
        R_BaseNamespace = R_NilValue;
        R_BraceSymbol = R_NilValue;
        R_BracketSymbol = R_NilValue;
        R_Bracket2Symbol = R_NilValue;
        R_DoubleColonSymbol = R_NilValue;
        R_TripleColonSymbol = R_NilValue;

        // Symbol sentinels
        SYM_NAMES.data = NAMES_STR.as_ptr() as *mut u8;
        SYM_NAMES.attrib = R_NilValue;
        R_NamesSymbol = &raw mut SYM_NAMES;

        SYM_DIM.data = DIM_STR.as_ptr() as *mut u8;
        SYM_DIM.attrib = R_NilValue;
        R_DimSymbol = &raw mut SYM_DIM;

        SYM_DIMNAMES.data = DIMNAMES_STR.as_ptr() as *mut u8;
        SYM_DIMNAMES.attrib = R_NilValue;
        R_DimNamesSymbol = &raw mut SYM_DIMNAMES;

        SYM_CLASS.data = CLASS_STR.as_ptr() as *mut u8;
        SYM_CLASS.attrib = R_NilValue;
        R_ClassSymbol = &raw mut SYM_CLASS;

        SYM_ROWNAMES.data = ROWNAMES_STR.as_ptr() as *mut u8;
        SYM_ROWNAMES.attrib = R_NilValue;
        R_RowNamesSymbol = &raw mut SYM_ROWNAMES;

        SYM_LEVELS.data = LEVELS_STR.as_ptr() as *mut u8;
        SYM_LEVELS.attrib = R_NilValue;
        R_LevelsSymbol = &raw mut SYM_LEVELS;

        SYM_DOTS.data = DOTS_STR.as_ptr() as *mut u8;
        SYM_DOTS.attrib = R_NilValue;
        R_DotsSymbol = &raw mut SYM_DOTS;
    }
}

// endregion

// region: C allocator wrappers

extern "C" {
    fn calloc(count: usize, size: usize) -> *mut u8;
    fn realloc(ptr: *mut u8, size: usize) -> *mut u8;
    fn free(ptr: *mut u8);
}

// endregion

// region: Rf_allocVector

#[no_mangle]
pub extern "C" fn Rf_allocVector(stype: c_int, length: isize) -> Sexp {
    let s = sexp::alloc_vector(stype as u8, length as i32);
    if !s.is_null() {
        unsafe {
            (*s).attrib = R_NilValue;
        }
    }
    track(s);
    s
}

#[no_mangle]
pub extern "C" fn Rf_allocMatrix(stype: c_int, nrow: c_int, ncol: c_int) -> Sexp {
    Rf_allocVector(stype, (nrow as isize) * (ncol as isize))
}

// endregion

// region: Scalar constructors

#[no_mangle]
pub extern "C" fn Rf_ScalarReal(x: f64) -> Sexp {
    let s = Rf_allocVector(sexp::REALSXP as c_int, 1);
    unsafe {
        *((*s).data as *mut f64) = x;
    }
    s
}

#[no_mangle]
pub extern "C" fn Rf_ScalarInteger(x: c_int) -> Sexp {
    let s = Rf_allocVector(sexp::INTSXP as c_int, 1);
    unsafe {
        *((*s).data as *mut i32) = x;
    }
    s
}

#[no_mangle]
pub extern "C" fn Rf_ScalarLogical(x: c_int) -> Sexp {
    let s = Rf_allocVector(sexp::LGLSXP as c_int, 1);
    unsafe {
        *((*s).data as *mut i32) = x;
    }
    s
}

#[no_mangle]
pub extern "C" fn Rf_ScalarString(x: Sexp) -> Sexp {
    let s = Rf_allocVector(sexp::STRSXP as c_int, 1);
    unsafe {
        let elts = (*s).data as *mut Sexp;
        *elts = x;
    }
    s
}

// endregion

// region: String functions

#[no_mangle]
pub extern "C" fn Rf_mkChar(str_ptr: *const c_char) -> Sexp {
    if str_ptr.is_null() {
        return unsafe { R_NaString };
    }
    let cstr = unsafe { CStr::from_ptr(str_ptr) };
    let s = sexp::mk_char(cstr.to_str().unwrap_or(""));
    track(s);
    s
}

#[no_mangle]
pub extern "C" fn Rf_mkCharLen(str_ptr: *const c_char, len: c_int) -> Sexp {
    if str_ptr.is_null() {
        return unsafe { R_NaString };
    }
    let bytes = unsafe { std::slice::from_raw_parts(str_ptr as *const u8, len as usize) };
    let st = std::str::from_utf8(bytes).unwrap_or("");
    let s = sexp::mk_char(st);
    track(s);
    s
}

#[no_mangle]
pub extern "C" fn Rf_mkCharCE(str_ptr: *const c_char, _encoding: c_int) -> Sexp {
    Rf_mkChar(str_ptr) // miniR is always UTF-8
}

#[no_mangle]
pub extern "C" fn Rf_getCharCE(_x: Sexp) -> c_int {
    1 // CE_UTF8
}

#[no_mangle]
pub extern "C" fn Rf_mkString(str_ptr: *const c_char) -> Sexp {
    let s = Rf_allocVector(sexp::STRSXP as c_int, 1);
    let ch = Rf_mkChar(str_ptr);
    unsafe {
        let elts = (*s).data as *mut Sexp;
        *elts = ch;
    }
    s
}

#[no_mangle]
pub extern "C" fn Rf_StringBlank(x: Sexp) -> c_int {
    if x.is_null() {
        return 1;
    }
    unsafe {
        if x == R_NilValue || x == R_BlankString {
            return 1;
        }
        if (*x).stype != sexp::CHARSXP {
            return 1;
        }
        if (*x).data.is_null() {
            return 1;
        }
        if *((*x).data) == 0 {
            return 1;
        }
    }
    0
}

// endregion

// region: Rf_length

#[no_mangle]
pub extern "C" fn Rf_length(x: Sexp) -> c_int {
    if x.is_null() {
        return 0;
    }
    unsafe {
        if x == R_NilValue {
            return 0;
        }
        (*x).length
    }
}

// endregion

// region: Symbols

#[no_mangle]
pub extern "C" fn Rf_install(name: *const c_char) -> Sexp {
    if name.is_null() {
        return unsafe { R_NilValue };
    }
    let cstr = unsafe { CStr::from_ptr(name) };
    let s = cstr.to_str().unwrap_or("");

    // Check well-known symbols
    unsafe {
        match s {
            "names" => return R_NamesSymbol,
            "dim" => return R_DimSymbol,
            "dimnames" => return R_DimNamesSymbol,
            "class" => return R_ClassSymbol,
            "row.names" => return R_RowNamesSymbol,
            "levels" => return R_LevelsSymbol,
            "..." => return R_DotsSymbol,
            _ => {}
        }
    }

    // Allocate new symbol
    let rec = sexp::mk_char(s); // reuse CHARSXP allocator for the name
    unsafe {
        (*rec).stype = sexp::SYMSXP;
    }
    track(rec);
    rec
}

// endregion

// region: Pairlists

#[no_mangle]
pub extern "C" fn Rf_cons(car: Sexp, cdr: Sexp) -> Sexp {
    unsafe {
        let s = calloc(1, std::mem::size_of::<SexpRec>()) as Sexp;
        if s.is_null() {
            return R_NilValue;
        }
        (*s).stype = sexp::LISTSXP;
        (*s).attrib = R_NilValue;
        let pd = calloc(1, std::mem::size_of::<PairlistData>()) as *mut PairlistData;
        if !pd.is_null() {
            (*pd).car = car;
            (*pd).cdr = cdr;
            (*pd).tag = R_NilValue;
        }
        (*s).data = pd as *mut u8;
        track(s);
        s
    }
}

#[no_mangle]
pub extern "C" fn Rf_lcons(car: Sexp, cdr: Sexp) -> Sexp {
    let s = Rf_cons(car, cdr);
    if !s.is_null() {
        unsafe {
            (*s).stype = 6;
        } // LANGSXP
    }
    s
}

// endregion

// region: PROTECT / UNPROTECT

#[no_mangle]
pub extern "C" fn Rf_protect(s: Sexp) -> Sexp {
    STATE.with(|state| {
        state.borrow_mut().protect_stack.push(s);
    });
    s
}

#[no_mangle]
pub extern "C" fn Rf_unprotect(n: c_int) {
    STATE.with(|state| {
        let mut st = state.borrow_mut();
        let n = n as usize;
        let new_len = st.protect_stack.len().saturating_sub(n);
        st.protect_stack.truncate(new_len);
    });
}

// endregion

// region: Type checking

#[no_mangle]
pub extern "C" fn Rf_isNull(x: Sexp) -> c_int {
    if x.is_null() {
        return 1;
    }
    (unsafe { (*x).stype } == sexp::NILSXP) as c_int
}

#[no_mangle]
pub extern "C" fn Rf_isReal(x: Sexp) -> c_int {
    if x.is_null() {
        0
    } else {
        (unsafe { (*x).stype } == sexp::REALSXP) as c_int
    }
}
#[no_mangle]
pub extern "C" fn Rf_isInteger(x: Sexp) -> c_int {
    if x.is_null() {
        0
    } else {
        (unsafe { (*x).stype } == sexp::INTSXP) as c_int
    }
}
#[no_mangle]
pub extern "C" fn Rf_isLogical(x: Sexp) -> c_int {
    if x.is_null() {
        0
    } else {
        (unsafe { (*x).stype } == sexp::LGLSXP) as c_int
    }
}
#[no_mangle]
pub extern "C" fn Rf_isString(x: Sexp) -> c_int {
    if x.is_null() {
        0
    } else {
        (unsafe { (*x).stype } == sexp::STRSXP) as c_int
    }
}

#[no_mangle]
pub extern "C" fn Rf_isVector(x: Sexp) -> c_int {
    if x.is_null() {
        return 0;
    }
    let t = unsafe { (*x).stype };
    matches!(
        t,
        sexp::REALSXP
            | sexp::INTSXP
            | sexp::LGLSXP
            | sexp::STRSXP
            | sexp::VECSXP
            | sexp::RAWSXP
            | sexp::CPLXSXP
    ) as c_int
}

#[no_mangle]
pub extern "C" fn Rf_inherits(x: Sexp, name: *const c_char) -> c_int {
    if x.is_null() || name.is_null() {
        return 0;
    }
    let target = unsafe { CStr::from_ptr(name) }.to_str().unwrap_or("");
    let klass = Rf_getAttrib(x, unsafe { R_ClassSymbol });
    if klass.is_null() || unsafe { (*klass).stype } != sexp::STRSXP {
        return 0;
    }
    let len = unsafe { (*klass).length } as usize;
    for i in 0..len {
        let elt = unsafe { *((*klass).data as *const Sexp).add(i) };
        if !elt.is_null() {
            let s = unsafe { sexp::char_data(elt) };
            if s == target {
                return 1;
            }
        }
    }
    0
}

// endregion

// region: Attributes

fn sym_eq(a: Sexp, b: Sexp) -> bool {
    if a == b {
        return true;
    }
    if a.is_null() || b.is_null() {
        return false;
    }
    unsafe {
        if (*a).stype != sexp::SYMSXP || (*b).stype != sexp::SYMSXP {
            return false;
        }
        if (*a).data.is_null() || (*b).data.is_null() {
            return false;
        }
        let a_str = CStr::from_ptr((*a).data as *const c_char);
        let b_str = CStr::from_ptr((*b).data as *const c_char);
        a_str == b_str
    }
}

#[no_mangle]
pub extern "C" fn Rf_getAttrib(x: Sexp, name: Sexp) -> Sexp {
    if x.is_null() {
        return unsafe { R_NilValue };
    }
    let mut attr = unsafe { (*x).attrib };
    while !attr.is_null() && unsafe { (*attr).stype } == sexp::LISTSXP {
        let pd = unsafe { (*attr).data as *const PairlistData };
        if !pd.is_null() && sym_eq(unsafe { (*pd).tag }, name) {
            return unsafe { (*pd).car };
        }
        attr = if pd.is_null() {
            ptr::null_mut()
        } else {
            unsafe { (*pd).cdr }
        };
    }
    unsafe { R_NilValue }
}

#[no_mangle]
pub extern "C" fn Rf_setAttrib(x: Sexp, name: Sexp, val: Sexp) -> Sexp {
    if x.is_null() {
        return val;
    }
    // Search for existing
    let mut attr = unsafe { (*x).attrib };
    while !attr.is_null() && unsafe { (*attr).stype } == sexp::LISTSXP {
        let pd = unsafe { (*attr).data as *mut PairlistData };
        if !pd.is_null() && sym_eq(unsafe { (*pd).tag }, name) {
            unsafe {
                (*pd).car = val;
            }
            return val;
        }
        attr = if pd.is_null() {
            ptr::null_mut()
        } else {
            unsafe { (*pd).cdr }
        };
    }
    // Prepend
    let node = Rf_cons(val, unsafe { (*x).attrib });
    unsafe {
        let pd = (*node).data as *mut PairlistData;
        if !pd.is_null() {
            (*pd).tag = name;
        }
        (*x).attrib = node;
    }
    val
}

// endregion

// region: Coercion

#[no_mangle]
pub extern "C" fn Rf_asReal(x: Sexp) -> f64 {
    if x.is_null() {
        return sexp::NA_REAL;
    }
    unsafe {
        match (*x).stype {
            sexp::REALSXP if (*x).length > 0 => *((*x).data as *const f64),
            sexp::INTSXP if (*x).length > 0 => {
                let v = *((*x).data as *const i32);
                if v == sexp::NA_INTEGER {
                    sexp::NA_REAL
                } else {
                    f64::from(v)
                }
            }
            sexp::LGLSXP if (*x).length > 0 => {
                let v = *((*x).data as *const i32);
                if v == sexp::NA_LOGICAL {
                    sexp::NA_REAL
                } else {
                    f64::from(v)
                }
            }
            _ => sexp::NA_REAL,
        }
    }
}

#[no_mangle]
pub extern "C" fn Rf_asInteger(x: Sexp) -> c_int {
    if x.is_null() {
        return sexp::NA_INTEGER;
    }
    unsafe {
        match (*x).stype {
            sexp::INTSXP if (*x).length > 0 => *((*x).data as *const i32),
            sexp::REALSXP if (*x).length > 0 => {
                let v = *((*x).data as *const f64);
                if sexp::is_na_real(v) {
                    sexp::NA_INTEGER
                } else {
                    v as i32
                }
            }
            sexp::LGLSXP if (*x).length > 0 => *((*x).data as *const i32),
            _ => sexp::NA_INTEGER,
        }
    }
}

#[no_mangle]
pub extern "C" fn Rf_asLogical(x: Sexp) -> c_int {
    if x.is_null() {
        return sexp::NA_LOGICAL;
    }
    unsafe {
        match (*x).stype {
            sexp::LGLSXP if (*x).length > 0 => *((*x).data as *const i32),
            sexp::INTSXP if (*x).length > 0 => {
                let v = *((*x).data as *const i32);
                if v == sexp::NA_INTEGER {
                    sexp::NA_LOGICAL
                } else {
                    (v != 0) as i32
                }
            }
            sexp::REALSXP if (*x).length > 0 => {
                let v = *((*x).data as *const f64);
                if sexp::is_na_real(v) {
                    sexp::NA_LOGICAL
                } else {
                    (v != 0.0) as i32
                }
            }
            _ => sexp::NA_LOGICAL,
        }
    }
}

#[no_mangle]
pub extern "C" fn Rf_coerceVector(x: Sexp, stype: c_int) -> Sexp {
    if x.is_null() {
        return unsafe { R_NilValue };
    }
    let from = unsafe { (*x).stype };
    if from == stype as u8 {
        return x;
    }
    let n = unsafe { (*x).length } as isize;
    let out = Rf_protect(Rf_allocVector(stype, n));
    for i in 0..(n as usize) {
        unsafe {
            match stype as u8 {
                sexp::REALSXP => {
                    let dst = ((*out).data as *mut f64).add(i);
                    *dst = match from {
                        sexp::INTSXP => {
                            let v = *((*x).data as *const i32).add(i);
                            if v == sexp::NA_INTEGER {
                                sexp::NA_REAL
                            } else {
                                f64::from(v)
                            }
                        }
                        sexp::LGLSXP => {
                            let v = *((*x).data as *const i32).add(i);
                            if v == sexp::NA_LOGICAL {
                                sexp::NA_REAL
                            } else {
                                f64::from(v)
                            }
                        }
                        _ => sexp::NA_REAL,
                    };
                }
                sexp::INTSXP => {
                    let dst = ((*out).data as *mut i32).add(i);
                    *dst = match from {
                        sexp::REALSXP => {
                            let v = *((*x).data as *const f64).add(i);
                            if sexp::is_na_real(v) {
                                sexp::NA_INTEGER
                            } else {
                                v as i32
                            }
                        }
                        sexp::LGLSXP => *((*x).data as *const i32).add(i),
                        _ => sexp::NA_INTEGER,
                    };
                }
                sexp::LGLSXP => {
                    let dst = ((*out).data as *mut i32).add(i);
                    *dst = match from {
                        sexp::INTSXP => {
                            let v = *((*x).data as *const i32).add(i);
                            if v == sexp::NA_INTEGER {
                                sexp::NA_LOGICAL
                            } else {
                                (v != 0) as i32
                            }
                        }
                        sexp::REALSXP => {
                            let v = *((*x).data as *const f64).add(i);
                            if sexp::is_na_real(v) {
                                sexp::NA_LOGICAL
                            } else {
                                (v != 0.0) as i32
                            }
                        }
                        _ => sexp::NA_LOGICAL,
                    };
                }
                _ => {}
            }
        }
    }
    Rf_unprotect(1);
    out
}

// endregion

// region: Duplication

#[no_mangle]
pub extern "C" fn Rf_duplicate(x: Sexp) -> Sexp {
    if x.is_null() {
        return unsafe { R_NilValue };
    }
    unsafe {
        if x == R_NilValue {
            return R_NilValue;
        }
    }
    let len = unsafe { (*x).length };
    let stype = unsafe { (*x).stype };
    let out = Rf_allocVector(stype as c_int, len as isize);
    if len > 0 {
        let elem_size = match stype {
            sexp::REALSXP => 8,
            sexp::INTSXP | sexp::LGLSXP => 4,
            sexp::RAWSXP => 1,
            sexp::CPLXSXP => 16,
            sexp::STRSXP | sexp::VECSXP => std::mem::size_of::<Sexp>(),
            _ => 0,
        };
        if elem_size > 0 {
            unsafe {
                ptr::copy_nonoverlapping((*x).data, (*out).data, len as usize * elem_size);
            }
        }
    }
    unsafe {
        (*out).attrib = (*x).attrib;
    }
    out
}

// endregion

// region: External pointers

#[repr(C)]
struct ExtPtrData {
    ptr: *mut c_void,
    tag: Sexp,
    prot: Sexp,
}

#[no_mangle]
pub extern "C" fn R_MakeExternalPtr(p: *mut c_void, tag: Sexp, prot: Sexp) -> Sexp {
    unsafe {
        let s = calloc(1, std::mem::size_of::<SexpRec>()) as Sexp;
        if s.is_null() {
            return R_NilValue;
        }
        (*s).stype = 22; // EXTPTRSXP
        (*s).flags = 1; // persistent — survives _minir_free_allocs
        (*s).attrib = R_NilValue;
        let d = calloc(1, std::mem::size_of::<ExtPtrData>()) as *mut ExtPtrData;
        if !d.is_null() {
            (*d).ptr = p;
            (*d).tag = tag;
            (*d).prot = prot;
        }
        (*s).data = d as *mut u8;
        track(s);
        s
    }
}

#[no_mangle]
pub extern "C" fn R_ExternalPtrAddr(s: Sexp) -> *mut c_void {
    if s.is_null() {
        return ptr::null_mut();
    }
    unsafe {
        if (*s).stype != 22 || (*s).data.is_null() {
            return ptr::null_mut();
        }
        (*((*s).data as *const ExtPtrData)).ptr
    }
}

#[no_mangle]
pub extern "C" fn R_ExternalPtrTag(s: Sexp) -> Sexp {
    if s.is_null() {
        return unsafe { R_NilValue };
    }
    unsafe {
        if (*s).stype != 22 || (*s).data.is_null() {
            return R_NilValue;
        }
        (*((*s).data as *const ExtPtrData)).tag
    }
}

#[no_mangle]
pub extern "C" fn R_ExternalPtrProtected(s: Sexp) -> Sexp {
    if s.is_null() {
        return unsafe { R_NilValue };
    }
    unsafe {
        if (*s).stype != 22 || (*s).data.is_null() {
            return R_NilValue;
        }
        (*((*s).data as *const ExtPtrData)).prot
    }
}

#[no_mangle]
pub extern "C" fn R_ClearExternalPtr(s: Sexp) {
    if !s.is_null() {
        unsafe {
            if (*s).stype == 22 && !(*s).data.is_null() {
                (*((*s).data as *mut ExtPtrData)).ptr = ptr::null_mut();
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn R_SetExternalPtrAddr(s: Sexp, p: *mut c_void) {
    if !s.is_null() {
        unsafe {
            if (*s).stype == 22 && !(*s).data.is_null() {
                (*((*s).data as *mut ExtPtrData)).ptr = p;
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn R_RegisterCFinalizer(_s: Sexp, _fun: *const c_void) {
    // No-op — miniR doesn't have GC-triggered finalizers
}

#[no_mangle]
pub extern "C" fn R_RegisterCFinalizerEx(_s: Sexp, _fun: *const c_void, _onexit: c_int) {
    // No-op
}

// endregion

// region: R_RegisterRoutines

#[repr(C)]
pub struct RCallMethodDef {
    name: *const c_char,
    fun: *const (),
    num_args: c_int,
}

/// Wrapper for raw pointers that is Send (safe because we only access from single-threaded contexts).
#[derive(Clone, Copy)]
pub struct SendPtr(pub *const ());
unsafe impl Send for SendPtr {}

/// Registered methods — shared across all packages in this runtime.
pub static REGISTERED_CALLS: std::sync::Mutex<Vec<(String, SendPtr)>> =
    std::sync::Mutex::new(Vec::new());

#[no_mangle]
pub extern "C" fn R_registerRoutines(
    _info: *mut c_void,
    _c_methods: *const c_void,
    call_methods: *const RCallMethodDef,
    _fortran_methods: *const c_void,
    _external_methods: *const c_void,
) -> c_int {
    if !call_methods.is_null() {
        let mut reg = REGISTERED_CALLS.lock().expect("lock registered calls");
        unsafe {
            let mut i = 0;
            loop {
                let entry = &*call_methods.add(i);
                if entry.name.is_null() {
                    break;
                }
                let name = CStr::from_ptr(entry.name)
                    .to_str()
                    .unwrap_or("")
                    .to_string();
                if !name.is_empty() {
                    reg.push((name, SendPtr(entry.fun)));
                }
                i += 1;
            }
        }
    }
    1
}

#[no_mangle]
pub extern "C" fn R_useDynamicSymbols(_info: *mut c_void, _value: c_int) {}
#[no_mangle]
pub extern "C" fn R_forceSymbols(_info: *mut c_void, _value: c_int) {}

/// Look up a registered .Call method by name. Returns the function pointer or null.
pub fn find_registered_call(name: &str) -> Option<*const ()> {
    let reg = REGISTERED_CALLS.lock().expect("lock registered calls");
    reg.iter().find(|(n, _)| n == name).map(|(_, ptr)| ptr.0)
}

/// Get all registered .Call method names.
pub fn registered_call_names() -> Vec<String> {
    let reg = REGISTERED_CALLS.lock().expect("lock registered calls");
    reg.iter().map(|(n, _)| n.clone()).collect()
}

// endregion

// region: Cross-package callable registry

static CCALLABLE: std::sync::Mutex<Vec<(String, String, SendPtr)>> =
    std::sync::Mutex::new(Vec::new());

#[no_mangle]
pub extern "C" fn R_RegisterCCallable(
    package: *const c_char,
    name: *const c_char,
    fptr: *const (),
) {
    if package.is_null() || name.is_null() {
        return;
    }
    let pkg = unsafe { CStr::from_ptr(package) }
        .to_str()
        .unwrap_or("")
        .to_string();
    let nm = unsafe { CStr::from_ptr(name) }
        .to_str()
        .unwrap_or("")
        .to_string();
    let mut reg = CCALLABLE.lock().expect("lock ccallable");
    reg.push((pkg, nm, SendPtr(fptr)));
}

#[no_mangle]
pub extern "C" fn R_GetCCallable(package: *const c_char, name: *const c_char) -> *const () {
    if package.is_null() || name.is_null() {
        return ptr::null();
    }
    let pkg = unsafe { CStr::from_ptr(package) }.to_str().unwrap_or("");
    let nm = unsafe { CStr::from_ptr(name) }.to_str().unwrap_or("");
    let reg = CCALLABLE.lock().expect("lock ccallable");
    reg.iter()
        .find(|(p, n, _)| p == pkg && n == nm)
        .map(|(_, _, ptr)| ptr.0)
        .unwrap_or(ptr::null())
}

// endregion

// region: Memory allocation

#[no_mangle]
pub extern "C" fn R_alloc(nelem: usize, eltsize: c_int) -> *mut c_char {
    let bytes = nelem * eltsize as usize;
    unsafe {
        let ptr = calloc(1, bytes);
        if !ptr.is_null() {
            // Track via a dummy SEXP so _minir_free_allocs frees it
            let dummy = calloc(1, std::mem::size_of::<SexpRec>()) as Sexp;
            if !dummy.is_null() {
                (*dummy).stype = sexp::RAWSXP;
                (*dummy).data = ptr;
                (*dummy).length = bytes as i32;
                (*dummy).attrib = R_NilValue;
                track(dummy);
            }
        }
        ptr as *mut c_char
    }
}

// endregion

// region: Misc

/// Rf_lengthgets — resize a vector (copy into a new allocation).
#[no_mangle]
pub extern "C" fn Rf_lengthgets(x: Sexp, new_len: c_int) -> Sexp {
    if x.is_null() {
        return unsafe { R_NilValue };
    }
    let stype = unsafe { (*x).stype };
    let old_len = unsafe { (*x).length };
    let out = Rf_allocVector(stype as c_int, new_len as isize);
    let copy_len = std::cmp::min(old_len, new_len) as usize;
    if copy_len > 0 {
        let elem_size = match stype {
            sexp::REALSXP => 8,
            sexp::INTSXP | sexp::LGLSXP => 4,
            sexp::RAWSXP => 1,
            sexp::CPLXSXP => 16,
            sexp::STRSXP | sexp::VECSXP => std::mem::size_of::<Sexp>(),
            _ => 0,
        };
        if elem_size > 0 {
            unsafe {
                ptr::copy_nonoverlapping((*x).data, (*out).data, copy_len * elem_size);
            }
        }
    }
    // Copy attributes
    unsafe {
        (*out).attrib = (*x).attrib;
    }
    out
}

#[no_mangle]
pub extern "C" fn R_CheckUserInterrupt() {}

#[no_mangle]
pub extern "C" fn R_do_slot(obj: Sexp, name: Sexp) -> Sexp {
    Rf_getAttrib(obj, name)
}

#[no_mangle]
pub extern "C" fn Rf_nrows(x: Sexp) -> c_int {
    let dim = Rf_getAttrib(x, unsafe { R_DimSymbol });
    if !dim.is_null() && unsafe { (*dim).stype } == sexp::INTSXP && unsafe { (*dim).length } >= 1 {
        return unsafe { *((*dim).data as *const i32) };
    }
    Rf_length(x)
}

#[no_mangle]
pub extern "C" fn Rf_ncols(x: Sexp) -> c_int {
    let dim = Rf_getAttrib(x, unsafe { R_DimSymbol });
    if !dim.is_null() && unsafe { (*dim).stype } == sexp::INTSXP && unsafe { (*dim).length } >= 2 {
        return unsafe { *((*dim).data as *const i32).add(1) };
    }
    1
}

// Rf_eval — evaluate an R expression via interpreter callback.
// Handles common patterns: symbol lookup (r_sym("name")) and parsed expressions.
#[no_mangle]
pub extern "C" fn Rf_eval(expr: Sexp, _env: Sexp) -> Sexp {
    if expr.is_null() {
        return unsafe { R_NilValue };
    }

    // Convert SEXP to RValue for the callback
    let rval = unsafe { super::convert::sexp_to_rvalue(expr) };

    // Try the eval callback
    let result = STATE.with(|state| {
        let st = state.borrow();

        // For symbol lookups (most common case in init functions):
        // r_eval(r_sym("function"), base_env) → look up "function" by name
        if let Some(find) = st.callbacks.find_var {
            if let crate::interpreter::value::RValue::Vector(ref rv) = rval {
                if let crate::interpreter::value::Vector::Character(ref c) = rv.inner {
                    if c.len() == 1 {
                        if let Some(Some(name)) = c.first() {
                            return find(name).map(Ok);
                        }
                    }
                }
            }
            // SYMSXP: the name is in the data field
            unsafe {
                if (*expr).stype == sexp::SYMSXP && !(*expr).data.is_null() {
                    let name = sexp::char_data(expr);
                    if !name.is_empty() {
                        return find(name).map(Ok);
                    }
                }
            }
        }

        // For general expressions, use the eval callback
        if let Some(eval_fn) = st.callbacks.eval_expr {
            return Some(eval_fn(&rval));
        }

        None
    });

    match result {
        Some(Ok(val)) => {
            let s = super::convert::rvalue_to_sexp(&val);
            track(s);
            s
        }
        _ => unsafe { R_NilValue },
    }
}

// R_Serialize stub
#[no_mangle]
pub extern "C" fn R_Serialize(_s: Sexp, _stream: *mut c_void) {
    let _ = std::io::Write::write_all(
        &mut std::io::stderr(),
        b"Warning: R_Serialize() is a stub in miniR -- serialization from C not supported\n",
    );
}

// Rf_xlength — long vector length (same as Rf_length for non-long vecs)
#[no_mangle]
pub extern "C" fn Rf_xlength(x: Sexp) -> isize {
    Rf_length(x) as isize
}

// Rf_xlengthgets — resize using long length
#[no_mangle]
pub extern "C" fn Rf_xlengthgets(x: Sexp, new_len: isize) -> Sexp {
    Rf_lengthgets(x, new_len as c_int)
}

// Rf_mkCharLenCE — create CHARSXP with length and encoding
#[no_mangle]
pub extern "C" fn Rf_mkCharLenCE(str_ptr: *const c_char, len: c_int, _encoding: c_int) -> Sexp {
    Rf_mkCharLen(str_ptr, len)
}

// Rf_translateChar — identity (miniR is UTF-8)
#[no_mangle]
pub extern "C" fn Rf_translateChar(x: Sexp) -> *const c_char {
    if x.is_null() {
        return c"".as_ptr();
    }
    unsafe { (*x).data as *const c_char }
}

// classgets — set class attribute (alias for Rf_setAttrib with R_ClassSymbol)
#[no_mangle]
pub extern "C" fn Rf_classgets(x: Sexp, klass: Sexp) -> Sexp {
    Rf_setAttrib(x, unsafe { R_ClassSymbol }, klass);
    x
}

// namesgets — set names attribute
#[no_mangle]
pub extern "C" fn Rf_namesgets(x: Sexp, names: Sexp) -> Sexp {
    Rf_setAttrib(x, unsafe { R_NamesSymbol }, names);
    x
}

// dimgets — set dim attribute
#[no_mangle]
pub extern "C" fn Rf_dimgets(x: Sexp, dim: Sexp) -> Sexp {
    Rf_setAttrib(x, unsafe { R_DimSymbol }, dim);
    x
}

// GetRNGstate / PutRNGstate — no-ops (RNG state is in Rust)
#[no_mangle]
pub extern "C" fn GetRNGstate() {}
#[no_mangle]
pub extern "C" fn PutRNGstate() {}

// unif_rand — thread-local xorshift64 RNG
#[no_mangle]
pub extern "C" fn unif_rand() -> f64 {
    use std::cell::RefCell;
    thread_local! {
        static RNG: RefCell<u64> = const { RefCell::new(0x12345678) };
    }
    RNG.with(|rng| {
        let mut state = rng.borrow_mut();
        // xorshift64
        *state ^= *state << 13;
        *state ^= *state >> 7;
        *state ^= *state << 17;
        (*state as f64) / (u64::MAX as f64)
    })
}

// R_EmptyEnv — stub (points to NilValue)
#[no_mangle]
pub static mut R_EmptyEnv: Sexp = ptr::null_mut();

#[no_mangle]
pub static mut R_MissingArg: Sexp = ptr::null_mut();

// MARK_NOT_MUTABLE — no-op in miniR
#[no_mangle]
pub extern "C" fn MARK_NOT_MUTABLE(_x: Sexp) {}

// PRENV — promise environment (stub)
#[no_mangle]
pub extern "C" fn PRENV(_x: Sexp) -> Sexp {
    unsafe { R_NilValue }
}

// PREXPR — promise expression (stub)
#[no_mangle]
pub extern "C" fn PREXPR(_x: Sexp) -> Sexp {
    unsafe { R_NilValue }
}

// Type checking
#[no_mangle]
pub extern "C" fn Rf_isVectorAtomic(x: Sexp) -> c_int {
    if x.is_null() {
        return 0;
    }
    let t = unsafe { (*x).stype };
    matches!(
        t,
        sexp::REALSXP | sexp::INTSXP | sexp::LGLSXP | sexp::STRSXP | sexp::RAWSXP | sexp::CPLXSXP
    ) as c_int
}

#[no_mangle]
pub extern "C" fn Rf_isVectorList(x: Sexp) -> c_int {
    if x.is_null() {
        return 0;
    }
    (unsafe { (*x).stype } == sexp::VECSXP) as c_int
}

#[no_mangle]
pub extern "C" fn Rf_isMatrix(x: Sexp) -> c_int {
    let dim = Rf_getAttrib(x, unsafe { R_DimSymbol });
    (!dim.is_null() && unsafe { (*dim).stype } == sexp::INTSXP && unsafe { (*dim).length } == 2)
        as c_int
}

#[no_mangle]
pub extern "C" fn Rf_isNumeric(x: Sexp) -> c_int {
    if x.is_null() {
        return 0;
    }
    let t = unsafe { (*x).stype };
    matches!(t, sexp::REALSXP | sexp::INTSXP) as c_int
}

#[no_mangle]
pub extern "C" fn Rf_isFunction(x: Sexp) -> c_int {
    if x.is_null() {
        return 0;
    }
    let t = unsafe { (*x).stype };
    matches!(t, 3 | 7 | 8) as c_int // CLOSXP | SPECIALSXP | BUILTINSXP
}

#[no_mangle]
pub extern "C" fn Rf_isEnvironment(x: Sexp) -> c_int {
    if x.is_null() {
        return 0;
    }
    (unsafe { (*x).stype } == 4) as c_int // ENVSXP
}

// PROTECT_INDEX support
#[no_mangle]
pub extern "C" fn R_ProtectWithIndex(s: Sexp, pi: *mut c_int) {
    Rf_protect(s);
    STATE.with(|state| {
        let st = state.borrow();
        if !pi.is_null() {
            unsafe {
                *pi = (st.protect_stack.len() - 1) as c_int;
            }
        }
    });
}

#[no_mangle]
pub extern "C" fn R_Reprotect(s: Sexp, i: c_int) {
    STATE.with(|state| {
        let mut st = state.borrow_mut();
        let idx = i as usize;
        if idx < st.protect_stack.len() {
            st.protect_stack[idx] = s;
        }
    });
}

// Rf_findVar — look up a variable via interpreter callback
#[no_mangle]
pub extern "C" fn Rf_findVar(sym: Sexp, _env: Sexp) -> Sexp {
    if sym.is_null() {
        return unsafe { R_UnboundValue };
    }
    // Extract variable name from the symbol SEXP
    let name = unsafe { sexp::char_data(sym) };
    if name.is_empty() {
        return unsafe { R_UnboundValue };
    }
    // Try the interpreter callback
    let result = STATE.with(|state| {
        let st = state.borrow();
        if let Some(find) = st.callbacks.find_var {
            find(name)
        } else {
            None
        }
    });
    match result {
        Some(val) => {
            // Convert RValue back to SEXP for C code
            let s = super::convert::rvalue_to_sexp(&val);
            track(s);
            s
        }
        None => unsafe { R_UnboundValue },
    }
}

#[no_mangle]
pub extern "C" fn Rf_findVarInFrame3(env: Sexp, sym: Sexp, _inherits: c_int) -> Sexp {
    Rf_findVar(sym, env)
}

// R_ExecWithCleanup — execute function with cleanup
#[no_mangle]
pub extern "C" fn R_ExecWithCleanup(
    fun: Option<unsafe extern "C" fn(*mut c_void) -> Sexp>,
    data: *mut c_void,
    cleanup: Option<unsafe extern "C" fn(*mut c_void)>,
    cleandata: *mut c_void,
) -> Sexp {
    let result = match fun {
        Some(f) => unsafe { f(data) },
        None => unsafe { R_NilValue },
    };
    if let Some(c) = cleanup {
        unsafe {
            c(cleandata);
        }
    }
    result
}

// R_ExternalPtrAddrFn — same as R_ExternalPtrAddr but returns fn ptr
#[no_mangle]
pub extern "C" fn R_ExternalPtrAddrFn(s: Sexp) -> *mut c_void {
    R_ExternalPtrAddr(s)
}

// Rf_lang1..4 — construct language call objects
#[no_mangle]
pub extern "C" fn Rf_lang1(s: Sexp) -> Sexp {
    Rf_lcons(s, unsafe { R_NilValue })
}

#[no_mangle]
pub extern "C" fn Rf_lang2(s: Sexp, t: Sexp) -> Sexp {
    Rf_lcons(s, Rf_cons(t, unsafe { R_NilValue }))
}

#[no_mangle]
pub extern "C" fn Rf_lang3(s: Sexp, t: Sexp, u: Sexp) -> Sexp {
    Rf_lcons(s, Rf_cons(t, Rf_cons(u, unsafe { R_NilValue })))
}

#[no_mangle]
pub extern "C" fn Rf_lang4(s: Sexp, t: Sexp, u: Sexp, v: Sexp) -> Sexp {
    Rf_lcons(s, Rf_cons(t, Rf_cons(u, Rf_cons(v, unsafe { R_NilValue }))))
}

// S_alloc — same as R_alloc but zeroed (already zeroed by calloc)
#[no_mangle]
pub extern "C" fn S_alloc(nelem: isize, eltsize: c_int) -> *mut c_char {
    R_alloc(nelem as usize, eltsize)
}

// Rf_type2char — type name as string
#[no_mangle]
pub extern "C" fn Rf_type2char(stype: c_int) -> *const c_char {
    match stype as u8 {
        sexp::NILSXP => c"NULL".as_ptr(),
        sexp::LGLSXP => c"logical".as_ptr(),
        sexp::INTSXP => c"integer".as_ptr(),
        sexp::REALSXP => c"double".as_ptr(),
        sexp::CPLXSXP => c"complex".as_ptr(),
        sexp::STRSXP => c"character".as_ptr(),
        sexp::VECSXP => c"list".as_ptr(),
        sexp::RAWSXP => c"raw".as_ptr(),
        _ => c"unknown".as_ptr(),
    }
}

// R_FINITE — exported as function for packages that don't include Arith.h
#[no_mangle]
pub extern "C" fn R_finite(x: f64) -> c_int {
    x.is_finite() as c_int
}

// Rf_nchar — string length
#[no_mangle]
pub extern "C" fn Rf_nchar(
    x: Sexp,
    _ntype: c_int,
    _allow_na: c_int,
    _keep_na: c_int,
    _msg_name: *const c_char,
) -> c_int {
    if x.is_null() {
        return 0;
    }
    unsafe {
        if (*x).stype == sexp::CHARSXP && !(*x).data.is_null() {
            let s = CStr::from_ptr((*x).data as *const c_char);
            s.to_bytes().len() as c_int
        } else {
            0
        }
    }
}

// Rf_isFrame — check if data.frame
#[no_mangle]
pub extern "C" fn Rf_isFrame(x: Sexp) -> c_int {
    Rf_inherits(x, c"data.frame".as_ptr())
}

// Rf_copyMostAttrib — copy attributes from one SEXP to another
#[no_mangle]
pub extern "C" fn Rf_copyMostAttrib(from: Sexp, to: Sexp) {
    if from.is_null() || to.is_null() {
        return;
    }
    unsafe {
        (*to).attrib = (*from).attrib;
    }
}

// Rf_nthcdr — walk n steps down a pairlist
#[no_mangle]
pub extern "C" fn Rf_nthcdr(mut s: Sexp, n: c_int) -> Sexp {
    for _ in 0..n {
        if s.is_null() {
            return unsafe { R_NilValue };
        }
        unsafe {
            if (*s).stype == sexp::LISTSXP && !(*s).data.is_null() {
                s = (*((*s).data as *const PairlistData)).cdr;
            } else {
                return R_NilValue;
            }
        }
    }
    s
}

// R_FlushConsole — no-op
#[no_mangle]
pub extern "C" fn R_FlushConsole() {}

// R_do_slot_assign — slot assignment stub
#[no_mangle]
pub extern "C" fn R_do_slot_assign(obj: Sexp, name: Sexp, val: Sexp) {
    Rf_setAttrib(obj, name, val);
}

// Rf_allocList — allocate a pairlist of n nodes
#[no_mangle]
pub extern "C" fn Rf_allocList(n: c_int) -> Sexp {
    let mut result = unsafe { R_NilValue };
    for _ in 0..n {
        result = Rf_cons(unsafe { R_NilValue }, result);
    }
    result
}

// Rf_match — match values (stub returns vector of nomatch)
#[no_mangle]
pub extern "C" fn Rf_match(_table: Sexp, x: Sexp, nomatch: c_int) -> Sexp {
    let n = if x.is_null() {
        0
    } else {
        (unsafe { (*x).length }) as isize
    };
    let result = Rf_allocVector(sexp::INTSXP as c_int, n);
    if n > 0 {
        unsafe {
            let ptr = (*result).data as *mut i32;
            for i in 0..n as usize {
                *ptr.add(i) = nomatch;
            }
        }
    }
    result
}

// Rf_asCharacterFactor — convert factor to character (stub)
#[no_mangle]
pub extern "C" fn Rf_asCharacterFactor(_x: Sexp) -> Sexp {
    Rf_allocVector(sexp::STRSXP as c_int, 0)
}

// R_isort — integer sort (in-place)
#[no_mangle]
pub extern "C" fn R_isort(x: *mut c_int, n: c_int) {
    if x.is_null() || n <= 0 {
        return;
    }
    let slice = unsafe { std::slice::from_raw_parts_mut(x, n as usize) };
    slice.sort_unstable();
}

// R_rsort — double sort (in-place)
#[no_mangle]
pub extern "C" fn R_rsort(x: *mut f64, n: c_int) {
    if x.is_null() || n <= 0 {
        return;
    }
    let slice = unsafe { std::slice::from_raw_parts_mut(x, n as usize) };
    slice.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
}

// Rf_allocArray — allocate array with dimensions
#[no_mangle]
pub extern "C" fn Rf_allocArray(stype: c_int, dims: Sexp) -> Sexp {
    // Compute total size from dims vector
    let total = if dims.is_null() {
        0
    } else {
        let n = unsafe { (*dims).length } as usize;
        let mut product: isize = 1;
        for i in 0..n {
            let d = unsafe { *((*dims).data as *const i32).add(i) } as isize;
            product *= d;
        }
        product
    };
    let result = Rf_allocVector(stype, total);
    Rf_setAttrib(result, unsafe { R_DimSymbol }, dims);
    result
}

// R_do_MAKE_CLASS — create an S4 class object (stub)
#[no_mangle]
pub extern "C" fn R_do_MAKE_CLASS(_name: *const c_char) -> Sexp {
    Rf_allocVector(sexp::VECSXP as c_int, 0)
}

// iPsort / rPsort — partial sort (sort enough to get k-th element)
#[no_mangle]
pub extern "C" fn iPsort(x: *mut c_int, n: c_int, _k: c_int) {
    R_isort(x, n); // full sort as fallback
}
#[no_mangle]
pub extern "C" fn rPsort(x: *mut f64, n: c_int, _k: c_int) {
    R_rsort(x, n);
}

// Rf_isPrimitive
#[no_mangle]
pub extern "C" fn Rf_isPrimitive(x: Sexp) -> c_int {
    if x.is_null() {
        return 0;
    }
    let t = unsafe { (*x).stype };
    matches!(t, 7 | 8) as c_int // SPECIALSXP | BUILTINSXP
}

// Rf_isSymbol
#[no_mangle]
pub extern "C" fn Rf_isSymbol(x: Sexp) -> c_int {
    if x.is_null() {
        return 0;
    }
    (unsafe { (*x).stype } == sexp::SYMSXP) as c_int
}

// Rf_lang5/6
#[no_mangle]
pub extern "C" fn Rf_lang5(s: Sexp, t: Sexp, u: Sexp, v: Sexp, w: Sexp) -> Sexp {
    Rf_lcons(
        s,
        Rf_cons(t, Rf_cons(u, Rf_cons(v, Rf_cons(w, unsafe { R_NilValue })))),
    )
}

#[no_mangle]
pub extern "C" fn Rf_lang6(s: Sexp, t: Sexp, u: Sexp, v: Sexp, w: Sexp, x: Sexp) -> Sexp {
    Rf_lcons(
        s,
        Rf_cons(
            t,
            Rf_cons(u, Rf_cons(v, Rf_cons(w, Rf_cons(x, unsafe { R_NilValue })))),
        ),
    )
}

// Rf_findFun — find a function (stub — delegates to Rf_findVar)
#[no_mangle]
pub extern "C" fn Rf_findFun(sym: Sexp, env: Sexp) -> Sexp {
    Rf_findVar(sym, env)
}

// R_tryEval — evaluate with error flag (stub delegates to Rf_eval)
#[no_mangle]
pub extern "C" fn R_tryEval(expr: Sexp, env: Sexp, error_occurred: *mut c_int) -> Sexp {
    let result = Rf_eval(expr, env);
    if !error_occurred.is_null() {
        unsafe {
            *error_occurred = 0;
        }
    }
    result
}

// R_tryEvalSilent — same as R_tryEval (no error printing)
#[no_mangle]
pub extern "C" fn R_tryEvalSilent(expr: Sexp, env: Sexp, error_occurred: *mut c_int) -> Sexp {
    R_tryEval(expr, env, error_occurred)
}

// R_ToplevelExec — execute function, return 1 (TRUE) on success
#[no_mangle]
pub extern "C" fn R_ToplevelExec(
    fun: Option<unsafe extern "C" fn(*mut c_void)>,
    data: *mut c_void,
) -> c_int {
    if let Some(f) = fun {
        unsafe {
            f(data);
        }
    }
    1 // TRUE — success
}

// Rf_mkNamed — allocate a named VECSXP/list
#[no_mangle]
pub extern "C" fn Rf_mkNamed(stype: c_int, names: *const *const c_char) -> Sexp {
    // Count names (null-terminated array, terminated by "" entry)
    let mut n: usize = 0;
    if !names.is_null() {
        unsafe {
            loop {
                let name_ptr = *names.add(n);
                if name_ptr.is_null() || *name_ptr == 0 {
                    break;
                }
                n += 1;
            }
        }
    }

    let vec = Rf_protect(Rf_allocVector(stype, n as isize));
    let names_vec = Rf_protect(Rf_allocVector(sexp::STRSXP as c_int, n as isize));
    for i in 0..n {
        unsafe {
            let name_ptr = *names.add(i);
            let ch = Rf_mkChar(name_ptr);
            let elts = (*names_vec).data as *mut Sexp;
            *elts.add(i) = ch;
        }
    }
    Rf_setAttrib(vec, unsafe { R_NamesSymbol }, names_vec);
    Rf_unprotect(2);
    vec
}

// Rf_isLanguage — check if LANGSXP
#[no_mangle]
pub extern "C" fn Rf_isLanguage(x: Sexp) -> c_int {
    if x.is_null() {
        return 0;
    }
    (unsafe { (*x).stype } == 6) as c_int // LANGSXP
}

// R_ExpandFileName — return filename unchanged (no tilde expansion)
#[no_mangle]
pub extern "C" fn R_ExpandFileName(fn_ptr: *const c_char) -> *const c_char {
    fn_ptr
}

// R_chk_calloc — checked calloc (delegates to system calloc)
#[no_mangle]
pub extern "C" fn R_chk_calloc(nelem: usize, elsize: usize) -> *mut c_void {
    unsafe { calloc(nelem, elsize) as *mut c_void }
}

// R_chk_realloc — checked realloc (delegates to system realloc)
#[no_mangle]
pub extern "C" fn R_chk_realloc(ptr: *mut c_void, size: usize) -> *mut c_void {
    unsafe { realloc(ptr as *mut u8, size) as *mut c_void }
}

// R_chk_free — checked free (delegates to system free)
#[no_mangle]
pub extern "C" fn R_chk_free(ptr: *mut c_void) {
    unsafe {
        free(ptr as *mut u8);
    }
}

// R_removeVarFromFrame — no-op (variable removal not supported from C)
#[no_mangle]
pub extern "C" fn R_removeVarFromFrame(_sym: Sexp, _env: Sexp) {}

// Rf_allocS4Object — stub: allocate a NILSXP
#[no_mangle]
pub extern "C" fn Rf_allocS4Object() -> Sexp {
    Rf_allocVector(sexp::NILSXP as c_int, 0)
}

// rlang stubs
#[no_mangle]
pub extern "C" fn R_CheckStack2(_extra: c_int) {}
#[no_mangle]
pub extern "C" fn R_MakeActiveBinding(_sym: Sexp, _fun: Sexp, _env: Sexp) {}
#[no_mangle]
pub extern "C" fn R_MakeExternalPtrFn(p: *const (), tag: Sexp, prot: Sexp) -> Sexp {
    R_MakeExternalPtr(p as *mut c_void, tag, prot)
}
#[no_mangle]
pub extern "C" fn Rf_allocSExp(stype: c_int) -> Sexp {
    Rf_allocVector(stype, 0)
}
#[no_mangle]
pub extern "C" fn Rf_any_duplicated(_x: Sexp, _from_last: c_int) -> isize {
    0
}
#[no_mangle]
pub extern "C" fn Rf_countContexts(_type: c_int, _subtype: c_int) -> c_int {
    0
}
#[no_mangle]
pub extern "C" fn R_PromiseExpr(_p: Sexp) -> Sexp {
    unsafe { R_NilValue }
}
#[no_mangle]
pub extern "C" fn R_ClosureFormals(_x: Sexp) -> Sexp {
    unsafe { R_NilValue }
}
#[no_mangle]
pub extern "C" fn R_ClosureBody(_x: Sexp) -> Sexp {
    unsafe { R_NilValue }
}
#[no_mangle]
pub extern "C" fn R_ClosureEnv(_x: Sexp) -> Sexp {
    unsafe { R_NilValue }
}
#[no_mangle]
pub extern "C" fn R_compute_identical(x: Sexp, y: Sexp, _flags: c_int) -> c_int {
    if x == y {
        return 1;
    }
    if x.is_null() || y.is_null() {
        return 0;
    }
    unsafe {
        if (*x).stype != (*y).stype {
            return 0;
        }
        if (*x).length != (*y).length {
            return 0;
        }
        let len = (*x).length as usize;
        if len == 0 {
            return 1;
        }
        // Compare data bytes directly for numeric/logical/raw/complex types
        let elem_size = match (*x).stype {
            sexp::REALSXP => 8,
            sexp::INTSXP => 4,
            sexp::LGLSXP => 4,
            sexp::RAWSXP => 1,
            sexp::CPLXSXP => 16,
            _ => 0,
        };
        if elem_size > 0 && !(*x).data.is_null() && !(*y).data.is_null() {
            let bytes = len * elem_size;
            return (std::ptr::eq((*x).data, (*y).data)
                || std::slice::from_raw_parts((*x).data, bytes)
                    == std::slice::from_raw_parts((*y).data, bytes)) as c_int;
        }
        // For STRSXP/VECSXP, compare element by element
        if (*x).stype == sexp::STRSXP || (*x).stype == sexp::VECSXP {
            let ex = (*x).data as *const Sexp;
            let ey = (*y).data as *const Sexp;
            for i in 0..len {
                if R_compute_identical(*ex.add(i), *ey.add(i), _flags) == 0 {
                    return 0;
                }
            }
            return 1;
        }
        // CHARSXP: compare string data
        if (*x).stype == sexp::CHARSXP {
            if (*x).data.is_null() && (*y).data.is_null() {
                return 1;
            }
            if (*x).data.is_null() || (*y).data.is_null() {
                return 0;
            }
            let sx = CStr::from_ptr((*x).data as *const c_char);
            let sy = CStr::from_ptr((*y).data as *const c_char);
            return (sx == sy) as c_int;
        }
        0
    }
}
#[no_mangle]
pub extern "C" fn R_envHasNoSpecialSymbols(_env: Sexp) -> c_int {
    1
}
#[no_mangle]
pub extern "C" fn R_OrderVector1(
    _indx: *mut c_int,
    _n: c_int,
    _x: Sexp,
    _nalast: c_int,
    _decreasing: c_int,
) {
}
#[no_mangle]
pub extern "C" fn SET_PRENV(_x: Sexp, _v: Sexp) {}
#[no_mangle]
pub extern "C" fn SET_PRCODE(_x: Sexp, _v: Sexp) {}
#[no_mangle]
pub extern "C" fn SET_PRVALUE(_x: Sexp, _v: Sexp) {}
#[no_mangle]
pub extern "C" fn PRCODE(_x: Sexp) -> Sexp {
    unsafe { R_NilValue }
}
#[no_mangle]
pub extern "C" fn PRVALUE(_x: Sexp) -> Sexp {
    unsafe { R_NilValue }
}

// Active bindings
#[no_mangle]
pub extern "C" fn R_BindingIsActive(_sym: Sexp, _env: Sexp) -> c_int {
    0
}
#[no_mangle]
pub extern "C" fn R_ActiveBindingFunction(_sym: Sexp, _env: Sexp) -> Sexp {
    unsafe { R_NilValue }
}
#[no_mangle]
pub extern "C" fn Rf_onintr() {}

// Symbol constants
#[no_mangle]
pub static mut R_BraceSymbol: Sexp = ptr::null_mut();
#[no_mangle]
pub static mut R_BracketSymbol: Sexp = ptr::null_mut();
#[no_mangle]
pub static mut R_Bracket2Symbol: Sexp = ptr::null_mut();
#[no_mangle]
pub static mut R_DoubleColonSymbol: Sexp = ptr::null_mut();
#[no_mangle]
pub static mut R_TripleColonSymbol: Sexp = ptr::null_mut();
#[no_mangle]
pub static mut R_Interactive: c_int = 0;

// Rf_type2str — SEXPTYPE to CHARSXP
#[no_mangle]
pub extern "C" fn Rf_type2str(stype: c_int) -> Sexp {
    Rf_mkChar(Rf_type2char(stype))
}

// Weak references
#[no_mangle]
pub extern "C" fn R_MakeWeakRef(key: Sexp, val: Sexp, _fin: Sexp, _onexit: c_int) -> Sexp {
    Rf_cons(key, Rf_cons(val, unsafe { R_NilValue }))
}
#[no_mangle]
pub extern "C" fn R_MakeWeakRefC(key: Sexp, val: Sexp, _fin: *const (), onexit: c_int) -> Sexp {
    R_MakeWeakRef(key, val, unsafe { R_NilValue }, onexit)
}
#[no_mangle]
pub extern "C" fn R_WeakRefKey(_w: Sexp) -> Sexp {
    unsafe { R_NilValue }
}
#[no_mangle]
pub extern "C" fn R_WeakRefValue(_w: Sexp) -> Sexp {
    unsafe { R_NilValue }
}
#[no_mangle]
pub extern "C" fn Rf_duplicated(_x: Sexp, _from_last: c_int) -> Sexp {
    Rf_allocVector(sexp::LGLSXP as c_int, 0)
}
#[no_mangle]
pub extern "C" fn Rf_any_duplicated3(_x: Sexp, _incomp: Sexp, _from_last: c_int) -> isize {
    0
}
#[no_mangle]
pub extern "C" fn Rf_reEnc(
    x: *const c_char,
    _ce_in: c_int,
    _ce_out: c_int,
    _subst: c_int,
) -> *const c_char {
    x
}
#[no_mangle]
pub extern "C" fn Rf_ucstoutf8(buf: *mut c_char, _wc: u32) -> *const c_char {
    buf as *const c_char
}
#[no_mangle]
pub extern "C" fn SET_BODY(_x: Sexp, _v: Sexp) {}
#[no_mangle]
pub extern "C" fn SET_FORMALS(_x: Sexp, _v: Sexp) {}
#[no_mangle]
pub extern "C" fn SET_CLOENV(_x: Sexp, _v: Sexp) {}
#[no_mangle]
pub static mut R_NamespaceRegistry: Sexp = ptr::null_mut();
#[no_mangle]
pub static mut R_Srcref: Sexp = ptr::null_mut();
#[no_mangle]
pub static mut R_BaseNamespace: Sexp = ptr::null_mut();
#[no_mangle]
pub extern "C" fn R_EnvironmentIsLocked(_env: Sexp) -> c_int {
    0
}

// Rf_installChar — install symbol from CHARSXP
#[no_mangle]
pub extern "C" fn Rf_installChar(x: Sexp) -> Sexp {
    if x.is_null() {
        return unsafe { R_NilValue };
    }
    let name = unsafe { sexp::char_data(x) };
    Rf_install(name.as_ptr() as *const c_char)
}
// Rf_ScalarRaw — scalar raw vector
#[no_mangle]
pub extern "C" fn Rf_ScalarRaw(x: u8) -> Sexp {
    let s = Rf_allocVector(sexp::RAWSXP as c_int, 1);
    unsafe {
        *(*s).data = x;
    }
    s
}

// Rf_shallow_duplicate — shallow copy (same as duplicate for our purposes)
#[no_mangle]
pub extern "C" fn Rf_shallow_duplicate(x: Sexp) -> Sexp {
    Rf_duplicate(x)
}

// R_NewEnv — create a new environment
#[no_mangle]
pub extern "C" fn R_NewEnv(parent: Sexp, _hash: c_int, _size: c_int) -> Sexp {
    // Stub — returns a list that acts as a pseudo-env
    // Real implementation would need interpreter callback
    let _ = parent;
    Rf_allocVector(sexp::VECSXP as c_int, 0)
}

// Rf_defineVar — define a variable via interpreter callback
#[no_mangle]
pub extern "C" fn Rf_defineVar(sym: Sexp, val: Sexp, _env: Sexp) {
    if sym.is_null() {
        return;
    }
    let name = unsafe { sexp::char_data(sym) };
    if name.is_empty() {
        return;
    }
    let rval = unsafe { super::convert::sexp_to_rvalue(val) };
    STATE.with(|state| {
        let st = state.borrow();
        if let Some(define) = st.callbacks.define_var {
            define(name, rval);
        }
    });
}

// BODY / CLOENV — closure internals
#[no_mangle]
pub extern "C" fn BODY(_x: Sexp) -> Sexp {
    unsafe { R_NilValue }
}

#[no_mangle]
pub extern "C" fn CLOENV(_x: Sexp) -> Sexp {
    unsafe { R_NilValue }
}

#[no_mangle]
pub extern "C" fn FORMALS(_x: Sexp) -> Sexp {
    unsafe { R_NilValue }
}

// Rf_isObject — check if object has a class attribute
#[no_mangle]
pub extern "C" fn Rf_isObject(x: Sexp) -> c_int {
    if x.is_null() {
        return 0;
    }
    let klass = Rf_getAttrib(x, unsafe { R_ClassSymbol });
    (!klass.is_null() && unsafe { (*klass).stype } == sexp::STRSXP) as c_int
}

// Rf_str2type — string to SEXPTYPE
#[no_mangle]
pub extern "C" fn Rf_str2type(s: *const c_char) -> c_int {
    if s.is_null() {
        return -1;
    }
    let name = unsafe { CStr::from_ptr(s) }.to_str().unwrap_or("");
    match name {
        "NULL" => sexp::NILSXP as c_int,
        "logical" => sexp::LGLSXP as c_int,
        "integer" => sexp::INTSXP as c_int,
        "double" | "numeric" => sexp::REALSXP as c_int,
        "complex" => sexp::CPLXSXP as c_int,
        "character" => sexp::STRSXP as c_int,
        "list" => sexp::VECSXP as c_int,
        "raw" => sexp::RAWSXP as c_int,
        _ => -1,
    }
}

#[repr(C)]
pub struct Rcomplex {
    r: f64,
    i: f64,
}

// Rf_ScalarComplex
#[no_mangle]
pub extern "C" fn Rf_ScalarComplex(c: Rcomplex) -> Sexp {
    let s = Rf_allocVector(sexp::CPLXSXP as c_int, 1);
    unsafe {
        let ptr = (*s).data as *mut Rcomplex;
        *ptr = c;
    }
    s
}

// Environment internals
#[no_mangle]
pub extern "C" fn ENCLOS(_x: Sexp) -> Sexp {
    unsafe { R_NilValue }
}
#[no_mangle]
pub extern "C" fn R_existsVarInFrame(_env: Sexp, _sym: Sexp) -> c_int {
    0
}
#[no_mangle]
pub extern "C" fn R_IsNamespaceEnv(_env: Sexp) -> c_int {
    0
}
#[no_mangle]
pub extern "C" fn R_lsInternal3(_env: Sexp, _all: c_int, _sorted: c_int) -> Sexp {
    Rf_allocVector(sexp::STRSXP as c_int, 0)
}
#[no_mangle]
pub extern "C" fn R_ClosureExpr(_x: Sexp) -> Sexp {
    unsafe { R_NilValue }
}
#[no_mangle]
pub extern "C" fn R_ParentEnv(_env: Sexp) -> Sexp {
    unsafe { R_NilValue }
}
#[no_mangle]
pub extern "C" fn R_LockBinding(_sym: Sexp, _env: Sexp) {}
#[no_mangle]
pub extern "C" fn SET_FRAME(_x: Sexp, _v: Sexp) {}
#[no_mangle]
pub extern "C" fn SET_ENCLOS(_x: Sexp, _v: Sexp) {}
#[no_mangle]
pub extern "C" fn SET_HASHTAB(_x: Sexp, _v: Sexp) {}
#[no_mangle]
pub extern "C" fn R_BindingIsLocked(_sym: Sexp, _env: Sexp) -> c_int {
    0
}
#[no_mangle]
pub extern "C" fn R_NamespaceEnvSpec(_ns: Sexp) -> Sexp {
    unsafe { R_NilValue }
}
#[no_mangle]
pub extern "C" fn R_FindNamespace(_name: Sexp) -> Sexp {
    unsafe { R_NilValue }
}
#[no_mangle]
pub extern "C" fn R_IsPackageEnv(_env: Sexp) -> c_int {
    0
}
#[no_mangle]
pub extern "C" fn R_PackageEnvName(_env: Sexp) -> Sexp {
    unsafe { R_NilValue }
}

// Rf_findVarInFrame — variable lookup (stub)
#[no_mangle]
pub extern "C" fn Rf_findVarInFrame(env: Sexp, sym: Sexp) -> Sexp {
    Rf_findVarInFrame3(env, sym, 1)
}

// Rf_GetOption1 — get option value (stub)
#[no_mangle]
pub extern "C" fn Rf_GetOption1(_tag: Sexp) -> Sexp {
    unsafe { R_NilValue }
}

// S_realloc — reallocate and zero-fill new portion
#[no_mangle]
pub extern "C" fn S_realloc(
    ptr: *mut c_char,
    new_size: isize,
    old_size: isize,
    elt_size: c_int,
) -> *mut c_char {
    extern "C" {
        fn realloc(ptr: *mut u8, size: usize) -> *mut u8;
    }
    let new_bytes = new_size as usize * elt_size as usize;
    let old_bytes = old_size as usize * elt_size as usize;
    unsafe {
        let new_ptr = realloc(ptr as *mut u8, new_bytes);
        if !new_ptr.is_null() && new_bytes > old_bytes {
            ptr::write_bytes(new_ptr.add(old_bytes), 0, new_bytes - old_bytes);
        }
        new_ptr as *mut c_char
    }
}

// norm_rand — Box-Muller transform using unif_rand()
#[no_mangle]
pub extern "C" fn norm_rand() -> f64 {
    let u1 = unif_rand();
    let u2 = unif_rand();
    (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
}

// exp_rand — exponential deviate using unif_rand()
#[no_mangle]
pub extern "C" fn exp_rand() -> f64 {
    -unif_rand().ln()
}

// R_ParseVector — parse R source text via interpreter callback.
#[no_mangle]
pub extern "C" fn R_ParseVector(
    text: Sexp,
    _n: c_int,
    parse_status: *mut c_int,
    _srcfile: Sexp,
) -> Sexp {
    // Extract the text string from the SEXP
    let src = if text.is_null() {
        String::new()
    } else {
        unsafe {
            if (*text).stype == sexp::STRSXP && (*text).length > 0 {
                let elt = *((*text).data as *const Sexp);
                if !elt.is_null() {
                    sexp::char_data(elt).to_string()
                } else {
                    String::new()
                }
            } else if (*text).stype == sexp::CHARSXP {
                sexp::char_data(text).to_string()
            } else {
                String::new()
            }
        }
    };

    if src.is_empty() {
        if !parse_status.is_null() {
            unsafe {
                *parse_status = 1;
            } // PARSE_OK
        }
        return unsafe { R_NilValue };
    }

    // Try the parse callback
    let result = STATE.with(|state| {
        let st = state.borrow();
        st.callbacks.parse_text.map(|parse_fn| parse_fn(&src))
    });

    match result {
        Some(Ok(val)) => {
            if !parse_status.is_null() {
                unsafe {
                    *parse_status = 1;
                } // PARSE_OK
            }
            // Wrap in a length-1 VECSXP — r_parse() extracts element 0
            let expr_sexp = super::convert::rvalue_to_sexp(&val);
            track(expr_sexp);
            let list = Rf_allocVector(sexp::VECSXP as c_int, 1);
            unsafe {
                let elts = (*list).data as *mut Sexp;
                *elts = expr_sexp;
            }
            list
        }
        Some(Err(_)) => {
            if !parse_status.is_null() {
                unsafe {
                    *parse_status = 3;
                } // PARSE_ERROR
            }
            unsafe { R_NilValue }
        }
        None => {
            // No callback — return NilValue but mark as OK
            if !parse_status.is_null() {
                unsafe {
                    *parse_status = 1;
                }
            }
            unsafe { R_NilValue }
        }
    }
}

// Fortran optimization stubs (called by MASS, nnet, class, etc.)
#[no_mangle]
pub extern "C" fn nmmin(
    _n: c_int,
    _xin: *mut f64,
    _x: *mut f64,
    _fmin: *mut f64,
    _fn_ptr: *const (),
    _fail: *mut c_int,
    _abstol: f64,
    _intol: f64,
    _ex: *mut c_void,
    _alpha: f64,
    _beta: f64,
    _gamma: f64,
    _trace: c_int,
    _fncount: *mut c_int,
    _maxit: c_int,
) {
    let _ = std::io::Write::write_all(
        &mut std::io::stderr(),
        b"Warning: nmmin() is a stub in miniR -- results will be incorrect\n",
    );
}

#[no_mangle]
pub extern "C" fn vmmin(
    _n: c_int,
    _x: *mut f64,
    _fmin: *mut f64,
    _fn_ptr: *const (),
    _gr: *const (),
    _maxit: c_int,
    _trace: c_int,
    _mask: *mut c_int,
    _abstol: f64,
    _reltol: f64,
    _nreport: c_int,
    _ex: *mut c_void,
    _fncount: *mut c_int,
    _grcount: *mut c_int,
    _fail: *mut c_int,
) {
    let _ = std::io::Write::write_all(
        &mut std::io::stderr(),
        b"Warning: vmmin() is a stub in miniR -- results will be incorrect\n",
    );
}

// endregion

// region: Cleanup

/// Free all tracked allocations (called by Rust after .Call).
/// Persistent SEXPs (external pointers, flags=1) are kept alive.
pub fn free_allocs() {
    STATE.with(|state| {
        let mut st = state.borrow_mut();
        let mut node = st.alloc_head;
        let mut persistent_head: *mut AllocNode = ptr::null_mut();

        // First pass: free data, separate persistent
        while !node.is_null() {
            let next = unsafe { (*node).next };
            let s = unsafe { (*node).sexp };
            if !s.is_null() && unsafe { (*s).flags } == 1 {
                // Persistent — keep
                unsafe {
                    (*node).next = persistent_head;
                }
                persistent_head = node;
            } else if !s.is_null() {
                unsafe {
                    if !(*s).data.is_null() {
                        free((*s).data);
                    }
                    free(s as *mut u8);
                }
                unsafe {
                    drop(Box::from_raw(node));
                }
            } else {
                unsafe {
                    drop(Box::from_raw(node));
                }
            }
            node = next;
        }

        st.alloc_head = persistent_head;
        st.protect_stack.clear();
    });
}

// endregion
