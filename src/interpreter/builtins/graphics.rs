//! Graphics builtins — high-level R plotting functions that accumulate
//! `PlotItem`s in the interpreter's `PlotState`, then display via
//! egui_plot (when the `plot` feature is enabled) or print a helpful
//! message (when it is not).

use super::CallArgs;
use crate::interpreter::graphics::plot_data::{BoxSpread, PlotItem, PlotState};
use crate::interpreter::value::*;
use crate::interpreter::BuiltinContext;
use minir_macros::{builtin, interpreter_builtin};

// region: Color helpers

/// Parse an R color specification to RGBA.
///
/// Supports: color names ("red", "blue"), hex strings ("#RRGGBB", "#RRGGBBAA"),
/// and integer indices into the default palette.
fn parse_color(value: &RValue) -> [u8; 4] {
    match value {
        RValue::Vector(rv) => {
            if let Some(s) = rv.inner.as_character_scalar() {
                parse_color_string(&s)
            } else if let Some(i) = rv.inner.as_integer_scalar() {
                default_palette_color(i)
            } else {
                [0, 0, 0, 255] // black fallback
            }
        }
        _ => [0, 0, 0, 255],
    }
}

/// Parse a color string: named color or hex.
fn parse_color_string(s: &str) -> [u8; 4] {
    match s.to_lowercase().as_str() {
        "black" => [0, 0, 0, 255],
        "white" => [255, 255, 255, 255],
        "red" => [255, 0, 0, 255],
        "green" | "green3" => [0, 205, 0, 255],
        "blue" => [0, 0, 255, 255],
        "cyan" => [0, 255, 255, 255],
        "magenta" => [255, 0, 255, 255],
        "yellow" => [255, 255, 0, 255],
        "gray" | "grey" => [190, 190, 190, 255],
        "orange" => [255, 165, 0, 255],
        "purple" => [160, 32, 240, 255],
        "brown" => [165, 42, 42, 255],
        "pink" => [255, 192, 203, 255],
        "darkred" => [139, 0, 0, 255],
        "darkgreen" => [0, 100, 0, 255],
        "darkblue" | "navyblue" | "navy" => [0, 0, 128, 255],
        "lightblue" => [173, 216, 230, 255],
        "lightgreen" => [144, 238, 144, 255],
        "lightgray" | "lightgrey" => [211, 211, 211, 255],
        "darkgray" | "darkgrey" => [169, 169, 169, 255],
        "transparent" => [0, 0, 0, 0],
        _ if s.starts_with('#') => parse_hex_color(s),
        _ => [0, 0, 0, 255], // unknown → black
    }
}

/// Parse a hex color string like "#RRGGBB" or "#RRGGBBAA".
fn parse_hex_color(s: &str) -> [u8; 4] {
    let hex = s.trim_start_matches('#');
    let parse_byte =
        |offset: usize| -> u8 { u8::from_str_radix(&hex[offset..offset + 2], 16).unwrap_or(0) };
    match hex.len() {
        6 => [parse_byte(0), parse_byte(2), parse_byte(4), 255],
        8 => [parse_byte(0), parse_byte(2), parse_byte(4), parse_byte(6)],
        _ => [0, 0, 0, 255],
    }
}

/// Default palette: R's default color cycle.
fn default_palette_color(index: i64) -> [u8; 4] {
    const PALETTE: [[u8; 4]; 8] = [
        [0, 0, 0, 255],       // 1: black
        [255, 0, 0, 255],     // 2: red
        [0, 205, 0, 255],     // 3: green3
        [0, 0, 255, 255],     // 4: blue
        [0, 255, 255, 255],   // 5: cyan
        [255, 0, 255, 255],   // 6: magenta
        [255, 255, 0, 255],   // 7: yellow
        [190, 190, 190, 255], // 8: gray
    ];
    if index < 1 {
        return [0, 0, 0, 255];
    }
    let idx = usize::try_from(index - 1).unwrap_or(0) % PALETTE.len();
    PALETTE[idx]
}

