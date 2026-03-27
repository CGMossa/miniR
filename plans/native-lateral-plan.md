# Native Code Lateral Plan — Systematic Fixes

## Phase 1: Makevars Parsing (blocks 33 packages)

The most common issue: `$(VARIABLE)` references in Makevars that our parser
passes literally to the compiler.

**Fix**: Strip or expand `$(...)` references.
- `$(C_VISIBILITY)` → `-fvisibility=hidden` (or empty)
- `$(CXX_VISIBILITY)` → `-fvisibility=hidden`
- `$(F_VISIBILITY)` → empty (Fortran)
- `$(SHLIB)`, `$(STATLIB)`, `$(OBJECTS)` → skip (build targets, not flags)
- `$(BLAS_LIBS)`, `$(LAPACK_LIBS)`, `$(FLIBS)` → system libs or empty
- `$(CC)`, `$(CXX)`, `$(AR)`, etc. → skip (build commands)
- `$(R_INCLUDE_DIR)` → our include dir
- `$(SHLIB_OPENMP_CFLAGS)`, `$(SHLIB_OPENMP_CXXFLAGS)` → empty (no OpenMP)
- `ifeq`/`ifdef` conditionals → skip lines inside conditionals

**Packages unblocked**: rlang, cli, glue, vctrs, purrr, fansi, nlme, zip, writexl

## Phase 2: Missing Headers (blocks 7 packages)

| Header | Packages | Action |
|--------|----------|--------|
| `Rmath.h` | forecast, igraph, s2 | Stub with math constants (M_PI, etc.) |
| `Rcpp.h` | forecast, httpuv, later, s2, terra | Skip — Rcpp is C++/R bridge, too complex |
| `R_ext/Applic.h` | forecast | Stub (optimization routines) |
| `R_ext/eventloop.h` | later | Stub |
| `R_ext/libextern.h` | later | Stub (just extern macros) |
| `R_ext/GraphicsDevice.h` | readxl | Stub |
| `R_ext/GraphicsEngine.h` | readxl | Stub |

## Phase 3: Missing C API Functions (by frequency)

Already have: Rf_mkChar, Rf_isNull, R_NilValue, Rf_error, Rf_allocVector,
Rf_asInteger, Rf_install, Rf_setAttrib, Rf_inherits, Rf_length, Rf_eval,
R_RegisterCCallable, R_ExternalPtrAddr, R_ClassSymbol, Rf_ScalarInteger/Real/Logical/String,
Rf_translateCharUTF8, Rf_getAttrib, Rf_duplicate, R_CheckUserInterrupt

Still needed (by usage count):
| Function | Count | Difficulty |
|----------|-------|-----------|
| `Rf_xlength` | 175 | Easy (same as Rf_length for non-long vectors) |
| `Rf_lengthgets` | ~10 | Done |
| `R_CHAR` macro on NA | - | Need to handle gracefully |
| `Rmath.h` constants | ~30 | Easy stubs |

## Phase 4: Makevars Variable Expansion Implementation

Replace `$(VAR)` with known values in our Makevars parser:

```
C_VISIBILITY → "-fvisibility=hidden"
CXX_VISIBILITY → "-fvisibility=hidden"
R_INCLUDE_DIR → <include_dir>
SHLIB_OPENMP_CFLAGS → ""
SHLIB_OPENMP_CXXFLAGS → ""
```

Strip unrecognized `$(VAR)` references (warn, don't error).
Skip Make targets (lines with `:`) and conditionals (`ifeq`/`ifdef`).
