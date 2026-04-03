# TODO — Remaining Work

## Stats

800+ builtins, 1700+ tests, 7014/7014 R parse, 10838/10841 Rd parse,
751 .Rd man pages generated. Interactive egui plotting with SVG/PDF/PNG export.
154/260 CRAN packages load (59%). Tidyverse core: rlang, dplyr, tibble, purrr, vctrs, forcats, tidyselect.
Also: knitr, bslib, htmlwidgets, rmarkdown, Rcpp, fs, xml2, lmtest, classInt, quadprog, tseries, urca.
Plus: ggpubr, plotly, broom, dbplyr, readr, tidyverse, sass, sodium, sp, fontawesome.

## Open

### S7 Class System (blocks ggplot2)

- [ ] `new_class()` / `class_function` / `class_any` / `class_missing`
- [ ] `new_generic()` / `method()` / `method<-()`
- [ ] `S7_object()` base class, `@` slot access via S7
- [ ] `convert()` / `super()` dispatch
- Blocks: ggplot2, S7, Hmisc

### Native Compilation Gaps

- [x] `fs` → system libuv via pkg-config (done)
- [x] `xml2` → system libxml2 via pkg-config (done)
- [x] `sass` → system libsass via pkg-config (done)
- [x] `openssl` → compiles via pkg-config, segfaults on load (C API gap)
- [ ] `later` → compiles, segfaults on load — blocks promises, httpuv, shiny, DT
- [ ] `ps` → compiles, segfaults on load — blocks processx, callr, testthat (5 pkgs)
- [ ] `stringi` → needs full configure emulation for ICU — blocks stringr, tidyr
- [ ] `Matrix` → needs SuiteSparse build + more Lapack — blocks igraph, car, survival
- [x] Fortran compilation (`.f` files) — gfortran invocation in compile.rs
- [ ] `delayedAssign` via `do.call` in rlang `on_load` hooks — blocks ~15 packages
- Strategy: pkg-config + configure emulation (plan: system-deps-strategy.md)

### Language Features

- [ ] Full S4 inheritance chain resolution
- [ ] `format(digits=)` ignores digits parameter
- [ ] `rm(x)` NSE for bare names
- [ ] `aggregate()` formula interface
- [ ] Grob editing (grid.edit, grid.get, grid.set)

### WASM Target

- [x] reedline/crossterm gated behind `repl` feature
- [ ] Fork linkme for wasm32 support (plan: linkme-wasm.md)

### Performance

- [x] `fnv` — faster HashMap for environment lookups
- [x] `smallvec` — stack-allocated call args
- [x] `zmij` — fast f64 formatting
- [x] `aho-corasick` — SIMD fixed-pattern grep
- [ ] Arrow/Polars backend for vector storage (deferred)

## Done (2026-04-01 — 2026-04-03 session)

- Language `[[<-` assignment + chained replacement (`body(f)[[2]][[2]] <- val`)
- `<<-` with compound targets at global level
- S3 dispatch for binary operators (`|`, `+`, `==`, etc.) with env-aware lookup
- Custom `%op%` dispatch (SpecialOp::Other carries operator name)
- List comparison operators (element-wise `==`/`!=`)
- `library()` for base packages (no-op), `character.only=TRUE`
- Base package synthetic namespace registration
- Namespace pre-registration (prevents infinite recursion)
- `I()` (AsIs), `args()`, `vapply` named args, `.Primitive()`, `as.function`
- `find.package`, `getNamespaceInfo`, `.GlobalEnv`/`.BaseNamespaceEnv` bindings
- Grob constructors (textGrob, rectGrob, etc.) with just normalization
- C API: Rconn struct, Rf_isPairList, R_check_class_etc, R_new_custom_connection
- C API: R_ext/Lapack.h, eventloop.h, libextern.h, Connections.h v1
- C API: Rf_warningcall_immediate, Rvprintf/REvprintf, R_CheckStack, R_interrupts_pending
- pkg-config integration for Makevars.in anticonf resolution
- Configure emulation for ps (config.h), fs (system libuv), sass (system libsass)
- Makevars parser: user-defined variable expansion, value trimming, quote stripping
- PCRE regex `\]` compatibility in character classes
- TRUE/FALSE macro conflict fix for macOS system headers
- NULL handling in sub/gsub/grep/grepl, unlist(list(NULL)) fix
- `raw()` zero-arg, `anyDuplicated`, `removeSource`, `duplicated`
- Adaptive eval depth from measured type sizes + stack pointer guard
- Fortran compilation via gfortran with runtime library discovery
- BLAS.h/Lapack.h with correct return types (ddot→double, etc.)
- R_ext/eventloop.h, libextern.h, BLAS.h headers
- `delayedAssign` pre-eval builtin + do.call handler
- NAMESPACE parser: semicolon-separated directives on one line
- unlist(list(NULL)) → NULL fix
- `emulate_configure_system_lib` for sass (system libsass)
- `emulate_configure_fs` for fs (system libuv)
- `emulate_configure_ps` for ps (config.h generation)

## Done (previous sessions)

- Named-arg dispatch for all builtins (formals matching at dispatch)
- data.frame() forward references
- Language `[[` indexing + length()
- REPL parameter completion
- Regex NA/coercion fixes (tolower, regexpr, gregexpr)
- Type-checking fixes (is.call, is.pairlist, is.recursive, is.numeric)
- Timing builtins (proc.time class, print methods)
- String builtins (vectorized startsWith/endsWith, trimws, encodeString)
- egui_plot interactive window (non-blocking, tabbed, windowed mode)
- plot(), hist(), barplot(), boxplot(), pairs() with rendering
- Color palettes (rainbow, heat.colors, hsv, hcl, colorRampPalette)
- SVG/PDF/PNG file devices
- View(df) in egui table with filter/sort/stats/CSV export
- Grid graphics: units, viewports, grobs, gpar, display list, layout
- `%>%` `%<>%` `%T>%` `%$%` pipes
- `_` placeholder for |> pipe (R 4.2+)
- `:=` walrus assignment
- Lazy evaluation (promises)
- 751 .Rd files via --generate-docs
