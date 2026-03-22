//! Graphics device builtins — device management and plotting stubs.
//!
//! Device management builtins (`dev.cur`, `dev.set`, `dev.off`, `dev.list`,
//! `dev.new`, `graphics.off`) are backed by the `DeviceManager` on the
//! interpreter. High-level plotting and low-level drawing builtins remain
//! stubs that silently return NULL until real graphics backends are added.

use super::CallArgs;
use crate::interpreter::graphics::NullDevice;
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

/// Close a graphics device.
///
/// Closes the device specified by `which` (default: the current device).
/// Returns the new current device number (invisibly). When the last real
/// device is closed, the current device reverts to 1 (the null device).
///
/// @param which device number to close (default: current device)
/// @return integer — the new current device number (invisibly)
#[interpreter_builtin(name = "dev.off", namespace = "grDevices")]
fn interp_dev_off(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let interp = context.interpreter();
    let mut mgr = interp.device_manager.borrow_mut();

    let which = call_args.integer_or("which", 0, i64::from(u16::try_from(mgr.current())?));
    let which_usize = usize::try_from(which).map_err(|_| {
        RError::new(
            RErrorKind::Argument,
            format!("dev.off(): invalid device number {which}"),
        )
    })?;

    mgr.close_device(which_usize)?;
    let new_current = i64::from(u16::try_from(mgr.current())?);
    interp.set_invisible();
    Ok(RValue::vec(Vector::Integer(vec![Some(new_current)].into())))
}

/// Return the current graphics device number.
///
/// Returns 1 (the null device) when no real graphics devices are open.
///
/// @return integer — the current device number
#[interpreter_builtin(name = "dev.cur", namespace = "grDevices")]
fn interp_dev_cur(
    _args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let mgr = context.interpreter().device_manager.borrow();
    let current = i64::from(u16::try_from(mgr.current())?);
    Ok(RValue::vec(Vector::Integer(vec![Some(current)].into())))
}

/// Switch the active graphics device.
///
/// Sets the current device to `which` and returns the previous current
/// device number.
///
/// @param which device number to make active
/// @return integer — the previous current device number
#[interpreter_builtin(name = "dev.set", namespace = "grDevices")]
fn interp_dev_set(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let which = call_args
        .value("which", 0)
        .and_then(|v| v.as_vector()?.as_integer_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "dev.set() requires a 'which' argument specifying the device number".to_string(),
            )
        })?;

    let which_usize = usize::try_from(which).map_err(|_| {
        RError::new(
            RErrorKind::Argument,
            format!("dev.set(): invalid device number {which}"),
        )
    })?;

    let mut mgr = context.interpreter().device_manager.borrow_mut();
    let prev = mgr.set_current(which_usize)?;
    let prev_i64 = i64::from(u16::try_from(prev)?);
    Ok(RValue::vec(Vector::Integer(vec![Some(prev_i64)].into())))
}

/// List all open graphics devices.
///
/// Returns a named integer vector where each element is a device number
/// and the names are the device type names. Returns NULL if no real
/// devices are open.
///
/// @return named integer vector of open devices, or NULL
#[interpreter_builtin(name = "dev.list", namespace = "grDevices")]
fn interp_dev_list(
    _args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let mgr = context.interpreter().device_manager.borrow();
    let devices = mgr.list();

    if devices.is_empty() {
        return Ok(RValue::Null);
    }

    let values: Vec<Option<i64>> = devices
        .iter()
        .map(|(idx, _)| i64::try_from(*idx).map(Some))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| RError::other(format!("device index overflow: {e}")))?;
    let names: Vec<Option<String>> = devices.iter().map(|(_, name)| Some(name.clone())).collect();

    let mut rv = RVector::from(Vector::Integer(values.into()));
    rv.set_attr(
        "names".to_string(),
        RValue::vec(Vector::Character(names.into())),
    );
    Ok(RValue::Vector(rv))
}

/// Open a new graphics device.
///
/// Currently opens a NullDevice (placeholder). Returns the new device
/// number invisibly.
///
/// @return integer — the new device number (invisibly)
#[interpreter_builtin(name = "dev.new", namespace = "grDevices")]
fn interp_dev_new(
    _args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let interp = context.interpreter();
    let mut mgr = interp.device_manager.borrow_mut();
    let idx = mgr.add_device(Box::new(NullDevice));
    let idx_i64 = i64::from(u16::try_from(idx)?);
    interp.set_invisible();
    Ok(RValue::vec(Vector::Integer(vec![Some(idx_i64)].into())))
}

/// Close all open graphics devices.
///
/// Equivalent to calling `dev.off()` on every open device. After this
/// call, the current device is 1 (the null device).
///
/// @return NULL (invisibly)
#[interpreter_builtin(name = "graphics.off", namespace = "grDevices")]
fn interp_graphics_off(
    _args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let interp = context.interpreter();
    interp.device_manager.borrow_mut().close_all();
    interp.set_invisible();
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
