# egui Graphics System Plan

Comprehensive plan for miniR's graphics subsystem: interactive plotting via egui, data frame viewing via View(), and non-interactive file output (SVG, PNG, PDF).

## Crate Inventory

### egui (0.33.3)

Core immediate-mode GUI library. Provides the widget system, layout engine, input handling, and the `Painter` API for low-level drawing (circle, rect, line_segment, line, text, arrow, hline/vline, path). All coordinates are screen-space points.

- MSRV: Rust 1.88
- License: MIT OR Apache-2.0
- Role: foundation for all interactive graphics

### eframe (0.33.3)

Official framework for running egui apps natively (Linux/Mac/Windows) and on web (Wasm). Wraps winit for windowing and provides two rendering backends:

- **glow** (default) — OpenGL-based, lightweight, ~10s compile on M3 Mac
- **wgpu** (optional) — modern GPU API, adds ~10s more compile time

Key facts:
- `eframe::run_native()` now returns control after window close (fixed in 0.22)
- macOS requires GUI on the main thread — cannot call `run_native` from a spawned thread
- Platform features: `x11`, `wayland`, `accesskit`
- NativeOptions controls window size, position, vsync, renderer choice

### egui_plot (0.34.1)

Standalone 2D plotting library extracted from egui in July 2024. Immediate-mode API via `Plot::show()` + `PlotUi`.

Plot item types:
- **Line** — series of PlotPoints, configurable stroke/width/color/style/fill/gradient
- **Points** — discrete markers with MarkerShape (Circle, Diamond, Square, Cross, Plus, Up, Down, Left, Right, Asterisk), configurable radius/color/filled/stems
- **BarChart** — vertical or horizontal bars, stackable via `stack_on()`, per-bar color
- **BoxPlot** — BoxElem with BoxSpread (lower_whisker, quartile1, median, quartile3, upper_whisker)
- **Polygon** — convex polygon fill+stroke
- **Arrows** — vector arrows from origin
- **HLine/VLine** — reference lines
- **Text** — text annotations in plot coordinates
- **PlotImage** — images in plot coordinates

Plot configuration:
- Axes: labels, position, custom formatters, grid spacers, multiple axes via AxisHints
- Bounds: default_x/y_bounds, include_x/y, auto_bounds, set_margin_fraction
- Aspect ratio: data_aspect, view_aspect
- Interaction: allow_zoom/scroll/drag/boxed_zoom/double_click_reset
- Visual: show_background/grid/axes, cursor_color, grid_spacing, clamp_grid
- Legend: Legend struct with placement and grouping
- Linking: link_axis and link_cursor between plots
- Coordinate transforms: PlotTransform, screen_from_plot, plot_from_screen

Dependencies: egui ^0.33.0, emath ^0.33.0, ahash
- MSRV: follows egui (1.88)
- License: MIT OR Apache-2.0

### egui_extras (0.33.3)

Extension crate in the egui monorepo. Provides:
- **TableBuilder/Table** — scrollable table with fixed headers, virtual row rendering, resizable columns, Column width spec
- **StripBuilder/Strip** — directional layout strips
- **DatePickerButton** — date selection widget
- Image loaders (GIF, WebP, SVG, HTTP)
- Syntax highlighting (syntect)

The TableBuilder is suitable for View() but is lower-level than egui_table.

### egui_table (0.7.x)

Separate crate (not in egui monorepo) providing a higher-level table widget with:
- **TableDelegate trait** — header_cell_ui(), cell_ui(), prepare() for prefetch
- Virtual scrolling for millions of rows
- Sticky columns (num_sticky_cols)
- Resizable columns
- CellInfo with col_nr/row_nr

This is the better choice for View() over egui_extras::TableBuilder because of the delegate pattern and sticky column support. See `plans/egui-table-view.md` for the existing detailed View() plan.

### File Output Crates (no egui dependency)

For non-interactive devices (scripts, Rscript), we need file backends that do NOT require a window:

