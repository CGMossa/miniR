# TODO — Remaining Work

## Done

Core interpreter, package runtime, serialization, builtins, parser, S4,
REPL, help system, bug fixes. 994 tests, 7014/7014 R parse, 10838/10841
Rd parse, 5669/6769 CRAN source files (83%), 95 packages at 100%.

## Open

### Near-term

- [ ] WASM target support (test `--no-default-features -F minimal` compiles for wasm32-unknown-unknown)
- [ ] Improve CRAN source rate: implement missing functions that block the most packages (.Call stub, getClassDef, .POSIXct)

### Graphics

- [ ] egui-based interactive plotting (see `plans/egui-graphics.md`)
- [ ] SVG/PNG file device output

### Deferred

- [ ] Arrow backend for vector storage
- [ ] Full S4 inheritance chain resolution (basic registry works)
- [ ] Polars-backed data frames
