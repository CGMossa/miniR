//! Text progress bar builtins backed by the `indicatif` crate.
//!
//! R's `utils::txtProgressBar` API lets scripts report progress for
//! long-running operations. This module stores progress bar state on the
//! `Interpreter` struct as a `Vec<Option<ProgressBarState>>` — each bar is
//! addressed by its index in this Vec. The R side sees an integer ID with
//! class `"txtProgressBar"`.
//!
//! Builtins:
//! - `txtProgressBar(min, max, style)` — create a bar, return integer ID
//! - `setTxtProgressBar(pb, value)` — update the bar's position
//! - `getTxtProgressBar(pb)` — return the bar's current value
//! - `close(pb)` — finish and remove the bar (dispatched from the
//!   connection-layer `close()` when the argument has class `"txtProgressBar"`)

use indicatif::{ProgressBar, ProgressStyle};

use super::CallArgs;
use crate::interpreter::value::*;
use crate::interpreter::BuiltinContext;
use crate::interpreter::Interpreter;
use minir_macros::interpreter_builtin;

// region: ProgressBarState

/// Per-bar state stored on the interpreter.
pub struct ProgressBarState {
    /// The `indicatif` bar handle.
    bar: ProgressBar,
    /// The R-level `min` value (default 0.0).
    min: f64,
    /// The R-level `max` value (default 1.0).
    max: f64,
    /// Current R-level value (between `min` and `max`).
    value: f64,
}

// endregion

// region: Interpreter helpers

impl Interpreter {
    /// Allocate a new progress bar, returning its integer ID.
    pub(crate) fn add_progress_bar(&self, state: ProgressBarState) -> usize {
        let mut bars = self.progress_bars.borrow_mut();
        let id = bars.len();
        bars.push(Some(state));
        id
    }

    /// Finish and remove a progress bar by ID. Returns `true` if the bar
    /// existed and was removed.
    pub(crate) fn close_progress_bar(&self, id: usize) -> bool {
        let mut bars = self.progress_bars.borrow_mut();
        if let Some(slot) = bars.get_mut(id) {
            if let Some(state) = slot.take() {
                state.bar.finish_and_clear();
                return true;
            }
        }
        false
    }
}

// endregion

// region: Helpers

/// Build an integer scalar with class `"txtProgressBar"` representing bar `id`.
fn progress_bar_value(id: usize) -> RValue {
    let mut rv = RVector::from(Vector::Integer(
        vec![Some(i64::try_from(id).unwrap_or(0))].into(),
    ));
    rv.set_attr(
        "class".to_string(),
        RValue::vec(Vector::Character(
            vec![Some("txtProgressBar".to_string())].into(),
        )),
    );
    RValue::Vector(rv)
}

/// Extract a progress bar ID from an argument that carries class `"txtProgressBar"`.
fn progress_bar_id(val: &RValue) -> Option<usize> {
    val.as_vector()
        .and_then(|v| v.as_integer_scalar())
        .and_then(|i| usize::try_from(i).ok())
}

/// Returns `true` if `val` carries the `"txtProgressBar"` class attribute.
pub fn is_progress_bar(val: &RValue) -> bool {
    match val {
        RValue::Vector(rv) => rv
            .class()
            .map(|cls| cls.iter().any(|c| c == "txtProgressBar"))
            .unwrap_or(false),
        _ => false,
    }
}

/// Map an R-level value in `[min, max]` to a `u64` position in `[0, total]`.
fn value_to_position(value: f64, min: f64, max: f64, total: u64) -> u64 {
    if max <= min {
        return 0;
    }
    let fraction = ((value - min) / (max - min)).clamp(0.0, 1.0);
    // Use f64 -> u64 via rounding to avoid lossy `as` cast.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let pos = (fraction * total as f64).round() as u64;
    pos
}

