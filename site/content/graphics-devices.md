+++
title = "Graphics And Devices"
weight = 7
description = "How base graphics, grid graphics, file devices, and the optional GUI fit together"
+++

miniR's graphics stack is layered. The important split is between data model, R-facing builtins, file devices, and the optional GUI.

## Interpreter-Owned Graphics State

Graphics state lives on `Interpreter`, not in process-global statics. The important pieces are:

- `par_state` for base-graphics parameters
- `current_plot` for the accumulated `PlotState`
- `file_device` for active SVG/PNG/JPEG/BMP/PDF output
- `color_palette` for indexed color lookup
- `grid_display_list` and `grid_viewport_stack` for R-level grid objects
- `grid_grob_store`, `grid_rust_display_list`, and `grid_rust_viewport_stack` for renderer-oriented grid state
- `plot_tx` for the optional GUI channel when the `plot` feature is enabled

That state split is what lets multiple interpreters coexist cleanly.

## Base Graphics Path

Base-graphics builtins live in `src/interpreter/builtins/graphics.rs`.

Their pattern is:

1. Decode R arguments into plotting parameters
2. Accumulate `PlotItem`s into a `PlotState`
3. Either send the plot to the GUI, or keep accumulating until `dev.off()` writes a file

The plotting builtins are not directly drawing pixels. They are building an intermediate plot model first.

## Grid Graphics Path

Grid support lives in `src/interpreter/grid.rs` and `src/interpreter/builtins/grid.rs`.

At the R level, grid objects are ordinary R lists with classes such as:

- `unit`
- `gpar`
- `viewport`
- `grob`

The builtin layer converts those into Rust-side grobs, display-list entries, and viewport-stack operations. `flush_grid()` then translates the grid display list into the same `PlotState` pipeline used by base graphics.

That reuse is deliberate: grid and base graphics can share downstream rendering infrastructure even though their front ends are different.

## File Devices

miniR supports file-device entry points such as:

- `svg()`
- `png()`
- `jpeg()`
- `bmp()`
- `pdf()`
- `dev.off()`

The current file-device flow is especially important:

1. Device builtins set `file_device` and start a fresh `PlotState`
2. Plotting commands accumulate into that state
3. `dev.off()` renders through the SVG pipeline first
4. Optional backends convert SVG into raster or PDF output when those features are enabled

So the device stack is intentionally layered rather than implementing five unrelated renderers.

## Why SVG Sits In The Middle

The rendering stack is feature-shaped:

| Feature | Role |
|------|------|
| `svg-device` | canonical vector rendering path |
| `raster-device` | rasterize SVG into PNG, JPEG, or BMP |
| `pdf-device` | convert SVG output into PDF |
| `plot` / `gui` | interactive display in egui |

If raster or PDF features are absent, miniR degrades pragmatically instead of pretending nothing happened. For example, raster output can fall back to writing SVG plus a warning.

## Interactive GUI

The egui viewer lives in `src/interpreter/graphics/egui_device.rs`.

The architecture is:

- main thread runs the egui event loop
- REPL and interpreter run on a background thread
- a channel carries `PlotMessage` values from interpreter to GUI

That keeps the REPL from blocking on plot windows while still satisfying platform requirements such as macOS's main-thread GUI rules.

## Where To Extend

| If you want to add... | Start here |
|------|-----------------|
| A new high-level plotting builtin | `src/interpreter/builtins/graphics.rs` |
| A new grid primitive or viewport behavior | `src/interpreter/builtins/grid.rs` and `src/interpreter/grid/` |
| A new file backend | `src/interpreter/graphics/` |
| GUI behavior or plot window UX | `src/interpreter/graphics/egui_device.rs` |

The graphics stack is easiest to work with when you keep the layers separate: R-facing builtins build plot state, renderers turn plot state into bytes or windows, and interpreter state owns the live device/session context.
