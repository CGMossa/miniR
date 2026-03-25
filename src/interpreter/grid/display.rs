//! Grid display list — records the sequence of viewport and drawing operations.
//!
//! The display list is the grid equivalent of a "scene graph". It records
//! push/pop viewport operations and grob draws in order, so the entire
//! scene can be replayed through any renderer (SVG, egui, PDF, etc.).

use super::grob::GrobId;
use super::viewport::Viewport;

// region: DisplayItem

/// A single item in the display list.
#[derive(Clone, Debug)]
pub enum DisplayItem {
    /// Push a viewport onto the viewport stack.
    PushViewport(Box<Viewport>),
    /// Pop the current viewport (return to parent).
    PopViewport,
    /// Draw a grob (referenced by its store ID).
    Draw(GrobId),
}

// endregion

// region: DisplayList

/// An ordered list of display items that records the full scene.
///
/// Can be replayed through a `GridRenderer` to produce output.
#[derive(Clone, Debug)]
pub struct DisplayList {
    items: Vec<DisplayItem>,
}

impl DisplayList {
    /// Create a new empty display list.
    pub fn new() -> Self {
        DisplayList { items: Vec::new() }
    }

    /// Record a display item.
    pub fn record(&mut self, item: DisplayItem) {
        self.items.push(item);
    }

    /// Clear all recorded items.
    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// Return a slice of all recorded items.
    pub fn items(&self) -> &[DisplayItem] {
        &self.items
    }

    /// Return the number of recorded items.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether the display list is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

impl Default for DisplayList {
    fn default() -> Self {
        Self::new()
    }
}

// endregion
