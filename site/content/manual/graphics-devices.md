+++
title = "Graphics Devices"
weight = 4
description = "miniR's graphics stack is intentionally layered around interpreter-owned state, a shared plot model, and device backends that build on each other instead of reimplementing rendering from scratch for every output format."
+++

miniR's graphics stack is intentionally layered around interpreter-owned state, a shared plot model, and device backends that build on each other instead of reimplementing rendering from scratch for every output format.

## Interpreter-Owned Graphics State

Graphics state belongs to the interpreter, not to process-global singletons. Important pieces include:

- `par_state` for base-graphics parameters
- `current_plot` for accumulated plot items
- `file_device` for active output devices
- `color_palette` for indexed colors
- grid display-list and viewport-stack state
- the optional GUI channel used by the plot viewer

That is what keeps multiple interpreters from trampling one another's plotting state.

## Base Graphics Path

Base-graphics builtins live in `src/interpreter/builtins/graphics.rs`.

Their rough pattern is:

1. Decode R arguments into plotting parameters.
2. Convert them into `PlotItem` values.
3. Accumulate those items into a `PlotState`.
4. Send the plot to the GUI or hold it until a file device flushes.

The builtins are not directly drawing pixels. They are building a reusable intermediate model first.

## Grid Graphics Path

Grid support lives in `src/interpreter/grid.rs` and `src/interpreter/builtins/grid.rs`.

At the R level, grid objects are ordinary R objects with classes such as:

- `unit`
- `gpar`
- `viewport`
- `grob`

The builtin layer converts those into Rust-side grobs, display-list entries, and viewport operations. `flush_grid()` then translates the result into the same `PlotState` pipeline used by base graphics.

That shared downstream path is important. It means grid and base graphics can diverge at the front end while still using one renderer stack.

## File Devices

miniR exposes file-device entry points such as:

- `svg()`
- `png()`
- `jpeg()`
- `bmp()`
- `pdf()`
- `dev.off()`

The current device flow is:

1. A device builtin sets `file_device` and starts a fresh `PlotState`.
2. Plot commands accumulate into that state.
3. `dev.off()` renders through the SVG pipeline first.
4. Optional raster and PDF backends convert that SVG output when those features are enabled.

That layering keeps the renderer stack smaller and easier to reason about.

## Why SVG Sits In The Middle

The device stack is feature-shaped:

| Feature | Role |
| ------- | ---- |
| `svg-device` | canonical vector rendering path |
| `raster-device` | rasterize SVG into PNG, JPEG, or BMP |
| `pdf-device` | convert SVG output into PDF |
| `plot` / `gui` | interactive display in egui |

SVG is not an incidental format choice here. It is the common rendering language that keeps the downstream device story unified.

## Grid State And Rust-Side Representation

miniR keeps both R-facing and renderer-facing grid state:

- R-facing lists and classes mirror the objects package code manipulates
- Rust-facing grob stores and viewport stacks support rendering and display-list execution

That split helps keep the interpreter honest about R semantics without forcing the renderer to manipulate raw R list structures directly.

## Device Shutdown Matters

`dev.off()` is not a trivial cleanup call. It is the point where deferred rendering work becomes a concrete file or output artifact.

If plots are missing, partially rendered, or written to the wrong backend, the bug often sits in:

- file-device selection
- plot-state lifetime
- grid flush timing
- SVG conversion to raster or PDF

Those are graphics-runtime bugs, not front-end plotting bugs.

## Where To Debug Graphics Problems

Start in this subsystem when the symptom looks like:

- plot commands run but no file is written
- grid output differs from base-graphics output in downstream rendering
- device state leaks across interpreters
- `dev.off()` writes the wrong format or drops elements
- the optional GUI shows a different result than file output

Most of those failures are about shared plot-state or device layering, not about a single plotting builtin.
