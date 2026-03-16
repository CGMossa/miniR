# TODO — Remaining Stubs and Partial Implementations

This file tracks behavior that is still stubbed, placeholder, or materially simplified.

## Remaining Builtin Stubs

- [ ] GNU-R-compatible binary serialization for `readRDS()`, `saveRDS()`, `load()`, and `save()`
- [ ] `install.packages(pkgs)`, `library(pkg)`, `require(pkg)` — package loading and management (see `plans/package-runtime.md`)

## Architecture and Cleanup

- [ ] Arrow backend for vector storage — replace `Vec<Option<T>>` with contiguous storage + validity bitmaps
- [ ] Split `value.rs` (1200+ lines) and `parser.rs` (900+ lines) into smaller modules

## Developer Experience

- [ ] Add tokei for file size tracking and refactoring detection
- [x] Add a vendor patch workflow for intentional edits under `vendor/`
