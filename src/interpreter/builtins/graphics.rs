//! Graphics device stubs — placeholder builtins so that R code calling
//! plotting functions does not crash. Real graphics output is not yet
//! supported; these stubs print an informative message or silently return
//! NULL depending on the function.

use crate::interpreter::value::*;
use crate::interpreter::BuiltinContext;
use minir_macros::{builtin, interpreter_builtin};

const DEVICE_MSG: &str = "graphics devices are not yet supported in miniR\n";

// region: Device management

/// Open a PDF graphics device.
///
/// Graphics devices are not yet implemented — this stub prints a message
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
/// Graphics devices are not yet implemented — this stub prints a message
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
/// Graphics devices are not yet implemented — this stub prints a message
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
/// Since no real device is open, this returns invisible integer 1
/// (the null device number), matching R's convention.
///
/// @return integer 1 (invisibly)
#[builtin(name = "dev.off", namespace = "grDevices")]
fn builtin_dev_off(_args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    Ok(RValue::vec(Vector::Integer(vec![Some(1i64)].into())))
}

/// Return the current graphics device number.
///
/// Always returns 1 (the null device) since no real graphics devices
/// are supported yet.
///
/// @return integer 1
#[builtin(name = "dev.cur", namespace = "grDevices")]
fn builtin_dev_cur(_args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    Ok(RValue::vec(Vector::Integer(vec![Some(1i64)].into())))
}

/// Open a new graphics device.
///
/// Graphics devices are not yet implemented — this stub prints a message
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

// region: High-level plotting

/// Create a scatter plot or other high-level plot.
///
/// Graphics output is not yet supported — this stub prints a message
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

// region: Low-level drawing (silently return NULL)

/// Add points to the current plot.
///
/// Silently returns NULL because graphics output is not yet supported.
///
/// @param x x-coordinates
/// @param y y-coordinates
/// @return NULL (invisibly)
#[builtin(namespace = "graphics")]
fn builtin_points(args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    let _x = args.first();
    let _y = args.get(1);
    Ok(RValue::Null)
}

/// Add connected line segments to the current plot.
///
/// Silently returns NULL because graphics output is not yet supported.
///
/// @param x x-coordinates
/// @param y y-coordinates
/// @return NULL (invisibly)
#[builtin(namespace = "graphics")]
fn builtin_lines(args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    let _x = args.first();
    let _y = args.get(1);
    Ok(RValue::Null)
}

/// Add straight lines (horizontal, vertical, or slope-intercept) to a plot.
///
/// Silently returns NULL because graphics output is not yet supported.
///
/// @return NULL (invisibly)
#[builtin(namespace = "graphics")]
fn builtin_abline(_args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    Ok(RValue::Null)
}

/// Add a legend to the current plot.
///
/// Silently returns NULL because graphics output is not yet supported.
///
/// @return NULL (invisibly)
#[builtin(namespace = "graphics")]
fn builtin_legend(_args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    Ok(RValue::Null)
}

/// Add a title or subtitle to the current plot.
///
/// Silently returns NULL because graphics output is not yet supported.
///
/// @param main main title text
/// @return NULL (invisibly)
#[builtin(namespace = "graphics")]
fn builtin_title(args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    let _main = args.first();
    Ok(RValue::Null)
}

/// Add an axis to the current plot.
///
/// Silently returns NULL because graphics output is not yet supported.
///
/// @param side which side of the plot (1=bottom, 2=left, 3=top, 4=right)
/// @return NULL (invisibly)
#[builtin(namespace = "graphics")]
fn builtin_axis(args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    let _side = args.first();
    Ok(RValue::Null)
}

// endregion

// region: Graphics parameters

/// Query or set graphical parameters.
///
/// Since graphics are not supported yet, this returns an empty list so
/// that code like `old <- par(mfrow = c(1,2))` does not crash.
///
/// @return an empty list
#[builtin(namespace = "graphics")]
fn builtin_par(_args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    Ok(RValue::List(RList::new(vec![])))
}

// endregion
