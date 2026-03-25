//! Grid viewport system — hierarchical coordinate contexts.
//!
//! Viewports define rectangular regions with their own coordinate systems,
//! scales, and graphical parameters. They form a stack (push/pop) that
//! determines how child grobs are positioned and sized.

use super::gpar::Gpar;
use super::units::{Unit, UnitContext};

// region: Justification

/// Justification for positioning within a viewport.
///
/// Maps to numeric values: Left=0.0, Centre=0.5, Right=1.0 (horizontal),
/// Bottom=0.0, Centre=0.5, Top=1.0 (vertical).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Justification {
    /// Left (horizontal) or Bottom (vertical) — 0.0.
    Left,
    /// Center — 0.5.
    Centre,
    /// Right (horizontal) or Top (vertical) — 1.0.
    Right,
    /// Top — 1.0 (vertical only, alias for Right in vertical context).
    Top,
    /// Bottom — 0.0 (vertical only, alias for Left in vertical context).
    Bottom,
}

impl Justification {
    /// Convert justification to a numeric offset fraction (0.0 to 1.0).
    pub fn as_fraction(self) -> f64 {
        match self {
            Justification::Left | Justification::Bottom => 0.0,
            Justification::Centre => 0.5,
            Justification::Right | Justification::Top => 1.0,
        }
    }

    /// Parse a justification from a string, as R's grid accepts.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "left" => Some(Justification::Left),
            "centre" | "center" => Some(Justification::Centre),
            "right" => Some(Justification::Right),
            "top" => Some(Justification::Top),
            "bottom" => Some(Justification::Bottom),
            _ => None,
        }
    }
}

// endregion

// region: GridLayout

/// A grid layout divides a viewport into rows and columns.
///
/// Row heights and column widths are expressed as units. Grobs can be
/// placed into specific cells (row, col) or span multiple cells.
#[derive(Clone, Debug)]
pub struct GridLayout {
    /// Number of rows.
    pub nrow: usize,
    /// Number of columns.
    pub ncol: usize,
    /// Row heights — one unit per row.
    pub heights: Vec<Unit>,
    /// Column widths — one unit per column.
    pub widths: Vec<Unit>,
    /// Whether to fill layout cells left-to-right (true) or top-to-bottom (false).
    pub respect: bool,
}

impl GridLayout {
    /// Create a layout with uniform NPC-sized rows and columns.
    pub fn uniform(nrow: usize, ncol: usize) -> Self {
        let row_height = 1.0 / nrow as f64;
        let col_width = 1.0 / ncol as f64;
        GridLayout {
            nrow,
            ncol,
            heights: (0..nrow).map(|_| Unit::npc(row_height)).collect(),
            widths: (0..ncol).map(|_| Unit::npc(col_width)).collect(),
            respect: false,
        }
    }
}

// endregion

// region: Viewport

/// A grid viewport — a rectangular region with its own coordinate system.
///
/// Viewports have position, size, justification, native scale, rotation,
/// clipping behavior, and graphical parameters. They can optionally contain
/// a layout for subdividing into cells.
#[derive(Clone, Debug)]
pub struct Viewport {
    /// Optional name for this viewport (for viewport navigation).
    pub name: Option<String>,
    /// X position within parent viewport.
    pub x: Unit,
    /// Y position within parent viewport.
    pub y: Unit,
    /// Width of this viewport.
    pub width: Unit,
    /// Height of this viewport.
    pub height: Unit,
    /// Justification: (horizontal, vertical).
    pub just: (Justification, Justification),
    /// Native x-coordinate scale (min, max).
    pub xscale: (f64, f64),
    /// Native y-coordinate scale (min, max).
    pub yscale: (f64, f64),
    /// Rotation angle in degrees.
    pub angle: f64,
    /// Whether to clip drawing to this viewport's bounds.
    pub clip: bool,
    /// Graphical parameters for this viewport.
    pub gp: Gpar,
    /// Optional layout for subdividing this viewport.
    pub layout: Option<GridLayout>,
}

impl Viewport {
    /// Create a root viewport that covers the entire device.
    pub fn root(width_cm: f64, height_cm: f64) -> Self {
        Viewport {
            name: Some("ROOT".to_string()),
            x: Unit::cm(0.0),
            y: Unit::cm(0.0),
            width: Unit::cm(width_cm),
            height: Unit::cm(height_cm),
            just: (Justification::Left, Justification::Bottom),
            xscale: (0.0, 1.0),
            yscale: (0.0, 1.0),
            angle: 0.0,
            clip: true,
            gp: Gpar::new(),
            layout: None,
        }
    }

    /// Create a new viewport with default settings (centered, full NPC extent).
    pub fn new() -> Self {
        Viewport {
            name: None,
            x: Unit::npc(0.5),
            y: Unit::npc(0.5),
            width: Unit::npc(1.0),
            height: Unit::npc(1.0),
            just: (Justification::Centre, Justification::Centre),
            xscale: (0.0, 1.0),
            yscale: (0.0, 1.0),
            angle: 0.0,
            clip: false,
            gp: Gpar::new(),
            layout: None,
        }
    }
}

impl Default for Viewport {
    fn default() -> Self {
        Self::new()
    }
}

// endregion

// region: ViewportStack

/// A stack of viewports, starting from the root (device) viewport.
///
/// The stack grows as viewports are pushed and shrinks as they are popped.
/// The current viewport (top of stack) determines the coordinate context
/// for drawing operations.
pub struct ViewportStack {
    stack: Vec<Viewport>,
}