| Crate | Version | Purpose | Notes |
|-------|---------|---------|-------|
| **svg** | 0.18+ | SVG generation | Lightweight SVG composer, no rendering. Perfect for svg() device |
| **tiny-skia** | 0.11+ | 2D software rasterizer | CPU-only Skia subset. Lines, rects, circles, paths, fills, strokes. ~200KB binary. No text rendering |
| **image** (png feature) | 0.25+ | PNG encoding | Pure Rust PNG encoder via the `png` crate |
| **printpdf** | 0.9+ | PDF generation | Pure Rust PDF writer with page layout |
| **resvg** | 0.45+ | SVG rendering to PNG | Uses tiny-skia internally. Could render SVG device output to PNG |

Text rendering gap: tiny-skia has no text support. Options:
1. Use `rusttype` or `ab_glyph` for glyph rasterization onto the tiny-skia pixmap
2. For SVG output, text is just SVG `<text>` elements — no rasterization needed
3. For PNG output, render via SVG intermediate + resvg (which handles text via usvg+fontdb)
4. For PDF output, printpdf has built-in text/font support

## R Graphics Model Overview

R's graphics system has three layers:

1. **High-level functions**: plot(), hist(), barplot(), boxplot(), pie(), pairs(), etc. These compute layout and call low-level primitives
2. **Low-level primitives**: points(), lines(), segments(), rect(), polygon(), text(), abline(), title(), axis(), legend(), etc. These draw on the current device
3. **Graphics devices**: pdf(), png(), svg(), x11(), quartz() — these implement the actual rendering

### Device Callback Interface (DevDesc)

Every R graphics device implements these callbacks:

| Callback | Purpose | Maps to |
|----------|---------|---------|
| `circle` | Draw circle at (x,y) with radius | egui Painter::circle / tiny-skia circle path |
| `line` | Draw line from (x0,y0) to (x1,y1) | Painter::line_segment / tiny-skia stroke |
| `polyline` | Connected line segments | Painter::line / tiny-skia path |
| `polygon` | Filled/stroked polygon | egui Shape::Path / tiny-skia fill+stroke |
| `rect` | Draw rectangle | Painter::rect / tiny-skia rect |
| `text` | Render text at position | Painter::text / SVG `<text>` / printpdf text |
| `strWidth` | Measure string width in device coords | egui layout_no_wrap + galley width |
| `metricInfo` | Character metrics (ascent, descent, width) | egui font metrics |
| `newPage` | Start new plot page | Clear canvas / new SVG doc / new PDF page |
| `clip` | Set clipping rectangle | egui clip_rect / tiny-skia clip |
| `close` | Close device, finalize output | Close window / write file |
| `size` | Query device dimensions | Window size / configured dimensions |
| `mode` | Signal draw start/end (for






































 buffering) | Begin/end frame in egui |
| `activate`/`deactivate` | Device gains/loses focus | Window focus events |

### Coordinate System

R uses "user coordinates" mapped to "device coordinates" via the `par("usr")` and `par("plt")` settings:
- **usr**: c(xmin, xmax, ymin, ymax) in data space
- **fig**: figure region as fraction of device
- **plt**: plot region as fraction of figure
- **mar**: margins in lines of text: c(bottom, left, top, right), default c(5.1, 4.1, 4.1, 2.1)

For egui_plot: the Plot widget handles its own coordinate system with PlotTransform mapping between plot coordinates and screen coordinates. This is a natural fit — R's plot() just needs to push data into egui_plot's coordinate system rather than manually managing usr/device transforms.

For low-level primitives drawn directly on an egui Painter (or on a tiny-skia pixmap), we need to maintain our own coordinate transform.

### Graphics Parameters (par)

Key parameters our device needs to track:

| Parameter | Description | Default |
|-----------|-------------|---------|
| `col` | Foreground color | "black" |
| `bg` | Background color | "white" |
| `lwd` | Line width | 1 |
| `lty` | Line type (solid, dashed, etc.) | "solid" |
| `pch` | Point character (0-25 + ASCII) | 1 |
| `cex` | Character expansion factor | 1 |
| `font` | Font face (1=plain, 2=bold, 3=italic, 4=bold-italic) | 1 |
| `ps` | Point size for text | 12 |
| `mar` | Margins | c(5.1, 4.1, 4.1, 2.1) |
| `mfrow`/`mfcol` | Multi-panel layout | c(1,1) |
| `xlab`/`ylab` | Axis labels | "" |
| `main`/`sub` | Title/subtitle | "" |
| `las` | Axis label orientation | 0 |
| `xlim`/`ylim` | Axis limits | auto |
| `log` | Log-scale axes | "" |