/// Build an indicatif `ProgressStyle` for the given R style number.
fn style_for(style: i64) -> ProgressStyle {
    match style {
        1 => ProgressStyle::with_template("  |{bar:50}|")
            .expect("valid progress template")
            .progress_chars("= "),
        // Style 2: no bar, just percentage
        2 => ProgressStyle::with_template("  {percent}%")
            .expect("valid progress template")
            .progress_chars("= "),
        // Style 3 (default): bar with percentage
        _ => ProgressStyle::with_template("  |{bar:50}| {percent}%")
            .expect("valid progress template")
            .progress_chars("= "),
    }
}

// endregion

// region: Builtins

/// Create a text progress bar.
///
/// Returns an integer ID with class "txtProgressBar". The bar is immediately
/// visible in the terminal.
///
/// @param min numeric scalar: minimum value (default 0)
/// @param max numeric scalar: maximum value (default 1)
/// @param style integer scalar: display style 1, 2, or 3 (default 3)
/// @return integer scalar with class "txtProgressBar"
#[interpreter_builtin(name = "txtProgressBar")]
fn interp_txt_progress_bar(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);

    let min = call_args
        .value("min", 0)
        .and_then(|v| v.as_vector()?.as_double_scalar())
        .unwrap_or(0.0);
    let max = call_args
        .value("max", 1)
        .and_then(|v| v.as_vector()?.as_double_scalar())
        .unwrap_or(1.0);
    let style = call_args.integer_or("style", 2, 3);

    if max <= min {
        return Err(RError::new(
            RErrorKind::Argument,
            format!("'max' ({max}) must be greater than 'min' ({min}) in txtProgressBar()"),
        ));
    }

    let total = 1000u64; // internal resolution
    let bar = ProgressBar::new(total);
    bar.set_style(style_for(style));
    bar.set_position(0);

    let state = ProgressBarState {
        bar,
        min,
        max,
        value: min,
    };

    let interp = context.interpreter();
    let id = interp.add_progress_bar(state);
    Ok(progress_bar_value(id))
}

/// Update a text progress bar's position.
///
/// @param pb integer scalar with class "txtProgressBar": the bar ID
/// @param value numeric scalar: the new position (between min and max)
/// @return NULL (invisibly)
#[interpreter_builtin(name = "setTxtProgressBar", min_args = 2)]
fn interp_set_txt_progress_bar(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);

    let pb_val = call_args
        .value("pb", 0)
        .ok_or_else(|| RError::new(RErrorKind::Argument, "argument 'pb' is missing".to_string()))?;
    let id = progress_bar_id(pb_val).ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "argument 'pb' is not a valid txtProgressBar".to_string(),
        )
    })?;

    let value = call_args
        .value("value", 1)
        .and_then(|v| v.as_vector()?.as_double_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "argument 'value' is missing or not numeric".to_string(),
            )
        })?;

    let interp = context.interpreter();
    let mut bars = interp.progress_bars.borrow_mut();
    if let Some(Some(state)) = bars.get_mut(id) {
        state.value = value;
        let total = state.bar.length().unwrap_or(1000);
        let pos = value_to_position(value, state.min, state.max, total);
        state.bar.set_position(pos);
    } else {
        return Err(RError::new(
            RErrorKind::Argument,
            format!("progress bar {id} has been closed or does not exist"),
        ));
    }

    context.interpreter().set_invisible();
    Ok(RValue::Null)
}

/// Get the current value of a text progress bar.
///
/// @param pb integer scalar with class "txtProgressBar": the bar ID
/// @return numeric scalar: the current value
#[interpreter_builtin(name = "getTxtProgressBar", min_args = 1)]
fn interp_get_txt_progress_bar(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);

    let pb_val = call_args
        .value("pb", 0)
        .ok_or_else(|| RError::new(RErrorKind::Argument, "argument 'pb' is missing".to_string()))?;
    let id = progress_bar_id(pb_val).ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "argument 'pb' is not a valid txtProgressBar".to_string(),
        )
    })?;

    let interp = context.interpreter();
    let bars = interp.progress_bars.borrow();
    if let Some(Some(state)) = bars.get(id) {
        Ok(RValue::vec(Vector::Double(vec![Some(state.value)].into())))
    } else {
        Err(RError::new(
            RErrorKind::Argument,
            format!("progress bar {id} has been closed or does not exist"),
        ))
    }
}

// endregion
