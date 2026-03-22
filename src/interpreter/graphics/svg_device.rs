//! SVG renderer — converts a `PlotState` into an SVG string.

use svg::node::element::{Circle, Group, Line, Polyline, Rectangle, Text as SvgText};
use svg::Document;

use super::plot_data::{PlotItem, PlotState};

const DPI: f64 = 96.0;
const MARGIN_LEFT: f64 = 70.0;
const MARGIN_RIGHT: f64 = 30.0;
const MARGIN_TOP: f64 = 50.0;
const MARGIN_BOTTOM: f64 = 60.0;

/// Render a PlotState to an SVG string.
pub fn render_svg(state: &PlotState, width_in: f64, height_in: f64) -> String {
    let w = width_in * DPI;
    let h = height_in * DPI;
    let plot_x0 = MARGIN_LEFT;
    let plot_x1 = w - MARGIN_RIGHT;
    let plot_y0 = MARGIN_TOP;
    let plot_y1 = h - MARGIN_BOTTOM;

    // Compute data bounds from all items
    let (dx0, dx1, dy0, dy1) = data_bounds(state);

    let map_x = |x: f64| plot_x0 + (x - dx0) / (dx1 - dx0) * (plot_x1 - plot_x0);
    let map_y = |y: f64| plot_y1 - (y - dy0) / (dy1 - dy0) * (plot_y1 - plot_y0);

    let mut doc = Document::new()
        .set("width", w)
        .set("height", h)
        .set("viewBox", (0.0, 0.0, w, h));

    // White background
    doc = doc.add(
        Rectangle::new()
            .set("width", w)
            .set("height", h)
            .set("fill", "white"),
    );

    // Plot area border
    doc = doc.add(
        Rectangle::new()
            .set("x", plot_x0)
            .set("y", plot_y0)
            .set("width", plot_x1 - plot_x0)
            .set("height", plot_y1 - plot_y0)
            .set("fill", "none")
            .set("stroke", "#cccccc"),
    );

    // Axes
    doc = add_axes(&doc, plot_x0, plot_x1, plot_y0, plot_y1, dx0, dx1, dy0, dy1);

    // Title
    if let Some(title) = &state.title {
        doc = doc.add(
            SvgText::new(title.clone())
                .set("x", w / 2.0)
                .set("y", 25.0)
                .set("text-anchor", "middle")
                .set("font-size", 16)
                .set("font-weight", "bold"),
        );
    }

    // Axis labels
    if let Some(xlab) = &state.x_label {
        doc = doc.add(
            SvgText::new(xlab.clone())
                .set("x", (plot_x0 + plot_x1) / 2.0)
                .set("y", h - 10.0)
                .set("text-anchor", "middle")
                .set("font-size", 12),
        );
    }
    if let Some(ylab) = &state.y_label {
        doc = doc.add(
            SvgText::new(ylab.clone())
                .set("x", 15.0)
                .set("y", (plot_y0 + plot_y1) / 2.0)
                .set("text-anchor", "middle")
                .set("font-size", 12)
                .set(
                    "transform",
                    format!("rotate(-90, 15, {})", (plot_y0 + plot_y1) / 2.0),
                ),
        );
    }

    // Render items
    let mut items_group = Group::new();
    for item in &state.items {
        items_group = render_item(items_group, item, &map_x, &map_y);
    }
    doc = doc.add(items_group);

    doc.to_string()
}

fn rgba_to_svg(c: [u8; 4]) -> String {
    if c[3] == 255 {
        format!("rgb({},{},{})", c[0], c[1], c[2])
    } else {
        format!(
            "rgba({},{},{},{})",
            c[0],
            c[1],
            c[2],
            f64::from(c[3]) / 255.0
        )
    }
}

