//! Graphics device management and drawing primitive builtins.
//!
//! Implements R's two-tier graphics system:
//! - **Device management**: `pdf()`, `png()`, `svg()`, `dev.off()`, `dev.cur()`, `dev.new()`
//! - **Drawing primitives**: `plot.new()`, `plot.window()`, `points()`, `lines()`,
//!   `segments()`, `rect()`, `polygon()`, `text()`, `abline()`, `title()`, `axis()`
//! - **Graphics parameters**: `par()`
//!
//! All drawing primitives dispatch to the current device via `DeviceManager`.
//! With only `NullDevice` available (Phase 1), the primitives validate their
//! arguments and issue device calls that are silently discarded.

use super::CallArgs;
use crate::interpreter::graphics::coord::{pretty_ticks, CoordTransform};
use crate::interpreter::graphics::GraphicsContext;
use crate::interpreter::value::*;
use crate::interpreter::BuiltinContext;
use minir_macros::{builtin, interpreter_builtin};

const DEVICE_MSG: &str = "graphics devices are not yet supported in miniR\n";

// region: Argument extraction helpers

/// Extract a vector of f64 values from an RValue, returning an empty vec for NULL.
fn extract_doubles(val: &RValue) -> Vec<f64> {
    match val {
        RValue::Null => vec![],
        RValue::Vector(rv) => rv.to_doubles().into_iter().flatten().collect(),
        _ => vec![],
    }
}

/// Extract a scalar f64 from an RValue, returning a default if NULL or missing.
fn extract_double_or(val: Option<&RValue>, default: f64) -> f64 {
    val.and_then(|v| v.as_vector()?.as_double_scalar())
        .unwrap_or(default)
}

/// Extract a scalar string from an RValue, returning a default if NULL or missing.
fn extract_string_or<'a>(val: Option<&'a RValue>, default: &'a str) -> String {
    val.and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_else(|| default.to_string())
}

/// Extract a scalar integer from an RValue, returning a default if NULL or missing.
fn extract_int_or(val: Option<&RValue>, default: i64) -> i64 {
    val.and_then(|v| v.as_vector()?.as_integer_scalar())
        .unwrap_or(default)
}

/// Build a GraphicsContext from named arguments, falling back to defaults.
fn build_gc(args: &CallArgs<'_>) -> GraphicsContext {
    let mut gc = GraphicsContext::default();
    if let Some(col) = args
        .named("col")
        .and_then(|v| v.as_vector()?.as_character_scalar())
    {
        gc.col = col;
    }
    if let Some(fill) = args
        .named("bg")
        .and_then(|v| v.as_vector()?.as_character_scalar())
    {
        gc.fill = fill;
    }
    if let Some(lwd) = args
        .named("lwd")
        .and_then(|v| v.as_vector()?.as_double_scalar())
    {
        gc.lwd = lwd;
    }
    if let Some(lty) = args
        .named("lty")
        .and_then(|v| v.as_vector()?.as_integer_scalar())
    {
        gc.lty = i32::try_from(lty).unwrap_or(1);
    }
    if let Some(cex) = args
        .named("cex")
        .and_then(|v| v.as_vector()?.as_double_scalar())
    {
        gc.cex = cex;
    }
    if let Some(pch) = args
        .named("pch")
        .and_then(|v| v.as_vector()?.as_integer_scalar())
    {
        gc.pch = i32::try_from(pch).unwrap_or(1);
    }
    gc
}

/// Build a CoordTransform from the current plot state and device dimensions.
fn build_coord_transform(usr: &[f64; 4], dev_width: f64, dev_height: f64) -> CoordTransform {
    // Simple margin: 10% on each side
    let margin_x = dev_width * 0.1;
    let margin_y = dev_height * 0.1;
    CoordTransform::new(
        *usr,
        [
            margin_x,
            dev_width - margin_x,
            margin_y,
            dev_height - margin_y,
        ],
    )
}

// endregion

// region: Device management

/// Open a PDF graphics device.
///
/// Graphics devices are not yet implemented -- this stub prints a message
/// and returns NULL so that scripts can continue.
///
/// @param file output file path (ignored)
/// @return NULL
#[interpreter_builtin(namespace = "grDevices")]
fn interp_pdf(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let _file = args.first();
    context.write_err(DEVICE_MSG);
    Ok(RValue::Null)
}

