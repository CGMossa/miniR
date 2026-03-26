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
8. Implement Makevars parser + cc-based compilation (`compile.rs`)
9. Wire into package loader — `useDynLib` triggers compile + load
10. Test with a simple C package (e.g. a custom one, then backports or bit)

## Compilation Pipeline (compile.rs)

When a package has `src/*.c` files, we need to compile them into a shared
library. R uses `R CMD SHLIB` which reads `Makevars` and calls the system
compiler. We replicate this with the `cc` crate.

### Makevars Parsing

`src/Makevars` (and `src/Makevars.win` on Windows) is a Makefile fragment.
Common variables:

| Variable | Purpose | Example |
|---|---|---|
| `PKG_CFLAGS` | C compiler flags | `-DHAVE_FOO -std=c11` |
| `PKG_CPPFLAGS` | Preprocessor flags (include paths) | `-I../inst/include` |
| `PKG_CXXFLAGS` | C++ compiler flags | `-std=c++17` |
| `PKG_LIBS` | Linker flags | `-lz -lpthread` |
| `OBJECTS` | Explicit object file list (overrides default) | `foo.o bar.o` |
| `CXX_STD` | C++ standard | `CXX11`, `CXX14`, `CXX17` |

Makevars can also have conditional logic (`ifeq`, shell commands, etc.)
but the vast majority of packages use only simple variable assignments.

### Parser Design

Don't try to run Make. Instead, parse Makevars as a simple key=value file:

```rust
struct Makevars {
    pkg_cflags: Vec<String>,
    pkg_cppflags: Vec<String>,
    pkg_cxxflags: Vec<String>,
    pkg_libs: Vec<String>,
    objects: Option<Vec<String>>,  // None = auto-discover from src/*.c
    cxx_std: Option<String>,
}

impl Makevars {
    fn parse(path: &Path) -> Self {
        // Read line by line
        // Handle VAR = value, VAR += value
        // Expand $(PKG_...) references between variables
        // Ignore ifeq/endif blocks (too complex for v1)
        // Ignore shell commands $(...) (too complex for v1)
    }
}
```

### Compilation Steps

```rust
fn compile_package_native(pkg_dir: &Path) -> Result<PathBuf, Error> {
    let src_dir = pkg_dir.join("src");
    let makevars = Makevars::parse(&src_dir.join("Makevars"));

    let mut build = cc::Build::new();

    // Add miniR headers
    build.include(minir_include_dir());

    // Add package-level includes
    build.include(&src_dir);
    build.include(&pkg_dir.join("inst/include"));

    // Apply Makevars flags
    for flag in &makevars.pkg_cflags {
        build.flag(flag);
    }
    for flag in &makevars.pkg_cppflags {
        build.flag(flag);
    }

    // Find source files
    let sources = match &makevars.objects {
        Some(objs) => objs.iter()
            .map(|o| src_dir.join(o.replace(".o", ".c")))
            .collect(),
        None => glob_sources(&src_dir),  // all .c and .cpp files
    };

    for src in &sources {
        build.file(src);
    }

    // Compile to shared library
    build.shared_flag(true);
    build.cargo_warnings(false);

    // Output path: pkg_dir/libs/pkg.so
    let out_dir = pkg_dir.join("libs");
    std::fs::create_dir_all(&out_dir)?;
    let lib_name = pkg_dir.file_name().unwrap().to_str().unwrap();
    let lib_path = out_dir.join(format!("{lib_name}.so")); // .dylib on macOS

    build.compile(lib_name);

    Ok(lib_path)
}
```

### configure Scripts

Some packages have `configure` scripts (autoconf) that generate `Makevars`
from `Makevars.in`. For v1, skip these — only support packages with
static `Makevars`. Packages needing `configure` would need manual
pre-configuration.

### System Libraries

`PKG_LIBS = -lz -lpng -lcurl` requires system libraries to be installed.
We don't manage this — the user needs `brew install zlib libpng curl` etc.
We should surface clear error messages when linking fails due to missing
system deps.

## What We Can Skip

- `.Fortran()` — very few packages use it directly
- `.C()` — legacy calling convention, mostly replaced by .Call
- `.External()` / `.External2()` — rare
- R_RegisterRoutines — nice to have but not blocking
- Complex GC (mark-and-sweep) — use simple refcount + protect stack initially
- `configure` scripts — too complex for v1, require autoconf
- Makevars with shell commands / conditionals — parse simple assignments only

## Testing Strategy

1. Write a tiny C package with one .Call function
2. Compile it with our cc pipeline
3. Load it with dyn.load
4. Call it with .Call and verify the result
5. Then try real CRAN packages: backports (simple), bit (medium), Rcpp-based (hard)
