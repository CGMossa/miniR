# TODO — Remaining Work

## Done

762 builtins, 971 tests, 7014/7014 R parse, 10838/10841 Rd parse,
5748/6769 CRAN source (84%), 109 packages at 100%.

## Open

### WASM target

- [x] reedline/crossterm/nu-ansi-term gated behind `repl` feature
- [ ] `linkme` (distributed_slice) doesn't support wasm32 — blocks WASM entirely
- [ ] Need alternative builtin registration for WASM (manual Vec or build.rs codegen)

### Graphics

- [ ] egui-based interactive plotting (see `plans/egui-graphics.md`)
- [ ] SVG/PNG file device output

### Performance

- [ ] `fnv` — faster HashMap for environment lookups (vendored, plan exists)
- [ ] `smallvec` — stack-allocated small vectors for attrs/short args (vendored, plan exists)
- [ ] `memchr` — SIMD-accelerated fixed=TRUE grep/grepl (vendored, plan exists)

### Deferred

- [ ] Arrow backend for vector storage
- [ ] Full S4 inheritance chain resolution
- [ ] Polars-backed data frames
- [ ] Language object `[[` indexing (blocks `body(f)[[2]] <- val`)
