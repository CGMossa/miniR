//! Grid graphics system — R's grid package data model.
//!
//! Provides viewports, grobs, a display list, and a renderer trait.
//! This is the foundation for ggplot2-style layered graphics.

pub mod display;
pub mod gpar;
pub mod grob;
pub mod render;
pub mod units;
pub mod viewport;