/// Open a PNG graphics device.
///
/// Graphics devices are not yet implemented -- this stub prints a message
/// and returns NULL so that scripts can continue.
///
/// @param filename output file path (ignored)
/// @return NULL
#[interpreter_builtin(namespace = "grDevices")]
fn interp_png(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let _filename = args.first();
    context.write_err(DEVICE_MSG);
    Ok(RValue::Null)
}

/// Open an SVG graphics device.
///
/// Graphics devices are not yet implemented -- this stub prints a message
/// and returns NULL so that scripts can continue.
///
/// @param filename output file path (ignored)
/// @return NULL
#[interpreter_builtin(namespace = "grDevices")]
fn interp_svg(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let _filename = args.first();
    context.write_err(DEVICE_MSG);
    Ok(RValue::Null)
}

/// Close the current graphics device.
///
/// Closes the current device and reverts to the previous one. Returns
/// the new current device number (invisibly).
///
/// @return integer device number (invisibly)
#[interpreter_builtin(name = "dev.off", namespace = "grDevices")]
fn interp_dev_off(
    _args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let new_dev = context
        .interpreter()
        .device_manager
        .borrow_mut()
        .close_current();
    Ok(RValue::vec(Vector::Integer(vec![Some(new_dev)].into())))
}

/// Return the current graphics device number.
///
/// Returns the 1-based device number of the current device.
/// Device 1 is the null device.
///
/// @return integer device number
#[interpreter_builtin(name = "dev.cur", namespace = "grDevices")]
fn interp_dev_cur(
    _args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let num = context
        .interpreter()
        .device_manager
        .borrow()
        .current_device_number();
    Ok(RValue::vec(Vector::Integer(vec![Some(num)].into())))
}

/// Open a new graphics device.
///
/// Graphics devices are not yet implemented -- this stub prints a message
/// and returns NULL.
///
/// @return NULL
#[interpreter_builtin(name = "dev.new", namespace = "grDevices")]
fn interp_dev_new(
    _args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    context.write_err(DEVICE_MSG);
    Ok(RValue::Null)
}

// endregion

// region: Plot setup

/// Initialize a new plot on the current device.
///
/// Calls device.new_page(), resets the plot state, and prepares for
/// subsequent plot.window() / drawing calls.
///
/// @return NULL (invisibly)
#[interpreter_builtin(name = "plot.new", namespace = "graphics")]
fn interp_plot_new(
    _args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let interp = context.interpreter();
    let mut dm = interp.device_manager.borrow_mut();
    let gc = GraphicsContext::default();
    dm.current_device().new_page(&gc);
    dm.plot_state.has_plot = true;
    dm.plot_state.usr = [0.0, 1.0, 0.0, 1.0];
    Ok(RValue::Null)
}

/// Set the user-coordinate window for the current plot.
///
/// Defines the mapping from data coordinates to device coordinates.
/// Must be called after plot.new() and before drawing primitives.
///
/// @param xlim numeric vector of length 2: c(xmin, xmax)
/// @param ylim numeric vector of length 2: c(ymin, ymax)
/// @param log character string indicating log-scale axes ("", "x", "y", "xy")
/// @return NULL (invisibly)
#[interpreter_builtin(name = "plot.window", namespace = "graphics", min_args = 2)]
fn interp_plot_window(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let xlim_val = call_args.value("xlim", 0).unwrap_or(&RValue::Null);
    let ylim_val = call_args.value("ylim", 1).unwrap_or(&RValue::Null);

    let xlim = extract_doubles(xlim_val);
    let ylim = extract_doubles(ylim_val);

    if xlim.len() < 2 {
        return Err(RError::new(
            RErrorKind::Argument,
            "plot.window() requires xlim of length 2".to_string(),
        ));
    }
    if ylim.len() < 2 {
        return Err(RError::new(
            RErrorKind::Argument,
            "plot.window() requires ylim of length 2".to_string(),
        ));
    }

    let interp = context.interpreter();
    let mut dm = interp.device_manager.borrow_mut();
    dm.plot_state.usr = [xlim[0], xlim[1], ylim[0], ylim[1]];

    Ok(RValue::Null)
}

