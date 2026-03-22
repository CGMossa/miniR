//! Graphics subsystem — plot data model, color system, and rendering backends.
//!
//! The plot data structures (`PlotState`, `PlotItem`), color system, and par state
//! are always available. The interactive egui rendering backend is gated behind
//! `feature = "plot"`.

pub mod color;
pub mod context;
pub mod device;
pub mod device_manager;
pub mod null_device;
pub mod par;
pub mod plot_data;

pub use context::{FontFace, GraphicsContext, LineType, PointChar, RColor};
pub use device::{CharMetric, DeviceSize, GraphicsDevice};
pub use device_manager::DeviceManager;
pub use null_device::NullDevice;

#[cfg(feature = "plot")]
pub mod egui_device;
