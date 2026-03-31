# Rust Native Runtime — Replace minir_runtime.c

## Why

`minir_runtime.c` is a 750-line C file compiled into every package `.so`.
Each package gets its own copy with its own allocation tracking, protect
stack, and error handling state. This is wasteful and inconsistent with
miniR being a Rust project.

## Target Architecture

Rust `extern "C"` functions in the miniR binary that implement the R C API.
Package `.so` files resolve these symbols at load time.

### Symbol Resolution

**macOS**: compile packages with `-undefined dynamic_lookup` (already done).
The `.so` leaves R API symbols unresolved; they're found in the host binary
at `dlopen` time.

**Linux**: build miniR with `-Wl,--export-dynamic` so its symbols are visible
to `dlopen`'d libraries. This requires a Cargo build flag:
```toml
[target.x86_64-unknown-linux-gnu]
rustflags = ["-C", "link-args=-Wl,--export-dynamic"]
```

### What Moves to Rust

All functions currently in `minir_runtime.c`:
- `Rf_allocVector`, `Rf_mkChar`, `Rf_mkString`, scalar constructors
- `Rf_protect`, `Rf_unprotect` (protect stack)
- `Rf_error`, `Rf_warning` (error handling via setjmp — tricky in Rust)
- `Rf_getAttrib`, `Rf_setAttrib` (pairlist attributes)
- `Rf_coerceVector`, `Rf_asReal`, `Rf_asInteger`
- `Rf_duplicate`
- `R_MakeExternalPtr`, `R_ExternalPtrAddr`, `R_ClearExternalPtr`
- `R_registerRoutines`
- `_minir_call_protected` (setjmp trampoline)
- `_minir_free_allocs` (cleanup)

### Challenges

1. **setjmp/longjmp from Rust**: `Rf_error` uses `longjmp` which is unsafe
   from Rust's perspective. The trampoline `_minir_call_protected` must
   remain in C (or use Rust's `catch_unwind` as an alternative).

2. **Global mutable state**: allocation list, protect stack, error message
   buffer. In Rust, use `thread_local!` or a mutex-protected global.

3. **Shared state across packages**: with one runtime in the binary,
   all packages share the same allocation tracker. This is actually
   better than the current per-package isolation.

### Implementation Order

1. Add `#[no_mangle] pub extern "C"` functions in a new `src/interpreter/native/runtime.rs`
2. Add build.rs or .cargo/config.toml for `--export-dynamic` on Linux
3. Test that macOS `-undefined dynamic_lookup` resolves to the binary's symbols
4. Compile packages WITHOUT `minir_runtime.c`
5. Keep a minimal C stub for `_minir_call_protected` (setjmp trampoline) —
   this one function is hard to do in pure Rust
6. Remove `minir_runtime.c` (keep as reference)