// endregion

// region: High-level plotting

/// Create a scatter plot or other high-level plot.
///
/// Graphics output is not yet supported -- this stub prints a message
/// and returns NULL so that scripts can continue.
///
/// @param x x-coordinates or a formula
/// @param y y-coordinates (optional)
/// @return NULL
#[interpreter_builtin(namespace = "graphics")]
fn interp_plot(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let _x = args.first();
    let _y = args.get(1);
    context.write_err(DEVICE_MSG);
    Ok(RValue::Null)
}

// endregion

// region: Drawing primitives

/// Add points to the current plot.
///
/// For each (x, y) pair, dispatches to the current device's point() method.
/// Points are drawn using the current graphics parameters (pch, col, cex).
///
/// @param x numeric vector of x-coordinates
/// @param y numeric vector of y-coordinates
/// @param pch integer point character (default 1 = open circle)
/// @param col character colour (default "black")
/// @param cex numeric character expansion factor (default 1)
/// @return NULL (invisibly)
#[interpreter_builtin(namespace = "graphics", min_args = 1)]
fn interp_points(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let x_val = call_args.value("x", 0).unwrap_or(&RValue::Null);
    let y_val = call_args.value("y", 1);

    let x = extract_doubles(x_val);
    let y = match y_val {
        Some(v) => extract_doubles(v),
        None => (0..x.len()).map(|i| (i + 1) as f64).collect(),
    };
    let gc = build_gc(&call_args);

    let interp = context.interpreter();
    let mut dm = interp.device_manager.borrow_mut();
    let usr = dm.plot_state.usr;
    let dev = dm.current_device();
    let ct = build_coord_transform(&usr, dev.width(), dev.height());

    let n = x.len().min(y.len());
    for i in 0..n {
        let dx = ct.usr_to_dev_x(x[i]);
        let dy = ct.usr_to_dev_y(y[i]);
        dev.point(dx, dy, &gc);
    }

    context.interpreter().set_invisible();
    Ok(RValue::Null)
}

/// Add connected line segments to the current plot.
///
/// Draws a polyline through the given (x, y) points using the current
/// device's polyline() method.
///
/// @param x numeric vector of x-coordinates
/// @param y numeric vector of y-coordinates
/// @param col character colour (default "black")
/// @param lwd numeric line width (default 1)
/// @param lty integer line type (default 1 = solid)
/// @return NULL (invisibly)
#[interpreter_builtin(namespace = "graphics", min_args = 1)]
fn interp_lines(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let x_val = call_args.value("x", 0).unwrap_or(&RValue::Null);
    let y_val = call_args.value("y", 1);

    let x = extract_doubles(x_val);
    let y = match y_val {
        Some(v) => extract_doubles(v),
        None => (0..x.len()).map(|i| (i + 1) as f64).collect(),
    };
    let gc = build_gc(&call_args);

    let interp = context.interpreter();
    let mut dm = interp.device_manager.borrow_mut();
    let usr = dm.plot_state.usr;
    let dev = dm.current_device();
    let ct = build_coord_transform(&usr, dev.width(), dev.height());

    let n = x.len().min(y.len());
    let dev_x: Vec<f64> = (0..n).map(|i| ct.usr_to_dev_x(x[i])).collect();
    let dev_y: Vec<f64> = (0..n).map(|i| ct.usr_to_dev_y(y[i])).collect();

    dev.polyline(&dev_x, &dev_y, &gc);

    context.interpreter().set_invisible();
    Ok(RValue::Null)
}

