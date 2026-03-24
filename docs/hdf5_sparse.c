/*
 * hdf5_sparse.c — Read CSC sparse matrix from HDF5 directly into dgCMatrix
 *
 * Single-copy path: H5Dread writes directly into R's SEXP vector buffers.
 * No intermediate allocations, no memcpy.
 *
 * Matrix C helpers used: none. Matrix doesn't export a raw-array → dgCMatrix
 * constructor — the registered callables are all CHOLMOD wrappers. So we build
 * the S4 object directly: R_do_MAKE_CLASS("dgCMatrix") + R_do_new_object +
 * R_do_slot_assign for Dim, p, i, x. This is exactly what Matrix's own
 * cholmod_sparse_as_sexp() does internally (cholmod-common.c:762-807),
 * minus the CHOLMOD intermediary.
 *
 * Usage from R:
 *   .Call(C_read_hdf5_sparse, "/path/to/file.h5", "matrix")
 *
 * Expects HDF5 layout (10x Genomics style):
 *   /{group}/data    — double[nnz]     (nonzero values)
 *   /{group}/indices — int[nnz]        (row indices, 0-based)
 *   /{group}/indptr  — int[ncol + 1]   (column pointers)
 *   /{group}/shape   — int[2]          (nrow, ncol)
 */

#include <Rinternals.h>
#include <R_ext/Error.h>
#include <hdf5.h>

static hsize_t dataset_length(hid_t ds) {
    hid_t space = H5Dget_space(ds);
    hsize_t len;
    H5Sget_simple_extent_dims(space, &len, NULL);
    H5Sclose(space);
    return len;
}

SEXP C_read_hdf5_sparse(SEXP filepath, SEXP groupname) {
    const char *file  = CHAR(STRING_ELT(filepath, 0));
    const char *group = CHAR(STRING_ELT(groupname, 0));

    /* Open HDF5 file + group */
    hid_t fid = H5Fopen(file, H5F_ACC_RDONLY, H5P_DEFAULT);
    if (fid < 0) Rf_error("cannot open HDF5 file '%s'", file);

    hid_t gid = H5Gopen2(fid, group, H5P_DEFAULT);
    if (gid < 0) { H5Fclose(fid); Rf_error("cannot open HDF5 group '%s'", group); }

    /* Open datasets */
    hid_t ds_shape   = H5Dopen2(gid, "shape",   H5P_DEFAULT);
    hid_t ds_indptr  = H5Dopen2(gid, "indptr",  H5P_DEFAULT);
    hid_t ds_indices = H5Dopen2(gid, "indices", H5P_DEFAULT);
    hid_t ds_data    = H5Dopen2(gid, "data",    H5P_DEFAULT);

    /* Read shape → [nrow, ncol] */
    int shape[2];
    H5Dread(ds_shape, H5T_NATIVE_INT, H5S_ALL, H5S_ALL, H5P_DEFAULT, shape);
    int nrow = shape[0], ncol = shape[1];
    hsize_t nnz = dataset_length(ds_data);

    /* Allocate R vectors — GC-managed, single owner */
    SEXP r_p = PROTECT(Rf_allocVector(INTSXP,  ncol + 1));
    SEXP r_i = PROTECT(Rf_allocVector(INTSXP,  (R_xlen_t) nnz));
    SEXP r_x = PROTECT(Rf_allocVector(REALSXP, (R_xlen_t) nnz));

    /* Single-copy: H5Dread writes directly into R vector buffers */
    H5Dread(ds_indptr,  H5T_NATIVE_INT,    H5S_ALL, H5S_ALL, H5P_DEFAULT, INTEGER(r_p));
    H5Dread(ds_indices, H5T_NATIVE_INT,    H5S_ALL, H5S_ALL, H5P_DEFAULT, INTEGER(r_i));
    H5Dread(ds_data,    H5T_NATIVE_DOUBLE, H5S_ALL, H5S_ALL, H5P_DEFAULT, REAL(r_x));

    /* Close HDF5 handles */
    H5Dclose(ds_data); H5Dclose(ds_indices); H5Dclose(ds_indptr); H5Dclose(ds_shape);
    H5Gclose(gid); H5Fclose(fid);

    /* Build dgCMatrix S4 object */
    SEXP class_def = PROTECT(R_do_MAKE_CLASS("dgCMatrix"));
    SEXP obj       = PROTECT(R_do_new_object(class_def));

    /* Dim — pre-allocated int[2] by R_do_new_object, just fill it */
    SEXP dim = R_do_slot(obj, Rf_install("Dim"));
    INTEGER(dim)[0] = nrow;
    INTEGER(dim)[1] = ncol;

    /* p, i, x — zero-copy handoff, R already owns the buffers */
    R_do_slot_assign(obj, Rf_install("p"), r_p);
    R_do_slot_assign(obj, Rf_install("i"), r_i);
    R_do_slot_assign(obj, Rf_install("x"), r_x);

    UNPROTECT(5);
    return obj;
}
