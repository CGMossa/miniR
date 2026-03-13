# TODO — Remaining Stubs and Partial Implementations

This file tracks behavior that is still stubbed, placeholder, or materially simplified.

## Simplified Data and Object Semantics

- [ ] `data.frame(...)` — the common constructor path now handles recycling, `row.names`, list/matrix expansion, and `stringsAsFactors`, but subsetting edge cases, coercions, and fuller R-compatible behavior still need work
- [ ] Attribute, name, and class propagation still needs cleanup across more subsetting and combination paths

## Remaining Builtin Stubs

- [ ] `qr(x)`, `svd(x)`, `eigen(x)`, `det(x)`, `chol(x)` — linear algebra decompositions
- [ ] `load(file)`, `save(..., file)` — workspace save/load on top of the new `readRDS()` / `saveRDS()` path
- [ ] `url(...)`, `connection(...)`, `open(con)`, `close(con)` — connection objects
- [ ] `install.packages(pkgs)`, `installed.packages()`, `library(pkg)`, `require(pkg)`, `loadNamespace(pkg)`, `requireNamespace(pkg)` — package loading and management
- [ ] `as.POSIXct(x)`, `as.POSIXlt(x)`, `ISOdate(...)`, `ISOdatetime(...)`, `strptime(x, format)`, `strftime(x, format)` — date/time support
- [ ] `setClass(Class, ...)`, `setMethod(f, ...)`, `setGeneric(name, ...)` — S4
- [ ] `pdf(...)`, `dev.off()`, `plot(...)`, `lm(formula, data)` — graphics and modeling stubs
- [ ] `reg.finalizer(e, f)` — finalizers

## Architecture and Cleanup

- [ ] Reentrant embedding API — keep TLS access for builtins, but stop treating the thread-local interpreter as the only public instance model
- [ ] `RError` cleanup — continue extracting module-specific error types as more external errors are wrapped
- [ ] Arrow backend for vector storage — replace `Vec<Option<T>>` with contiguous storage + validity bitmaps
- [ ] Feature-gate the I/O module for sandboxed/WASM environments
- [ ] Plan an R package builder
- [ ] Add Typst conversion of R documentation and produce the manual

## Developer Experience

- [ ] Add tokei for file size tracking and refactoring detection
- [ ] Add a vendor patch workflow for intentional edits under `vendor/`
