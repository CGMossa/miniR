//! SEXP type definitions — the C-compatible memory layout for R values.
//!
//! This defines miniR's own SEXP ABI. Package C code compiled against our
//! `Rinternals.h` header uses this layout to access R values.
//!
//! All allocations use the C allocator (`calloc`/`malloc`/`free`) so that
//! SEXPs created by Rust and SEXPs created by C code are interchangeable
//! and can be freed by either side.

use std::ffi::CStr;
use std::os::raw::c_char;

// region: C allocator FFI

extern "C" {
    fn calloc(count: usize, size: usize) -> *mut u8;
    fn malloc(size: usize) -> *mut u8;
    fn free(ptr: *mut u8);
}

// endregion

// region: SEXPTYPE constants (matching GNU R)

pub const NILSXP: u8 = 0;
pub const SYMSXP: u8 = 1;
pub const LISTSXP: u8 = 2;
pub const CHARSXP: u8 = 9;
pub const LGLSXP: u8 = 10;
pub const INTSXP: u8 = 13;
pub const REALSXP: u8 = 14;
pub const CPLXSXP: u8 = 15;
pub const STRSXP: u8 = 16;
pub const VECSXP: u8 = 19;
pub const RAWSXP: u8 = 24;

// endregion

// region: NA sentinel values

/// R's NA_REAL — a specific NaN with payload 1954 (0x7FF00000000007A2).
pub const NA_REAL: f64 = f64::from_bits(0x7FF00000000007A2);
/// R's NA_INTEGER — i32::MIN.
pub const NA_INTEGER: i32 = i32::MIN;
/// R's NA_LOGICAL — same as NA_INTEGER.
pub const NA_LOGICAL: i32 = i32::MIN;

/// Check if a f64 is R's NA_REAL (not just any NaN).
pub fn is_na_real(x: f64) -> bool {
    x.to_bits() == NA_REAL.to_bits()
}

// endregion

// region: SEXPREC struct

/// A heap-allocated R value in C-compatible layout.
///
/// This struct is read/written by both Rust (for conversion) and C code
/// (via the accessor macros in `Rinternals.h`). The layout must match
/// the C struct exactly.
#[repr(C)]
pub struct SexpRec {
    /// SEXPTYPE tag (REALSXP=14, INTSXP=13, etc.)
    pub stype: u8,
    /// Flags (currently unused — reserved for GC marks, NAMED count)
    pub flags: u8,
    /// Padding for alignment
    pub padding: u16,
    /// Vector length
    pub length: i32,
    /// Pointer to the data buffer (type depends on stype).
    pub data: *mut u8,
    /// Attributes (NULL for no attributes). Currently unused.
    pub attrib: *mut SexpRec,
}

/// The SEXP pointer type — equivalent to C's `SEXP`.
pub type Sexp = *mut SexpRec;

/// Pairlist node data — matches `minir_pairlist_data` in Rinternals.h.
/// Used by LISTSXP/LANGSXP nodes. Stored at `data` pointer of the SexpRec.
#[repr(C)]
pub struct PairlistData {
    pub car: Sexp,
    pub cdr: Sexp,
    pub tag: Sexp,
}

/// Null SEXP sentinel.
pub const R_NIL_VALUE: Sexp = std::ptr::null_mut();

// endregion

// region: Allocation (using C allocator for compatibility with Rinternals.h)

/// Allocate a SEXPREC with a typed data buffer using the C allocator.
pub fn alloc_vector(stype: u8, length: i32) -> Sexp {
    unsafe {
        let rec = calloc(1, std::mem::size_of::<SexpRec>()) as Sexp;
        if rec.is_null() {
            return R_NIL_VALUE;
        }
        (*rec).stype = stype;
        (*rec).length = length;
        (*rec).attrib = R_NIL_VALUE;

        if length > 0 {
            let len = length as usize;
            (*rec).data = match stype {
                REALSXP => calloc(len, std::mem::size_of::<f64>()),
                INTSXP | LGLSXP => calloc(len, std::mem::size_of::<i32>()),
                // Rcomplex = { double r, i } = 16 bytes
                CPLXSXP => calloc(len, 2 * std::mem::size_of::<f64>()),
                STRSXP | VECSXP => calloc(len, std::mem::size_of::<Sexp>()),
                RAWSXP => calloc(len, 1),
                _ => std::ptr::null_mut(),
            };
        }

        rec
    }
}

/// Allocate a CHARSXP from a Rust string.
pub fn mk_char(s: &str) -> Sexp {
    unsafe {
        let rec = calloc(1, std::mem::size_of::<SexpRec>()) as Sexp;
        if rec.is_null() {
            return R_NIL_VALUE;
        }
        (*rec).stype = CHARSXP;
        (*rec).length = s.len() as i32;

        let buf = malloc(s.len() + 1);
        if !buf.is_null() {
            std::ptr::copy_nonoverlapping(s.as_ptr(), buf, s.len());
            *buf.add(s.len()) = 0; // null terminator
        }
        (*rec).data = buf;
        (*rec).attrib = R_NIL_VALUE;

        rec
    }
}

