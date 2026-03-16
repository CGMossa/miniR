# Missing Tests Plan

Test coverage gap analysis based on a full codebase audit.

## Priority 1 — Critical Gaps

### Matrix decomposition error handling

- `qr()`, `svd()`, `chol()`, `solve()` on singular/ill-conditioned matrices
- `eigen()` on non-symmetric input
- `det()` on singular matrix (should return 0)
- Test type: Session API with `stopifnot` and `tryCatch`

### CSV/table I/O robustness

- `read.csv()` with malformed CSV (missing quotes, uneven columns)
- `read.table()` with various `sep`, `quote`, `header` combos
- NA strings, encoding variations, CRLF line endings
- Test type: temp file + Session API

### Indexing and assignment edge cases

- `x[100] <- value` on length-10 vector (auto-extension)
- NA indices in `[` and `[[`
- Recycling in assignment (`x[1:3] <- c(10, 20)`)
- Named index with duplicates
- `drop=FALSE` on matrix single-row/column selection
- Test type: Session API with `stopifnot`

### Factor operations

- `factor()` with `labels` parameter
- `factor()` with `exclude = NA`
- `as.numeric()` on factor (returns underlying integer codes)
- Ordered factor comparisons
- `table()` with multiple factors, `useNA` options
- Test type: Session API

## Priority 2 — High Gaps

### Type coercion rules

- Mixed-type arithmetic: `1L + 1.0` stays double, `1L + 1L` stays integer
- `as.numeric()` on character with non-numeric values (should produce NA + warning)
- `as.integer()` overflow behavior
- Coercion ordering: raw < logical < integer < double < complex < character
- Test type: Session API with type checks

### Regex edge cases

- `strsplit()` with regex vs `fixed = TRUE`
- `gsub()` with backreferences
- `grepl()` with `ignore.case` and `perl` flags
- Empty pattern matching
- Test type: Session API

### Date/time edge cases

- Timezone conversions across DST boundaries
- `as.Date()` with various format strings
- `difftime()` with mixed units
- `strptime()` with locale-specific formats
- Test type: Session API

### Condition system depth

- `tryCatch()` with multiple handlers and ordering
- `withCallingHandlers()` nesting
- `tryCatch` with `finally` in error path
- Warning/message muffling
- Test type: Session API

### S3 dispatch depth

- Group generic dispatch (Ops, Math, Summary groups)
- `NextMethod()` with modified arguments
- Method lookup in package namespaces (when package loading lands)
- Test type: Session API

## Priority 3 — Medium Gaps

### Connection objects

- `file()` + `readLines()` + `close()` lifecycle
- `writeLines()` to connection vs path
- `isOpen()` state tracking
- stdin/stdout/stderr connection IDs
- Test type: Session API with temp files

### Graphics stubs

- Verify stubs don't crash: `pdf()`, `plot()`, `dev.off()`
- `par()` returns a list
- Test type: Session API (just verify no errors)

### S4 stubs

- `setClass()` + `new()` creates object with class attr
- `slot()` extracts named elements
- `is()` checks class membership
- Test type: Session API

### Parser edge cases

- Raw string literals `r"(...)"`, `R"[...]"`
- Unicode escape sequences in strings
- Very deeply nested expressions
- Hex float literals (`0x1.5p3`)
- Empty function bodies, trailing commas in arg lists
- Test type: Session API (parse + eval)

### Environment edge cases

- Complex closure capture (closures over loop variables)
- `<<-` from deeply nested scopes
- `environment()` on no-arg returns calling env
- Test type: Session API

### Options system

- `options(digits = 3)` affects print output
- `getOption()` with default value (named `default` arg)
- `options()` with no args returns all options
- Test type: Session API

### Signal handling

- Ctrl-C during `for` loop interrupts (hard to test automatically)
- `Sys.sleep()` can be interrupted
- Test type: manual or timeout-based

## Existing Coverage Summary

| Module | Builtins | Coverage | Notes |
|--------|----------|----------|-------|
| math.rs | ~80 | Moderate | Good basic coverage, weak on edge cases |
| strings.rs | ~37 | Good | regex.R covers most paths |
| types.rs | ~24 | Good | primitives.R covers basics |
| system.rs | ~24 | Moderate | File ops partially tested |
| conditions.rs | ~10 | Good | conditions.R is thorough |
| datetime.rs | ~15 | Good | Multiple datetime test files |
| random.rs | ~17 | Excellent | Dedicated random.R + distribution tests |
| io.rs | ~8 | Moderate | CSV edge cases untested |
| graphics.rs | ~14 | Poor | Stubs only, no real tests |
| s4.rs | ~12 | Poor | Stubs only |
| factors.rs | ~3 | Poor | Very limited |
| tables.rs | ~2 | Poor | Very limited |
| connections.rs | ~8 | Poor | New, untested |
| coercion.rs | ~6 | Moderate | Basic paths covered |

## Test file structure

New tests should use the Session API pattern:
```rust
use r::Session;

#[test]
fn descriptive_test_name() {
    let mut r = Session::new();
    r.eval_source(r#"
        stopifnot(...)
    "#).unwrap();
}
```

Use `r` as the variable name for Session (it's an R session).

Group related tests in the same file:
- `tests/stdlib.rs` — stdlib builtins (options, .Machine, etc.)
- `tests/linalg.rs` — matrix decompositions (planned)
- `tests/io_edge_cases.rs` — CSV/table edge cases (planned)
- `tests/factor_table.rs` — factor and table ops (planned)
- `tests/coercion.rs` — type coercion rules (planned)
