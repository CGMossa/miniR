//! SVG rendering — converts a `PlotState` into an SVG document using the `svg` crate.
//!
//! The renderer maps data coordinates to SVG pixel coordinates using a linear
//! transform with margins for axes, tick marks, and labels. Output is a
//! self-contained SVG string suitable for writing to a file.

use svg::node::element::{Circle, Line, Polyline, Rectangle, Text};
use svg::Document;

use super::plot_data::{PlotItem, PlotState};

/// Default dots-per-inch used to convert R's inch-based device dimensions
/// to SVG pixel units.
const DPI: f64 = 96.0;

/// Margin (in pixels) around the plot area for axes and labels.
const MARGIN_LEFT: f64 = 70.0;
const MARGIN_RIGHT: f64 = 30.0;
const MARGIN_TOP: f64 = 50.0;
const MARGIN_BOTTOM: f64 = 60.0;

/// Render a `PlotState` to an SVG string.
///
/// `width_in` and `height_in` are the device dimensions in inches (R defaults
/// to 7x7 for `svg()`). They are converted to pixels at 96 DPI.
pub fn render_svg(plot: &PlotState, width_in: f64, height_in: f64) -> String {
    let total_w = width_in * DPI;
    let total_h = height_in * DPI;

    let plot_x0 = MARGIN_LEFT;
    let plot_x1 = total_w - MARGIN_RIGHT;
    let plot_y0 = MARGIN_TOP;
    let plot_y1 = total_h - MARGIN_BOTTOM;
    let plot_w = plot_x1 - plot_x0;
    let plot_h = plot_y1 - plot_y0;

    let (x_min, x_max, y_min, y_max) = plot.compute_bounds();

    // Linear mapping helpers: data -> SVG pixel coordinates.
    let map_x = |xv: f64| -> f64 { plot_x0 + (xv - x_min) / (x_max - x_min) * plot_w };
    let map_y = |yv: f64| -> f64 {
        // SVG y-axis is top-down; data y-axis is bottom-up.
        plot_y1 - (yv - y_min) / (y_max - y_min) * plot_h
    };

    // Start building the SVG document.
    let mut doc = Document::new()
        .set("width", total_w)
        .set("height", total_h)
        .set("viewBox", format!("0 0 {total_w} {total_h}"));

    // White background
    let bg = Rectangle::new()
        .set("width", total_w)
        .set("height", total_h)
        .set("fill", "white");
    doc = doc.add(bg);

    // Plot area border
    let border = Rectangle::new()
        .set("x", plot_x0)
        .set("y", plot_y0)
        .set("width", plot_w)
        .set("height", plot_h)
        .set("fill", "none")
        .set("stroke", "black")
        .set("stroke-width", 1);
    doc = doc.add(border);

    // Draw items
    for item in &plot.items {
        match item {
            PlotItem::Points { x, y, color } => {
                for (xv, yv) in x.iter().zip(y.iter()) {
                    let circle = Circle::new()
                        .set("cx", format!("{:.2}", map_x(*xv)))
                        .set("cy", format!("{:.2}", map_y(*yv)))
                        .set("r", 3)
                        .set("fill", color.as_str());
                    doc = doc.add(circle);
                }
            }
            PlotItem::Line { x, y, color } => {
                if x.len() >= 2 {
                    let points_str: String = x
                        .iter()
                        .zip(y.iter())
                        .map(|(xv, yv)| format!("{:.2},{:.2}", map_x(*xv), map_y(*yv)))
                        .collect::<Vec<_>>()
                        .join(" ");
                    let polyline = Polyline::new()
                        .set("points", points_str)
                        .set("fill", "none")
                        .set("stroke", color.as_str())
                        .set("stroke-width", "1.5");
                    doc = doc.add(polyline);
                }
            }
            PlotItem::Bars { x, heights, color } => {
                let n = x.len();
                // Bar width: fraction of the plot-area width divided by bar count.
                let bar_w = if n > 1 {
                    (map_x(x[1]) - map_x(x[0])) * 0.8
                } else {
                    plot_w * 0.4
                };
                for (xv, hv) in x.iter().zip(heights.iter()) {
                    let sx = map_x(*xv) - bar_w / 2.0;
                    let sy_top = map_y(*hv);
                    let sy_bot = map_y(0.0);
                    let bar_h = sy_bot - sy_top;
                    let rect = Rectangle::new()
                        .set("x", format!("{sx:.2}"))
                        .set("y", format!("{sy_top:.2}"))
                        .set("width", format!("{bar_w:.2}"))
                        .set("height", format!("{bar_h:.2}"))
                        .set("fill", color.as_str())
                        .set("stroke", "black")
                        .set("stroke-width", "0.5");
                    doc = doc.add(rect);
                }
            }
            PlotItem::HLine { y, color } => {
                let sy = map_y(*y);
                let line = Line::new()
                    .set("x1", plot_x0)
                    .set("y1", format!("{sy:.2}"))
                    .set("x2", plot_x1)
                    .set("y2", format!("{sy:.2}"))
                    .set("stroke", color.as_str())
                    .set("stroke-width", 1)
                    .set("stroke-dasharray", "4,4");
                doc = doc.add(line);
            }
            PlotItem::VLine { x, color } => {
                let sx = map_x(*x);
                let line = Line::new()
                    .set("x1", format!("{sx:.2}"))
                    .set("y1", plot_y0)
                    .set("x2", format!("{sx:.2}"))
                    .set("y2", plot_y1)
                    .set("stroke", color.as_str())
                    .set("stroke-width", 1)
                    .set("stroke-dasharray", "4,4");
                doc = doc.add(line);
            }
            PlotItem::Text { x, y, label } => {
                let text = Text::new(label.as_str())
                    .set("x", format!("{:.2}", map_x(*x)))
                    .set("y", format!("{:.2}", map_y(*y)))
                    .set("font-size", 12)
                    .set("text-anchor", "middle")
                    .set("fill", "black");
                doc = doc.add(text);
            }
        }
    }

    // Axes: compute nice tick positions.
    let x_ticks = nice_ticks(x_min, x_max, 5);
    let y_ticks = nice_ticks(y_min, y_max, 5);

    // X-axis ticks and labels
    for &t in &x_ticks {
        let sx = map_x(t);
        // Tick mark
        let tick = Line::new()
            .set("x1", format!("{sx:.2}"))
            .set("y1", plot_y1)
            .set("x2", format!("{sx:.2}"))
            .set("y2", plot_y1 + 5.0)
            .set("stroke", "black")
            .set("stroke-width", 1);
        doc = doc.add(tick);
        // Label
        let label = Text::new(format_tick(t))
            .set("x", format!("{sx:.2}"))
            .set("y", plot_y1 + 20.0)
            .set("font-size", 11)
            .set("text-anchor", "middle")
            .set("fill", "black");
        doc = doc.add(label);
    }

    // Y-axis ticks and labels
    for &t in &y_ticks {
        let sy = map_y(t);
        // Tick mark
        let tick = Line::new()
            .set("x1", plot_x0 - 5.0)
            .set("y1", format!("{sy:.2}"))
            .set("x2", plot_x0)
            .set("y2", format!("{sy:.2}"))
            .set("stroke", "black")
            .set("stroke-width", 1);
        doc = doc.add(tick);
        // Label
        let label = Text::new(format_tick(t))
            .set("x", plot_x0 - 8.0)
            .set("y", format!("{sy:.2}"))
            .set("font-size", 11)
            .set("text-anchor", "end")
            .set("dominant-baseline", "middle")
            .set("fill", "black");
        doc = doc.add(label);
    }

    // X-axis label
    if !plot.xlab.is_empty() {
        let cx = (plot_x0 + plot_x1) / 2.0;
        let label = Text::new(plot.xlab.as_str())
            .set("x", format!("{cx:.2}"))
            .set("y", plot_y1 + 45.0)
            .set("font-size", 13)
            .set("text-anchor", "middle")
            .set("fill", "black");
        doc = doc.add(label);
    }

    // Y-axis label (rotated)
    if !plot.ylab.is_empty() {
        let cy = (plot_y0 + plot_y1) / 2.0;
        let label = Text::new(plot.ylab.as_str())
            .set("x", 15)
            .set("y", format!("{cy:.2}"))
            .set("font-size", 13)
            .set("text-anchor", "middle")
            .set("fill", "black")
            .set("transform", format!("rotate(-90, 15, {cy:.2})"));
        doc = doc.add(label);
    }

    // Title
    if !plot.main.is_empty() {
        let cx = (plot_x0 + plot_x1) / 2.0;
        let title = Text::new(plot.main.as_str())
            .set("x", format!("{cx:.2}"))
            .set("y", plot_y0 - 15.0)
            .set("font-size", 16)
            .set("font-weight", "bold")
            .set("text-anchor", "middle")
            .set("fill", "black");
        doc = doc.add(title);
    }

    doc.to_string()
}