// endregion

// region: Vector extraction helpers

/// Extract a numeric vector from an RValue, filtering out NAs.
fn extract_doubles(value: &RValue) -> Result<Vec<f64>, RError> {
    match value.as_vector() {
        Some(v) => Ok(v.to_doubles().into_iter().flatten().collect()),
        None => Err(RError::new(
            RErrorKind::Argument,
            "expected a numeric vector".to_string(),
        )),
    }
}

/// Try to extract a double vector from an optional RValue.
fn try_extract_doubles(value: Option<&RValue>) -> Result<Option<Vec<f64>>, RError> {
    match value {
        Some(RValue::Null) | None => Ok(None),
        Some(v) => Ok(Some(extract_doubles(v)?)),
    }
}

/// Extract a (lo, hi) range from a two-element numeric vector.
fn extract_limits(value: Option<&RValue>) -> Option<(f64, f64)> {
    let v = value?;
    if matches!(v, RValue::Null) {
        return None;
    }
    let vec = v.as_vector()?;
    let doubles = vec.to_doubles();
    if doubles.len() >= 2 {
        match (doubles[0], doubles[1]) {
            (Some(lo), Some(hi)) => Some((lo, hi)),
            _ => None,
        }
    } else {
        None
    }
}

// endregion

// region: show_or_accumulate

/// Show the plot window if the `plot` feature is enabled, otherwise
/// print a helpful message. If `immediate` is false, just accumulate
/// (the plot will be shown when dev.off() is called or at the next
/// top-level prompt).
#[cfg(feature = "plot")]
fn show_current_plot(ctx: &BuiltinContext) -> Result<(), RError> {
    let state = ctx.interpreter().current_plot.borrow().clone();
    if let Some(plot_state) = state {
        // Blocks until user closes the window (X button).
        crate::interpreter::graphics::egui_device::show_plot_window(&plot_state)
            .map_err(|e| RError::new(RErrorKind::Other, format!("failed to display plot: {e}")))?;
        *ctx.interpreter().current_plot.borrow_mut() = None;
    }
    Ok(())
}

#[cfg(not(feature = "plot"))]
fn show_current_plot(ctx: &BuiltinContext) -> Result<(), RError> {
    ctx.write_err("plot() requires the 'plot' feature. Build with: cargo build --features plot\n");
    // Still clear the plot data
    *ctx.interpreter().current_plot.borrow_mut() = None;
    Ok(())
}

/// Ensure a current plot exists, creating a new one if needed.
fn ensure_plot<'a>(ctx: &'a BuiltinContext<'_>) -> std::cell::RefMut<'a, Option<PlotState>> {
    let mut plot = ctx.interpreter().current_plot.borrow_mut();
    if plot.is_none() {
        *plot = Some(PlotState::new());
    }
    plot
}

// endregion

// region: Device management

/// Open a PDF graphics device.
///
/// File-based graphics devices are not yet implemented — this stub prints
/// a message and returns NULL so that scripts can continue.
///
/// @param file output file path (ignored)
/// @return NULL
#[interpreter_builtin(namespace = "grDevices")]
fn interp_pdf(
    _args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    context.write_err("pdf() file device is not yet supported in miniR\n");
    Ok(RValue::Null)
}

/// Open a PNG graphics device.
///
/// File-based graphics devices are not yet implemented — this stub prints
/// a message and returns NULL so that scripts can continue.
///
/// @param filename output file path (ignored)
/// @return NULL
#[interpreter_builtin(namespace = "grDevices")]
fn interp_png(
    _args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    context.write_err("png() file device is not yet supported in miniR\n");
    Ok(RValue::Null)
}

/// Open an SVG graphics device.
///
/// File-based graphics devices are not yet implemented — this stub prints
/// a message and returns NULL so that scripts can continue.
///
/// @param filename output file path (ignored)
/// @return NULL
#[interpreter_builtin(namespace = "grDevices")]
fn interp_svg(
    _args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    context.write_err("svg() file device is not yet supported in miniR\n");
    Ok(RValue::Null)
}