/// Draw individual line segments.
///
/// For each i, draws a line from (x0[i], y0[i]) to (x1[i], y1[i]).
///
/// @param x0 numeric vector of start x-coordinates
/// @param y0 numeric vector of start y-coordinates
/// @param x1 numeric vector of end x-coordinates
/// @param y1 numeric vector of end y-coordinates
/// @return NULL (invisibly)
#[interpreter_builtin(namespace = "graphics", min_args = 4)]
fn interp_segments(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let x0 = extract_doubles(call_args.value("x0", 0).unwrap_or(&RValue::Null));
    let y0 = extract_doubles(call_args.value("y0", 1).unwrap_or(&RValue::Null));
    let x1 = extract_doubles(call_args.value("x1", 2).unwrap_or(&RValue::Null));
    let y1 = extract_doubles(call_args.value("y1", 3).unwrap_or(&RValue::Null));
    let gc = build_gc(&call_args);

    let interp = context.interpreter();
    let mut dm = interp.device_manager.borrow_mut();
    let usr = dm.plot_state.usr;
    let dev = dm.current_device();
    let ct = build_coord_transform(&usr, dev.width(), dev.height());

    let n = x0.len().min(y0.len()).min(x1.len()).min(y1.len());
    for i in 0..n {
        dev.line(
            ct.usr_to_dev_x(x0[i]),
            ct.usr_to_dev_y(y0[i]),
            ct.usr_to_dev_x(x1[i]),
            ct.usr_to_dev_y(y1[i]),
            &gc,
        );
    }

    context.interpreter().set_invisible();
    Ok(RValue::Null)
}

/// Draw rectangles.
///
/// For each i, draws a rectangle from (xleft[i], ybottom[i]) to
/// (xright[i], ytop[i]).
///
/// @param xleft numeric vector of left x-coordinates
/// @param ybottom numeric vector of bottom y-coordinates
/// @param xright numeric vector of right x-coordinates
/// @param ytop numeric vector of top y-coordinates
/// @param col fill colour (default NA = no fill)
/// @param border border colour (default "black")
/// @return NULL (invisibly)
#[interpreter_builtin(namespace = "graphics", min_args = 4)]
fn interp_rect(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let xleft = extract_doubles(call_args.value("xleft", 0).unwrap_or(&RValue::Null));
    let ybottom = extract_doubles(call_args.value("ybottom", 1).unwrap_or(&RValue::Null));
    let xright = extract_doubles(call_args.value("xright", 2).unwrap_or(&RValue::Null));
    let ytop = extract_doubles(call_args.value("ytop", 3).unwrap_or(&RValue::Null));
    let mut gc = build_gc(&call_args);

    // Handle fill colour from `col` argument (rect uses col for fill, border for stroke)
    if let Some(col) = call_args
        .named("col")
        .and_then(|v| v.as_vector()?.as_character_scalar())
    {
        gc.fill = col;
    }
    if let Some(border) = call_args
        .named("border")
        .and_then(|v| v.as_vector()?.as_character_scalar())
    {
        gc.col = border;
    }

    let interp = context.interpreter();
    let mut dm = interp.device_manager.borrow_mut();
    let usr = dm.plot_state.usr;
    let dev = dm.current_device();
    let ct = build_coord_transform(&usr, dev.width(), dev.height());

    let n = xleft
        .len()
        .min(ybottom.len())
        .min(xright.len())
        .min(ytop.len());
    for i in 0..n {
        dev.rect(
            ct.usr_to_dev_x(xleft[i]),
            ct.usr_to_dev_y(ybottom[i]),
            ct.usr_to_dev_x(xright[i]),
            ct.usr_to_dev_y(ytop[i]),
            &gc,
        );
    }

    context.interpreter().set_invisible();
    Ok(RValue::Null)
}

/// Draw a filled/stroked polygon.
///
/// Draws a polygon through the given (x, y) vertices.
///
/// @param x numeric vector of x-coordinates
/// @param y numeric vector of y-coordinates
/// @param col fill colour (default NA = no fill)
/// @param border border colour (default "black")
/// @return NULL (invisibly)
#[interpreter_builtin(namespace = "graphics", min_args = 2)]
fn interp_polygon(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let x = extract_doubles(call_args.value("x", 0).unwrap_or(&RValue::Null));
    let y = extract_doubles(call_args.value("y", 1).unwrap_or(&RValue::Null));
    let mut gc = build_gc(&call_args);

    // polygon uses col for fill, border for stroke
    if let Some(col) = call_args
        .named("col")
        .and_then(|v| v.as_vector()?.as_character_scalar())
    {
        gc.fill = col;
    }
    if let Some(border) = call_args
        .named("border")
        .and_then(|v| v.as_vector()?.as_character_scalar())
    {
        gc.col = border;
    }

    let interp = context.interpreter();
    let mut dm = interp.device_manager.borrow_mut();
    let usr = dm.plot_state.usr;
    let dev = dm.current_device();
    let ct = build_coord_transform(&usr, dev.width(), dev.height());

    let n = x.len().min(y.len());
    let dev_x: Vec<f64> = (0..n).map(|i| ct.usr_to_dev_x(x[i])).collect();
    let dev_y: Vec<f64> = (0..n).map(|i| ct.usr_to_dev_y(y[i])).collect();

    dev.polygon(&dev_x, &dev_y, &gc);

    context.interpreter().set_invisible();
    Ok(RValue::Null)
}

