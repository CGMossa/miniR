# Implementation Plan

Current ordered plan for improving CRAN compatibility without reopening foundations that have already landed.

## Current State (2026-03-15)

- The parser and custom parse-error pipeline are in place.
- The runtime already has attributes on vectors and lists, language objects, factors, matrices/arrays, regex helpers, and a basic `data.frame()`.
- The interpreter also has substantial metaprogramming support (`quote`, `parse`, `eval`, `substitute`, `deparse`, `bquote`) plus partial S3 dispatch and `NextMethod()`.
- A scan of the checked-in `cran/` corpus (`analysis/cran-corpus-scan.md`) shows that the next blockers are package-runtime-heavy: namespace/package loading, native routine support, base/recommended package namespaces, object/data-model fidelity, graphics, connections/serialization, and date/time.
- The biggest remaining compatibility gaps are therefore call-stack semantics, package and namespace runtime, native extension loading, fuller data-frame behavior, graphics/I/O/date-time support, and the multi-interpreter embedding story.
- This plan is intentionally flat. Older phase-based plans in this repo are historical snapshots unless they have been refreshed.

## Ordered Priorities

1. Call stack and supplied-argument tracking
   - Add real call frames for closure calls.
   - Implement `sys.call()`, `sys.frame()`, `sys.frames()`, `sys.parents()`, `sys.function()`, `sys.on.exit()`, `parent.frame()`, `sys.nframe()`, `nargs()`, and `missing()`.
   - Reconcile the existing `on.exit()` environment mechanism with the new frame model.

2. S3 cleanup and generic consistency
   - Replace the direct `UseMethod()` noop with real dispatch.
   - Keep `NextMethod()` aligned with the same call-frame context.
   - Audit generics that still rely on partial or implicit dispatch paths.

3. Finish the simplified object/data model
   - Complete `data.frame()` semantics: recycling, validation, row/column subsetting edge cases, and coercions.
   - Fix helpers such as `as.vector()` that still return simplified or attribute-preserving results.
   - Continue name/class/dim propagation through subsetting and combination operations.

4. Package and namespace runtime
   - Replace package-loading stubs with a real namespace/package loading story.
   - Parse `DESCRIPTION` and `NAMESPACE`, build namespace environments, and support `library()`, `require()`, `requireNamespace()`, `loadNamespace()`, `::`, and `:::`.
   - Run package hooks such as `.onLoad()` and `.onAttach()`, expose package environments on the search path, and make package datasets/lazy data discoverable.
   - Treat package incorporation as asset staging, not just sourcing `R/`: preserve `data/`, `inst/`, `man/`, and other package assets.
   - Make core package namespaces (`utils`, `stats`, `methods`, `graphics`, `grDevices`, `grid`, `tools`) importable as packages instead of treating them as loose builtin buckets.

5. Native extension loading
   - Compile package `src/` trees with the platform C/C++/Fortran toolchain and expose `inst/include` for `LinkingTo:` dependencies.
   - Load package shared libraries declared via `useDynLib()` and keep interpreter-local `DllInfo`-style state for them.
   - Support registered native routines and `.Call()` first, then `.External()` / `.C()` / `.Fortran()`.
   - Support `R_registerRoutines()`, `R_useDynamicSymbols()`, `R_forceSymbols()`, `R_RegisterCCallable()`, and `R_GetCCallable()`.
   - Keep native-library state interpreter-local so multiple miniR instances can coexist safely.

6. Package documentation and help assets
   - Preserve and index `man/*.Rd`, `inst/doc/`, and vignette assets as part of package incorporation.
   - Back `help()`, `?topic`, package help pages, and alias lookup with a real Rd/help index instead of stubs.
   - Build a staged Rd parser/indexer: metadata and alias extraction first, richer rendering and macro support later.

7. Runtime I/O and serialization
   - Implement connection objects and `url()`, `connection()`, `open()`, `close()`.
   - Improve serialization compatibility for `readRDS()`, `saveRDS()`, `load()`, and `save()`.
   - Keep temp paths, environment variables, and working directory behavior coherent for package code and tests.

8. Date/time support
   - `as.POSIXct()`, `as.POSIXlt()`, `ISOdate()`, `ISOdatetime()`, `strptime()`, `strftime()`, and related helpers.
   - Fill in the remaining high-frequency date/time helpers once the basic representation is chosen.

9. Reentrancy and embedding cleanup
   - Keep thread-local access for builtin plumbing, but stop treating the TLS interpreter as the only public instance model.
   - Expose an instance-oriented API that can host multiple interpreters on the same thread/process.

10. Long-tail stubs
   - S4 depth, graphics device depth, linear algebra decompositions, finalizers, package install UX, and other lower-frequency compatibility work.

## Verified Low-Risk Cleanup

- Keep repo docs in sync with the current runtime before using them to prioritize work.
- Keep tooling aligned with the current parser error format and CLI behavior.

## Discipline

- Split logical changes into small commits.
- Before each commit run `cargo fmt`, `cargo clippy -- -D warnings`, and `cargo test`.
- Keep `README.md`, `TODO.md`, and `DONE.md` in sync with any behavior change.
