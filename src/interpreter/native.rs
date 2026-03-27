//! Native code loading — compile, load, and call C code from R packages.
//!
//! Implements the `.Call()` → C function pipeline:
//! 1. Parse `src/Makevars` for compiler flags
//! 2. Compile `src/*.c` into a shared library using the system C compiler
//! 3. Load the `.so`/`.dylib` via `libloading`
//! 4. Dispatch `.Call()` — convert `RValue` → SEXP, call, convert back

pub mod compile;
pub mod convert;
pub mod dll;
pub mod runtime;
pub mod sexp;
