# Implementation Plan

Current ordered plan for improving CRAN compatibility without reopening foundations that have already landed.

## Current State (2026-03-12)

- The parser and custom parse-error pipeline are in place.
- The runtime already has attributes on vectors and lists, language objects, factors, matrices/arrays, regex helpers, and a basic `data.frame()`.
- The interpreter also has substantial metaprogramming support (`quote`, `parse`, `eval`, `substitute`, `deparse`, `bquote`) plus partial S3 dispatch and `NextMethod()`.
- The biggest remaining compatibility gaps are now call-stack semantics, direct `UseMethod()`, fuller data-frame behavior, package/runtime I/O features, date/time, and the multi-interpreter embedding story.
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

4. Package/runtime surface
   - Implement serialization: `readRDS()`, `saveRDS()`, `load()`, `save()`.
   - Implement connection objects and `url()`, `connection()`, `open()`, `close()`.
   - Replace package-loading stubs with a minimal namespace/package loading story.

5. Date/time support
   - `as.POSIXct()`, `as.POSIXlt()`, `ISOdate()`, `ISOdatetime()`, `strptime()`, `strftime()`, and related helpers.
   - Fill in the remaining high-frequency date/time helpers once the basic representation is chosen.

6. Reentrancy and embedding cleanup
   - Keep thread-local access for builtin plumbing, but stop treating the TLS interpreter as the only public instance model.
   - Expose an instance-oriented API that can host multiple interpreters on the same thread/process.

7. Long-tail stubs
   - Linear algebra decompositions, S4, graphics, finalizers, package install UX, and other lower-frequency compatibility work.

## Verified Low-Risk Cleanup

- Keep repo docs in sync with the current runtime before using them to prioritize work.
- Keep tooling aligned with the current parser error format and CLI behavior.

## Discipline

- Split logical changes into small commits.
- Before each commit run `cargo fmt`, `cargo clippy -- -D warnings`, and `cargo test`.
- Keep `README.md`, `TODO.md`, and `DONE.md` in sync with any behavior change.
