# Grid Graphics System

R's `grid` package is the scene-graph rendering layer beneath ggplot2. Unlike
base graphics (immediate-mode), grid uses persistent grobs (graphical objects),
nested viewports (coordinate systems), and a unit system for mixed-unit positioning.

## Why Grid Matters

ggplot2 renders entirely through grid. Implementing grid unlocks ggplot2 support.

## Core Components

### Units
~25 unit types: npc (normalized 0-1), cm, inches, native (data coords), null
(flexible), strwidth/strheight (text-dependent), grobwidth/grobheight (grob-dependent).
Units support arithmetic and late resolution (resolved at render time, not creation).

### Viewports
Nested coordinate systems with position, size, justification, data scales,
rotation, clipping, gpar inheritance, and optional grid layout. Viewport tree
is pushed/popped; current viewport determines coordinate mapping.

### Grobs
Persistent drawing objects: lines, rect, circle, polygon, text, points, segments,
collections, axes. Stored in a display list for replay/editing.

### Gpar
Inherited graphical parameters: col, fill, lwd, lty, fontsize, font, fontfamily.
Cascades through viewport tree.

### Display List
Sequence of push/pop/draw commands. Replayed on every render, enabling editing.

## Architecture

```
grid builtins (R API)
    ↓
grid module (grobs, viewports, units, display list)
    ↓
renderer trait (convert resolved coords to device primitives)
    ↓
egui / SVG / PDF backends
```

Grid sits between builtins and rendering. Our current PlotState/egui_plot path
stays for simple plot() — grid complements it for ggplot2 and explicit grid code.

## Module Layout

```
src/interpreter/grid.rs           # module root
src/interpreter/grid/units.rs     # Unit type, arithmetic, resolution
src/interpreter/grid/viewport.rs  # Viewport tree, transforms
src/interpreter/grid/grob.rs      # Grob enum, GrobStore
src/interpreter/grid/gpar.rs      # graphical parameters
src/interpreter/grid/display.rs   # DisplayList, replay engine
src/interpreter/grid/layout.rs    # GridLayout, cell sizing
src/interpreter/grid/render.rs    # Renderer trait, coord conversion
src/interpreter/builtins/grid.rs  # grid.newpage, pushViewport, grid.lines, etc.
```

## Implementation Order

1. Unit system (arithmetic, ~15 most common types, resolution to cm)
2. Viewport (push/pop, stacking, affine transforms, data scales)
3. Gpar (inheritance, defaults)
4. Grob primitives (lines, rect, circle, polygon, text, points)
5. Display list (record, replay)
6. Builtins: grid.newpage, pushViewport, popViewport, grid.lines, grid.rect, etc.
7. egui rendering integration (convert resolved coords to egui shapes)
8. SVG rendering integration (reuse svg_device)
9. Layout system (grid.layout, cell positioning)
10. Axes and labels (grid.xaxis, grid.yaxis)
11. Collection/frame grobs
12. Grob editing (grid.edit, grid.get, grid.set)
13. Width/height computation (grobwidth/grobheight units)

## Key Design Decisions

- Units resolve lazily at render time (font-dependent units need current gpar)
- Viewport root is implicit (device bounds, npc 0-1)
- Grobs identified by ID in a store (not R object references)
- Grid and current plot() coexist — don't merge until grid is stable
- Y-axis flip (R: bottom-left origin, egui: top-left) handled in renderer