/// Close the current graphics device.
///
/// Closes any open plot window and clears the accumulated plot state.
///
/// @return integer 1 (invisibly)
#[interpreter_builtin(name = "dev.off", namespace = "grDevices")]
fn interp_dev_off(
    _args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    // Clear any accumulated plot data
    *context.interpreter().current_plot.borrow_mut() = None;
    context.interpreter().set_invisible();
    Ok(RValue::vec(Vector::Integer(vec![Some(1i64)].into())))
}

/// Return the current graphics device number.
///
/// Returns 2 if a plot is being accumulated (an "active" device),
/// or 1 (the null device) otherwise.
///
/// @return integer device number
#[interpreter_builtin(name = "dev.cur", namespace = "grDevices")]
fn interp_dev_cur(
    _args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let num = if context.interpreter().current_plot.borrow().is_some() {
        2i64
    } else {
        1i64
    };
    Ok(RValue::vec(Vector::Integer(vec![Some(num)].into())))
}

/// Open a new graphics device.
///
/// Creates a fresh PlotState for accumulating plot items.
///
/// @return integer device number (invisibly)
#[interpreter_builtin(name = "dev.new", namespace = "grDevices")]
fn interp_dev_new(
    _args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    *context.interpreter().current_plot.borrow_mut() = Some(PlotState::new());
    Ok(RValue::vec(Vector::Integer(vec![Some(2i64)].into())))
}

// endregion

// region: High-level plotting

/// Create a scatter plot, line plot, or combined plot.
///
/// This is R's main `plot()` function. It creates a new PlotState,
/// adds Points/Lines/Both based on the `type` parameter, then shows
/// the plot window immediately.
///
/// @param x numeric vector of x-coordinates (or sole y if y is missing)
/// @param y numeric vector of y-coordinates (optional)
/// @param type character: "p" (points), "l" (lines), "b" (both), "n" (none)
/// @param main plot title
/// @param xlab x-axis label
/// @param ylab y-axis label
/// @param col color specification
/// @param pch point character (0-25)
/// @param cex character expansion factor
/// @param lwd line width
/// @param xlim numeric vector c(lo, hi) for x-axis limits
/// @param ylim numeric vector c(lo, hi) for y-axis limits
/// @return NULL (invisibly)
#[interpreter_builtin(namespace = "graphics", min_args = 1)]
fn interp_plot(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let ca = CallArgs::new(args, named);

    // Extract x and y vectors
    let first = &args[0];
    let second = ca.value("y", 1);

    let (x_data, y_data) = if let Some(y_val) = second {
        // plot(x, y)
        (extract_doubles(first)?, extract_doubles(y_val)?)
    } else {
        // plot(x) — x becomes y, x is 1:length(x)
        let y = extract_doubles(first)?;
        let x: Vec<f64> = (1..=y.len()).map(|i| i as f64).collect();
        (x, y)
    };

    // Truncate to the shorter length
    let len = x_data.len().min(y_data.len());
    let x_data: Vec<f64> = x_data.into_iter().take(len).collect();
    let y_data: Vec<f64> = y_data.into_iter().take(len).collect();

    // Parse plot parameters
    let plot_type = ca
        .optional_string("type", 2)
        .unwrap_or_else(|| "p".to_string());
    let title = ca.optional_string("main", 3);
    let xlab = ca.optional_string("xlab", 4);
    let ylab = ca.optional_string("ylab", 5);
    let color = ca
        .value("col", 6)
        .map(parse_color)
        .unwrap_or([0, 0, 0, 255]);
    let pch = u8::try_from(ca.integer_or("pch", 7, 1)).unwrap_or(1);
    let cex = ca
        .value("cex", 8)
        .and_then(|v| v.as_vector()?.as_double_scalar())
        .unwrap_or(1.0);
    let lwd = ca
        .value("lwd", 9)
        .and_then(|v| v.as_vector()?.as_double_scalar())
        .unwrap_or(1.0);
    let xlim = extract_limits(ca.value("xlim", 10));
    let ylim = extract_limits(ca.value("ylim", 11));

    // Create a new plot
    let mut state = PlotState::new();
    state.title = title;
    state.x_label = xlab;
    state.y_label = ylab;
    state.x_lim = xlim;
    state.y_lim = ylim;

    // Add items based on type
    let point_size = (3.0 * cex) as f32;
    let line_width = lwd as f32;

    match plot_type.as_str() {
        "p" => {
            state.items.push(PlotItem::Points {
                x: x_data,
                y: y_data,
                color,
                size: point_size,
                shape: pch,
                label: None,
            });
        }
        "l" => {
            state.items.push(PlotItem::Line {
                x: x_data,
                y: y_data,
                color,
                width: line_width,
                label: None,
            });
        }
        "b" | "o" => {
            state.items.push(PlotItem::Line {
                x: x_data.clone(),
                y: y_data.clone(),
                color,
                width: line_width,
                label: None,
            });
            state.items.push(PlotItem::Points {
                x: x_data,
                y: y_data,
                color,
                size: point_size,
                shape: pch,
                label: None,
            });
        }
        "h" => {
            // Histogram-like vertical lines from x-axis
            for (&xi, &yi) in x_data.iter().zip(y_data.iter()) {
                state.items.push(PlotItem::Line {
                    x: vec![xi, xi],
                    y: vec![0.0, yi],
                    color,
                    width: line_width,
                    label: None,
                });
            }
        }
        "n" => {
            // Plot nothing — just set up axes
        }
        _ => {
            return Err(RError::new(
                RErrorKind::Argument,
                format!(
                    "invalid plot type '{plot_type}': expected 'p', 'l', 'b', 'o', 'h', or 'n'"
                ),
            ));
        }
    }

    // Store and show
    *context.interpreter().current_plot.borrow_mut() = Some(state);
    show_current_plot(context)?;

    Ok(RValue::Null)
}