## Architecture

### Graphics Device Trait

```rust
/// A miniR graphics device — anything that can render R graphics primitives.
///
/// Mirrors the essential callbacks from R's DevDesc struct.
/// Devices are stored on the Interpreter and managed via dev.cur(), dev.set(), etc.
pub trait GraphicsDevice {
    // -- primitives --
    fn circle(&mut self, x: f64, y: f64, r: f64, gc: &GraphicsContext);
    fn line(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, gc: &GraphicsContext);
    fn polyline(&mut self, x: &[f64], y: &[f64], gc: &GraphicsContext);
    fn polygon(&mut self, x: &[f64], y: &[f64], gc: &GraphicsContext);
    fn rect(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, gc: &GraphicsContext);
    fn text(&mut self, x: f64, y: f64, text: &str, rot: f64, hadj: f64, gc: &GraphicsContext);
    fn path(&mut self, x: &[f64], y: &[f64], nper: &[usize], winding: bool, gc: &GraphicsContext);

    // -- metrics --
    fn str_width(&self, text: &str, gc: &GraphicsContext) -> f64;
    fn char_metric(&self, ch: char, gc: &GraphicsContext) -> CharMetric;

    // -- page/clip/size --
    fn new_page(&mut self, gc: &GraphicsContext);
    fn clip(&mut self, x0: f64, y0: f64, x1: f64, y1: f64);
    fn size(&self) -> DeviceSize;

    // -- lifecycle --
    fn close(&mut self);
    fn activate(&mut self) {}
    fn deactivate(&mut self) {}

    // -- device info --
    fn name(&self) -> &str;
    fn is_interactive(&self) -> bool;
}
```

### GraphicsContext (gc)

Passed to every draw call, carries the current pen/brush state:

```rust
pub struct GraphicsContext {
    pub col: RColor,        // stroke/foreground color (with alpha)
    pub fill: RColor,       // fill color (with alpha)
    pub lwd: f64,           // line width
    pub lty: LineType,      // solid, dashed, dotted, etc.
    pub pch: PointChar,     // point symbol
    pub cex: f64,           // character expansion
    pub ps: f64,            // point size
    pub font: FontFace,     // plain/bold/italic/bold-italic
    pub font_family: String,
}
```

### Device Manager (on Interpreter)

```rust
pub struct DeviceManager {
    devices: Vec<Option<Box<dyn GraphicsDevice>>>,
    current: usize,  // 1-indexed like R (0 = null device)
}
```

R functions that map to device management:
- `dev.cur()` / `dev.set(n)` / `dev.off()` / `dev.list()`
- `dev.new()` — open a new device
- `graphics.off()` — close all devices

### Module Layout

```
src/
  interpreter/
    graphics.rs              # DeviceManager, GraphicsDevice trait, GraphicsContext
    graphics/
      device_manager.rs      # DeviceManager impl, dev.cur/set/off/list
      par.rs                 # par() state and parameter parsing
      coord.rs               # Coordinate transforms (usr <-> device)
      color.rs               # RColor, color name table, hex parsing
      high_level.rs          # plot(), hist(), barplot() implementations
      low_level.rs           # points(), lines(), segments(), rect(), etc.
  interpreter/
    builtins/
      graphics.rs            # Builtin registrations for plot/points/lines/etc.
      device.rs              # Builtin registrations for dev.cur/dev.set/pdf/png/svg

# Feature-gated backends:
  interpreter/
    graphics/
      egui_device.rs         # #[cfg(feature = "plot")] — interactive egui window device
      svg_device.rs          # #[cfg(feature = "svg-device")] — SVG file device
      png_device.rs          # #[cfg(feature = "png-device")] — PNG file device
      pdf_device.rs          # #[cfg(feature = "pdf-device")] — PDF file device
      null_device.rs         # Always available — discards all drawing (R's null device)
    view.rs                  # #[cfg(feature = "view")] — View() data frame viewer
```

## Feature Gating Strategy

