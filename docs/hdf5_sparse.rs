//! Read CSC sparse matrix from HDF5 directly into dgCMatrix (R's Matrix package)
//!
//! Single-copy path: HDF5 reads directly into R's SEXP vector buffers.
//! No intermediate allocations, no memcpy.
//!
//! Expects HDF5 layout (10x Genomics style):
//!   /{group}/data    — f64[nnz]        (nonzero values)
//!   /{group}/indices — i32[nnz]        (row indices, 0-based)
//!   /{group}/indptr  — i32[ncol + 1]   (column pointers)
//!   /{group}/shape   — i32[2]          (nrow, ncol)
//!
//! Matrix C helpers used: none — Matrix doesn't export a raw-array → dgCMatrix
//! constructor. The registered callables are all CHOLMOD wrappers. So we build
//! the S4 object directly: R_do_MAKE_CLASS("dgCMatrix") + R_do_new_object +
//! R_do_slot_assign for "Dim", "p", "i", "x". This is exactly what Matrix's
//! own cholmod_sparse_as_sexp() does internally (cholmod-common.c:762-807),
//! minus the CHOLMOD intermediary.

use std::os::raw::c_int;
use std::ptr;

// In practice these come from libR-sys or extendr_api::prelude
type SEXP = *mut std::ffi::c_void;
type hid_t = i64;
type hsize_t = u64;

extern "C" {
    // R API
    fn Rf_allocVector(sexptype: c_int, length: isize) -> SEXP;
    fn Rf_protect(s: SEXP) -> SEXP;
    fn Rf_unprotect(n: c_int);
    fn Rf_error(fmt: *const i8, ...) -> !;
    fn Rf_install(name: *const i8) -> SEXP;
    fn R_do_MAKE_CLASS(what: *const i8) -> SEXP;
    fn R_do_new_object(class_def: SEXP) -> SEXP;
    fn R_do_slot(obj: SEXP, name: SEXP) -> SEXP;
    fn R_do_slot_assign(obj: SEXP, name: SEXP, value: SEXP);
    fn INTEGER(x: SEXP) -> *mut c_int;
    fn REAL(x: SEXP) -> *mut f64;
    fn STRING_ELT(x: SEXP, i: isize) -> SEXP;
    fn R_CHAR(x: SEXP) -> *const i8;

    // HDF5 C API (link against libhdf5)
    fn H5Fopen(name: *const i8, flags: u32, fapl: hid_t) -> hid_t;
    fn H5Fclose(file: hid_t) -> c_int;
    fn H5Gopen2(loc: hid_t, name: *const i8, gapl: hid_t) -> hid_t;
    fn H5Gclose(group: hid_t) -> c_int;
    fn H5Dopen2(loc: hid_t, name: *const i8, dapl: hid_t) -> hid_t;
    fn H5Dclose(dataset: hid_t) -> c_int;
    fn H5Dread(
        dataset: hid_t, mem_type: hid_t, mem_space: hid_t,
        file_space: hid_t, xfer: hid_t, buf: *mut std::ffi::c_void,
    ) -> c_int;
    fn H5Dget_space(dataset: hid_t) -> hid_t;
    fn H5Sget_simple_extent_dims(space: hid_t, dims: *mut hsize_t, maxdims: *mut hsize_t) -> c_int;
    fn H5Sclose(space: hid_t) -> c_int;

    // HDF5 type globals (resolved at link time from libhdf5)
    static H5T_NATIVE_INT_g: hid_t;
    static H5T_NATIVE_DOUBLE_g: hid_t;
}

const H5F_ACC_RDONLY: u32 = 0x0000;
const H5P_DEFAULT: hid_t = 0;
const H5S_ALL: hid_t = 0;
const INTSXP: c_int = 13;
const REALSXP: c_int = 14;

unsafe fn h5_dataset_len(ds: hid_t) -> hsize_t {
    let space = H5Dget_space(ds);
    let mut len: hsize_t = 0;
    H5Sget_simple_extent_dims(space, &mut len, ptr::null_mut());
    H5Sclose(space);
    len
}