/// Allocate a scalar REALSXP.
pub fn scalar_real(x: f64) -> Sexp {
    let s = alloc_vector(REALSXP, 1);
    if !s.is_null() {
        unsafe { *((*s).data as *mut f64) = x };
    }
    s
}

/// Allocate a scalar INTSXP.
pub fn scalar_integer(x: i32) -> Sexp {
    let s = alloc_vector(INTSXP, 1);
    if !s.is_null() {
        unsafe { *((*s).data as *mut i32) = x };
    }
    s
}

/// Allocate a scalar LGLSXP.
pub fn scalar_logical(x: i32) -> Sexp {
    let s = alloc_vector(LGLSXP, 1);
    if !s.is_null() {
        unsafe { *((*s).data as *mut i32) = x };
    }
    s
}

/// Allocate a length-1 STRSXP from a Rust string.
pub fn mk_string(s: &str) -> Sexp {
    let strsxp = alloc_vector(STRSXP, 1);
    let charsxp = mk_char(s);
    if !strsxp.is_null() {
        unsafe {
            let elts = (*strsxp).data as *mut Sexp;
            *elts = charsxp;
        }
    }
    strsxp
}

/// Allocate a NILSXP.
pub fn mk_null() -> Sexp {
    unsafe {
        let rec = calloc(1, std::mem::size_of::<SexpRec>()) as Sexp;
        if rec.is_null() {
            return R_NIL_VALUE;
        }
        (*rec).stype = NILSXP;
        rec
    }
}

// endregion

// region: Accessors (Rust side)

/// Read the CHARSXP data as a Rust &str.
///
/// # Safety
/// `s` must be a valid CHARSXP pointer.
pub unsafe fn char_data(s: Sexp) -> &'static str {
    if s.is_null() || (*s).data.is_null() {
        return "";
    }
    let cstr = CStr::from_ptr((*s).data as *const c_char);
    cstr.to_str().unwrap_or("")
}

/// Read REAL data pointer.
///
/// # Safety
/// `s` must be a valid REALSXP pointer.
pub unsafe fn real_ptr(s: Sexp) -> *mut f64 {
    (*s).data as *mut f64
}

/// Read INTEGER data pointer.
///
/// # Safety
/// `s` must be a valid INTSXP pointer.
pub unsafe fn integer_ptr(s: Sexp) -> *mut i32 {
    (*s).data as *mut i32
}

/// Read LOGICAL data pointer.
///
/// # Safety
/// `s` must be a valid LGLSXP pointer.
pub unsafe fn logical_ptr(s: Sexp) -> *mut i32 {
    (*s).data as *mut i32
}

/// Read STRING_ELT.
///
/// # Safety
/// `s` must be a valid STRSXP pointer, `i` must be in bounds.
pub unsafe fn string_elt(s: Sexp, i: usize) -> Sexp {
    let elts = (*s).data as *const Sexp;
    *elts.add(i)
}

/// Read VECTOR_ELT.
///
/// # Safety
/// `s` must be a valid VECSXP pointer, `i` must be in bounds.
pub unsafe fn vector_elt(s: Sexp, i: usize) -> Sexp {
    let elts = (*s).data as *const Sexp;
    *elts.add(i)
}

// endregion

// region: Deallocation

/// Free a SEXP and its data buffer using the C allocator.
///
/// # Safety
/// `s` must have been allocated by `alloc_vector`, `mk_char`, `mk_null`,
/// or by the C runtime in `Rinternals.h` (which also uses calloc/malloc).
/// Must not be called twice on the same pointer.
pub unsafe fn free_sexp(s: Sexp) {
    if s.is_null() {
        return;
    }
    let rec = &*s;
    if !rec.data.is_null() {
        let len = rec.length.max(0) as usize;
        match rec.stype {
            STRSXP => {
                // Free each CHARSXP element first
                let elts = rec.data as *mut Sexp;
                for i in 0..len {
                    let elt = *elts.add(i);
                    if !elt.is_null() {
                        free_sexp(elt);
                    }
                }
                free(rec.data);
            }
            VECSXP => {
                // Free each list element first
                let elts = rec.data as *mut Sexp;
                for i in 0..len {
                    let elt = *elts.add(i);
                    if !elt.is_null() {
                        free_sexp(elt);
                    }
                }
                free(rec.data);
            }
            _ => {
                // REALSXP, INTSXP, LGLSXP, RAWSXP, CHARSXP — simple data buffer
                free(rec.data);
            }
        }
    }
    // Free the SexpRec itself
    free(s as *mut u8);
}

// endregion
