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

#[cfg(feature = "svg-device")]
pub mod svg_device;

#[cfg(feature = "pdf-device")]
pub mod pdf;

#[cfg(all(feature = "plot", feature = "io"))]
pub mod csv_drop;

// region: FileDevice

/// A file-based graphics device (SVG, PNG, PDF).
#[derive(Debug, Clone)]
pub struct FileDevice {
    pub filename: String,
    pub format: FileFormat,
    pub width: f64,
    pub height: f64,
}

/// Supported file device formats.
#[derive(Debug, Clone, Copy)]
pub enum FileFormat {
    Svg,
    Png,
    Pdf,
}

// endregion