/// Compute a histogram and display it as a bar chart.
///
/// @param x numeric vector of data values
/// @param breaks number of bins (default 10) or a vector of break points
/// @param col bar color
/// @param main plot title
/// @param xlab x-axis label
/// @return a list with breaks, counts, and mids (invisibly)
#[interpreter_builtin(namespace = "graphics", min_args = 1)]
fn interp_hist(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let ca = CallArgs::new(args, named);
    let data = extract_doubles(&args[0])?;

    if data.is_empty() {
        return Err(RError::new(
            RErrorKind::Argument,
            "'x' must have at least one non-NA value".to_string(),
        ));
    }

    // Determine breaks
    let breaks_val = ca.value("breaks", 1);
    let n_bins = match breaks_val {
        Some(v) => {
            // Could be a single integer or a vector of breakpoints
            if let Some(n) = v.as_vector().and_then(|vec| vec.as_integer_scalar()) {
                usize::try_from(n.max(1)).unwrap_or(10)
            } else {
                10
            }
        }
        None => 10,
    };

    // Compute bin edges using Sturges-like approach
    let min_val = data.iter().copied().fold(f64::INFINITY, f64::min);
    let max_val = data.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let range = max_val - min_val;
    let bin_width = if range == 0.0 {
        1.0
    } else {
        range / n_bins as f64
    };

    let mut break_points: Vec<f64> = (0..=n_bins)
        .map(|i| min_val + i as f64 * bin_width)
        .collect();
    // Adjust last break to include max_val
    if let Some(last) = break_points.last_mut() {
        *last = max_val + bin_width * 0.001;
    }

    // Count values in each bin
    let mut counts = vec![0usize; n_bins];
    for &val in &data {
        for (i, window) in break_points.windows(2).enumerate() {
            if val >= window[0] && val < window[1] {
                counts[i] += 1;
                break;
            }
        }
    }

    // Compute bar positions (midpoints)
    let mids: Vec<f64> = break_points
        .windows(2)
        .map(|w| (w[0] + w[1]) / 2.0)
        .collect();
    let heights: Vec<f64> = counts.iter().map(|&c| c as f64).collect();

    let color = ca
        .value("col", 2)
        .map(parse_color)
        .unwrap_or([173, 216, 230, 255]); // lightblue default
    let title = ca.optional_string("main", 3);
    let xlab = ca.optional_string("xlab", 4);

    let mut state = PlotState::new();
    state.title = title.or_else(|| Some("Histogram of x".to_string()));
    state.x_label = xlab;
    state.y_label = Some("Frequency".to_string());

    state.items.push(PlotItem::Bars {
        x: mids.clone(),
        heights: heights.clone(),
        color,
        width: bin_width * 0.9,
        label: None,
    });

    *context.interpreter().current_plot.borrow_mut() = Some(state);
    show_current_plot(context)?;

    // Return a list with breaks, counts, mids (like R's hist())
    let breaks_rv = RValue::vec(Vector::Double(
        break_points
            .iter()
            .map(|&v| Some(v))
            .collect::<Vec<_>>()
            .into(),
    ));
    let counts_rv = RValue::vec(Vector::Integer(
        counts
            .iter()
            .map(|&c| Some(i64::try_from(c).unwrap_or(0)))
            .collect::<Vec<_>>()
            .into(),
    ));
    let mids_rv = RValue::vec(Vector::Double(
        mids.iter().map(|&v| Some(v)).collect::<Vec<_>>().into(),
    ));

    let result = RList::new(vec![
        (Some("breaks".to_string()), breaks_rv),
        (Some("counts".to_string()), counts_rv),
        (Some("mids".to_string()), mids_rv),
    ]);
    Ok(RValue::List(result))
}

