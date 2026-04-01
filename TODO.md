# TODO — Remaining Work

## Stats

800+ builtins, 1700+ tests, 7014/7014 R parse, 10838/10841 Rd parse,
751 .Rd man pages generated. Interactive egui plotting with SVG/PDF/PNG export.
131/260 CRAN packages load (50%+). Tidyverse core: rlang, dplyr, tibble, purrr, vctrs, forcats, tidyselect.
Also: knitr, shiny, ggpubr, plotly, broom, dbplyr, readr, rvest, renv, xml2, tidyverse.

## Open

### S7 Class System (blocks ggplot2)

- [ ] `new_class()` / `class_function` / `class_any` / `class_missing`
- [ ] `new_generic()` / `method()` / `method<-()`
- [ ] `S7_object()` base class, `@` slot access via S7
- [ ] `convert()` / `super()` dispatch
- Blocks: ggplot2, S7, Hmisc

### Native Compilation Gaps (system deps)

- [ ] `fs` → needs libuv headers — blocks DT, devtools, gargle, htmlwidgets, pkgdown, rmarkdown, usethis (7 pkgs)
- [ ] `ps` → needs configure-generated config.h — blocks processx, rcmdcheck, reprex, testthat (4 pkgs)
- [ ] `stringi` → needs ICU headers — blocks stringr, tidyr (2 pkgs)
- [ ] `openssl` → needs OpenSSL headers — blocks httr, covr
- [ ] `Matrix` → needs SuiteSparse — blocks igraph, car
- [ ] `timechange` → C++ compilation failure — blocks lubridate
- Strategy: Rust -sys crates for curl/openssl (plan: system-deps-strategy.md)

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

## Done (2026-04-01 session)

- Language `[[<-` assignment + chained replacement (`body(f)[[2]][[2]] <- val`)
- `<<-` with compound targets at global level
- S3 dispatch for binary operators (`|`, `+`, `==`, etc.)
- List comparison operators (element-wise `==`/`!=`)
- `library()` for base packages (no-op), `character.only=TRUE`
- Base package synthetic namespace registration
- `I()` (AsIs), `args()` fix, `vapply` named args
- Grob constructors (textGrob, rectGrob, etc.) with just normalization
- C API: Rconn struct, Rf_isPairList, R_check_class_etc, R_new_custom_connection
- NULL handling in sub/gsub/grep/grepl
- Modulo-zero fix for empty vector comparison
- stats4/translations added to base package list

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
