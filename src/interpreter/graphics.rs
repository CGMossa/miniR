//! Graphics subsystem — plot data model, color system, and rendering backends.
//!
//! The plot data structures (`PlotState`, `PlotItem`), color system, and par state
//! are always available. The interactive egui rendering backend is gated behind
//! `feature = "plot"`.

pub mod color;
pub mod par;
pub mod plot_data;

#[cfg(feature = "plot")]
pub mod egui_device;