/// Create a bar plot from a numeric vector.
///
/// @param height numeric vector of bar heights
/// @param names.arg character vector of bar names
/// @param col bar color
/// @param main plot title
/// @return numeric vector of bar midpoints (invisibly)
#[interpreter_builtin(namespace = "graphics", min_args = 1)]
fn interp_barplot(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let ca = CallArgs::new(args, named);
    let heights = extract_doubles(&args[0])?;

    if heights.is_empty() {
        return Err(RError::new(
            RErrorKind::Argument,
            "'height' must have at least one value".to_string(),
        ));
    }

    let color = ca
        .value("col", 1)
        .map(parse_color)
        .unwrap_or([173, 216, 230, 255]); // lightblue
    let title = ca.optional_string("main", 2);

    // Bar positions: 1, 2, 3, ...
    let x_positions: Vec<f64> = (1..=heights.len()).map(|i| i as f64).collect();

    let mut state = PlotState::new();
    state.title = title;

    state.items.push(PlotItem::Bars {
        x: x_positions.clone(),
        heights,
        color,
        width: 0.8,
        label: None,
    });

    *context.interpreter().current_plot.borrow_mut() = Some(state);
    show_current_plot(context)?;

    // Return bar midpoints (like R)
    Ok(RValue::vec(Vector::Double(
        x_positions
            .iter()
            .map(|&v| Some(v))
            .collect::<Vec<_>>()
            .into(),
    )))
}

