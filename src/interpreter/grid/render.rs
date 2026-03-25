//! Grid rendering dispatch — converts grobs to device-specific drawing calls.
//!
//! The renderer walks the display list, resolves units in each viewport's
//! context, and emits draw commands to the active graphics device (egui, SVG,
//! PDF, etc.). This decouples the grid data model from any specific backend.
//!
//! This module is a stub — rendering will be implemented when grob types
//! and the viewport tree are in place.