/// Add text labels to the current plot.
///
/// Draws text at each (x, y) position using the corresponding label.
///
/// @param x numeric vector of x-coordinates
/// @param y numeric vector of y-coordinates
/// @param labels character vector of text labels
/// @param adj numeric horizontal adjustment (0=left, 0.5=center, 1=right)
/// @param cex numeric character expansion factor (default 1)
/// @param col character colour (default "black")
/// @return NULL (invisibly)
#[interpreter_builtin(namespace = "graphics", min_args = 2)]
fn interp_text(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let x = extract_doubles(call_args.value("x", 0).unwrap_or(&RValue::Null));
    let y = extract_doubles(call_args.value("y", 1).unwrap_or(&RValue::Null));

    // Extract labels — can be positional arg 2 or named "labels"
    let labels: Vec<String> = match call_args.value("labels", 2) {
        Some(RValue::Vector(rv)) => match &rv.inner {
            Vector::Character(cv) => cv.iter().filter_map(|s| s.clone()).collect(),
            other => other
                .to_doubles()
                .iter()
                .map(|d| match d {
                    Some(v) => format!("{v}"),
                    None => "NA".to_string(),
                })
                .collect(),
        },
        _ => (1..=x.len()).map(|i| i.to_string()).collect(),
    };

    let adj = extract_double_or(call_args.named("adj"), 0.5);
    let gc = build_gc(&call_args);

    let interp = context.interpreter();
    let mut dm = interp.device_manager.borrow_mut();
    let usr = dm.plot_state.usr;
    let dev = dm.current_device();
    let ct = build_coord_transform(&usr, dev.width(), dev.height());

    let n = x.len().min(y.len());
    for i in 0..n {
        let label = labels
            .get(i % labels.len().max(1))
            .map_or("", |s| s.as_str());
        dev.text(
            ct.usr_to_dev_x(x[i]),
            ct.usr_to_dev_y(y[i]),
            label,
            adj,
            &gc,
        );
    }

    context.interpreter().set_invisible();
    Ok(RValue::Null)
}

/// Add straight lines to a plot.
///
/// Supports three modes:
/// - Slope-intercept: `abline(a, b)` draws y = a + b*x across the plot
/// - Horizontal: `abline(h = y)` draws horizontal lines at the given y values
/// - Vertical: `abline(v = x)` draws vertical lines at the given x values
///
/// @param a numeric intercept
/// @param b numeric slope
/// @param h numeric y-values for horizontal lines
/// @param v numeric x-values for vertical lines
/// @return NULL (invisibly)
#[interpreter_builtin(namespace = "graphics")]
fn interp_abline(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let gc = build_gc(&call_args);

    let interp = context.interpreter();
    let mut dm = interp.device_manager.borrow_mut();
    let usr = dm.plot_state.usr;
    let dev = dm.current_device();
    let ct = build_coord_transform(&usr, dev.width(), dev.height());

    // Check for h= (horizontal lines)
    if let Some(h_val) = call_args.named("h") {
        let h_vals = extract_doubles(h_val);
        for &h in &h_vals {
            dev.line(
                ct.usr_to_dev_x(usr[0]),
                ct.usr_to_dev_y(h),
                ct.usr_to_dev_x(usr[1]),
                ct.usr_to_dev_y(h),
                &gc,
            );
        }
    }

    // Check for v= (vertical lines)
    if let Some(v_val) = call_args.named("v") {
        let v_vals = extract_doubles(v_val);
        for &v in &v_vals {
            dev.line(
                ct.usr_to_dev_x(v),
                ct.usr_to_dev_y(usr[2]),
                ct.usr_to_dev_x(v),
                ct.usr_to_dev_y(usr[3]),
                &gc,
            );
        }
    }

    // Check for a=, b= (slope-intercept)
    let a_val = call_args.value("a", 0);
    let b_val = call_args.value("b", 1);
    if let (Some(a_rv), Some(b_rv)) = (a_val, b_val) {
        let a = extract_double_or(Some(a_rv), 0.0);
        let b = extract_double_or(Some(b_rv), 0.0);
        // Line: y = a + b * x, clip to usr range
        let x0 = usr[0];
        let x1 = usr[1];
        let y0 = a + b * x0;
        let y1 = a + b * x1;
        dev.line(
            ct.usr_to_dev_x(x0),
            ct.usr_to_dev_y(y0),
            ct.usr_to_dev_x(x1),
            ct.usr_to_dev_y(y1),
            &gc,
        );
    }

    context.interpreter().set_invisible();
    Ok(RValue::Null)
}

