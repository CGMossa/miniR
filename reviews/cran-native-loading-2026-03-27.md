# CRAN Native Code Loading — First Real Test (2026-03-27)

## What worked

**base64enc** — 4 C files, uses standard C API only (REAL, LENGTH, allocVector, mkChar, R_alloc, SETLENGTH). Compiles, loads, and runs correctly via `library(base64enc)`.

Blocker for its R wrapper functions: `missing()` doesn't work correctly for default arguments. Direct `.Call("B64_encode", ...)` works fine.

## What didn't work

**backports** — C code uses deep R internals not in the public C API:
- `findVar`, `R_DotsSymbol`, `R_UnboundValue` (environment lookup)
- `nthcdr` (pairlist traversal)
- `eval` (R evaluator from C)
- `PRINTNAME`, `DOTSXP` (internal type access)

These functions reach into the R evaluator itself. Deferred — would require exposing interpreter internals to C, which is a large project and conflicts with miniR's goal of clean modern internals.

## Missing builtins discovered

- `missing()` — needed for default argument detection in R wrappers
- `getFromNamespace()` — added during this session
- `tools` package — backports tries to import from it

## Headers added

- `Rversion.h` — reports R 4.4.0
- `R_ext/Rdynload.h` — for R_registerRoutines
- `R_alloc()` — session-scoped allocator
- `SETLENGTH` macro — vector length mutation
