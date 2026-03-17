//! R condition system builtins — stop, warning, message, signalCondition,
//! condition constructors, condition accessors, and restart invocation.

use crate::interpreter::value::*;
use crate::interpreter::BuiltinContext;
use itertools::Itertools;
use minir_macros::{builtin, interpreter_builtin};

// region: Helpers

/// Check whether a named argument is a truthy boolean (default `default`).
fn named_bool(named: &[(String, RValue)], key: &str, default: bool) -> bool {
    named
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| match v {
            RValue::Vector(rv) => rv.as_logical_scalar().unwrap_or(default),
            _ => default,
        })
        .unwrap_or(default)
}

// endregion

// region: stop / warning / message

/// Signal an error condition and stop execution.
///
/// @param ... character strings concatenated into the error message, or a condition object
/// @param call. logical, whether to include the call in the condition (default TRUE, currently ignored)
/// @return does not return; signals an error
#[builtin]
fn builtin_stop(args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    // If the first arg is already a condition object, re-signal it
    if let Some(first) = args.first() {
        let classes = get_class(first);
        if classes.iter().any(|c| c == "condition") {
            return Err(RError::Condition {
                condition: first.clone(),
                kind: ConditionKind::Error,
            });
        }
    }
    let msg = args
        .iter()
        .map(|v| match v {
            RValue::Vector(vec) => vec.as_character_scalar().unwrap_or_default(),
            other => format!("{}", other),
        })
        .join("");
    let condition = make_condition(&msg, &["simpleError", "error", "condition"]);
    Err(RError::Condition {
        condition,
        kind: ConditionKind::Error,
    })
}

/// Signal a warning condition.
///
/// @param ... character strings concatenated into the warning message
/// @param call. logical, whether to include the call (default TRUE, currently ignored)
/// @param immediate. logical, whether to print immediately (default FALSE, currently ignored)
/// @return NULL, invisibly
#[interpreter_builtin]
fn interp_warning(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    // If the first arg is already a condition object, re-signal it
    if let Some(first) = args.first() {
        let classes = get_class(first);
        if classes.iter().any(|c| c == "condition") {
            let muffled = context
                .with_interpreter(|interp| interp.signal_condition(first, &interp.global_env))?;
            if !muffled {
                // Extract message from condition for display
                let msg = condition_message_str(first);
                eprintln!("Warning message:\n{}", msg);
            }
            return Ok(RValue::Null);
        }
    }
    let msg = args
        .iter()
        .map(|v| match v {
            RValue::Vector(vec) => vec.as_character_scalar().unwrap_or_default(),
            other => format!("{}", other),
        })
        .join("");
    let condition = make_condition(&msg, &["simpleWarning", "warning", "condition"]);
    let muffled = context
        .with_interpreter(|interp| interp.signal_condition(&condition, &interp.global_env))?;
    if !muffled {
        eprintln!("Warning message:\n{}", msg);
    }
    Ok(RValue::Null)
}

/// Print a diagnostic message to stderr.
///
/// @param ... character strings concatenated into the message
/// @param domain character string for translation domain (currently ignored)
/// @param appendLF logical, whether to append a newline (default TRUE)
/// @return NULL, invisibly
#[interpreter_builtin]
fn interp_message(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let append_lf = named_bool(named, "appendLF", true);

    // If the first arg is already a condition object, re-signal it
    if let Some(first) = args.first() {
        let classes = get_class(first);
        if classes.iter().any(|c| c == "condition") {
            let muffled = context
                .with_interpreter(|interp| interp.signal_condition(first, &interp.global_env))?;
            if !muffled {
                let msg = condition_message_str(first);
                if append_lf {
                    eprintln!("{}", msg);
                } else {
                    eprint!("{}", msg);
                }
            }
            return Ok(RValue::Null);
        }
    }

    let msg = args
        .iter()
        .map(|v| match v {
            RValue::Vector(vec) => vec.as_character_scalar().unwrap_or_default(),
            other => format!("{}", other),
        })
        .join("");
    let condition = make_condition(&msg, &["simpleMessage", "message", "condition"]);
    let muffled = context
        .with_interpreter(|interp| interp.signal_condition(&condition, &interp.global_env))?;
    if !muffled {
        if append_lf {
            eprintln!("{}", msg);
        } else {
            eprint!("{}", msg);
        }
    }
    Ok(RValue::Null)
}

// endregion

// region: signalCondition

/// Signal a condition object to calling handlers without unwinding.
///
/// This is the low-level primitive used by the condition system. It walks the
/// handler stack and invokes matching handlers in calling-handler style (the
/// handler runs and then returns to the signaler).
///
/// @param c a condition object (list with class attribute containing condition classes)
/// @return NULL, invisibly
#[interpreter_builtin(name = "signalCondition", min_args = 1)]
fn interp_signal_condition(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let condition = args.first().ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "argument 'cond' is missing, with no default".to_string(),
        )
    })?;
    context.with_interpreter(|interp| {
        interp.signal_condition(condition, &interp.global_env)?;
        Ok(RValue::Null)
    })
}

// endregion

