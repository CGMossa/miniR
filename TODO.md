# TODO — Remaining Work

## Stats

800+ builtins, 1100+ tests, 7014/7014 R parse, 10838/10841 Rd parse,
751 .Rd man pages generated. Interactive egui plotting with SVG/PDF/PNG export.

## Open

### Language Features

- [x] `%>%` as alias for `|>` with `.` placeholder (plan: magrittr-pipes.md)
- [x] `%<>%` assignment pipe (plan: magrittr-pipes.md)
- [x] `%T>%` tee pipe (plan: magrittr-pipes.md)
- [x] `%$%` exposition pipe (plan: magrittr-pipes.md)
- [x] Chained replacement: `body(f)[[2]][[2]] <- val` (review: session-issues)
- [x] `<<-` with compound targets at global level (review: session-issues)
- [ ] Full S4 inheritance chain resolution

### Grid Graphics

- [ ] Unit system (npc, cm, inches, native, null, strwidth, etc.)
- [ ] Viewport tree (push/pop, transforms, data scales)
- [ ] Grob primitives (lines, rect, circle, polygon, text, points)
- [ ] Display list (record/replay/edit)
- [ ] Layout system (grid.layout, cell positioning)
- [ ] ggplot2 rendering via grid
- Plan: grid-graphics.md

### WASM Target

- [x] reedline/crossterm gated behind `repl` feature
- [ ] Fork linkme for wasm32 support (plan: linkme-wasm.md)
- Alternative: build-script registry for WASM builds

### Package System (from review)

- [ ] `library(pkg)` should accept unquoted names (NSE)
- [ ] `.libPaths()` and `get_lib_paths()` disagree — FIXED
- [ ] `search()` order wrong after attach
- [ ] `asNamespace()` returns NULL — FIXED
- [ ] `isNamespace()` wrong for non-namespace envs — FIXED
- [ ] Stub warnings bypass session writers — FIXED

### Performance

- [x] `fnv` — faster HashMap for environment lookups
- [x] `smallvec` — stack-allocated call args
- [x] `zmij` — fast f64 formatting
- [x] `aho-corasick` — SIMD fixed-pattern grep
- [ ] Arrow/Polars backend for vector storage (deferred)

## Done (this session)

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
- plot(y ~ x) formula + log="xy" axes
- Color palettes (rainbow, heat.colors, hsv, hcl, colorRampPalette)
- 657 named colors, par() state
- SVG file device (real SVG output)
- PDF file device (real PDF via krilla)
- PNG export (SVG→resvg→PNG)
- View(df) in egui table with filter/sort/stats/CSV export
- Native file dialogs (rfd)
- Dark/light theme with persistence
- Collapsible plot sidebar with sliders
- Context menus (plot + table)
- CSV drag-and-drop
- Floating Column Statistics window
- Keyboard shortcuts (Cmd+W, Ctrl+Tab)
- `_` placeholder for |> pipe (R 4.2+)
- 751 .Rd files via --generate-docs
- Dep consolidation (removed env_logger, termcolor, log)
- Upgraded arrow/parquet to v58, krilla to v0.6