```toml
[features]
default = ["random", "datetime", "io"]

# Interactive GUI features (heavyweight — not in default)
view = ["dep:eframe", "dep:egui", "dep:egui_table"]
plot = ["dep:eframe", "dep:egui", "dep:egui_plot"]
gui  = ["view", "plot"]  # convenience: all interactive GUI features

# File output devices (lightweight — candidates for default)
svg-device = ["dep:svg"]
png-device = ["dep:tiny-skia", "dep:image"]
pdf-device = ["dep:printpdf"]
file-devices = ["svg-device", "png-device", "pdf-device"]

# Everything
full = ["gui", "file-devices", "random", "datetime", "io"]
```

Rationale:
- `view` and `plot` are NOT in default because eframe pulls ~150-200 transitive crates and adds ~10-20s compile time
- `svg-device` is very lightweight (the `svg` crate is tiny) — could be in default later
- `png-device` requires tiny-skia (~200KB binary addition) — reasonable for default
- `pdf-device` is moderate weight — keep optional initially
- The `gui` convenience feature enables both `view` and `plot` since they share eframe/egui
- File devices share zero dependencies with the GUI features

### Shared vs Separate eframe Dependency

`view` and `plot` both need eframe+egui. When both are enabled, they share the same dependency. The `gui` feature makes this explicit. If only `plot` is enabled, View() falls back to `print()` (same as today).

### Backend Choice: glow (not wgpu)

Use eframe with the **glow** (OpenGL) backend, not wgpu:
- glow is the default, compiles faster (~10s vs ~20s), fewer dependencies
- We do not need GPU compute or advanced 3D rendering
- glow works on all desktop platforms (macOS, Linux, Windows)
- If wgpu becomes eframe's default in a future version, we can switch then

Configure: `eframe = { version = "0.33", default-features = false, features = ["glow", "default_fonts"] }`

## How View() Works

Already planned in detail in `plans/egui-table-view.md`. Summary:

1. `View(df)` extracts column names, row names, and cell data from the data frame
2. Pre-formats all cells to String
3. Launches an eframe window with egui_table's TableDelegate pattern
4. Virtual scrolling handles large data frames efficiently
5. Sticky first column for row names
6. Window blocks the REPL until closed (non-blocking is a future enhancement)
7. Falls back to `print()` when the `view` feature is disabled

## How plot() Works

### High-Level Strategy: egui_plot for Common Plots

For the interactive device, use egui_plot for the standard plot types. egui_plot handles:
- Coordinate system management (PlotTransform)
- Axes with labels, ticks, grid
- Pan/zoom interaction
- Legend
- Multiple series

This means plot(), hist(), barplot(), boxplot() translate R data into egui_plot items:

| R function | egui_plot item | Notes |
|------------|---------------|-------|
| `plot(x, y, type="p")` | Points | MarkerShape from pch |
| `plot(x, y, type="l")` | Line | |
| `plot(x, y, type="b")` | Line + Points | Both overlaid |
| `plot(x, y, type="h")` | Points with stems | `Points::stems()` |
| `lines()` | Line | Added to existing plot |
| `points()` | Points | Added to existing plot |
| `abline(h=)` | HLine | |
| `abline(v=)` | VLine | |
| `abline(a, b)` | Line | Compute endpoints from plot bounds |
| `hist()` | BarChart | Compute bins, create Bar per bin |
| `barplot()` | BarChart | Vertical or horizontal, stacked via stack_on() |
| `boxplot()` | BoxPlot | Compute BoxSpread from data |
| `segments()` | Multiple Line items | Each segment is a separate Line |
| `legend()` | Plot Legend | Via Plot::legend() + named items |

### Interactive Device Architecture

```
EguiDevice {
    // Accumulated plot items for the current page
    items: Vec<PlotItem>,

    // Current graphics parameters
    par: ParState,

    // Plot configuration
    title: Option<String>,
    x_label: Option<String>,
    y_label: Option<String>,
    x_lim: Option<(f64, f64)>,
    y_lim: Option<(f64, f64)>,
    log_x: bool,
    log_y: bool,

    // Window state
    window_tx: Option<Sender<DeviceCommand>>,
}

enum PlotItem {
    Line { points: Vec<[f64; 2]>, gc: GraphicsContext },
    Points { points: Vec<[f64; 2]>, gc: GraphicsContext },
    BarChart { bars: Vec<Bar>, gc: GraphicsContext },
    BoxPlot { elements: Vec<BoxElem>, gc: GraphicsContext },
    HLine { y: f64, gc: GraphicsContext },
    VLine { x: f64, gc: GraphicsContext },
    Text { x: f64, y: f64, text: String, rot: f64, gc: GraphicsContext },
    Polygon { points: Vec<[f64; 2]>, gc: GraphicsContext },
}
```