/// Extract the message string from a condition object (list with "message" element).
fn condition_message_str(cond: &RValue) -> String {
    if let RValue::List(list) = cond {
        for (name, val) in &list.values {
            if name.as_deref() == Some("message") {
                if let RValue::Vector(rv) = val {
                    if let Some(s) = rv.as_character_scalar() {
                        return s;
                    }
                }
            }
        }
    }
    String::new()
}

/// Construct a simple condition object.
///
/// @param msg character string giving the condition message
/// @param call the call associated with the condition (default NULL)
/// @param class additional class to prepend to "condition"
/// @return a condition list with message, call, and class attributes
#[builtin(name = "simpleCondition", min_args = 1)]
fn builtin_simple_condition(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let msg = args
        .first()
        .and_then(|v| v.as_vector().and_then(|rv| rv.as_character_scalar()))
        .unwrap_or_default();
    let call = args.get(1).cloned().unwrap_or(RValue::Null);
    let extra_class = named
        .iter()
        .find(|(k, _)| k == "class")
        .and_then(|(_, v)| v.as_vector().and_then(|rv| rv.as_character_scalar()))
        .unwrap_or_default();
    let mut classes: Vec<&str> = Vec::new();
    if !extra_class.is_empty() {
        classes.push(&extra_class);
    }
    classes.push("condition");
    let mut list = RList::new(vec![
        (
            Some("message".to_string()),
            RValue::vec(Vector::Character(vec![Some(msg)].into())),
        ),
        (Some("call".to_string()), call),
    ]);
    let class_vec: Vec<Option<String>> = classes.iter().map(|s| Some(s.to_string())).collect();
    list.set_attr(
        "class".to_string(),
        RValue::vec(Vector::Character(class_vec.into())),
    );
    Ok(RValue::List(list))
}

/// Construct a simple error condition object.
///
/// @param message character string giving the error message
/// @param call the call associated with the error (default NULL)
/// @return a condition list with class c("simpleError", "error", "condition")
#[builtin(name = "simpleError", min_args = 1)]
fn builtin_simple_error(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let msg = args
        .first()
        .and_then(|v| v.as_vector().and_then(|rv| rv.as_character_scalar()))
        .unwrap_or_default();
    let call = args.get(1).cloned().unwrap_or(RValue::Null);
    Ok(make_condition_with_call(
        &msg,
        call,
        &["simpleError", "error", "condition"],
    ))
}

/// Construct a simple warning condition object.
///
/// @param message character string giving the warning message
/// @param call the call associated with the warning (default NULL)
/// @return a condition list with class c("simpleWarning", "warning", "condition")
#[builtin(name = "simpleWarning", min_args = 1)]
fn builtin_simple_warning(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let msg = args
        .first()
        .and_then(|v| v.as_vector().and_then(|rv| rv.as_character_scalar()))
        .unwrap_or_default();
    let call = args.get(1).cloned().unwrap_or(RValue::Null);
    Ok(make_condition_with_call(
        &msg,
        call,
        &["simpleWarning", "warning", "condition"],
    ))
}

/// Construct a simple message condition object.
///
/// @param message character string giving the message
/// @param call the call associated with the message (default NULL)
/// @return a condition list with class c("simpleMessage", "message", "condition")
#[builtin(name = "simpleMessage", min_args = 1)]
fn builtin_simple_message(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let msg = args
        .first()
        .and_then(|v| v.as_vector().and_then(|rv| rv.as_character_scalar()))
        .unwrap_or_default();
    let call = args.get(1).cloned().unwrap_or(RValue::Null);
    Ok(make_condition_with_call(
        &msg,
        call,
        &["simpleMessage", "message", "condition"],
    ))
}

/// Extract the message from a condition object.
///
/// @param c a condition object (list with "message" element)
/// @return character string giving the condition message
#[builtin(name = "conditionMessage", min_args = 1)]
fn builtin_condition_message(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::List(list)) => {
            for (name, val) in &list.values {
                if name.as_deref() == Some("message") {
                    return Ok(val.clone());
                }
            }
            Ok(RValue::Null)
        }
        _ => Err(RError::new(
            RErrorKind::Argument,
            "conditionMessage requires a condition object".to_string(),
        )),
    }
}

/// Extract the call from a condition object.
///
/// @param c a condition object (list with "call" element)
/// @return the call associated with the condition, or NULL
#[builtin(name = "conditionCall", min_args = 1)]
fn builtin_condition_call(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::List(list)) => {
            for (name, val) in &list.values {
                if name.as_deref() == Some("call") {
                    return Ok(val.clone());
                }
            }
            Ok(RValue::Null)
        }
        _ => Err(RError::new(
            RErrorKind::Argument,
            "conditionCall requires a condition object".to_string(),
        )),
    }
}

/// Invoke a restart by name, transferring control to the corresponding handler.
///
/// @param r character string naming the restart to invoke
/// @return does not return; transfers control to the restart handler
#[builtin(name = "invokeRestart", min_args = 1)]
fn builtin_invoke_restart(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let restart_name = args
        .first()
        .and_then(|v| v.as_vector().and_then(|rv| rv.as_character_scalar()))
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "restart name must be a string".to_string(),
            )
        })?;
    // Signal the restart by throwing it as an RError::other — caught by signal_condition
    Err(RError::other(restart_name))
}
