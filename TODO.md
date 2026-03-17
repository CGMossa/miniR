# TODO — Remaining Work

## Package Runtime (highest priority)

- [ ] `library(pkg)`, `require(pkg)`, `requireNamespace(pkg)`, `loadNamespace(pkg)` — package loading
- [ ] DESCRIPTION / NAMESPACE file parsing
- [ ] Namespace environments and search path
- [ ] `.onLoad()` / `.onAttach()` hooks
- [ ] See `plans/package-runtime.md`

## Serialization

- [ ] GNU-R-compatible binary serialization for `readRDS()`, `saveRDS()`, `load()`, `save()`

## Architecture

- [ ] Implement feature gates from `plans/feature-gates.md` for remaining deps
- [ ] Add `tracing` crate as default logging framework (see `plans/tracing.md`)
- [ ] Consider `nalgebra` for improved linear algebra (see `plans/nalgebra.md`)

## Graphics

- [ ] egui-based interactive plotting (see `plans/egui-graphics.md`)
- [ ] SVG/PNG file device output

## Deferred

- [ ] Arrow backend for vector storage
- [ ] WASM target support (needs `--no-default-features` testing)