impl ViewportStack {
    /// Create a new viewport stack with a root viewport for the given device size.
    pub fn new(viewport_width_cm: f64, viewport_height_cm: f64) -> Self {
        ViewportStack {
            stack: vec![Viewport::root(viewport_width_cm, viewport_height_cm)],
        }
    }

    /// Push a child viewport onto the stack.
    pub fn push(&mut self, vp: Viewport) {
        self.stack.push(vp);
    }

    /// Pop the top viewport. Returns `None` if only the root remains
    /// (the root viewport cannot be popped).
    pub fn pop(&mut self) -> Option<Viewport> {
        if self.stack.len() > 1 {
            self.stack.pop()
        } else {
            None
        }
    }

    /// Return a reference to the current (topmost) viewport.
    pub fn current(&self) -> &Viewport {
        self.stack
            .last()
            .expect("viewport stack always has at least the root viewport")
    }

    /// Return the depth of the viewport stack (1 = root only).
    pub fn depth(&self) -> usize {
        self.stack.len()
    }
}

// endregion

// region: ViewportTransform

/// The computed absolute transform for a viewport, in device cm coordinates.
///
/// This is produced by walking the viewport stack from root to current,
/// accumulating position, size, and scale transforms at each level.
#[derive(Clone, Debug)]
pub struct ViewportTransform {
    /// X offset of the viewport's bottom-left corner from device origin, in cm.
    pub x_offset_cm: f64,
    /// Y offset of the viewport's bottom-left corner from device origin, in cm.
    pub y_offset_cm: f64,
    /// Viewport width in cm.
    pub width_cm: f64,
    /// Viewport height in cm.
    pub height_cm: f64,
    /// Accumulated rotation angle in degrees.
    pub angle: f64,
    /// Native x-scale.
    pub xscale: (f64, f64),
    /// Native y-scale.
    pub yscale: (f64, f64),
}

impl ViewportTransform {
    /// Create the root transform for the device.
    pub fn root(width_cm: f64, height_cm: f64) -> Self {
        ViewportTransform {
            x_offset_cm: 0.0,
            y_offset_cm: 0.0,
            width_cm,
            height_cm,
            angle: 0.0,
            xscale: (0.0, 1.0),
            yscale: (0.0, 1.0),
        }
    }

    /// Compute the transform for a child viewport given its parent's transform.
    pub fn from_viewport(vp: &Viewport, parent: &ViewportTransform) -> Self {
        let ctx = UnitContext {
            viewport_width_cm: parent.width_cm,
            viewport_height_cm: parent.height_cm,
            xscale: parent.xscale,
            yscale: parent.yscale,
            fontsize_pt: 12.0,
            lineheight: 1.2,
        };

        // Resolve the viewport's position and size in parent cm
        let vp_x = ctx.resolve_x(&vp.x, 0);
        let vp_y = ctx.resolve_y(&vp.y, 0);

        let vp_width = ctx.resolve_x(&vp.width, 0);
        let vp_height = ctx.resolve_y(&vp.height, 0);

        // Apply justification: the (x, y) is the justification point,
        // so we offset to get the bottom-left corner.
        let hjust = vp.just.0.as_fraction();
        let vjust = vp.just.1.as_fraction();
        let x_offset = parent.x_offset_cm + vp_x - hjust * vp_width;
        let y_offset = parent.y_offset_cm + vp_y - vjust * vp_height;

        ViewportTransform {
            x_offset_cm: x_offset,
            y_offset_cm: y_offset,
            width_cm: vp_width,
            height_cm: vp_height,
            angle: parent.angle + vp.angle,
            xscale: vp.xscale,
            yscale: vp.yscale,
        }
    }

    /// Convert a native (data) x coordinate to cm from device origin.
    pub fn native_to_cm_x(&self, x: f64) -> f64 {
        let range = self.xscale.1 - self.xscale.0;
        if range == 0.0 {
            self.x_offset_cm
        } else {
            self.x_offset_cm + ((x - self.xscale.0) / range) * self.width_cm
        }
    }

    /// Convert a native (data) y coordinate to cm from device origin.
    pub fn native_to_cm_y(&self, y: f64) -> f64 {
        let range = self.yscale.1 - self.yscale.0;
        if range == 0.0 {
            self.y_offset_cm
        } else {
            self.y_offset_cm + ((y - self.yscale.0) / range) * self.height_cm
        }
    }

    /// Convert an NPC x coordinate (0..1) to cm from device origin.
    pub fn npc_to_cm_x(&self, x: f64) -> f64 {
        self.x_offset_cm + x * self.width_cm
    }

    /// Convert an NPC y coordinate (0..1) to cm from device origin.
    pub fn npc_to_cm_y(&self, y: f64) -> f64 {
        self.y_offset_cm + y * self.height_cm
    }

    /// Build a UnitContext for resolving units within this viewport transform.
    pub fn unit_context(&self) -> UnitContext {
        UnitContext {
            viewport_width_cm: self.width_cm,
            viewport_height_cm: self.height_cm,
            xscale: self.xscale,
            yscale: self.yscale,
            fontsize_pt: 12.0,
            lineheight: 1.2,
        }
    }
}

/// Compute the full transform stack for a ViewportStack, returning the
/// transform for the current (topmost) viewport.
pub fn compute_transform(stack: &ViewportStack) -> ViewportTransform {
    let root = &stack.stack[0];
    let mut transform = ViewportTransform::root(root.width.value(), root.height.value());
    for vp in stack.stack.iter().skip(1) {
        transform = ViewportTransform::from_viewport(vp, &transform);
    }
    transform
}

// endregion