### Rendering Flow

1. R code calls `plot(x, y)` which invokes the `plot.default` high-level function
2. High-level function calls low-level primitives: `plot.new()`, `plot.window()`, `axis()`, `points()`, `title()`, etc.
3. Each primitive call goes through the current device's GraphicsDevice trait methods
4. EguiDevice accumulates PlotItem entries
5. On `mode(0)` (draw complete) or when the user evaluates the next expression, the device sends accumulated items to the egui window
6. The egui window's `update()` renders all items using egui_plot

### Window Threading Model

macOS requires GUI on the main thread. The architecture must account for this:

**Option A — Blocking (MVP):**
The REPL runs, accumulates plot commands, then when a plot is ready, spawns the eframe window on the main thread. The REPL blocks until the window closes. Simple, works everywhere.

**Option B — Main-thread GUI, background REPL (future):**
Flip the architecture: the main thread runs the eframe event loop, and the REPL runs on a background thread. Communication via channels. This enables non-blocking plots that update while the REPL is active.

Start with Option A. Option B is a larger architectural change that can be done later.

### Low-Level Primitives on egui_plot

For primitives that don't map cleanly to egui_plot items (arbitrary circles at specific coordinates, filled rectangles in plot space, rotated text), we have two approaches:

1. **Use egui_plot's PlotUi custom painting**: After plot_ui.show(), we can use the PlotTransform to convert plot coordinates to screen coordinates and then draw directly on the Painter
2. **Custom PlotItem implementations**: egui_plot's PlotUi has `add()` which accepts any `PlotItem` trait implementor — we could implement custom items

For MVP, the egui_plot built-in items cover the most important cases. Custom painting can fill gaps later.

### pch (Point Character) Mapping

R's pch values map to egui_plot MarkerShape:

| pch | R shape | egui MarkerShape |
|-----|---------|-----------------|
| 0 | Open square | Square (filled=false) |
| 1 | Open circle | Circle (filled=false) |
| 2 | Open triangle up | Up (filled=false) |
| 3 | Plus | Plus |
| 4 | Cross | Cross |
| 5 | Open diamond | Diamond (filled=false) |
| 6 | Open triangle down | Down (filled=false) |
| 7 | Square cross | Square + Cross overlay |
| 8 | Asterisk | Asterisk |
| 15 | Filled square | Square (filled=true) |
| 16 | Filled circle | Circle (filled=true) |
| 17 | Filled triangle up | Up (filled=true) |
| 18 | Filled diamond | Diamond (filled=true) |
| 19-20 | Filled circle variants | Circle (filled=true) |

Some pch values (7-14, 21-25) have no direct egui_plot equivalent and would need custom painting.

## How File Devices Work

### SVG Device (`svg()`)

```rust
struct SvgDevice {
    doc: svg::Document,
    width: f64,   // inches
    height: f64,
    filename: String,
    // Current page elements are added to doc
}
```

Each GraphicsDevice callback appends SVG elements:
- `circle` -> `<circle cx=".." cy=".." r=".." />`
- `line` -> `<line x1=".." y1=".." x2=".." y2=".." />`
- `polyline` -> `<polyline points=".." />`
- `polygon` -> `<polygon points=".." />`
- `rect` -> `<rect x=".." y=".." width=".." height=".." />`
- `text` -> `<text x=".." y=".." transform="rotate(..)">...</text>`
- `clip` -> `<clipPath><rect .../></clipPath>` + `clip-path="url(#...)"`

On `close()`, write the SVG document to file via `svg::save()`.

The `svg` crate is extremely lightweight — just string building. No rendering, no GPU, no windowing. Ideal for this use case.

### PNG Device (`png()`)

Two implementation strategies:

**Strategy A — tiny-skia direct (no text):**
```rust
struct PngDevice {
    pixmap: tiny_skia::Pixmap,
    width: u32,
    height: u32,
    filename: String,
}
```
Rasterize directly onto a tiny-skia Pixmap. On `close()`, encode to PNG via the `image` crate. Fast, simple, but no text rendering.

