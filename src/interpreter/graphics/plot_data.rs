//! Data structures for accumulated plot items.
//!
//! These types represent the plot data model that is independent of any
//! rendering backend. Plot builtins build up `PlotState` by appending
//! `PlotItem`s, and the rendering backend (egui_plot when enabled, or
//! just data capture when not) converts them for display.

// region: PlotItem

/// A single drawable item in a plot.
#[derive(Debug, Clone)]
pub enum PlotItem {
    /// A connected line series.
    Line {
        x: Vec<f64>,
        y: Vec<f64>,
        color: [u8; 4],
        width: f32,
        label: Option<String>,
    },
    /// Discrete point markers.
    Points {
        x: Vec<f64>,
        y: Vec<f64>,
        color: [u8; 4],
        size: f32,
        shape: u8,
        label: Option<String>,
    },
    /// Vertical bar chart.
    Bars {
        x: Vec<f64>,
        heights: Vec<f64>,
        color: [u8; 4],
        width: f64,
        label: Option<String>,
    },
    /// Box-and-whisker plots.
    BoxPlot {
        positions: Vec<f64>,
        spreads: Vec<BoxSpread>,
        color: [u8; 4],
    },
    /// Horizontal reference line.
    HLine { y: f64, color: [u8; 4], width: f32 },
    /// Vertical reference line.
    VLine { x: f64, color: [u8; 4], width: f32 },
    /// Text annotation at plot coordinates.
    Text {
        x: f64,
        y: f64,
        text: String,
        color: [u8; 4],
    },
}

// endregion

// region: BoxSpread

/// Five-number summary for a single box in a box plot.
#[derive(Debug, Clone)]
pub struct BoxSpread {
    pub lower_whisker: f64,
    pub q1: f64,
    pub median: f64,
    pub q3: f64,
    pub upper_whisker: f64,
}

// endregion

// region: PlotState

/// The accumulated state for a single plot.
///
/// High-level R plot functions (plot, hist, barplot, etc.) populate this
/// struct. The rendering backend reads it to produce visual output.
#[derive(Debug, Clone)]
pub struct PlotState {
    pub items: Vec<PlotItem>,
    pub title: Option<String>,
    pub x_label: Option<String>,
    pub y_label: Option<String>,
    pub x_lim: Option<(f64, f64)>,
    pub y_lim: Option<(f64, f64)>,
    pub show_legend: bool,
}

impl PlotState {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            title: None,
            x_label: None,
            y_label: None,
            x_lim: None,
            y_lim: None,
            show_legend: false,
        }
    }
}

impl Default for PlotState {
    fn default() -> Self {
        Self::new()
    }
}

// endregion
