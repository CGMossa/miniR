# TODO — Remaining Stubs and Partial Implementations

This file tracks behavior that is still stubbed, placeholder, or materially simplified.

## Remaining Builtin Stubs

- [ ] GNU-R-compatible binary serialization for `readRDS()`, `saveRDS()`, `load()`, and `save()`
- [ ] `install.packages(pkgs)`, `library(pkg)`, `require(pkg)` — package loading and management (see `plans/package-runtime.md`)
- [ ] `reg.finalizer(e, f)` — finalizers

## Architecture and Cleanup

- [ ] `RError` cleanup — continue extracting module-specific error types
- [ ] Arrow backend for vector storage — replace `Vec<Option<T>>` with contiguous storage + validity bitmaps
- [ ] Large files: `value.rs` (1200+ lines), `parser.rs` (900+ lines)

## Developer Experience

- [ ] Add tokei for file size tracking and refactoring detection
- [ ] Add a vendor patch workflow for intentional edits under `vendor/`