**Strategy B — SVG intermediate + resvg (with text):**
```rust
struct PngDevice {
    svg_device: SvgDevice,  // accumulate as SVG
    width: u32,
    height: u32,
    filename: String,
}
```
Accumulate drawing commands as SVG (reusing SvgDevice), then on `close()`, render the SVG to a pixmap via resvg (which uses tiny-skia + fontdb internally). This gets text rendering for free.

**Recommendation:** Start with Strategy B. It reuses the SVG device logic, gets text rendering, and resvg is already pure Rust. The indirection cost is negligible for typical plot sizes.

### PDF Device (`pdf()`)

```rust
struct PdfDevice {
    doc: printpdf::PdfDocument,
    current_page: printpdf::PdfPageIndex,
    current_layer: printpdf::PdfLayerIndex,
    width: f64,   // inches
    height: f64,
    filename: String,
}
```

printpdf supports lines, rects, polygons, text (with embedded fonts), and images. The coordinate system is bottom-left origin (same as PDF spec), so we need to flip Y coordinates from R's top-left convention.

On `close()`, save the PDF document to file.

## Implementation Order

### Phase 1: Graphics Infrastructure (no GUI dependencies)

1. Define `GraphicsDevice` trait in `src/interpreter/graphics.rs`
2. Implement `GraphicsContext`, `RColor`, `LineType`, `FontFace`, `PointChar`
3. Implement `DeviceManager` on Interpreter
4. Implement `NullDevice` (discards all drawing)
5. Implement `dev.cur()`, `dev.set()`, `dev.off()`, `dev.list()` builtins
6. Implement `par()` builtin for getting/setting graphics parameters
7. Implement coordinate transform helpers (usr <-> device)

### Phase 2: SVG Device (lightest file backend)

8. Add `svg` as optional dependency behind `svg-device` feature
9. Implement `SvgDevice` — all GraphicsDevice callbacks produce SVG elements
10. Implement `svg()` builtin: opens SvgDevice with filename, width, height
11. Implement low-level builtins that draw on current device: `plot.new()`, `plot.window()`
12. Implement: `points()`, `lines()`, `segments()`, `rect()`, `polygon()`, `text()`, `abline()`, `title()`, `axis()`
13. Test: `svg("test.svg"); plot(1:10); dev.off()` produces valid SVG

### Phase 3: High-Level Plot Functions

14. Implement `plot.default()` — the workhorse: dispatches to points/lines/both, sets up axes, labels
15. Implement `hist()` — compute bins, draw bars
16. Implement `barplot()` — draw bars from data
17. Implement `boxplot()` — compute five-number summary, draw box elements
18. Implement `legend()` — draw legend box with entries
19. Implement `par()` integration — margins, multi-panel (mfrow/mfcol)

### Phase 4: Interactive egui Device

20. Add eframe, egui, egui_plot as optional dependencies behind `plot` feature
21. Implement `EguiDevice` — accumulates PlotItems, renders via egui_plot
22. Wire up as the default device when `plot` feature is enabled and session is interactive
23. Implement blocking window display (Option A threading model)
24. Test: `plot(1:10)` opens an interactive window with pan/zoom

### Phase 5: View()

25. Already planned in `plans/egui-table-view.md`
26. Add eframe, egui, egui_table as optional dependencies behind `view` feature
27. Implement View() with TableDelegate pattern

### Phase 6: PNG and PDF Devices

28. Implement PngDevice via SVG intermediate + resvg
29. Implement PdfDevice via printpdf
30. Implement `png()` and `pdf()` builtins

### Phase 7: Polish

31. NA styling in View() (gray italic)
32. Column sorting in View()
33. Non-blocking window mode (Option B threading)
34. Multi-panel layouts (par(mfrow=c(2,2)))
35. Log-scale axes
36. Custom color palettes (rainbow, heat.colors, etc.)
37. plot.formula (y ~ x) support
38. pairs() — scatterplot matrix

## Build Cost Analysis

