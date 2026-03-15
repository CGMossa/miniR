# TODO — Remaining Stubs and Partial Implementations

This file tracks behavior that is still stubbed, placeholder, or materially simplified.

## Simplified Data and Object Semantics

- [ ] `data.frame(...)` — subsetting edge cases, coercions, and fuller R-compatible behavior still need work

## Remaining Builtin Stubs

- [ ] `qr(x)`, `svd(x)`, `eigen(x)`, `det(x)`, `chol(x)` — linear algebra decompositions
- [ ] GNU-R-compatible binary serialization for `readRDS()`, `saveRDS()`, `load()`, and `save()`
- [ ] `url(...)`, `connection(...)`, `open(con)`, `close(con)` — connection objects
- [ ] `install.packages(pkgs)`, `library(pkg)`, `require(pkg)` — package loading and management
- [ ] `setClass(Class, ...)`, `setMethod(f, ...)`, `setGeneric(name, ...)` — S4
- [ ] `pdf(...)`, `dev.off()`, `plot(...)`, `lm(formula, data)` — graphics and modeling stubs
- [ ] `reg.finalizer(e, f)` — finalizers

## Architecture and Cleanup

- [ ] `RError` cleanup — continue extracting module-specific error types
- [ ] Arrow backend for vector storage — replace `Vec<Option<T>>` with contiguous storage + validity bitmaps
- [ ] Feature-gate the I/O module for sandboxed/WASM environments
- [ ] Large files: `builtins.rs` (3212 lines), `value.rs` (1157 lines), `parser.rs` (906 lines)

## Developer Experience

- [ ] Add tokei for file size tracking and refactoring detection
- [ ] Add a vendor patch workflow for intentional edits under `vendor/`
