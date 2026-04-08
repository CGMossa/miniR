+++
title = "Native Code Support"
weight = 4
description = "C, C++, and Fortran compilation for R packages"
+++

miniR compiles and loads native code from CRAN packages using the `cc` crate for C/C++ and `gfortran` for Fortran.

## How It Works

When `loadNamespace("pkg")` encounters a package with `src/` containing C/C++/Fortran files:

1. **Parse Makevars** --- reads `src/Makevars` (or `Makevars.in` with pkg-config resolution)
2. **Compile sources** --- C via `cc`, C++ via `cc` with `-std=c++17`, Fortran via `gfortran`
3. **Link** --- produces `libs/pkg.dylib` with `-shared`, linking against system BLAS/LAPACK
4. **Load** --- `dyn.load()` loads the dylib, R API symbols resolve from the miniR binary

## Calling Conventions

| Convention | Implementation |
|-----------|---------------|
| `.Call(name, args...)` | Direct C function call, SEXP args/return |
| `.External(name, args...)` | Pairlist-based argument passing |
| `.C(name, args...)` | C-style pass-by-reference with type coercion |
| `.Fortran(name, args...)` | Same as `.C` with Fortran name mangling |

## pkg-config Integration

For packages with `Makevars.in` (anticonf pattern), miniR resolves `@cflags@` and `@libs@` placeholders via `pkg-config`:

```
openssl  -> pkg-config openssl
xml2     -> pkg-config libxml-2.0
stringi  -> pkg-config icu-i18n
sodium   -> pkg-config libsodium
```

## Configure Emulation

Some packages need platform-specific configuration that normally comes from `./configure`:

| Package | Emulation |
|---------|-----------|
| **ps** | Generates `config.h` with `PS__POSIX`, `PS__MACOS` macros + platform-specific object list |
| **fs** | Uses system libuv via pkg-config instead of bundled autotools build |
| **sass** | Uses system libsass via pkg-config instead of bundled make build |

## C API Coverage

miniR implements 200+ C API functions as `extern "C"` Rust functions:

- Memory: `Rf_allocVector`, `Rf_protect`/`UNPROTECT`, `R_PreserveObject`
- Access: `REAL`, `INTEGER`, `LOGICAL`, `STRING_ELT`, `VECTOR_ELT`, `SET_*`
- Strings: `Rf_mkChar`, `Rf_mkString`, `CHAR`, `Rf_translateCharUTF8`
- Eval: `Rf_eval`, `Rf_findVar`, `Rf_defineVar`, `Rf_setAttrib`/`Rf_getAttrib`
- Types: `TYPEOF`, `Rf_length`, `Rf_isNull`, `Rf_inherits`, `Rf_coerceVector`
- Math: Full Rmath library (distributions, special functions)
- Error: `Rf_error` (via setjmp/longjmp C trampoline), `Rf_warning`

## Headers

All R-compatible headers are in `include/miniR/`:

- `Rinternals.h` --- SEXP types, macros, function declarations
- `R.h` --- basic types, `NORET`, Fortran name mangling
- `Rmath.h` --- statistical distribution functions
- `R_ext/Rdynload.h` --- native routine registration
- `R_ext/Lapack.h` / `BLAS.h` --- LAPACK/BLAS with `FORTRAN_ARGS` macro
- `R_ext/GraphicsEngine.h` --- GEUnit, R_GE_gcontext, device descriptors
- `R_ext/Connections.h` --- Rconn struct, R_CONNECTIONS_VERSION
- `R_ext/eventloop.h` --- InputHandler for later/httpuv
