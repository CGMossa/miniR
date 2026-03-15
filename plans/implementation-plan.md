# Implementation Plan

Current ordered plan for improving CRAN compatibility without reopening foundations that have already landed.

## Current State (2026-03-15)

- The parser and custom parse-error pipeline are in place.
- The runtime already has attributes on vectors and lists, language objects, factors, matrices/arrays, regex helpers, and a basic `data.frame()`.
- The interpreter also has substantial metaprogramming support (`quote`, `parse`, `eval`, `substitute`, `deparse`, `bquote`) plus partial S3 dispatch and `NextMethod()`.
- Call stack semantics are implemented: `sys.call()`, `sys.frame()`, `parent.frame()`, `nargs()`, `missing()`, `on.exit()`.
- S3 dispatch is coherent: `UseMethod()` dispatches properly, `print()` and `format()` are S3 generics.
- Date/time support is implemented via jiff: `Sys.Date()`, `as.Date()`, `as.POSIXct()`, `strptime()`, `strftime()`, etc.
- Reentrancy is complete: all builtins use `BuiltinContext`, per-interpreter env vars/cwd, parallel thread isolation tested.
- Type stability landed: assignment/indexing/arithmetic preserve vector types and attributes.
- Three-pass argument matching: exact → partial → positional, with unused-argument errors.
- Help system: `?name` displays docs from rustdoc comments on builtins.
- A scan of the checked-in `cran/` corpus (`analysis/cran-corpus-scan.md`) shows that the next blockers are package-runtime-heavy: namespace/package loading, native routine support, base/recommended package namespaces, graphics, connections/serialization.

## Shipped (no longer priorities)

- ~~Call stack and supplied-argument tracking~~ — landed
- ~~S3 cleanup and generic consistency~~ — landed
- ~~Date/time support~~ — landed (jiff)
- ~~Reentrancy and embedding cleanup~~ — landed (BuiltinContext, per-interpreter state)
- ~~Type stability and attribute preservation~~ — landed

## Ordered Priorities

1. Package and namespace runtime
   - Replace package-loading stubs with a real namespace/package loading story.
   - Parse `DESCRIPTION` and `NAMESPACE`, build namespace environments, and support `library()`, `require()`, `requireNamespace()`, `loadNamespace()`, `::`, and `:::`.
   - Run package hooks such as `.onLoad()` and `.onAttach()`, expose package environments on the search path, and make package datasets/lazy data discoverable.
   - Treat package incorporation as asset staging, not just sourcing `R/`: preserve `data/`, `inst/`, `man/`, and other package assets.
   - Make core package namespaces (`utils`, `stats`, `methods`, `graphics`, `grDevices`, `grid`, `tools`) importable as packages instead of treating them as loose builtin buckets.

2. Finish the simplified object/data model
   - Complete `data.frame()` semantics: recycling, validation, row/column subsetting edge cases, and coercions.
   - Fix helpers such as `as.vector()` that still return simplified or attribute-preserving results.
   - Continue name/class/dim propagation through subsetting and combination operations.

3. Native extension loading
   - Compile package `src/` trees with the platform C/C++/Fortran toolchain and expose `inst/include` for `LinkingTo:` dependencies.
   - Load package shared libraries declared via `useDynLib()` and keep interpreter-local `DllInfo`-style state for them.
   - Support registered native routines and `.Call()` first, then `.External()` / `.C()` / `.Fortran()`.
   - Keep native-library state interpreter-local so multiple miniR instances can coexist safely.

4. Package documentation and help assets
   - Preserve and index `man/*.Rd`, `inst/doc/`, and vignette assets as part of package incorporation.
   - Back `help()`, `?topic`, package help pages, and alias lookup with a real Rd/help index instead of stubs.
   - Build a staged Rd parser/indexer: metadata and alias extraction first, richer rendering and macro support later.

5. Runtime I/O and serialization
   - Implement connection objects and `url()`, `connection()`, `open()`, `close()`.
   - Improve serialization compatibility for `readRDS()`, `saveRDS()`, `load()`, and `save()`.

6. Long-tail stubs
   - S4 depth, graphics device depth, linear algebra decompositions, finalizers, package install UX, and other lower-frequency compatibility work.

## Discipline

- Split logical changes into small commits.
- Before each commit run `cargo fmt`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test`.
- Keep `README.md`, `TODO.md`, and `DONE.md` in sync with any behavior change.