/// Create box-and-whisker plots.
///
/// Accepts one or more numeric vectors (as positional args) and computes
/// five-number summaries for each.
///
/// @param ... numeric vectors
/// @param col box color
/// @param main plot title
/// @return NULL (invisibly)
#[interpreter_builtin(namespace = "graphics", min_args = 1)]
fn interp_boxplot(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let ca = CallArgs::new(args, named);

    let color = ca
        .named("col")
        .map(parse_color)
        .unwrap_or([173, 216, 230, 255]);
    let title = ca.named_string("main");

    let mut positions = Vec::new();
    let mut spreads = Vec::new();

    for (i, arg) in args.iter().enumerate() {
        let mut data = extract_doubles(arg)?;
        if data.is_empty() {
            continue;
        }
        data.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let n = data.len();
        let median = percentile(&data, 50.0);
        let q1 = percentile(&data, 25.0);
        let q3 = percentile(&data, 75.0);
        let iqr = q3 - q1;
        let lower_fence = q1 - 1.5 * iqr;
        let upper_fence = q3 + 1.5 * iqr;
        let lower_whisker = data
            .iter()
            .copied()
            .find(|&v| v >= lower_fence)
            .unwrap_or(data[0]);
        let upper_whisker = data
            .iter()
            .rev()
            .copied()
            .find(|&v| v <= upper_fence)
            .unwrap_or(data[n - 1]);

        positions.push((i + 1) as f64);
        spreads.push(BoxSpread {
            lower_whisker,
            q1,
            median,
            q3,
            upper_whisker,
        });
    }

    let mut state = PlotState::new();
    state.title = title;

    state.items.push(PlotItem::BoxPlot {
        positions,
        spreads,
        color,
    });

    *context.interpreter().current_plot.borrow_mut() = Some(state);
    show_current_plot(context)?;

    Ok(RValue::Null)
}

/// Compute a percentile from sorted data using linear interpolation.
fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    if sorted.len() == 1 {
        return sorted[0];
    }
    let rank = p / 100.0 * (sorted.len() - 1) as f64;
    let lo = rank.floor() as usize;
    let hi = rank.ceil() as usize;
    let frac = rank - lo as f64;
    if lo == hi {
        sorted[lo]
    } else {
        sorted[lo] * (1.0 - frac) + sorted[hi] * frac
    }
}

// endregion

// region: Low-level plot additions

/// Add points to the current plot.
///
/// @param x numeric vector of x-coordinates
/// @param y numeric vector of y-coordinates
/// @param col color
/// @param pch point character
/// @param cex character expansion factor
/// @return NULL (invisibly)
#[interpreter_builtin(namespace = "graphics", min_args = 1)]
fn interp_points(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let ca = CallArgs::new(args, named);

    let first = &args[0];
    let second = ca.value("y", 1);

    let (x_data, y_data) = if let Some(y_val) = second {
        (extract_doubles(first)?, extract_doubles(y_val)?)
    } else {
        let y = extract_doubles(first)?;
        let x: Vec<f64> = (1..=y.len()).map(|i| i as f64).collect();
        (x, y)
    };

    let color = ca
        .value("col", 2)
        .map(parse_color)
        .unwrap_or([0, 0, 0, 255]);
    let pch = u8::try_from(ca.integer_or("pch", 3, 1)).unwrap_or(1);
    let cex = ca
        .value("cex", 4)
        .and_then(|v| v.as_vector()?.as_double_scalar())
        .unwrap_or(1.0);

    let mut plot = ensure_plot(context);
    if let Some(ref mut state) = *plot {
        state.items.push(PlotItem::Points {
            x: x_data,
            y: y_data,
            color,
            size: (3.0 * cex) as f32,
            shape: pch,
            label: None,
        });
    }

    Ok(RValue::Null)
}

/// Add connected line segments to the current plot.
///
/// @param x numeric vector of x-coordinates
/// @param y numeric vector of y-coordinates
/// @param col color
/// @param lwd line width
/// @return NULL (invisibly)
#[interpreter_builtin(namespace = "graphics", min_args = 1)]
fn interp_lines(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let ca = CallArgs::new(args, named);

    let first = &args[0];
    let second = ca.value("y", 1);

    let (x_data, y_data) = if let Some(y_val) = second {
        (extract_doubles(first)?, extract_doubles(y_val)?)
    } else {
        let y = extract_doubles(first)?;
        let x: Vec<f64> = (1..=y.len()).map(|i| i as f64).collect();
        (x, y)
    };

    let color = ca
        .value("col", 2)
        .map(parse_color)
        .unwrap_or([0, 0, 0, 255]);
    let lwd = ca
        .value("lwd", 3)
        .and_then(|v| v.as_vector()?.as_double_scalar())
        .unwrap_or(1.0);

    let mut plot = ensure_plot(context);
    if let Some(ref mut state) = *plot {
        state.items.push(PlotItem::Line {
            x: x_data,
            y: y_data,
            color,
            width: lwd as f32,
            label: None,
        });
    }

    Ok(RValue::Null)
}

