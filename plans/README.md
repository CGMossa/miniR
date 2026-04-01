# plans

Design notes, implementation plans, and dependency evaluations for miniR.

This directory mixes three kinds of documents:

- Active implementation plans for still-open interpreter work
- Dependency and vendor evaluation notes
- Historical snapshots written before later implementation landed

Before acting on a plan, compare it against the current code plus `README.md`, `TODO.md`, and `DONE.md`. Dated "Current State" sections are especially likely to drift.

## Status (2026-04-01)

131/260 CRAN packages load (50%+). Tidyverse core works. Key active plans:

- `package-runtime.md` — **Phases A-D DONE.** Remaining: S7, system deps
- `grid-graphics.md` — **Core DONE.** Remaining: grob editing, ggplot2 (needs S7)
- `system-deps-strategy.md` — strategy for fs/ps/openssl/ICU/SuiteSparse
- `gc-arena.md` — deferred GC for Rc cycles
- `polars-dataframe.md` — Polars backend for data frames (not started)

Most crate-named plans are dependency evaluations. Keep them for integration context, but do not treat them as the current execution order.
