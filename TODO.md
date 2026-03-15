# TODO — Remaining Stubs and Partial Implementations

This file tracks behavior that is still stubbed, placeholder, or materially simplified.

## Evaluator Correctness (from 2026-03-15 review)

- [ ] `<<-` creates bindings in base instead of global — `set_super()` walks past global into base when no existing binding is found
- [ ] Builtin `min_args` never enforced — dispatcher checks `max_args` only; `length()` silently returns 0 instead of erroring
- [ ] Closure argument matching too weak — no partial matching, no unused-argument error, no duplicate detection
- [ ] `system.time()` measures post-eval — eager builtin times the already-evaluated result, needs pre-eval conversion
- [ ] Top-level `NULL` always hidden — `session.rs` visibility suppresses `!value.is_null()`, should only suppress `invisible()`

## Type Stability and Attributes (from 2026-03-15 review)

- [ ] Assignment collapses types — `x[1] <- 2L` on integer vector produces double; replacement always goes through `to_doubles()`
- [ ] Assignment strips attributes — `m[1] <- 9L` on matrix drops `dim`/`dimnames`
- [ ] Arithmetic strips attributes — `m + 1` on matrix drops `dim`; `c(a=1, b=2)` loses names
- [ ] Matrix subsetting collapses to double — `m[1, ]` on integer matrix returns double; character matrices return NA
- [ ] Logical index recycling missing — `x[c(TRUE, FALSE)]` on length-4 vector returns only element 1 instead of 1 and 3
- [ ] Mixed positive/negative indices not validated — `x[c(-1, 2)]` should error, currently returns wrong result
- [ ] Matrix dimname indexing not supported — `m["r1", "c1"]` returns empty instead of looking up by dimnames
- [ ] Data frame row-name preservation broken — `df[2:3, ]` resets row names to 1:2 instead of preserving selected names

## Interpreter Isolation (from 2026-03-15 review)

- [ ] `Sys.setenv()` and `setwd()` mutate process-global state — should be interpreter-local or explicitly documented as host-level
- [ ] `Sys.which()` not portable — splits PATH with `:` directly, ignores Windows PATHEXT

## Testing

- [ ] Parse corpus test has no regression baseline — counts failures but never asserts on the count
- [ ] REPL startup can crash on history-file issues — `FileBackedHistory::with_file().expect()` should fall back to in-memory

## Simplified Data and Object Semantics

- [ ] `data.frame(...)` — subsetting edge cases, coercions, and fuller R-compatible behavior still need work

## Remaining Builtin Stubs

- [ ] `qr(x)`, `svd(x)`, `eigen(x)`, `det(x)`, `chol(x)` — linear algebra decompositions
- [ ] GNU-R-compatible binary serialization for `readRDS()`, `saveRDS()`, `load()`, and `save()`
- [ ] `url(...)`, `connection(...)`, `open(con)`, `close(con)` — connection objects
- [ ] `install.packages(pkgs)`, `library(pkg)`, `require(pkg)` — package loading and management
- [ ] `setClass(Class, ...)`, `setMethod(f, ...)`, `setGeneric(name, ...)` — S4
- [ ] `pdf(...)`, `dev.off()`, `plot(...)`, `lm(formula, data)` — graphics and modeling stubs
- [ ] `reg.finalizer(e, f)` — finalizers

## Architecture and Cleanup

- [ ] `RError` cleanup — continue extracting module-specific error types
- [ ] Arrow backend for vector storage — replace `Vec<Option<T>>` with contiguous storage + validity bitmaps
- [ ] Feature-gate the I/O module for sandboxed/WASM environments
- [ ] Large files: `builtins.rs` (3212 lines), `value.rs` (1157 lines), `parser.rs` (906 lines)

## Developer Experience

- [ ] Add tokei for file size tracking and refactoring detection
- [ ] Add a vendor patch workflow for intentional edits under `vendor/`