/// Add horizontal, vertical, or slope-intercept lines to the current plot.
///
/// @param a intercept (for slope-intercept form)
/// @param b slope (for slope-intercept form)
/// @param h y-value for horizontal line
/// @param v x-value for vertical line
/// @param col color
/// @param lwd line width
/// @return NULL (invisibly)
#[interpreter_builtin(namespace = "graphics")]
fn interp_abline(
    _args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let ca = CallArgs::new(&[], named);

    let color = ca.named("col").map(parse_color).unwrap_or([0, 0, 0, 255]);
    let lwd = ca
        .named("lwd")
        .and_then(|v| v.as_vector()?.as_double_scalar())
        .unwrap_or(1.0);

    let mut plot = ensure_plot(context);
    if let Some(ref mut state) = *plot {
        // Horizontal line(s)
        if let Some(h_vals) = try_extract_doubles(ca.named("h"))? {
            for h in h_vals {
                state.items.push(PlotItem::HLine {
                    y: h,
                    color,
                    width: lwd as f32,
                });
            }
        }

        // Vertical line(s)
        if let Some(v_vals) = try_extract_doubles(ca.named("v"))? {
            for v in v_vals {
                state.items.push(PlotItem::VLine {
                    x: v,
                    color,
                    width: lwd as f32,
                });
            }
        }

        // Slope-intercept: a + b*x — represented as a line through
        // the visible range, which we approximate with a wide range
        let a_val = ca
            .named("a")
            .and_then(|v| v.as_vector()?.as_double_scalar());
        let b_val = ca
            .named("b")
            .and_then(|v| v.as_vector()?.as_double_scalar());
        if let (Some(a), Some(b)) = (a_val, b_val) {
            let x_lo = -1e6_f64;
            let x_hi = 1e6_f64;
            state.items.push(PlotItem::Line {
                x: vec![x_lo, x_hi],
                y: vec![a + b * x_lo, a + b * x_hi],
                color,
                width: lwd as f32,
                label: None,
            });
        }
    }

    Ok(RValue::Null)
}

/// Add a legend to the current plot.
///
/// Enables the egui_plot legend display. In the MVP the legend labels
/// come from the PlotItem label fields.
///
/// @return NULL (invisibly)
#[interpreter_builtin(namespace = "graphics")]
fn interp_legend(
    _args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let mut plot = ensure_plot(context);
    if let Some(ref mut state) = *plot {
        state.show_legend = true;
    }
    Ok(RValue::Null)
}

/// Set or update plot titles.
///
/// @param main main title
/// @param sub subtitle (ignored for now)
/// @param xlab x-axis label
/// @param ylab y-axis label
/// @return NULL (invisibly)
#[interpreter_builtin(namespace = "graphics")]
fn interp_title(
    _args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let ca = CallArgs::new(&[], named);

    let mut plot = ensure_plot(context);
    if let Some(ref mut state) = *plot {
        if let Some(main) = ca.named_string("main") {
            state.title = Some(main);
        }
        if let Some(xlab) = ca.named_string("xlab") {
            state.x_label = Some(xlab);
        }
        if let Some(ylab) = ca.named_string("ylab") {
            state.y_label = Some(ylab);
        }
    }

    Ok(RValue::Null)
}

/// Add an axis to the current plot.
///
/// Axes are handled automatically by egui_plot, so this is a no-op
/// that exists for compatibility with R code that calls `axis()`.
///
/// @param side which side (1=bottom, 2=left, 3=top, 4=right)
/// @return NULL (invisibly)
#[builtin(namespace = "graphics")]
fn builtin_axis(_args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    // egui_plot handles axes automatically
    Ok(RValue::Null)
}

// endregion

// region: Graphics parameters

// par() is implemented in graphics/par.rs

// endregion
