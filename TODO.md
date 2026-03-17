# TODO — Remaining Work

## Package Runtime (highest priority)

- [ ] `library(pkg)`, `require(pkg)`, `requireNamespace(pkg)`, `loadNamespace(pkg)` — package loading
- [ ] DESCRIPTION / NAMESPACE file parsing
- [ ] Namespace environments and search path
- [ ] `.onLoad()` / `.onAttach()` hooks
- [ ] See `plans/package-runtime.md`

## Serialization

- [x] GNU-R-compatible binary deserialization for `readRDS()` (XDR binary, gzip via flate2)
- [ ] Binary serialization for `saveRDS()` (write XDR binary)
- [ ] `load()` / `save()` support (RDX2 header + pairlist)

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