/// Add a title or subtitle to the current plot.
///
/// Draws title text at standard positions around the plot region.
///
/// @param main character main title (top, centered)
/// @param sub character subtitle (bottom, centered)
/// @param xlab character x-axis label (bottom, centered)
/// @param ylab character y-axis label (left, rotated)
/// @return NULL (invisibly)
#[interpreter_builtin(namespace = "graphics")]
fn interp_title(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let gc = build_gc(&call_args);

    let main = extract_string_or(call_args.value("main", 0), "");
    let sub = extract_string_or(call_args.named("sub"), "");
    let xlab = extract_string_or(call_args.named("xlab"), "");
    let ylab = extract_string_or(call_args.named("ylab"), "");

    let interp = context.interpreter();
    let mut dm = interp.device_manager.borrow_mut();
    let dev = dm.current_device();
    let w = dev.width();
    let h = dev.height();

    // Title positions in device coordinates (approximate)
    if !main.is_empty() {
        // Main title: centered at top, just above plot area
        dev.text(w / 2.0, h * 0.05, &main, 0.5, &gc);
    }
    if !sub.is_empty() {
        // Subtitle: centered at bottom, below x-axis label
        dev.text(w / 2.0, h * 0.98, &sub, 0.5, &gc);
    }
    if !xlab.is_empty() {
        // X-axis label: centered at bottom, between axis and subtitle
        dev.text(w / 2.0, h * 0.94, &xlab, 0.5, &gc);
    }
    if !ylab.is_empty() {
        // Y-axis label: centered on left, rotated (rendered as normal text for now)
        dev.text(w * 0.02, h / 2.0, &ylab, 0.5, &gc);
    }

    context.interpreter().set_invisible();
    Ok(RValue::Null)
}

