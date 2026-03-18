# TODO — Remaining Work

## Done

Everything from the core interpreter, package runtime, serialization,
builtins, parser, S4, REPL, help system, and bug fixes is complete.
994 tests pass. 7014/7014 R files parse. 10838/10841 Rd files parse.

## Open

### Near-term

- [ ] `pnorm()` precision — erfc approximation gives ~1.5e-7 accuracy, use libm::erfc or better algorithm
- [ ] `Sys.time()` — verify subsecond precision works correctly across platforms

### Graphics

- [ ] egui-based interactive plotting (see `plans/egui-graphics.md`)
- [ ] SVG/PNG file device output

### Deferred

- [ ] Arrow backend for vector storage
- [ ] WASM target support (test with `--no-default-features -F minimal`)
- [ ] Full S4 inheritance chain resolution (basic registry works)
- [ ] Polars-backed data frames