| Feature | New crates (approx) | Compile time impact | Binary size impact |
|---------|---------------------|--------------------|--------------------|
| `svg-device` | ~1 (svg) | Negligible (<1s) | ~10KB |
| `png-device` | ~5 (tiny-skia, image, png, resvg, fontdb) | ~3-5s | ~500KB |
| `pdf-device` | ~3 (printpdf, lopdf) | ~2-3s | ~200KB |
| `plot` | ~150+ (eframe, egui, egui_plot, winit, glow, glutin, ...) | ~10-20s | ~2-5MB |
| `view` | ~150+ (shared with plot) + egui_table | ~10-20s (shared) | ~2-5MB |
| `gui` (plot+view) | ~150+ (shared) | ~10-20s total | ~2-5MB |

Key takeaway: file devices are cheap; GUI features are expensive. Feature-gating keeps default builds fast.

## MSRV Considerations

- Current project MSRV: 1.87 (workspace Cargo.toml)
- egui/eframe/egui_plot require: 1.88
- egui_table requires: 1.88+

When we add GUI features, we will need to bump the workspace MSRV to 1.88. Since these features are optional and not in default, we could alternatively set per-feature MSRV documentation (Cargo does not enforce per-feature MSRV, but we can document it).

Practical recommendation: bump to 1.88 when adding GUI features. Rust 1.88 is the 2024 edition stabilization release and is widely available.

## Alternatives Considered

### Why not iced?

iced is a retained-mode Elm-architecture GUI. It's well-designed but:
- No equivalent to egui_plot (would need custom canvas drawing)
- No equivalent to egui_table (would need custom widget)
- Retained mode is harder to integrate with an immediate-mode interpreter
- egui has better library ecosystem for data visualization

### Why not plotters?

plotters is a Rust plotting library that generates static images (SVG, PNG, etc.). It could be used for file devices, but:
- It duplicates what we get from egui_plot for interactive use
- We would need two different plotting APIs (plotters for files, egui_plot for interactive)
- Better to have one plot data model that renders to both egui_plot (interactive) and SVG/PNG (file)

### Why not raw winit + wgpu?

Maximum control but enormous implementation effort. egui_plot gives us interactive 2D plotting (pan, zoom, hover, legend) essentially for free.

## Open Questions

1. **How to handle `par(mfrow=c(2,2))` multi-panel layouts in egui_plot?** — egui_plot's Plot widget is a single plot. Multi-panel would need multiple Plot widgets laid out in a grid via egui's layout system.

2. **How to handle log-scale axes?** — egui_plot supports custom axis formatters and grid spacers. We would implement log-scale by transforming data coordinates and providing log-scale tick formatters.

3. **Should the interactive device use egui_plot for everything, or a raw Painter for some primitives?** — Start with egui_plot for high-level plots. If we need arbitrary drawing (like R's base graphics low-level primitives mixed with plot coordinates), we can drop down to Painter with coordinate transforms from PlotTransform.

4. **How to handle `Rscript` (non-interactive) with plot() calls?** — When there is no interactive device available, plot() should auto-open a file device (SVG or PNG). R does this too — `Rscript -e 'plot(1:10)'` produces a `Rplots.pdf` file.

5. **Font handling for file devices** — SVG can specify font-family and let the viewer handle it. PNG via resvg needs fontdb to resolve fonts. PDF via printpdf embeds fonts. We need a shared font resolution strategy.

## References

- [egui GitHub](https://github.com/emilk/egui) — main repository
- [egui_plot GitHub](https://github.com/emilk/egui_plot) — 2D plotting library
- [egui_plot docs](https://docs.rs/egui_plot/latest/egui_plot/) — API documentation
- [eframe docs](https://docs.rs/eframe/latest/eframe/) — native app framework
- [egui_extras docs](https://docs.rs/egui_extras/latest/egui_extras/) — TableBuilder and extras
- [egui_table crate](https://crates.io/crates/egui_table) — high-level table widget
- [R Internals: Graphics Devices](https://rstudio.github.io/r-manuals/r-ints/Graphics-Devices.html) — R's device API
- [svg crate](https://crates.io/crates/svg) — SVG generation
- [tiny-skia](https://github.com/linebender/tiny-skia) — 2D software rasterizer
- [resvg](https://github.com/linebender/resvg) — SVG rendering
- [printpdf](https://crates.io/crates/printpdf) — PDF generation
- [plans/egui-table-view.md](egui-table-view.md) — existing View() plan