/// Compute "nice" tick positions for an axis spanning [lo, hi] with
/// approximately `target` ticks.
fn nice_ticks(lo: f64, hi: f64, target: usize) -> Vec<f64> {
    let range = hi - lo;
    if range <= 0.0 || !range.is_finite() {
        return vec![lo];
    }

    let rough_step = range / target as f64;
    let mag = libm::pow(10.0, libm::floor(libm::log10(rough_step)));
    let norm = rough_step / mag;

    let nice_step = if norm <= 1.5 {
        mag
    } else if norm <= 3.5 {
        2.0 * mag
    } else if norm <= 7.5 {
        5.0 * mag
    } else {
        10.0 * mag
    };

    let start = libm::ceil(lo / nice_step) * nice_step;
    let mut ticks = Vec::new();
    let mut t = start;
    // Safety bound: never produce more than 100 ticks.
    while t <= hi + nice_step * 0.001 && ticks.len() < 100 {
        ticks.push(t);
        t += nice_step;
    }
    ticks
}

/// Format a tick value, stripping unnecessary trailing zeros.
fn format_tick(v: f64) -> String {
    if v == v.floor() && v.abs() < 1e15 {
        // Integer-valued: format without decimals.
        format!("{v:.0}")
    } else {
        // Use up to 4 decimal places, then trim trailing zeros.
        let s = format!("{v:.4}");
        let s = s.trim_end_matches('0');
        let s = s.trim_end_matches('.');
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nice_ticks_basic() {
        let ticks = nice_ticks(0.0, 10.0, 5);
        assert!(!ticks.is_empty());
        assert!(ticks.iter().all(|&t| (0.0..=10.0).contains(&t)));
    }

    #[test]
    fn format_tick_integer() {
        assert_eq!(format_tick(5.0), "5");
        assert_eq!(format_tick(0.0), "0");
    }

    #[test]
    fn format_tick_decimal() {
        assert_eq!(format_tick(2.5), "2.5");
    }

    #[test]
    fn render_svg_empty_plot() {
        let plot = PlotState::new();
        let svg = render_svg(&plot, 7.0, 7.0);
        assert!(svg.contains("<svg"));
        assert!(svg.contains("</svg>"));
    }

    #[test]
    fn render_svg_with_points() {
        let mut plot = PlotState::new();
        plot.items.push(PlotItem::Points {
            x: vec![1.0, 2.0, 3.0],
            y: vec![4.0, 5.0, 6.0],
            color: "black".to_string(),
        });
        plot.main = "Test".to_string();
        plot.xlab = "X".to_string();
        plot.ylab = "Y".to_string();
        let svg = render_svg(&plot, 7.0, 7.0);
        assert!(svg.contains("<circle"));
        assert!(svg.contains("Test"));
    }
}
