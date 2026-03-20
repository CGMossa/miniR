# TODO — Remaining Work

## Done

768 builtins, 1048 tests, 7014/7014 R parse, 10838/10841 Rd parse,
5956/6748 CRAN source (88%), 117 packages at 100%, 0 crashes.

## Open

### Near-term — improve CRAN source rate

The remaining 12% failures are dominated by cross-package dependencies:
- `R6Class` / `R6::R6Class` — need library(R6) to work
- `rlang::abort`, `on_load` — need library(rlang)
- `magrittr::%>%` — need library(magrittr)
- `tidyselect::contains` — need library(tidyselect)

These need `library()` to actually load R packages from .libPaths(),
which is implemented but needs real installed packages on disk.

### WASM target

- [x] reedline/crossterm/nu-ansi-term gated behind `repl` feature
- [ ] `linkme` (distributed_slice) doesn't support wasm32

### Graphics

- [ ] egui-based interactive plotting
- [ ] SVG/PNG file device output

### Performance

- [x] `fnv` — faster HashMap for environment lookups
- [ ] `smallvec` — stack-allocated small vectors for attrs/short args

### Deferred

- [ ] Arrow backend for vector storage
- [ ] Full S4 inheritance chain resolution
- [ ] Polars-backed data frames
- [ ] Language object `[[` indexing (blocks `body(f)[[2]] <- val`)
