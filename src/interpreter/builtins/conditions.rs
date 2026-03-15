//! R condition system builtins — stop, warning, message, condition constructors,
//! condition accessors, and restart invocation.

use crate::interpreter::value::*;
use crate::interpreter::BuiltinContext;
use minir_macros::{builtin, interpreter_builtin};

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
