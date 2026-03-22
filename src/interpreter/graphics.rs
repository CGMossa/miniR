//! Graphics subsystem — plot data model, View() data model, color system,
//! and rendering backends.
//!
//! The data structures are always available. The interactive egui backends
//! are gated behind `feature = "plot"` and `feature = "view"`.

pub mod color;
pub mod par;
pub mod plot_data;
pub mod view;

#[cfg(feature = "plot")]
pub mod egui_device;
