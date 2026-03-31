# Native Code Loading (.Call / dyn.load)

134 of 222 CRAN packages in the corpus use `useDynLib` — they have C/C++ code
that needs to be compiled and loaded. This is the biggest remaining blocker
for real package support.

## Approach: miniR's Own SEXP ABI

Define our own C-compatible memory layout that R package C code can link against.
No dependency on GNU R. miniR is the R implementation — it provides the headers,
the runtime, and the C API.

### What Package C Code Expects

```c
SEXP my_func(SEXP x) {
    int n = LENGTH(x);
    double *px = REAL(x);
    SEXP result = PROTECT(allocVector(REALSXP, n));
    double *pr = REAL(result);
    for (int i = 0; i < n; i++) pr[i] = px[i] * 2;
    UNPROTECT(1);
    return result;
}
```

It needs:
- `SEXP` — a pointer to a tagged object
- `LENGTH()`, `REAL()`, `INTEGER()`, `LOGICAL()`, `STRING_ELT()` — accessors
- `allocVector()`, `PROTECT()`, `UNPROTECT()` — allocation + GC protection
- Type constants: `REALSXP`, `INTSXP`, `STRSXP`, `LGLSXP`, `VECSXP`

### miniR SEXP Design

```c
// minir_sexp.h — our C ABI header

typedef struct SEXPREC *SEXP;

struct SEXPREC {
    uint8_t type;       // SEXPTYPE (REALSXP=14, INTSXP=13, etc.)
    uint8_t flags;      // GC mark, named count
    uint16_t padding;
    int32_t length;     // vector length
    union {
        double *real;   // REALSXP data pointer
        int *integer;   // INTSXP data pointer
        int *logical;   // LGLSXP data pointer (R uses int for logical)
        SEXP *list;     // VECSXP data pointer
        void *raw;      // RAWSXP data pointer
    } data;
    SEXP attrib;        // attributes pairlist (or NULL)
};
```

This is much simpler than GNU R's SEXPREC (which has sxpinfo_struct bitfields,
GC links, etc.). We only implement what the public C API exposes.

### C API Functions to Implement (~80 core functions)

**Allocation:**
- `Rf_allocVector(SEXPTYPE, R_xlen_t)` → allocate typed vector
- `Rf_allocMatrix(SEXPTYPE, int nrow, int ncol)` → matrix
- `Rf_allocList(int n)` → pairlist
- `Rf_ScalarReal(double)`, `Rf_ScalarInteger(int)`, `Rf_ScalarLogical(int)`, `Rf_ScalarString(SEXP)`

**Protection (GC):**
- `Rf_protect(SEXP)` → push onto protect stack
- `Rf_unprotect(int n)` → pop n from protect stack
- `R_PreserveObject(SEXP)` / `R_ReleaseObject(SEXP)`

**Accessors:**
- `LENGTH(SEXP)`, `XLENGTH(SEXP)` → vector length
- `REAL(SEXP)` → `double*` pointer
- `INTEGER(SEXP)` → `int*` pointer
- `LOGICAL(SEXP)` → `int*` pointer
- `RAW(SEXP)` → `Rbyte*` pointer
- `COMPLEX(SEXP)` → `Rcomplex*` pointer
- `STRING_ELT(SEXP, int)` → single string element
- `VECTOR_ELT(SEXP, int)` → list element
- `SET_STRING_ELT`, `SET_VECTOR_ELT`
- `TYPEOF(SEXP)` → SEXPTYPE

**Strings:**
- `Rf_mkChar(const char*)` → CHARSXP
- `Rf_mkString(const char*)` → length-1 STRSXP
- `R_CHAR(SEXP)` → `const char*`
- `Rf_translateCharUTF8(SEXP)` → UTF-8 string

**Attributes:**
- `Rf_getAttrib(SEXP, SEXP)` / `Rf_setAttrib(SEXP, SEXP, SEXP)`
- `Rf_namesgets`, `Rf_dimgets`, `Rf_classgets`
- `R_NamesSymbol`, `R_DimSymbol`, `R_ClassSymbol` (symbol constants)

**Type checking:**
- `Rf_isReal`, `Rf_isInteger`, `Rf_isLogical`, `Rf_isString`, `Rf_isNull`
- `Rf_isVector`, `Rf_isList`, `Rf_isMatrix`, `Rf_isDataFrame`
- `Rf_inherits(SEXP, const char*)`

**Coercion:**
- `Rf_coerceVector(SEXP, SEXPTYPE)`
- `Rf_asReal`, `Rf_asInteger`, `Rf_asLogical`, `Rf_asChar`

**NA values:**
- `R_NaReal`, `R_NaInt`, `R_NaString`, `R_NilValue`
- `ISNA`, `ISNAN`, `R_IsNA`

**Error handling:**
- `Rf_error(const char*, ...)` → longjmp to R error handler
- `Rf_warning(const char*, ...)`

**Pairlist (for attributes and calls):**
- `CAR`, `CDR`, `TAG`, `SETCAR`, `SETCDR`, `SET_TAG`
- `Rf_cons`, `Rf_lcons`, `Rf_install` (symbols)

## Crate Dependencies

```toml
cc = { version = "1", optional = true }
libloading = { version = "0.8", optional = true }
```

Feature: `native = ["dep:cc", "dep:libloading"]`

## Module Layout

```
src/interpreter/native.rs             # module root
src/interpreter/native/sexp.rs        # SEXP type, SEXPREC struct
src/interpreter/native/api.rs         # C API function implementations
src/interpreter/native/protect.rs     # PROTECT/UNPROTECT stack
src/interpreter/native/convert.rs     # RValue ↔ SEXP conversion
src/interpreter/native/dll.rs         # dyn.load, .Call dispatch
src/interpreter/native/compile.rs     # cc-based compilation of package src/
include/miniR/Rinternals.h            # C header for packages to compile against
include/miniR/R.h                     # Top-level include
```

## Implementation Order

1. Define SEXP layout in Rust (`sexp.rs`)
2. Implement RValue → SEXP and SEXP → RValue conversion (`convert.rs`)
3. Implement PROTECT stack (`protect.rs`)
4. Implement core C API functions as `extern "C"` (`api.rs`)
5. Write `include/miniR/Rinternals.h` header
6. Implement `dyn.load()` via libloading (`dll.rs`)
7. Implement `.Call()` dispatch — find symbol, convert args, call, convert result
8. Implement `cc`-based compilation (`compile.rs`) — find system C compiler, compile src/*.c against our headers
9. Wire into package loader — `useDynLib` triggers compile + load
10. Test with a simple C package (e.g. a custom one, then backports or bit)

## What We Can Skip

- `.Fortran()` — very few packages use it directly
- `.C()` — legacy calling convention, mostly replaced by .Call
- `.External()` / `.External2()` — rare
- R_RegisterRoutines — nice to have but not blocking
- Complex GC (mark-and-sweep) — use simple refcount + protect stack initially

## Testing Strategy

1. Write a tiny C package with one .Call function
2. Compile it with our cc pipeline
3. Load it with dyn.load
4. Call it with .Call and verify the result
5. Then try real CRAN packages: backports (simple), bit (medium), Rcpp-based (hard)
