//! R condition system builtins — stop, warning, message, condition constructors,
//! condition accessors, and restart invocation.

use crate::interpreter::value::*;
use crate::interpreter::BuiltinContext;
use minir_macros::{builtin, interpreter_builtin};

/// Signal an error condition and stop execution.
///
/// @param ... character strings concatenated into the error message, or a condition object
/// @return does not return; signals an error
#[builtin]
fn builtin_stop(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
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
        .collect::<Vec<_>>()
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
/// @return NULL, invisibly
#[interpreter_builtin]
fn interp_warning(
    args: &[RValue],
    _: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let msg = args
        .iter()
        .map(|v| match v {
            RValue::Vector(vec) => vec.as_character_scalar().unwrap_or_default(),
            other => format!("{}", other),
        })
        .collect::<Vec<_>>()
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
/// @return NULL, invisibly
#[interpreter_builtin]
fn interp_message(
    args: &[RValue],
    _: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let msg = args
        .iter()
        .map(|v| match v {
            RValue::Vector(vec) => vec.as_character_scalar().unwrap_or_default(),
            other => format!("{}", other),
        })
        .collect::<Vec<_>>()
        .join("");
    let condition = make_condition(&msg, &["simpleMessage", "message", "condition"]);
    let muffled = context
        .with_interpreter(|interp| interp.signal_condition(&condition, &interp.global_env))?;
    if !muffled {
        eprintln!("{}", msg);
    }
    Ok(RValue::Null)
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
