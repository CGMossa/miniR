//! Grid graphics system — R's `grid` package unit system and graphical parameters.
//!
//! This module implements the core data structures from R's grid graphics:
//! - `Unit` — flexible measurement system (npc, cm, inches, etc.)
//! - `Gpar` — graphical parameter sets with inheritance
//! - `Viewport` — coordinate system containers (stub)
//! - `Grob` — graphical objects (stub)
//! - `Display` — display list management (stub)
//! - `Render` — rendering dispatch (stub)

pub mod display;
pub mod gpar;
pub mod grob;
pub mod render;
pub mod units;
pub mod viewport;
