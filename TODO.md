# TODO — Remaining Work

## Package Runtime

- [x] `library(pkg)`, `require(pkg)`, `requireNamespace(pkg)`, `loadNamespace(pkg)`
- [x] DESCRIPTION / NAMESPACE file parsing
- [x] Namespace environments and search path
- [x] `.onLoad()` / `.onAttach()` hooks
- [x] `system.file()` — searches .libPaths() for package files
- [x] `packageVersion(pkg)` — reads version from DESCRIPTION
- [x] `getNamespace(ns)` — auto-loads if not loaded
- [ ] Collation order for R/ source files (currently alphabetical)
- [ ] S3method() registration from NAMESPACE directives

## Serialization

- [x] GNU-R-compatible binary deserialization for `readRDS()`
- [x] Binary serialization for `saveRDS()` (write XDR binary, gzip)
- [x] `load()` / `save()` / `save.image()` (RDX2 format)
- [ ] ASCII serialization format (format 'A')
- [ ] Closure/environment serialization (currently written as NULL)

## Builtins — All verified working

- [x] `quantile()` — type 7, named output
- [x] `rep(each=, length.out=)`
- [x] `sample(prob=)` — weighted sampling
- [x] `sort(na.last=)` — TRUE/FALSE/NA
- [x] `diff(differences=)` — iterative differencing
- [x] `ordered()` — ordered factor constructor
- [x] `makeActiveBinding()` — re-evaluates function on each access
- [x] `on.exit()` — fires on normal return, explicit return, and error
- [x] `all.equal()` — proper R semantics (returns TRUE or descriptive string)
- [x] `identical()` — structural comparison (NaN==NaN is TRUE)
- [x] `invisible()` — per-interpreter visibility flag
- [x] Session-scoped output — `println!` replaced with per-interpreter writers
- [x] `blake3()` / `blake3_raw()` / `blake3_file()` — fast hashing
- [x] `ChaCha20` RNG via `RNGkind("ChaCha20")` — deterministic
- [x] `txtProgressBar()` / `setTxtProgressBar()` — via indicatif
- [x] 48 distribution functions with `lower.tail`, `log.p`, `log` params

## Architecture

- [x] `tracing` crate as default logging framework
- [x] `nalgebra` for linear algebra decompositions
- [x] Session-scoped output (per-interpreter writers)
- [x] `all.equal()` rewrite with proper numeric comparison

## Graphics

- [ ] egui-based interactive plotting (see `plans/egui-graphics.md`)
- [ ] SVG/PNG file device output

## Deferred

- [ ] Arrow backend for vector storage
- [ ] WASM target support (needs `--no-default-features` testing)
- [ ] Full S4 inheritance chain resolution (basic registry works)
- [ ] Polars-backed data frames