fn data_bounds(state: &PlotState) -> (f64, f64, f64, f64) {
    let mut xmin = f64::INFINITY;
    let mut xmax = f64::NEG_INFINITY;
    let mut ymin = f64::INFINITY;
    let mut ymax = f64::NEG_INFINITY;

    for item in &state.items {
        match item {
            PlotItem::Points { x, y, .. } | PlotItem::Line { x, y, .. } => {
                for &v in x {
                    if v < xmin {
                        xmin = v;
                    }
                    if v > xmax {
                        xmax = v;
                    }
                }
                for &v in y {
                    if v < ymin {
                        ymin = v;
                    }
                    if v > ymax {
                        ymax = v;
                    }
                }
            }
            PlotItem::Bars { x, heights, .. } => {
                for &v in x {
                    if v < xmin {
                        xmin = v;
                    }
                    if v > xmax {
                        xmax = v;
                    }
                }
                for &v in heights {
                    if v > ymax {
                        ymax = v;
                    }
                }
                if 0.0 < ymin {
                    ymin = 0.0;
                }
            }
            PlotItem::HLine { y, .. } => {
                if *y < ymin {
                    ymin = *y;
                }
                if *y > ymax {
                    ymax = *y;
                }
            }
            PlotItem::VLine { x, .. } => {
                if *x < xmin {
                    xmin = *x;
                }
                if *x > xmax {
                    xmax = *x;
                }
            }
            PlotItem::Text { x, y, .. } => {
                if *x < xmin {
                    xmin = *x;
                }
                if *x > xmax {
                    xmax = *x;
                }
                if *y < ymin {
                    ymin = *y;
                }
                if *y > ymax {
                    ymax = *y;
                }
            }
            PlotItem::BoxPlot {
                positions, spreads, ..
            } => {
                for &p in positions {
                    if p < xmin {
                        xmin = p;
                    }
                    if p > xmax {
                        xmax = p;
                    }
                }
                for s in spreads {
                    if s.lower_whisker < ymin {
                        ymin = s.lower_whisker;
                    }
                    if s.upper_whisker > ymax {
                        ymax = s.upper_whisker;
                    }
                }
            }
        }
    }

    if let Some((lo, hi)) = state.x_lim {
        xmin = lo;
        xmax = hi;
    }
    if let Some((lo, hi)) = state.y_lim {
        ymin = lo;
        ymax = hi;
    }

    // Add 4% padding
    let xpad = (xmax - xmin).abs() * 0.04;
    let ypad = (ymax - ymin).abs() * 0.04;
    if xmin == xmax {
        xmin -= 1.0;
        xmax += 1.0;
    }
    if ymin == ymax {
        ymin -= 1.0;
        ymax += 1.0;
    }

    (xmin - xpad, xmax + xpad, ymin - ypad, ymax + ypad)
}

fn render_item(
    mut group: Group,
    item: &PlotItem,
    map_x: &dyn Fn(f64) -> f64,
    map_y: &dyn Fn(f64) -> f64,
) -> Group {
    match item {
        PlotItem::Points {
            x, y, color, size, ..
        } => {
            let fill = rgba_to_svg(*color);
            for (&xi, &yi) in x.iter().zip(y.iter()) {
                group = group.add(
                    Circle::new()
                        .set("cx", map_x(xi))
                        .set("cy", map_y(yi))
                        .set("r", *size as f64)
                        .set("fill", fill.as_str()),
                );
            }
        }
        PlotItem::Line {
            x, y, color, width, ..
        } => {
            let points: String = x
                .iter()
                .zip(y.iter())
                .map(|(&xi, &yi)| format!("{},{}", map_x(xi), map_y(yi)))
                .collect::<Vec<_>>()
                .join(" ");
            group = group.add(
                Polyline::new()
                    .set("points", points)
                    .set("fill", "none")
                    .set("stroke", rgba_to_svg(*color))
                    .set("stroke-width", *width as f64),
            );
        }
        PlotItem::Bars {
            x,
            heights,
            color,
            width,
            ..
        } => {
            let fill = rgba_to_svg(*color);
            for (&xi, &hi) in x.iter().zip(heights.iter()) {
                let sx = map_x(xi - width / 2.0);
                let sy = map_y(hi);
                let sw = map_x(xi + width / 2.0) - sx;
                let sh = map_y(0.0) - sy;
                group = group.add(
                    Rectangle::new()
                        .set("x", sx)
                        .set("y", sy)
                        .set("width", sw.abs())
                        .set("height", sh.abs())
                        .set("fill", fill.as_str())
                        .set("stroke", "black")
                        .set("stroke-width", 0.5),
                );
            }
        }
        PlotItem::HLine { y, color, width } => {
            let sy = map_y(*y);
            group = group.add(
                Line::new()
                    .set("x1", map_x(f64::NEG_INFINITY).max(0.0))
                    .set("x2", map_x(f64::INFINITY).min(10000.0))
                    .set("y1", sy)
                    .set("y2", sy)
                    .set("stroke", rgba_to_svg(*color))
                    .set("stroke-width", *width as f64),
            );
        }
        PlotItem::VLine { x, color, width } => {
            let sx = map_x(*x);
            group = group.add(
                Line::new()
                    .set("x1", sx)
                    .set("x2", sx)
                    .set("y1", map_y(f64::NEG_INFINITY).min(10000.0))
                    .set("y2", map_y(f64::INFINITY).max(0.0))
                    .set("stroke", rgba_to_svg(*color))
                    .set("stroke-width", *width as f64),
            );
        }
        PlotItem::Text { x, y, text, color } => {
            group = group.add(
                SvgText::new(text.clone())
                    .set("x", map_x(*x))
                    .set("y", map_y(*y))
                    .set("fill", rgba_to_svg(*color))
                    .set("font-size", 12),
            );
        }
        PlotItem::BoxPlot { .. } => {
            // Box plots in SVG are complex — defer to future work
        }
    }
    group
}

