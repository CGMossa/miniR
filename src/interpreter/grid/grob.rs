//! Grid graphical objects (grobs) — the drawing primitives.
//!
//! Grobs are the leaf nodes of grid graphics: lines, rectangles, circles,
//! text, points, etc. Each grob carries its own unit-based coordinates
//! and graphical parameters.

use super::gpar::Gpar;
use super::units::Unit;
use super::viewport::Justification;

// region: GrobId

/// Opaque identifier for a grob stored in a `GrobStore`.
pub type GrobId = usize;

// endregion

// region: Grob

/// A graphical object (grob) — one of grid's drawing primitives.
///
/// All coordinates are stored as `Unit` values and resolved at render time
/// using the current viewport transform and unit context.
#[derive(Clone, Debug)]
pub enum Grob {
    /// Connected line segments.
    Lines { x: Unit, y: Unit, gp: Gpar },
    /// Disconnected line segments (pairs of start/end points).
    Segments {
        x0: Unit,
        y0: Unit,
        x1: Unit,
        y1: Unit,
        gp: Gpar,
    },
    /// Points (scatter plot symbols).
    Points {
        x: Unit,
        y: Unit,
        /// Plotting character (symbol type: 0-25).
        pch: u8,
        /// Point size.
        size: Unit,
        gp: Gpar,
    },
    /// Rectangles.
    Rect {
        x: Unit,
        y: Unit,
        width: Unit,
        height: Unit,
        just: (Justification, Justification),
        gp: Gpar,
    },
    /// Circles.
    Circle {
        x: Unit,
        y: Unit,
        /// Radius.
        r: Unit,
        gp: Gpar,
    },
    /// Filled polygon.
    Polygon { x: Unit, y: Unit, gp: Gpar },
    /// Text labels.
    Text {
        label: Vec<String>,
        x: Unit,
        y: Unit,
        just: (Justification, Justification),
        /// Rotation in degrees.
        rot: f64,
        gp: Gpar,
    },
    /// A collection of child grobs.
    Collection { children: Vec<GrobId> },
}

impl Grob {
    /// Return the graphical parameters for this grob, if it has any.
    /// Collections don't have their own gpar.
    pub fn gpar(&self) -> Option<&Gpar> {
        match self {
            Grob::Lines { gp, .. }
            | Grob::Segments { gp, .. }
            | Grob::Points { gp, .. }
            | Grob::Rect { gp, .. }
            | Grob::Circle { gp, .. }
            | Grob::Polygon { gp, .. }
            | Grob::Text { gp, .. } => Some(gp),
            Grob::Collection { .. } => None,
        }
    }
}

// endregion

// region: GrobStore

/// A store for grobs, indexed by `GrobId`.
///
/// Grobs are added to the store and referenced by their ID in the display
/// list and in collection grobs.
pub struct GrobStore {
    grobs: Vec<Grob>,
}

impl GrobStore {
    /// Create a new empty grob store.
    pub fn new() -> Self {
        GrobStore { grobs: Vec::new() }
    }

    /// Add a grob and return its ID.
    pub fn add(&mut self, grob: Grob) -> GrobId {
        let id = self.grobs.len();
        self.grobs.push(grob);
        id
    }

    /// Get a grob by ID.
    pub fn get(&self, id: GrobId) -> Option<&Grob> {
        self.grobs.get(id)
    }

    /// Return the number of grobs in the store.
    pub fn len(&self) -> usize {
        self.grobs.len()
    }

    /// Whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.grobs.is_empty()
    }
}

impl Default for GrobStore {
    fn default() -> Self {
        Self::new()
    }
}

// endregion
