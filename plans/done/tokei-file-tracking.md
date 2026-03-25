# File Size Tracking with tokei

Add `tokei` as a development tool to routinely monitor file sizes and flag files that need refactoring (splitting into submodules).

## Motivation

Several files in the codebase are very large:

- `builtins.rs` — 93KB
- `math.rs` — 63KB
- `interp.rs` — 62KB
- `strings.rs` — 38KB
- `value.rs` — 28KB
- `system.rs` — 23KB

Large files are harder to navigate, review, and reason about. A routine `tokei` check surfaces which files have grown past a threshold and should be split.

## Implementation

1. Add a `just loc` recipe that runs `tokei -f -e vendor/ -e doc/ -e share/ -e plans/ -e tests/ -e scripts/ -e tools/` and sorts by lines of code
2. Add a `just big-files` recipe that filters for files over 1000 lines and prints them as candidates for refactoring
3. Document the threshold in CLAUDE.md: routinely consider running `just big-files` to see if there are a refactoring that is in order.

## Refactoring candidates (current)

- `builtins.rs` — split into more submodules (data-structure builtins, type-conversion builtins, attribute builtins)
- `math.rs` — split into arithmetic, sequences, matrix ops, bitwise
- `interp.rs` — split into apply-family, eval-family, environment-builtins, S3-dispatch
- `value.rs` — split the Vector impl methods into a separate `value/vector_ops.rs`

## Prerequisites

- `tokei` installed (`cargo install tokei` or `brew install tokei`)
