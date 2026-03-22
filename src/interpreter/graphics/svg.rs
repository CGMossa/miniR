//! SVG renderer — converts recorded plot commands into an SVG string.
//!
//! The renderer produces a standalone SVG document with axes, tick marks,
//! labels, and data rendered as circles (points) or polylines (lines).
//! The SVG is suitable both for direct file output and as input to
//! krilla-svg for PDF conversion.

use std::fmt::Write;

use super::{GraphicsDevice, PlotCommand, PlotType};

/// Render the commands recorded on a graphics device into an SVG string.
///
/// `width_in` and `height_in` are the page dimensions in inches. The SVG
/// uses 72 points per inch, matching R's default PDF coordinate system.
pub(crate) fn render_svg(device: &GraphicsDevice) -> String {
    let pts_per_inch = 72.0;
    let svg_w = device.width * pts_per_inch;
    let svg_h = device.height * pts_per_inch;

    // Plot area margins (in points)
    let margin_left = 60.0;
    let margin_right = 20.0;
    let margin_top = 40.0;
    let margin_bottom = 50.0;

    let plot_w = svg_w - margin_left - margin_right;
    let plot_h = svg_h - margin_top - margin_bottom;

    let mut svg = String::with_capacity(4096);

    writeln!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{svg_w}" height="{svg_h}" viewBox="0 0 {svg_w} {svg_h}">"#,
    )
    .unwrap();

    // White background
    writeln!(
        svg,
        r#"<rect width="{svg_w}" height="{svg_h}" fill="white"/>"#,
    )
    .unwrap();

    // Collect all data bounds across every command so we can compute a single
    // axis range that fits everything.
    let (x_min, x_max, y_min, y_max) = compute_bounds(&device.commands);
    if x_min >= x_max || y_min >= y_max {
        // Degenerate range — nothing meaningful to draw.
        svg.push_str("</svg>");
        return svg;
    }

    // Map data coordinates to SVG pixel coordinates inside the plot area.
    let map_x = |x: f64| -> f64 { margin_left + (x - x_min) / (x_max - x_min) * plot_w };
    let map_y = |y: f64| -> f64 { margin_top + (1.0 - (y - y_min) / (y_max - y_min)) * plot_h };

    // Draw plot area border
    writeln!(
        svg,
        r#"<rect x="{margin_left}" y="{margin_top}" width="{plot_w}" height="{plot_h}" fill="none" stroke="black" stroke-width="1"/>"#,
    )
    .unwrap();

    // Draw axes (tick marks and labels)
    draw_axis_x(&mut svg, x_min, x_max, &map_x, margin_top + plot_h, svg_h);
    draw_axis_y(&mut svg, y_min, y_max, &map_y, margin_left);

    // Title (from first Plot command or from a Title command)
    let mut title: Option<String> = None;
    let mut xlab: Option<String> = None;
    let mut ylab: Option<String> = None;

    for cmd in &device.commands {
        match cmd {
            PlotCommand::Plot {
                main,
                xlab: xl,
                ylab: yl,
                ..
            } => {
                if title.is_none() {
                    title.clone_from(main);
                }
                if xlab.is_none() {
                    xlab.clone_from(xl);
                }
                if ylab.is_none() {
                    ylab.clone_from(yl);
                }
            }
            PlotCommand::Title { main: Some(m) } => {
                title = Some(m.clone());
            }
            _ => {}
        }
    }

    if let Some(ref t) = title {
        let cx = svg_w / 2.0;
        writeln!(
            svg,
            r#"<text x="{cx}" y="{ty}" text-anchor="middle" font-size="14" font-family="sans-serif">{text}</text>"#,
            ty = margin_top - 10.0,
            text = xml_escape(t),
        )
        .unwrap();
    }
    if let Some(ref lab) = xlab {
        let cx = margin_left + plot_w / 2.0;
        writeln!(
            svg,
            r#"<text x="{cx}" y="{ty}" text-anchor="middle" font-size="12" font-family="sans-serif">{text}</text>"#,
            ty = svg_h - 5.0,
            text = xml_escape(lab),
        )
        .unwrap();
    }
    if let Some(ref lab) = ylab {
        let cy = margin_top + plot_h / 2.0;
        writeln!(
            svg,
            r#"<text x="{tx}" y="{cy}" text-anchor="middle" font-size="12" font-family="sans-serif" transform="rotate(-90,{tx},{cy})">{text}</text>"#,
            tx = 15.0,
            text = xml_escape(lab),
        )
        .unwrap();
    }

    // Render each command
    for cmd in &device.commands {
        match cmd {
            PlotCommand::Plot {
                x, y, plot_type, ..
            } => {
                if matches!(plot_type, PlotType::Lines | PlotType::Both) {
                    draw_lines(&mut svg, x, y, &map_x, &map_y, "black");
                }
                if matches!(plot_type, PlotType::Points | PlotType::Both) {
                    draw_points(&mut svg, x, y, &map_x, &map_y, "black");
                }
            }
            PlotCommand::Points { x, y } => {
                draw_points(&mut svg, x, y, &map_x, &map_y, "black");
            }
            PlotCommand::Lines { x, y } => {
                draw_lines(&mut svg, x, y, &map_x, &map_y, "black");
            }
            PlotCommand::Abline {
                intercept,
                slope,
                h,
                v,
            } => {
                if let (Some(a), Some(b)) = (intercept, slope) {
                    // y = a + b*x, clip to plot bounds
                    let lx = x_min;
                    let rx = x_max;
                    let ly = a + b * lx;
                    let ry = a + b * rx;
                    writeln!(
                        svg,
                        r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="black" stroke-width="1"/>"#,
                        map_x(lx), map_y(ly), map_x(rx), map_y(ry),
                    )
                    .unwrap();
                }
                if let Some(hv) = h {
                    writeln!(
                        svg,
                        r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="black" stroke-width="1" stroke-dasharray="4,2"/>"#,
                        map_x(x_min), map_y(*hv), map_x(x_max), map_y(*hv),
                    )
                    .unwrap();
                }
                if let Some(vv) = v {
                    writeln!(
                        svg,
                        r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="black" stroke-width="1" stroke-dasharray="4,2"/>"#,
                        map_x(*vv), map_y(y_min), map_x(*vv), map_y(y_max),
                    )
                    .unwrap();
                }
            }
            PlotCommand::Title { .. } => { /* handled above */ }
        }
    }

    svg.push_str("</svg>");
    svg
}

