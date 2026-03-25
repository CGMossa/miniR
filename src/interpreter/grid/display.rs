//! Grid display list — records drawing operations for replay.
//!
//! The display list stores the sequence of grobs and viewport operations
//! so that the entire scene can be redrawn when the device is resized or
//! refreshed. This is analogous to R's `grid.ls()` and `grid.refresh()`.
//!
//! This module is a stub — display list recording and replay will be
//! implemented alongside the viewport tree and grob types.
