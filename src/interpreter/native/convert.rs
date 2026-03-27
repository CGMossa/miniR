//! RValue ↔ SEXP conversion.
//!
//! Converts between miniR's `RValue` types and the C-compatible SEXP layout
//! used by native package code. Handles the type differences:
//! - miniR Integer is i64, R INTEGER is i32 → truncate with overflow check
//! - miniR Logical is `Option<bool>`, R LOGICAL is i32 (TRUE=1, FALSE=0, NA=NA_INTEGER)
//! - miniR uses NullableBuffer bitmaps, R uses sentinel values (NA_REAL, NA_INTEGER)

use super::sexp::{self, Sexp, SexpRec};
use crate::interpreter::value::*;

// region: RValue → SEXP

/// Convert an RValue to a SEXP for passing to native C code.
///
/// The returned SEXP (and any sub-allocations) must be freed after the call.
/// Caller should track it in the allocation list.
pub fn rvalue_to_sexp(val: &RValue) -> Sexp {
    match val {
        RValue::Null => sexp::mk_null(),
        RValue::Vector(rv) => vector_to_sexp(&rv.inner),
        RValue::List(list) => list_to_sexp(list),
        // Functions, environments, language objects can't be passed to C.
        // Return R_NilValue as a safe fallback.
        RValue::Function(_) | RValue::Environment(_) | RValue::Language(_) => sexp::mk_null(),
    }
}

fn vector_to_sexp(vec: &Vector) -> Sexp {
    match vec {
        Vector::Double(d) => double_to_sexp(d),
        Vector::Integer(i) => integer_to_sexp(i),
        Vector::Logical(l) => logical_to_sexp(l),
        Vector::Character(c) => character_to_sexp(c),
        Vector::Raw(r) => raw_to_sexp(r),
        Vector::Complex(_) => {
            // TODO: complex SEXP support
            sexp::mk_null()
        }
    }
}

fn double_to_sexp(d: &Double) -> Sexp {
    let len = d.len();
    let s = sexp::alloc_vector(sexp::REALSXP, len as i32);
    unsafe {
        let ptr = sexp::real_ptr(s);
        for i in 0..len {
            *ptr.add(i) = d.get_opt(i).unwrap_or(sexp::NA_REAL);
        }
    }
    s
}

fn integer_to_sexp(int: &Integer) -> Sexp {
    let len = int.len();
    let s = sexp::alloc_vector(sexp::INTSXP, len as i32);
    unsafe {
        let ptr = sexp::integer_ptr(s);
        for i in 0..len {
            *ptr.add(i) = match int.get_opt(i) {
                Some(v) => i32::try_from(v).unwrap_or(sexp::NA_INTEGER),
                None => sexp::NA_INTEGER,
            };
        }
    }
    s
}

fn logical_to_sexp(l: &Logical) -> Sexp {
    let len = l.len();
    let s = sexp::alloc_vector(sexp::LGLSXP, len as i32);
    unsafe {
        let ptr = sexp::logical_ptr(s);
        for i in 0..len {
            *ptr.add(i) = match l[i] {
                Some(true) => 1,
                Some(false) => 0,
                None => sexp::NA_LOGICAL,
            };
        }
    }
    s
}

fn character_to_sexp(c: &Character) -> Sexp {
    let len = c.len();
    let s = sexp::alloc_vector(sexp::STRSXP, len as i32);
    unsafe {
        let elts = (*s).data as *mut Sexp;
        for i in 0..len {
            *elts.add(i) = match &c[i] {
                Some(st) => sexp::mk_char(st),
                None => sexp::mk_char("NA"), // R_NaString placeholder
            };
        }
    }
    s
}

fn raw_to_sexp(r: &[u8]) -> Sexp {
    let len = r.len();
    let s = sexp::alloc_vector(sexp::RAWSXP, len as i32);
    if len > 0 {
        unsafe {
            std::ptr::copy_nonoverlapping(r.as_ptr(), (*s).data, len);
        }
    }
    s
}