// region: Drawing helpers

fn draw_points(
    svg: &mut String,
    x: &[f64],
    y: &[f64],
    map_x: &dyn Fn(f64) -> f64,
    map_y: &dyn Fn(f64) -> f64,
    color: &str,
) {
    for (xi, yi) in x.iter().zip(y.iter()) {
        let px = map_x(*xi);
        let py = map_y(*yi);
        writeln!(svg, r#"<circle cx="{px}" cy="{py}" r="3" fill="{color}"/>"#,).unwrap();
    }
}

fn draw_lines(
    svg: &mut String,
    x: &[f64],
    y: &[f64],
    map_x: &dyn Fn(f64) -> f64,
    map_y: &dyn Fn(f64) -> f64,
    color: &str,
) {
    if x.len() < 2 {
        return;
    }
    let mut points = String::new();
    for (xi, yi) in x.iter().zip(y.iter()) {
        if !points.is_empty() {
            points.push(' ');
        }
        write!(points, "{},{}", map_x(*xi), map_y(*yi)).unwrap();
    }
    writeln!(
        svg,
        r#"<polyline points="{points}" fill="none" stroke="{color}" stroke-width="1"/>"#,
    )
    .unwrap();
}

fn draw_axis_x(
    svg: &mut String,
    x_min: f64,
    x_max: f64,
    map_x: &dyn Fn(f64) -> f64,
    axis_y: f64,
    svg_h: f64,
) {
    let ticks = nice_ticks(x_min, x_max, 5);
    let label_y = axis_y + 15.0;
    // Only draw labels if they fit above the bottom of the SVG
    let draw_labels = label_y < svg_h - 2.0;
    for &t in &ticks {
        let px = map_x(t);
        writeln!(
            svg,
            r#"<line x1="{px}" y1="{axis_y}" x2="{px}" y2="{ty}" stroke="black" stroke-width="1"/>"#,
            ty = axis_y + 5.0,
        )
        .unwrap();
        if draw_labels {
            writeln!(
                svg,
                r#"<text x="{px}" y="{label_y}" text-anchor="middle" font-size="10" font-family="sans-serif">{label}</text>"#,
                label = format_tick(t),
            )
            .unwrap();
        }
    }
}

fn draw_axis_y(svg: &mut String, y_min: f64, y_max: f64, map_y: &dyn Fn(f64) -> f64, axis_x: f64) {
    let ticks = nice_ticks(y_min, y_max, 5);
    for &t in &ticks {
        let py = map_y(t);
        writeln!(
            svg,
            r#"<line x1="{tx}" y1="{py}" x2="{axis_x}" y2="{py}" stroke="black" stroke-width="1"/>"#,
            tx = axis_x - 5.0,
        )
        .unwrap();
        writeln!(
            svg,
            r#"<text x="{tx}" y="{ty}" text-anchor="end" font-size="10" font-family="sans-serif">{label}</text>"#,
            tx = axis_x - 8.0,
            ty = py + 3.5,
            label = format_tick(t),
        )
        .unwrap();
    }
}

// endregion

// region: Axis helpers

/// Compute the data bounds from all commands.
fn compute_bounds(commands: &[PlotCommand]) -> (f64, f64, f64, f64) {
    let mut x_min = f64::INFINITY;
    let mut x_max = f64::NEG_INFINITY;
    let mut y_min = f64::INFINITY;
    let mut y_max = f64::NEG_INFINITY;

    let mut extend = |xs: &[f64], ys: &[f64]| {
        for &x in xs {
            if x < x_min {
                x_min = x;
            }
            if x > x_max {
                x_max = x;
            }
        }
        for &y in ys {
            if y < y_min {
                y_min = y;
            }
            if y > y_max {
                y_max = y;
            }
        }
    };

    for cmd in commands {
        match cmd {
            PlotCommand::Plot { x, y, .. } => extend(x, y),
            PlotCommand::Points { x, y } => extend(x, y),
            PlotCommand::Lines { x, y } => extend(x, y),
            PlotCommand::Abline { .. } | PlotCommand::Title { .. } => {}
        }
    }

    // Add 5% padding
    if x_min < x_max {
        let pad = (x_max - x_min) * 0.05;
        x_min -= pad;
        x_max += pad;
    }
    if y_min < y_max {
        let pad = (y_max - y_min) * 0.05;
        y_min -= pad;
        y_max += pad;
    }

    (x_min, x_max, y_min, y_max)
}

/// Generate "nice" tick positions for an axis.
fn nice_ticks(min: f64, max: f64, target_count: usize) -> Vec<f64> {
    let range = max - min;
    if range <= 0.0 || !range.is_finite() {
        return vec![];
    }

    let rough_step = range / target_count as f64;
    let magnitude = libm::pow(10.0, libm::floor(libm::log10(rough_step)));
    let residual = rough_step / magnitude;

    let nice_step = if residual <= 1.5 {
        magnitude
    } else if residual <= 3.5 {
        2.0 * magnitude
    } else if residual <= 7.5 {
        5.0 * magnitude
    } else {
        10.0 * magnitude
    };

    let start = libm::ceil(min / nice_step) * nice_step;
    let mut ticks = Vec::new();
    let mut t = start;
    while t <= max + nice_step * 1e-9 {
        ticks.push(t);
        t += nice_step;
    }
    ticks
}

/// Format a tick value, removing unnecessary trailing zeros.
fn format_tick(value: f64) -> String {
    if value == value.floor() && value.abs() < 1e15 {
        format!("{}", value as i64)
    } else {
        format!("{:.2}", value)
    }
}

/// XML-escape a string for embedding in SVG text content.
fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

// endregion
