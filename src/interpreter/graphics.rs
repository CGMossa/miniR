//! Graphics infrastructure: device trait, drawing context, and null device.
//!
//! This module provides the foundational types for R's graphics system:
//!
//! - [`GraphicsDevice`] — the trait every rendering backend implements
//! - [`GraphicsContext`] — colors, line styles, fonts passed to each draw call
//! - [`NullDevice`] — the default no-op device (R's "null device")

pub mod context;
pub mod device;
pub mod null_device;

pub use context::{FontFace, GraphicsContext, LineType, PointChar, RColor};
pub use device::{CharMetric, DeviceSize, GraphicsDevice};
pub use null_device::NullDevice;

pub mod device_manager;
pub use device_manager::DeviceManager;
