# TODO — Remaining Work

## Package Runtime

- [x] `library(pkg)`, `require(pkg)`, `requireNamespace(pkg)`, `loadNamespace(pkg)` — basic package loading
- [x] DESCRIPTION / NAMESPACE file parsing
- [x] Namespace environments and search path
- [ ] `.onLoad()` / `.onAttach()` hooks
- [ ] `detach()` full semantics (currently basic)
- [ ] Collation order for R/ source files (currently alphabetical)
- [ ] S3method() registration from NAMESPACE directives
- [ ] `system.file()` — find files in installed packages (currently a stub)

## Serialization

- [x] GNU-R-compatible binary deserialization for `readRDS()` (XDR binary, gzip via flate2)
- [x] Binary serialization for `saveRDS()` (write XDR binary, gzip)
- [x] `load()` / `save()` / `save.image()` support (RDX2 header + pairlist)
- [ ] ASCII serialization format (format 'A')
- [ ] Closure/environment serialization (currently written as NULL)

## Builtins — Remaining Gaps

- [ ] `quantile()` — not implemented
- [ ] `rep(each=)` — `each` parameter missing
- [ ] `sample(prob=)` — weighted sampling not implemented
- [ ] `sort(na.last=)` — `na.last` parameter missing
- [ ] `diff(differences=)` — `differences` parameter missing
- [ ] `ordered()` — constructor for ordered factors
- [ ] `Sys.time()` improvements — subsecond precision
- [ ] `on.exit()` — partially implemented but not firing reliably
- [ ] `makeActiveBinding()` — stores static value, doesn't re-evaluate

## Architecture

- [x] `tracing` crate as default logging framework
- [x] `nalgebra` for linear algebra decompositions
- [ ] Session-scoped output (replace `println!` with per-interpreter writers) — see `plans/session-output.md`
- [ ] `all.equal()` rewrite using `approx` crate — see `plans/approx.md`

## Graphics

- [ ] egui-based interactive plotting (see `plans/egui-graphics.md`)
- [ ] SVG/PNG file device output

## Deferred

- [ ] Arrow backend for vector storage
- [ ] WASM target support (needs `--no-default-features` testing)
- [ ] Full S4 inheritance chain resolution (basic registry works)
- [ ] Polars-backed data frames
