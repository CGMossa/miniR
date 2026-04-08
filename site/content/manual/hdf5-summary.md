+++
title = "HDF5 → dgCMatrix: Single-Copy Path"
weight = 50
+++

## Goal

Read CSC sparse matrices from HDF5 files into R's `dgCMatrix` (from the Matrix package) as fast as possible — single copy, no intermediate buffers.

## dgCMatrix Memory Layout

An S4 object with slots:

| Slot | R type | Contents |
|------|--------|----------|
| `Dim` | `integer[2]` | `[nrow, ncol]` |
| `p` | `integer[ncol+1]` | Column pointers (0-based) |
| `i` | `integer[nnz]` | Row indices (0-based) |
| `x` | `numeric[nnz]` | Nonzero values |
| `Dimnames` | `list[2]` | Row/column names (optional) |

HDF5 10x Genomics layout stores the same four arrays under `/{group}/shape`, `indptr`, `indices`, `data`.

## Matrix Package C API — What's Available

Matrix registers ~60 callables via `R_RegisterCCallable` (see `src/init.c:289-348`). They are **all CHOLMOD wrappers**:

- `cholmod_allocate_sparse` — allocates a `cholmod_sparse` struct (owns its own buffers)
- `cholmod_sparse_as_sexp` — converts `cholmod_sparse*` → dgCMatrix SEXP (memcpys p/i/x into R vectors)
- `sexp_as_cholmod_sparse` — converts dgCMatrix SEXP → `cholmod_sparse*`

**No helper exists** for "build dgCMatrix from raw `int*`/`double*` arrays". The internal `newObject()` (`src/objects.c:4`) is just `R_do_MAKE_CLASS` + `R_do_new_object` — trivial, not exported.

## Three Approaches Considered

### 1. CHOLMOD bridge (two copies)

```
H5Dread → temp buffer → cholmod_sparse.p/i/x
cholmod_sparse_as_sexp → memcpy into R SEXP vectors
```

Two copies. Can skip `cholmod_allocate_sparse` by stack-allocating the struct and pointing its `p`/`i`/`x` at HDF5 buffers, but `cholmod_sparse_as_sexp` still memcpys into fresh R vectors.

### 2. CHOLMOD struct with HDF5 pointers (still two copies)

Stack-allocate `cholmod_sparse`, point `A.p`/`A.i`/`A.x` at HDF5 read buffers, call `M_cholmod_sparse_as_sexp(&A, 0, ...)` with `doFree=0`. Avoids cholmod allocation but the sexp conversion still memcpys. Same copy count as approach 1.

### 3. Direct H5Dread into R vectors (single copy) ✓

```c
SEXP r_p = PROTECT(Rf_allocVector(INTSXP, ncol + 1));
H5Dread(ds_indptr, H5T_NATIVE_INT, H5S_ALL, H5S_ALL, H5P_DEFAULT, INTEGER(r_p));
// ... same for r_i (INTSXP) and r_x (REALSXP) ...
R_do_slot_assign(obj, Rf_install("p"), r_p);
```

One copy: HDF5 file → R's GC-managed heap. No intermediate buffer, no memcpy. Class selection is trivial — `"dgCMatrix"` is a 2-char prefix (`d`=double, `g`=general) + `"CMatrix"` (CSC).

## Implementation

Both C and Rust versions are in `src/`:

- `src/hdf5_sparse.c` — C version, callable from R via `.Call(C_read_hdf5_sparse, path, group)`
- `src/hdf5_sparse.rs` — Rust version using raw R + HDF5 FFI

The S4 object construction follows the same pattern as Matrix's internal `cholmod_sparse_as_sexp()` (`src/cholmod-common.c:762-807`) but without the CHOLMOD intermediary.

## Build Requirements

- HDF5 library: `brew install hdf5` (provides `/opt/homebrew/include/hdf5.h`)
- Compile flags: `PKG_CPPFLAGS += -I/opt/homebrew/include`, `PKG_LIBS += -lhdf5`
- LinkingTo: Matrix (for class definitions, though we only use base R API)

## gfortran / FLIBS Fix

R 4.5.2 on this machine was built expecting gfortran 14.2.0 at `/opt/gfortran`, but the installed version is 12.2.0. The path `/opt/gfortran/lib/gcc/aarch64-apple-darwin20.0/14.2.0` doesn't exist, and `libheapt_w` is a 14.2.0-only library.

Fix in `~/.R/Makevars`:

```
FLIBS=-L/opt/gfortran/lib/gcc/aarch64-apple-darwin20.0/12.2.0 -L/opt/gfortran/lib -lemutls_w -lgfortran -lquadmath
```

## Bear / compile_commands.json

The `justfile` wraps `R CMD INSTALL` with `bear` to generate `src/compile_commands.json` for clangd. This gives full IDE support (go-to-definition, hover, diagnostics) for all Matrix C source files.