fn list_to_sexp(list: &RList) -> Sexp {
    let len = list.values.len();
    let s = sexp::alloc_vector(sexp::VECSXP, len as i32);
    unsafe {
        let elts = (*s).data as *mut Sexp;
        for (i, (_, val)) in list.values.iter().enumerate() {
            *elts.add(i) = rvalue_to_sexp(val);
        }
    }
    s
}

// endregion

// region: SEXP → RValue

/// Convert a SEXP result from native C code back to an RValue.
///
/// # Safety
/// `s` must be a valid SEXP pointer allocated by our runtime or by C code
/// using our allocVector.
pub unsafe fn sexp_to_rvalue(s: Sexp) -> RValue {
    if s.is_null() {
        return RValue::Null;
    }
    let rec: &SexpRec = &*s;
    match rec.stype {
        sexp::NILSXP => RValue::Null,
        sexp::REALSXP => sexp_real_to_rvalue(rec),
        sexp::INTSXP => sexp_int_to_rvalue(rec),
        sexp::LGLSXP => sexp_lgl_to_rvalue(rec),
        sexp::STRSXP => sexp_str_to_rvalue(rec),
        sexp::VECSXP => sexp_vec_to_rvalue(rec),
        sexp::RAWSXP => sexp_raw_to_rvalue(rec),
        sexp::CHARSXP => {
            // Single CHARSXP — wrap as length-1 character vector
            let st = sexp::char_data(s);
            RValue::vec(Vector::Character(vec![Some(st.to_string())].into()))
        }
        _ => RValue::Null,
    }
}

unsafe fn sexp_real_to_rvalue(rec: &SexpRec) -> RValue {
    let len = rec.length.max(0) as usize;
    let ptr = rec.data as *const f64;
    let mut vals = Vec::with_capacity(len);
    for i in 0..len {
        let v = *ptr.add(i);
        if sexp::is_na_real(v) {
            vals.push(None);
        } else {
            vals.push(Some(v));
        }
    }
    RValue::vec(Vector::Double(vals.into()))
}

unsafe fn sexp_int_to_rvalue(rec: &SexpRec) -> RValue {
    let len = rec.length.max(0) as usize;
    let ptr = rec.data as *const i32;
    let mut vals = Vec::with_capacity(len);
    for i in 0..len {
        let v = *ptr.add(i);
        if v == sexp::NA_INTEGER {
            vals.push(None);
        } else {
            vals.push(Some(i64::from(v)));
        }
    }
    RValue::vec(Vector::Integer(vals.into()))
}

unsafe fn sexp_lgl_to_rvalue(rec: &SexpRec) -> RValue {
    let len = rec.length.max(0) as usize;
    let ptr = rec.data as *const i32;
    let mut vals = Vec::with_capacity(len);
    for i in 0..len {
        let v = *ptr.add(i);
        if v == sexp::NA_LOGICAL {
            vals.push(None);
        } else {
            vals.push(Some(v != 0));
        }
    }
    RValue::vec(Vector::Logical(vals.into()))
}

unsafe fn sexp_str_to_rvalue(rec: &SexpRec) -> RValue {
    let len = rec.length.max(0) as usize;
    let elts = rec.data as *const Sexp;
    let mut vals = Vec::with_capacity(len);
    for i in 0..len {
        let elt = *elts.add(i);
        if elt.is_null() {
            vals.push(None);
        } else {
            vals.push(Some(sexp::char_data(elt).to_string()));
        }
    }
    RValue::vec(Vector::Character(vals.into()))
}

unsafe fn sexp_vec_to_rvalue(rec: &SexpRec) -> RValue {
    let len = rec.length.max(0) as usize;
    let elts = rec.data as *const Sexp;
    let mut vals = Vec::with_capacity(len);
    for i in 0..len {
        let elt = *elts.add(i);
        vals.push((None, sexp_to_rvalue(elt)));
    }
    RValue::List(RList::new(vals))
}

unsafe fn sexp_raw_to_rvalue(rec: &SexpRec) -> RValue {
    let len = rec.length.max(0) as usize;
    let mut buf = vec![0u8; len];
    if len > 0 {
        std::ptr::copy_nonoverlapping(rec.data, buf.as_mut_ptr(), len);
    }
    RValue::vec(Vector::Raw(buf))
}

// endregion