/// Draw an axis on the current plot.
///
/// Draws axis line, tick marks, and labels on the specified side of the plot.
/// Uses `pretty_ticks()` to compute default tick positions when `at` is not provided.
///
/// @param side integer: 1=bottom, 2=left, 3=top, 4=right
/// @param at numeric vector of tick positions (default: auto-computed)
/// @param labels logical or character vector of tick labels (default TRUE = use at values)
/// @return NULL (invisibly)
#[interpreter_builtin(namespace = "graphics", min_args = 1)]
fn interp_axis(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let side = extract_int_or(call_args.value("side", 0), 1);

    if !(1..=4).contains(&side) {
        return Err(RError::new(
            RErrorKind::Argument,
            format!("axis() side must be 1, 2, 3, or 4, got {side}"),
        ));
    }

    let gc = build_gc(&call_args);

    let interp = context.interpreter();
    let mut dm = interp.device_manager.borrow_mut();
    let usr = dm.plot_state.usr;
    let dev = dm.current_device();
    let ct = build_coord_transform(&usr, dev.width(), dev.height());

    // Compute tick positions
    let at: Vec<f64> = match call_args.value("at", 1) {
        Some(v) if !v.is_null() => extract_doubles(v),
        _ => {
            // Auto-compute tick positions using pretty_ticks
            match side {
                1 | 3 => pretty_ticks(usr[0], usr[1], 5),
                2 | 4 => pretty_ticks(usr[2], usr[3], 5),
                _ => vec![],
            }
        }
    };

    // Determine labels
    let label_strings: Vec<String> = match call_args.value("labels", 2) {
        Some(RValue::Vector(rv)) => match &rv.inner {
            Vector::Character(cv) => cv.iter().filter_map(|s| s.clone()).collect(),
            Vector::Logical(lv) => {
                // If labels=TRUE (default), use stringified tick positions
                if lv.first().copied().flatten().unwrap_or(true) {
                    at.iter().map(|v| format!("{v}")).collect()
                } else {
                    vec![]
                }
            }
            _ => at.iter().map(|v| format!("{v}")).collect(),
        },
        _ => at.iter().map(|v| format!("{v}")).collect(),
    };

    let tick_len = 5.0; // tick length in device units

    for (i, &tick_pos) in at.iter().enumerate() {
        let label = label_strings.get(i).map_or("", |s| s.as_str());

        match side {
            1 => {
                // Bottom axis
                let dx = ct.usr_to_dev_x(tick_pos);
                let dy = ct.usr_to_dev_y(usr[2]);
                dev.line(dx, dy, dx, dy + tick_len, &gc);
                if !label.is_empty() {
                    dev.text(dx, dy + tick_len + 2.0, label, 0.5, &gc);
                }
            }
            2 => {
                // Left axis
                let dx = ct.usr_to_dev_x(usr[0]);
                let dy = ct.usr_to_dev_y(tick_pos);
                dev.line(dx, dy, dx - tick_len, dy, &gc);
                if !label.is_empty() {
                    dev.text(dx - tick_len - 2.0, dy, label, 1.0, &gc);
                }
            }
            3 => {
                // Top axis
                let dx = ct.usr_to_dev_x(tick_pos);
                let dy = ct.usr_to_dev_y(usr[3]);
                dev.line(dx, dy, dx, dy - tick_len, &gc);
                if !label.is_empty() {
                    dev.text(dx, dy - tick_len - 2.0, label, 0.5, &gc);
                }
            }
            4 => {
                // Right axis
                let dx = ct.usr_to_dev_x(usr[1]);
                let dy = ct.usr_to_dev_y(tick_pos);
                dev.line(dx, dy, dx + tick_len, dy, &gc);
                if !label.is_empty() {
                    dev.text(dx + tick_len + 2.0, dy, label, 0.0, &gc);
                }
            }
            _ => unreachable!("side validated above"),
        }
    }

    // Draw the axis line
    match side {
        1 => {
            let y = ct.usr_to_dev_y(usr[2]);
            dev.line(ct.usr_to_dev_x(usr[0]), y, ct.usr_to_dev_x(usr[1]), y, &gc);
        }
        2 => {
            let x = ct.usr_to_dev_x(usr[0]);
            dev.line(x, ct.usr_to_dev_y(usr[2]), x, ct.usr_to_dev_y(usr[3]), &gc);
        }
        3 => {
            let y = ct.usr_to_dev_y(usr[3]);
            dev.line(ct.usr_to_dev_x(usr[0]), y, ct.usr_to_dev_x(usr[1]), y, &gc);
        }
        4 => {
            let x = ct.usr_to_dev_x(usr[1]);
            dev.line(x, ct.usr_to_dev_y(usr[2]), x, ct.usr_to_dev_y(usr[3]), &gc);
        }
        _ => unreachable!("side validated above"),
    }

    context.interpreter().set_invisible();
    Ok(RValue::Null)
}

/// Add a legend to the current plot.
///
/// Silently returns NULL because full legend layout is not yet supported.
///
/// @return NULL (invisibly)
#[builtin(namespace = "graphics")]
fn builtin_legend(_args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    Ok(RValue::Null)
}

// endregion

// region: Graphics parameters

/// Query or set graphical parameters.
///
/// Since graphics are not fully supported yet, this returns an empty list so
/// that code like `old <- par(mfrow = c(1,2))` does not crash.
///
/// @return an empty list
#[builtin(namespace = "graphics")]
fn builtin_par(_args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    Ok(RValue::List(RList::new(vec![])))
}

// endregion