/// Read a CSC sparse matrix from HDF5 into a dgCMatrix.
///
/// Called from R as: `.Call(read_hdf5_sparse, "/path/to/file.h5", "matrix")`
///
/// # Safety
/// Must be called from R's main thread. Pointers come from R's GC-managed heap.
#[no_mangle]
pub unsafe extern "C" fn read_hdf5_sparse(filepath: SEXP, groupname: SEXP) -> SEXP {
    let file_ptr  = R_CHAR(STRING_ELT(filepath, 0));
    let group_ptr = R_CHAR(STRING_ELT(groupname, 0));

    // Open HDF5 file + group
    let fid = H5Fopen(file_ptr, H5F_ACC_RDONLY, H5P_DEFAULT);
    if fid < 0 {
        Rf_error(b"cannot open HDF5 file\0".as_ptr() as _);
    }
    let gid = H5Gopen2(fid, group_ptr, H5P_DEFAULT);
    if gid < 0 {
        H5Fclose(fid);
        Rf_error(b"cannot open HDF5 group\0".as_ptr() as _);
    }

    // Open the four datasets
    let ds_shape   = H5Dopen2(gid, b"shape\0".as_ptr()   as _, H5P_DEFAULT);
    let ds_indptr  = H5Dopen2(gid, b"indptr\0".as_ptr()  as _, H5P_DEFAULT);
    let ds_indices = H5Dopen2(gid, b"indices\0".as_ptr()  as _, H5P_DEFAULT);
    let ds_data    = H5Dopen2(gid, b"data\0".as_ptr()     as _, H5P_DEFAULT);

    // Read shape → [nrow, ncol]
    let mut shape = [0i32; 2];
    H5Dread(ds_shape, H5T_NATIVE_INT_g, H5S_ALL, H5S_ALL, H5P_DEFAULT,
            shape.as_mut_ptr() as _);
    let nrow = shape[0];
    let ncol = shape[1];
    let nnz = h5_dataset_len(ds_data);

    // Allocate R vectors — GC-managed, single owner
    let r_p = Rf_protect(Rf_allocVector(INTSXP,  (ncol as isize) + 1));
    let r_i = Rf_protect(Rf_allocVector(INTSXP,  nnz as isize));
    let r_x = Rf_protect(Rf_allocVector(REALSXP, nnz as isize));

    // Single-copy: HDF5 reads directly into R's vector buffers
    H5Dread(ds_indptr,  H5T_NATIVE_INT_g,    H5S_ALL, H5S_ALL, H5P_DEFAULT, INTEGER(r_p) as _);
    H5Dread(ds_indices, H5T_NATIVE_INT_g,    H5S_ALL, H5S_ALL, H5P_DEFAULT, INTEGER(r_i) as _);
    H5Dread(ds_data,    H5T_NATIVE_DOUBLE_g, H5S_ALL, H5S_ALL, H5P_DEFAULT, REAL(r_x)    as _);

    // Close HDF5 handles (order doesn't matter)
    H5Dclose(ds_data);
    H5Dclose(ds_indices);
    H5Dclose(ds_indptr);
    H5Dclose(ds_shape);
    H5Gclose(gid);
    H5Fclose(fid);

    // Build dgCMatrix S4 object
    let class_def = Rf_protect(R_do_MAKE_CLASS(b"dgCMatrix\0".as_ptr() as _));
    let obj       = Rf_protect(R_do_new_object(class_def));

    // Dim slot — already allocated as int[2] by new_object, just fill it
    let dim_sym = Rf_install(b"Dim\0".as_ptr() as _);
    let dim = R_do_slot(obj, dim_sym);
    *INTEGER(dim).offset(0) = nrow;
    *INTEGER(dim).offset(1) = ncol;

    // p, i, x slots — zero-copy handoff, R already owns the buffers
    R_do_slot_assign(obj, Rf_install(b"p\0".as_ptr() as _), r_p);
    R_do_slot_assign(obj, Rf_install(b"i\0".as_ptr() as _), r_i);
    R_do_slot_assign(obj, Rf_install(b"x\0".as_ptr() as _), r_x);

    Rf_unprotect(5); // r_p, r_i, r_x, class_def, obj
    obj
}