#[allow(clippy::too_many_arguments)]
fn add_axes(
    doc: &Document,
    px0: f64,
    px1: f64,
    py0: f64,
    py1: f64,
    dx0: f64,
    dx1: f64,
    dy0: f64,
    dy1: f64,
) -> Document {
    let mut d = doc.clone();
    let x_ticks = nice_ticks(dx0, dx1, 6);
    let y_ticks = nice_ticks(dy0, dy1, 6);
    let map_x = |v: f64| px0 + (v - dx0) / (dx1 - dx0) * (px1 - px0);
    let map_y = |v: f64| py1 - (v - dy0) / (dy1 - dy0) * (py1 - py0);

    for &t in &x_ticks {
        let sx = map_x(t);
        d = d.add(
            Line::new()
                .set("x1", sx)
                .set("x2", sx)
                .set("y1", py1)
                .set("y2", py1 + 5.0)
                .set("stroke", "black"),
        );
        d = d.add(
            SvgText::new(format_tick(t))
                .set("x", sx)
                .set("y", py1 + 18.0)
                .set("text-anchor", "middle")
                .set("font-size", 10),
        );
    }
    for &t in &y_ticks {
        let sy = map_y(t);
        d = d.add(
            Line::new()
                .set("x1", px0 - 5.0)
                .set("x2", px0)
                .set("y1", sy)
                .set("y2", sy)
                .set("stroke", "black"),
        );
        d = d.add(
            SvgText::new(format_tick(t))
                .set("x", px0 - 8.0)
                .set("y", sy + 4.0)
                .set("text-anchor", "end")
                .set("font-size", 10),
        );
    }
    d
}

fn nice_ticks(lo: f64, hi: f64, target: usize) -> Vec<f64> {
    let range = hi - lo;
    if range <= 0.0 {
        return vec![lo];
    }
    let rough = range / target as f64;
    let mag = 10f64.powf(rough.log10().floor());
    let step = if rough / mag < 1.5 {
        mag
    } else if rough / mag < 3.5 {
        2.0 * mag
    } else if rough / mag < 7.5 {
        5.0 * mag
    } else {
        10.0 * mag
    };
    let start = (lo / step).ceil() * step;
    let mut ticks = Vec::new();
    let mut t = start;
    while t <= hi + step * 0.01 {
        ticks.push(t);
        t += step;
    }
    ticks
}

fn format_tick(v: f64) -> String {
    if v == v.floor() && v.abs() < 1e6 {
        format!("{}", v as i64)
    } else {
        format!("{:.2}", v)
    }
}
