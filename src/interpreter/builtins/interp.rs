//! Interpreter-level builtins — functions that receive `BuiltinContext` so
//! they can call back into the active interpreter without direct TLS lookups.
//! Each is auto-registered via `#[interpreter_builtin]`.

use super::CallArgs;
use crate::interpreter::environment::Environment;
use crate::interpreter::value::*;
use crate::interpreter::BuiltinContext;
use crate::parser::ast::{BinaryOp, UnaryOp};
use minir_macros::interpreter_builtin;

/// Extract `fail_fast` from named args and return the remaining named args.
/// Default is `false` (collect all errors).
fn extract_fail_fast(named: &[(String, RValue)]) -> (bool, Vec<(String, RValue)>) {
    let mut fail_fast = false;
    let mut remaining = Vec::with_capacity(named.len());
    for (name, val) in named {
        if name == "fail_fast" {
            fail_fast = val
                .as_vector()
                .and_then(|v| v.as_logical_scalar())
                .unwrap_or(false);
        } else {
            remaining.push((name.clone(), val.clone()));
        }
    }
    (fail_fast, remaining)
}

/// Resolve a function specification: accepts an RValue::Function directly,
/// or a string naming a function to look up in the environment.
/// Equivalent to R's match.fun().
fn match_fun(f: &RValue, env: &Environment) -> Result<RValue, RError> {
    match f {
        RValue::Function(_) => Ok(f.clone()),
        RValue::Vector(rv) => match &rv.inner {
            Vector::Character(s) => {
                let name = s.first().and_then(|x| x.as_ref()).ok_or_else(|| {
                    RError::new(
                        RErrorKind::Argument,
                        "not a valid function name".to_string(),
                    )
                })?;
                env.get_function(name)
                    .ok_or_else(|| RError::other(format!("could not find function '{}'", name)))
            }
            _ => Err(RError::new(
                RErrorKind::Argument,
                "FUN is not a function and not a string naming a function".to_string(),
            )),
        },
        _ => Err(RError::new(
            RErrorKind::Argument,
            "FUN is not a function and not a string naming a function".to_string(),
        )),
    }
}

fn optional_frame_index(positional: &[RValue], default: i64) -> Result<i64, RError> {
    match positional.first() {
        None => Ok(default),
        Some(value) => value
            .as_vector()
            .and_then(|v| v.as_integer_scalar())
            .ok_or_else(|| {
                RError::new(
                    RErrorKind::Argument,
                    "frame index must be an integer".to_string(),
                )
            }),
    }
}

fn language_or_null(expr: Option<crate::parser::ast::Expr>) -> RValue {
    match expr {
        Some(expr) => RValue::Language(Language::new(expr)),
        None => RValue::Null,
    }
}

// region: S3-dispatching generics (print, format)

/// Get explicit class attributes from an RValue.
/// Returns an empty vec for objects without a class attribute.
fn explicit_classes(val: &RValue) -> Vec<String> {
    match val {
        RValue::Vector(rv) => rv.class().unwrap_or_default(),
        RValue::List(list) => {
            if let Some(RValue::Vector(rv)) = list.get_attr("class") {
                if let Vector::Character(classes) = &rv.inner {
                    classes.iter().filter_map(|c| c.clone()).collect()
                } else {
                    vec![]
                }
            } else {
                vec![]
            }
        }
        RValue::Language(lang) => lang.class().unwrap_or_default(),
        _ => vec![],
    }
}

/// Try S3 dispatch for a generic function. Returns `Ok(Some(result))` if a
/// method was found and called, `Ok(None)` if no method exists (caller should
/// fall through to default behavior).
fn try_s3_dispatch(
    generic: &str,
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<Option<RValue>, RError> {
    let Some(val) = args.first() else {
        return Ok(None);
    };
    let classes = explicit_classes(val);
    if classes.is_empty() {
        return Ok(None);
    }
    let env = context.env();
    for class in &classes {
        let method_name = format!("{generic}.{class}");
        if let Some(method) = env.get(&method_name) {
            let result = context
                .with_interpreter(|interp| interp.call_function(&method, args, named, env))?;
            return Ok(Some(result));
        }
    }
    Ok(None)
}

#[interpreter_builtin(min_args = 1)]
fn interp_print(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    // Try S3 dispatch (print.Date, print.POSIXct, print.data.frame, etc.)
    if let Some(result) = try_s3_dispatch("print", args, named, context)? {
        return Ok(result);
    }
    // Default print
    if let Some(val) = args.first() {
        println!("{}", val);
    }
    Ok(args.first().cloned().unwrap_or(RValue::Null))
}

#[interpreter_builtin(min_args = 1)]
fn interp_format(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    // Try S3 dispatch (format.Date, format.POSIXct, etc.)
    if let Some(result) = try_s3_dispatch("format", args, named, context)? {
        return Ok(result);
    }
    // Default format
    match args.first() {
        Some(val) => Ok(RValue::vec(Vector::Character(
            vec![Some(format!("{}", val))].into(),
        ))),
        None => Ok(RValue::vec(Vector::Character(
            vec![Some(String::new())].into(),
        ))),
    }
}

// endregion

#[interpreter_builtin(name = "sapply", min_args = 2)]
fn interp_sapply(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    eval_apply(args, named, true, context)
}

#[interpreter_builtin(name = "lapply", min_args = 2)]
fn interp_lapply(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    eval_apply(args, named, false, context)
}

#[interpreter_builtin(name = "vapply", min_args = 3)]
fn interp_vapply(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    eval_apply(args, named, true, context)
}

fn eval_apply(
    positional: &[RValue],
    named: &[(String, RValue)],
    simplify: bool,
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    if positional.len() < 2 {
        return Err(RError::new(
            RErrorKind::Argument,
            "need at least 2 arguments for apply".to_string(),
        ));
    }
    let env = context.env();
    let (fail_fast, _extra_named) = extract_fail_fast(named);
    let x = &positional[0];
    let f = match_fun(&positional[1], env)?;

    let items: Vec<RValue> = match x {
        RValue::Vector(v) => match &v.inner {
            Vector::Raw(vals) => vals
                .iter()
                .map(|&x| RValue::vec(Vector::Raw(vec![x])))
                .collect(),
            Vector::Double(vals) => vals
                .iter()
                .map(|x| RValue::vec(Vector::Double(vec![*x].into())))
                .collect(),
            Vector::Integer(vals) => vals
                .iter()
                .map(|x| RValue::vec(Vector::Integer(vec![*x].into())))
                .collect(),
            Vector::Complex(vals) => vals
                .iter()
                .map(|x| RValue::vec(Vector::Complex(vec![*x].into())))
                .collect(),
            Vector::Character(vals) => vals
                .iter()
                .map(|x| RValue::vec(Vector::Character(vec![x.clone()].into())))
                .collect(),
            Vector::Logical(vals) => vals
                .iter()
                .map(|x| RValue::vec(Vector::Logical(vec![*x].into())))
                .collect(),
        },
        RValue::List(l) => l.values.iter().map(|(_, v)| v.clone()).collect(),
        _ => vec![x.clone()],
    };

    let env = context.env();
    context.with_interpreter(|interp| {
        let mut results: Vec<RValue> = Vec::new();
        for item in &items {
            if fail_fast {
                let result = interp.call_function(&f, std::slice::from_ref(item), &[], env)?;
                results.push(result);
            } else {
                match interp.call_function(&f, std::slice::from_ref(item), &[], env) {
                    Ok(result) => results.push(result),
                    Err(_) => results.push(RValue::Null),
                }
            }
        }

        if simplify {
            let all_scalar = results.iter().all(|r| r.length() == 1);
            if all_scalar && !results.is_empty() {
                let first_type = results[0].type_name();
                let all_same = results.iter().all(|r| r.type_name() == first_type);
                if all_same {
                    match first_type {
                        "double" => {
                            let vals: Vec<Option<f64>> = results
                                .iter()
                                .filter_map(|r| {
                                    r.as_vector()
                                        .map(|v| v.to_doubles().into_iter().next().unwrap_or(None))
                                })
                                .collect();
                            return Ok(RValue::vec(Vector::Double(vals.into())));
                        }
                        "integer" => {
                            let vals: Vec<Option<i64>> = results
                                .iter()
                                .filter_map(|r| {
                                    r.as_vector()
                                        .map(|v| v.to_integers().into_iter().next().unwrap_or(None))
                                })
                                .collect();
                            return Ok(RValue::vec(Vector::Integer(vals.into())));
                        }
                        "character" => {
                            let vals: Vec<Option<String>> = results
                                .iter()
                                .filter_map(|r| {
                                    r.as_vector().map(|v| {
                                        v.to_characters().into_iter().next().unwrap_or(None)
                                    })
                                })
                                .collect();
                            return Ok(RValue::vec(Vector::Character(vals.into())));
                        }
                        "logical" => {
                            let vals: Vec<Option<bool>> = results
                                .iter()
                                .filter_map(|r| {
                                    r.as_vector()
                                        .map(|v| v.to_logicals().into_iter().next().unwrap_or(None))
                                })
                                .collect();
                            return Ok(RValue::vec(Vector::Logical(vals.into())));
                        }
                        _ => {}
                    }
                }
            }
        }

        let values: Vec<(Option<String>, RValue)> =
            results.into_iter().map(|v| (None, v)).collect();
        Ok(RValue::List(RList::new(values)))
    })
}

#[interpreter_builtin(name = "do.call", min_args = 2)]
fn interp_do_call(
    positional: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let env = context.env();
    if positional.len() >= 2 {
        let f = match_fun(&positional[0], env)?;
        return context.with_interpreter(|interp| match &positional[1] {
            RValue::List(l) => {
                let args: Vec<RValue> = l.values.iter().map(|(_, v)| v.clone()).collect();
                interp
                    .call_function(&f, &args, named, env)
                    .map_err(RError::from)
            }
            _ => interp
                .call_function(&f, &positional[1..], named, env)
                .map_err(RError::from),
        });
    }
    Err(RError::new(
        RErrorKind::Argument,
        "do.call requires at least 2 arguments".to_string(),
    ))
}

#[interpreter_builtin(name = "Vectorize", min_args = 1)]
fn interp_vectorize(
    positional: &[RValue],
    _named: &[(String, RValue)],
    _context: &BuiltinContext,
) -> Result<RValue, RError> {
    Ok(positional.first().cloned().unwrap_or(RValue::Null))
}

#[interpreter_builtin(name = "Reduce", min_args = 2)]
fn interp_reduce(
    positional: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    if positional.len() < 2 {
        return Err(RError::new(
            RErrorKind::Argument,
            "Reduce requires at least 2 arguments".to_string(),
        ));
    }
    let env = context.env();
    let (_fail_fast, _extra_named) = extract_fail_fast(named);
    let f = match_fun(&positional[0], env)?;
    let x = &positional[1];
    let init = positional
        .get(2)
        .or_else(|| named.iter().find(|(n, _)| n == "init").map(|(_, v)| v));
    let accumulate = named
        .iter()
        .find(|(n, _)| n == "accumulate")
        .and_then(|(_, v)| v.as_vector()?.as_logical_scalar())
        .unwrap_or(false);

    let items: Vec<RValue> = rvalue_to_items(x);

    if items.is_empty() {
        return Ok(init.cloned().unwrap_or(RValue::Null));
    }

    let (mut acc, start) = match init {
        Some(v) => (v.clone(), 0),
        None => (items[0].clone(), 1),
    };

    let mut accum_results = if accumulate {
        vec![acc.clone()]
    } else {
        vec![]
    };

    // Reduce is inherently sequential — each step depends on the previous.
    // fail_fast has no meaningful "collect errors" behavior here; errors always propagate.
    let env = context.env();
    context.with_interpreter(|interp| {
        for item in items.iter().skip(start) {
            acc = interp.call_function(&f, &[acc, item.clone()], &[], env)?;
            if accumulate {
                accum_results.push(acc.clone());
            }
        }

        if accumulate {
            let values: Vec<(Option<String>, RValue)> =
                accum_results.into_iter().map(|v| (None, v)).collect();
            Ok(RValue::List(RList::new(values)))
        } else {
            Ok(acc)
        }
    })
}

#[interpreter_builtin(name = "Filter", min_args = 2)]
fn interp_filter(
    positional: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    if positional.len() < 2 {
        return Err(RError::new(
            RErrorKind::Argument,
            "Filter requires 2 arguments".to_string(),
        ));
    }
    let env = context.env();
    let (fail_fast, _extra_named) = extract_fail_fast(named);
    let f = match_fun(&positional[0], env)?;
    let x = &positional[1];

    let items: Vec<RValue> = rvalue_to_items(x);

    let mut results = Vec::new();
    context.with_interpreter(|interp| {
        for item in &items {
            if fail_fast {
                let keep = interp.call_function(&f, std::slice::from_ref(item), &[], env)?;
                if keep
                    .as_vector()
                    .and_then(|v| v.as_logical_scalar())
                    .unwrap_or(false)
                {
                    results.push(item.clone());
                }
            } else if let Ok(keep) = interp.call_function(&f, std::slice::from_ref(item), &[], env)
            {
                if keep
                    .as_vector()
                    .and_then(|v| v.as_logical_scalar())
                    .unwrap_or(false)
                {
                    results.push(item.clone());
                }
                // Errors are silently skipped (element excluded from results)
            }
        }
        Ok::<(), RError>(())
    })?;

    match x {
        RValue::List(_) => {
            let values: Vec<(Option<String>, RValue)> =
                results.into_iter().map(|v| (None, v)).collect();
            Ok(RValue::List(RList::new(values)))
        }
        _ => {
            if results.is_empty() {
                Ok(RValue::Null)
            } else {
                crate::interpreter::builtins::builtin_c(&results, &[])
            }
        }
    }
}

#[interpreter_builtin(name = "Map", min_args = 2)]
fn interp_map(
    positional: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    if positional.len() < 2 {
        return Err(RError::new(
            RErrorKind::Argument,
            "Map requires at least 2 arguments".to_string(),
        ));
    }
    let env = context.env();
    let (fail_fast, _extra_named) = extract_fail_fast(named);
    let f = match_fun(&positional[0], env)?;

    let seqs: Vec<Vec<RValue>> = positional[1..].iter().map(rvalue_to_items).collect();

    let max_len = seqs.iter().map(|s| s.len()).max().unwrap_or(0);
    let mut results = Vec::new();

    context.with_interpreter(|interp| {
        for i in 0..max_len {
            let call_args: Vec<RValue> = seqs
                .iter()
                .map(|s| {
                    if s.is_empty() {
                        RValue::Null
                    } else {
                        s[i % s.len()].clone()
                    }
                })
                .collect();
            let result = if fail_fast {
                interp.call_function(&f, &call_args, &[], env)?
            } else {
                interp
                    .call_function(&f, &call_args, &[], env)
                    .unwrap_or(RValue::Null)
            };
            results.push((None, result));
        }
        Ok::<(), RError>(())
    })?;

    Ok(RValue::List(RList::new(results)))
}

#[interpreter_builtin(name = "switch", min_args = 1)]
fn interp_switch(
    positional: &[RValue],
    named: &[(String, RValue)],
    _context: &BuiltinContext,
) -> Result<RValue, RError> {
    let expr = positional
        .first()
        .ok_or_else(|| RError::new(RErrorKind::Argument, "'EXPR' is missing".to_string()))?;

    let is_character =
        matches!(expr, RValue::Vector(rv) if matches!(rv.inner, Vector::Character(_)));

    if is_character {
        let s = match expr {
            RValue::Vector(v) => v.as_character_scalar().unwrap_or_default(),
            _ => String::new(),
        };
        let mut found = false;
        for (name, val) in named {
            if name == &s {
                found = true;
                if !matches!(val, RValue::Null) {
                    return Ok(val.clone());
                }
            } else if found && !matches!(val, RValue::Null) {
                return Ok(val.clone());
            }
        }
        if let Some(default) = positional.get(1) {
            return Ok(default.clone());
        }
        Ok(RValue::Null)
    } else {
        let idx = match expr {
            RValue::Vector(v) => v.as_integer_scalar(),
            _ => None,
        };
        match idx {
            Some(i) if i >= 1 => {
                let mut alts: Vec<&RValue> = positional.iter().skip(1).collect();
                for (_, v) in named {
                    alts.push(v);
                }
                Ok(alts
                    .get(usize::try_from(i - 1)?)
                    .map(|v| (*v).clone())
                    .unwrap_or(RValue::Null))
            }
            _ => Ok(RValue::Null),
        }
    }
}

#[interpreter_builtin(name = "get", min_args = 1)]
fn interp_get(
    positional: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let env = context.env();
    let call_args = CallArgs::new(positional, named);
    let name = call_args.string("x", 0)?;
    let target_env = call_args.environment_or("envir", usize::MAX, env)?;
    target_env
        .get(&name)
        .ok_or_else(|| RError::other(format!("object '{}' not found", name)))
}

#[interpreter_builtin(name = "assign", min_args = 2)]
fn interp_assign(
    positional: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let env = context.env();
    let call_args = CallArgs::new(positional, named);
    let name = call_args.string("x", 0)?;
    let value = call_args.value("value", 1).cloned().unwrap_or(RValue::Null);
    let target_env = call_args.environment_or("envir", usize::MAX, env)?;
    target_env.set(name, value.clone());
    Ok(value)
}

#[interpreter_builtin(name = "exists", min_args = 1)]
fn interp_exists(
    positional: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let env = context.env();
    let call_args = CallArgs::new(positional, _named);
    let name = call_args.optional_string("x", 0).unwrap_or_default();
    let found = call_args
        .environment_or("envir", usize::MAX, env)?
        .get(&name)
        .is_some();
    Ok(RValue::vec(Vector::Logical(vec![Some(found)].into())))
}

#[interpreter_builtin(name = "source", min_args = 1)]
fn interp_source(
    positional: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let path = positional
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| RError::new(RErrorKind::Argument, "invalid 'file' argument".to_string()))?;
    let source = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::InvalidData => {
            let bytes = std::fs::read(&path)
                .map_err(|e2| RError::other(format!("cannot open file '{}': {}", path, e2)))?;
            String::from_utf8_lossy(&bytes).into_owned()
        }
        Err(e) => return Err(RError::other(format!("cannot open file '{}': {}", path, e))),
    };
    let ast = crate::parser::parse_program(&source)
        .map_err(|e| RError::other(format!("parse error in '{}': {}", path, e)))?;
    context.with_interpreter(|interp| interp.eval(&ast).map_err(RError::from))
}

// system.time() is in pre_eval.rs — it must time unevaluated expressions

// --- Operator builtins: R operators as first-class functions ---
// These allow `Reduce("+", 1:10)`, `sapply(x, "-")`, `do.call("*", list(3,4))`, etc.

fn eval_binop(op: BinaryOp, args: &[RValue], context: &BuiltinContext) -> Result<RValue, RError> {
    let left = args.first().cloned().unwrap_or(RValue::Null);
    let right = args.get(1).cloned().unwrap_or(RValue::Null);
    context
        .with_interpreter(|interp| interp.eval_binary(op, &left, &right))
        .map_err(RError::from)
}

#[interpreter_builtin(name = "+", min_args = 1)]
fn interp_op_add(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    if args.len() == 1 {
        context
            .with_interpreter(|interp| interp.eval_unary(UnaryOp::Pos, &args[0]))
            .map_err(RError::from)
    } else {
        eval_binop(BinaryOp::Add, args, context)
    }
}

#[interpreter_builtin(name = "-", min_args = 1)]
fn interp_op_sub(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    if args.len() == 1 {
        context
            .with_interpreter(|interp| interp.eval_unary(UnaryOp::Neg, &args[0]))
            .map_err(RError::from)
    } else {
        eval_binop(BinaryOp::Sub, args, context)
    }
}

#[interpreter_builtin(name = "*", min_args = 2)]
fn interp_op_mul(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    eval_binop(BinaryOp::Mul, args, context)
}

#[interpreter_builtin(name = "/", min_args = 2)]
fn interp_op_div(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    eval_binop(BinaryOp::Div, args, context)
}

#[interpreter_builtin(name = "^", min_args = 2)]
fn interp_op_pow(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    eval_binop(BinaryOp::Pow, args, context)
}

#[interpreter_builtin(name = "%%", min_args = 2)]
fn interp_op_mod(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    eval_binop(BinaryOp::Mod, args, context)
}

#[interpreter_builtin(name = "%/%", min_args = 2)]
fn interp_op_intdiv(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    eval_binop(BinaryOp::IntDiv, args, context)
}

#[interpreter_builtin(name = "==", min_args = 2)]
fn interp_op_eq(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    eval_binop(BinaryOp::Eq, args, context)
}

#[interpreter_builtin(name = "!=", min_args = 2)]
fn interp_op_ne(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    eval_binop(BinaryOp::Ne, args, context)
}

#[interpreter_builtin(name = "<", min_args = 2)]
fn interp_op_lt(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    eval_binop(BinaryOp::Lt, args, context)
}

#[interpreter_builtin(name = ">", min_args = 2)]
fn interp_op_gt(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    eval_binop(BinaryOp::Gt, args, context)
}

#[interpreter_builtin(name = "<=", min_args = 2)]
fn interp_op_le(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    eval_binop(BinaryOp::Le, args, context)
}

#[interpreter_builtin(name = ">=", min_args = 2)]
fn interp_op_ge(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    eval_binop(BinaryOp::Ge, args, context)
}

#[interpreter_builtin(name = "&", min_args = 2)]
fn interp_op_and(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    eval_binop(BinaryOp::And, args, context)
}

#[interpreter_builtin(name = "|", min_args = 2)]
fn interp_op_or(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    eval_binop(BinaryOp::Or, args, context)
}

#[interpreter_builtin(name = "!", min_args = 1)]
fn interp_op_not(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    context
        .with_interpreter(|interp| interp.eval_unary(UnaryOp::Not, &args[0]))
        .map_err(RError::from)
}

/// Convert an RValue to a Vec of individual items (for apply/map/filter/reduce).
fn rvalue_to_items(x: &RValue) -> Vec<RValue> {
    match x {
        RValue::Vector(v) => match &v.inner {
            Vector::Raw(vals) => vals
                .iter()
                .map(|&x| RValue::vec(Vector::Raw(vec![x])))
                .collect(),
            Vector::Double(vals) => vals
                .iter()
                .map(|x| RValue::vec(Vector::Double(vec![*x].into())))
                .collect(),
            Vector::Integer(vals) => vals
                .iter()
                .map(|x| RValue::vec(Vector::Integer(vec![*x].into())))
                .collect(),
            Vector::Complex(vals) => vals
                .iter()
                .map(|x| RValue::vec(Vector::Complex(vec![*x].into())))
                .collect(),
            Vector::Character(vals) => vals
                .iter()
                .map(|x| RValue::vec(Vector::Character(vec![x.clone()].into())))
                .collect(),
            Vector::Logical(vals) => vals
                .iter()
                .map(|x| RValue::vec(Vector::Logical(vec![*x].into())))
                .collect(),
        },
        RValue::List(l) => l.values.iter().map(|(_, v)| v.clone()).collect(),
        _ => vec![x.clone()],
    }
}

#[interpreter_builtin(name = "NextMethod")]
fn interp_next_method(
    positional: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let env = context.env();
    context
        .with_interpreter(|interp| interp.dispatch_next_method(positional, named, env))
        .map_err(RError::from)
}

#[interpreter_builtin(name = "environment")]
fn interp_environment(
    positional: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    match positional.first() {
        Some(RValue::Function(RFunction::Closure { env, .. })) => {
            Ok(RValue::Environment(env.clone()))
        }
        Some(_) => Ok(RValue::Null),
        // No args: return the current (calling) environment
        None => Ok(RValue::Environment(context.env().clone())),
    }
}

#[interpreter_builtin(name = "as.environment", min_args = 1)]
fn interp_as_environment(
    positional: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let x = positional
        .first()
        .ok_or_else(|| RError::new(RErrorKind::Argument, "argument 'x' is missing".to_string()))?;

    match x {
        RValue::Environment(_) => Ok(x.clone()),
        RValue::Vector(rv) => {
            if let Some(n) = rv.as_integer_scalar() {
                return context.with_interpreter(|interp| {
                    match n {
                    1 => Ok(RValue::Environment(interp.global_env.clone())),
                    -1 => {
                        let base = interp
                            .global_env
                            .parent()
                            .unwrap_or_else(|| interp.global_env.clone());
                        Ok(RValue::Environment(base))
                    }
                    _ => Err(RError::new(RErrorKind::Argument, format!(
                        "as.environment({}): only search path positions 1 (global) and -1 (base) are currently supported",
                        n
                    ))),
                }
                });
            }
            if let Some(s) = rv.as_character_scalar() {
                return context.with_interpreter(|interp| match s.as_str() {
                    ".GlobalEnv" | "R_GlobalEnv" => {
                        Ok(RValue::Environment(interp.global_env.clone()))
                    }
                    "package:base" => {
                        let base = interp
                            .global_env
                            .parent()
                            .unwrap_or_else(|| interp.global_env.clone());
                        Ok(RValue::Environment(base))
                    }
                    _ => Err(RError::new(
                        RErrorKind::Argument,
                        format!(
                        "no environment called '{}' was found. Use '.GlobalEnv' or 'package:base'",
                        s
                    ),
                    )),
                });
            }
            Err(RError::new(
                RErrorKind::Argument,
                format!(
                "cannot coerce {} to an environment — expected a number, string, or environment",
                x.type_name()
            ),
            ))
        }
        _ => Err(RError::new(
            RErrorKind::Argument,
            format!(
                "cannot coerce {} to an environment — expected a number, string, or environment",
                x.type_name()
            ),
        )),
    }
}

#[interpreter_builtin(name = "globalenv", max_args = 0)]
fn interp_globalenv(
    _positional: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    context.with_interpreter(|interp| Ok(RValue::Environment(interp.global_env.clone())))
}

#[interpreter_builtin(name = "baseenv", max_args = 0)]
fn interp_baseenv(
    _positional: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    context.with_interpreter(|interp| {
        Ok(RValue::Environment(
            interp
                .global_env
                .parent()
                .unwrap_or_else(|| interp.global_env.clone()),
        ))
    })
}

#[interpreter_builtin(name = "emptyenv", max_args = 0)]
fn interp_emptyenv(
    _positional: &[RValue],
    _named: &[(String, RValue)],
    _context: &BuiltinContext,
) -> Result<RValue, RError> {
    Ok(RValue::Environment(Environment::new_empty()))
}

#[interpreter_builtin(name = "sys.call", max_args = 1)]
fn interp_sys_call(
    positional: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let which = optional_frame_index(positional, 0)?;
    context.with_interpreter(|interp| {
        if which == 0 {
            return Ok(language_or_null(interp.current_call_expr()));
        }

        if which < 0 {
            return Err(RError::other(
                "negative frame indices are not yet supported",
            ));
        }

        let which = usize::try_from(which).map_err(RError::from)?;
        let frame = interp
            .call_frame(which)
            .ok_or_else(|| RError::other("not that many frames on the stack"))?;
        Ok(language_or_null(frame.call))
    })
}

#[interpreter_builtin(name = "sys.function", max_args = 1)]
fn interp_sys_function(
    positional: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let which = optional_frame_index(positional, 0)?;
    context.with_interpreter(|interp| {
        if which == 0 {
            return interp
                .current_call_frame()
                .map(|frame| frame.function)
                .ok_or_else(|| RError::other("not that many frames on the stack"));
        }

        if which < 0 {
            return Err(RError::other(
                "negative frame indices are not yet supported",
            ));
        }

        let which = usize::try_from(which).map_err(RError::from)?;
        interp
            .call_frame(which)
            .map(|frame| frame.function)
            .ok_or_else(|| RError::other("not that many frames on the stack"))
    })
}

#[interpreter_builtin(name = "sys.frame", max_args = 1)]
fn interp_sys_frame(
    positional: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let which = optional_frame_index(positional, 0)?;
    context.with_interpreter(|interp| {
        if which == 0 {
            return Ok(RValue::Environment(interp.global_env.clone()));
        }

        if which < 0 {
            return Err(RError::other(
                "negative frame indices are not yet supported",
            ));
        }

        let which = usize::try_from(which).map_err(RError::from)?;
        interp
            .call_frame(which)
            .map(|frame| RValue::Environment(frame.env))
            .ok_or_else(|| RError::other("not that many frames on the stack"))
    })
}

#[interpreter_builtin(name = "sys.calls", max_args = 0)]
fn interp_sys_calls(
    _positional: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    context.with_interpreter(|interp| {
        let values = interp
            .call_frames()
            .into_iter()
            .map(|frame| (None, language_or_null(frame.call)))
            .collect();
        Ok(RValue::List(RList::new(values)))
    })
}

#[interpreter_builtin(name = "sys.frames", max_args = 0)]
fn interp_sys_frames(
    _positional: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    context.with_interpreter(|interp| {
        let values = interp
            .call_frames()
            .into_iter()
            .map(|frame| (None, RValue::Environment(frame.env)))
            .collect();
        Ok(RValue::List(RList::new(values)))
    })
}

#[interpreter_builtin(name = "sys.parents", max_args = 0)]
fn interp_sys_parents(
    _positional: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    context.with_interpreter(|interp| {
        let len = interp.call_frames().len();
        let parents: Vec<Option<i64>> = (0..len)
            .map(|i| i64::try_from(i).map(Some))
            .collect::<Result<_, _>>()
            .map_err(RError::from)?;
        Ok(RValue::vec(Vector::Integer(parents.into())))
    })
}

#[interpreter_builtin(name = "sys.on.exit", max_args = 0)]
fn interp_sys_on_exit(
    _positional: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    context.with_interpreter(|interp| {
        let frame = match interp.current_call_frame() {
            Some(frame) => frame,
            None => return Ok(RValue::Null),
        };

        let exprs = frame.env.peek_on_exit();
        match exprs.len() {
            0 => Ok(RValue::Null),
            1 => Ok(RValue::Language(Language::new(exprs[0].clone()))),
            _ => Ok(RValue::Language(Language::new(
                crate::parser::ast::Expr::Block(exprs),
            ))),
        }
    })
}

#[interpreter_builtin(name = "sys.nframe", max_args = 0)]
fn interp_sys_nframe(
    _positional: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    context.with_interpreter(|interp| {
        let len = i64::try_from(interp.call_frames().len()).map_err(RError::from)?;
        Ok(RValue::vec(Vector::Integer(vec![Some(len)].into())))
    })
}

#[interpreter_builtin(name = "nargs", max_args = 0)]
fn interp_nargs(
    _positional: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    context.with_interpreter(|interp| {
        let count = interp
            .current_call_frame()
            .map(|frame| frame.supplied_arg_count)
            .unwrap_or(0);
        Ok(RValue::vec(Vector::Integer(
            vec![Some(i64::try_from(count).map_err(RError::from)?)].into(),
        )))
    })
}

#[interpreter_builtin(name = "parent.frame", max_args = 1)]
fn interp_parent_frame(
    positional: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let n = optional_frame_index(positional, 1)?;
    if n <= 0 {
        return Err(RError::new(
            RErrorKind::Argument,
            "invalid 'n' value".to_string(),
        ));
    }

    context.with_interpreter(|interp| {
        let depth = interp.call_frames().len();
        let n = usize::try_from(n).map_err(RError::from)?;
        if n >= depth {
            return Ok(RValue::Environment(interp.global_env.clone()));
        }

        let target = depth - n;
        interp
            .call_frame(target)
            .map(|frame| RValue::Environment(frame.env))
            .ok_or_else(|| RError::other("not that many frames on the stack"))
    })
}

#[interpreter_builtin(name = "ls", names = ["objects"])]
fn interp_ls(
    positional: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let env = context.env();
    let target_env = CallArgs::new(positional, named).environment_or("envir", 0, env)?;

    let names = target_env.ls();
    let chars: Vec<Option<String>> = names.into_iter().map(Some).collect();
    Ok(RValue::vec(Vector::Character(chars.into())))
}

#[interpreter_builtin(name = "eval", min_args = 1)]
fn interp_eval(
    positional: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let env = context.env();
    let call_args = CallArgs::new(positional, named);
    let expr = positional.first().ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "argument 'expr' is missing".to_string(),
        )
    })?;

    let eval_env = call_args.environment_or("envir", 1, env)?;

    match expr {
        // Language object: evaluate the AST
        RValue::Language(ast) => context
            .with_interpreter(|interp| interp.eval_in(ast, &eval_env))
            .map_err(RError::from),
        // Character string: parse then eval
        RValue::Vector(rv) if matches!(rv.inner, Vector::Character(_)) => {
            let text = rv.as_character_scalar().unwrap_or_default();
            let parsed = crate::parser::parse_program(&text)
                .map_err(|e| RError::new(RErrorKind::Parse, format!("{}", e)))?;
            context
                .with_interpreter(|interp| interp.eval_in(&parsed, &eval_env))
                .map_err(RError::from)
        }
        // Already evaluated value: return as-is
        _ => Ok(expr.clone()),
    }
}

#[interpreter_builtin(name = "parse", min_args = 0)]
fn interp_parse(
    positional: &[RValue],
    named: &[(String, RValue)],
    _context: &BuiltinContext,
) -> Result<RValue, RError> {
    let text = named
        .iter()
        .find(|(n, _)| n == "text")
        .and_then(|(_, v)| v.as_vector()?.as_character_scalar())
        .or_else(|| {
            positional
                .first()
                .and_then(|v| v.as_vector()?.as_character_scalar())
        })
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "argument 'text' is missing".to_string(),
            )
        })?;

    let parsed = crate::parser::parse_program(&text)
        .map_err(|e| RError::new(RErrorKind::Parse, format!("{}", e)))?;
    Ok(RValue::Language(Language::new(parsed)))
}

// --- apply family: apply, mapply, tapply, by ---

#[interpreter_builtin(name = "apply", min_args = 3)]
fn interp_apply(
    positional: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let env = context.env();
    let (fail_fast, extra_named) = extract_fail_fast(named);
    let x = positional
        .first()
        .ok_or_else(|| RError::new(RErrorKind::Argument, "argument 'X' is missing".to_string()))?;
    let margin_val = positional.get(1).ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "argument 'MARGIN' is missing".to_string(),
        )
    })?;
    let fun = match_fun(
        positional.get(2).ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "argument 'FUN' is missing".to_string(),
            )
        })?,
        env,
    )?;

    let margin = margin_val
        .as_vector()
        .and_then(|v| v.as_integer_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "MARGIN must be 1 (rows) or 2 (columns) — got a non-integer value".to_string(),
            )
        })?;

    // Extract dim attribute — X must be a matrix
    let (nrow, ncol, data) = match x {
        RValue::Vector(rv) => {
            let dims = super::get_dim_ints(rv.get_attr("dim")).ok_or_else(|| {
                RError::new(
                    RErrorKind::Argument,
                    "X must have a 'dim' attribute (i.e. be a matrix or array). \
                     Use matrix() to create one."
                        .to_string(),
                )
            })?;
            if dims.len() < 2 {
                return Err(RError::new(
                    RErrorKind::Argument,
                    "X must be a 2D matrix for apply() — got an array with fewer than 2 dimensions"
                        .to_string(),
                ));
            }
            let nr = usize::try_from(dims[0].unwrap_or(0))?;
            let nc = usize::try_from(dims[1].unwrap_or(0))?;
            (nr, nc, rv.to_doubles())
        }
        _ => {
            return Err(RError::new(
                RErrorKind::Argument,
                "apply() requires a matrix (vector with dim attribute) as the first argument"
                    .to_string(),
            ))
        }
    };

    // Extra args to pass to FUN (positional args beyond the first 3)
    let extra_args: Vec<RValue> = positional.iter().skip(3).cloned().collect();

    match margin {
        1 => {
            // Apply FUN to each row
            let mut results: Vec<RValue> = Vec::with_capacity(nrow);
            context.with_interpreter(|interp| {
                for i in 0..nrow {
                    let row: Vec<Option<f64>> = (0..ncol).map(|j| data[i + j * nrow]).collect();
                    let row_val = RValue::vec(Vector::Double(row.into()));
                    let mut call_args = vec![row_val];
                    call_args.extend(extra_args.iter().cloned());
                    if fail_fast {
                        let result = interp.call_function(&fun, &call_args, &extra_named, env)?;
                        results.push(result);
                    } else {
                        match interp.call_function(&fun, &call_args, &extra_named, env) {
                            Ok(result) => results.push(result),
                            Err(_) => results.push(RValue::Null),
                        }
                    }
                }
                Ok::<(), RError>(())
            })?;
            simplify_apply_results(results)
        }
        2 => {
            // Apply FUN to each column
            let mut results: Vec<RValue> = Vec::with_capacity(ncol);
            context.with_interpreter(|interp| {
                for j in 0..ncol {
                    let col: Vec<Option<f64>> = (0..nrow).map(|i| data[i + j * nrow]).collect();
                    let col_val = RValue::vec(Vector::Double(col.into()));
                    let mut call_args = vec![col_val];
                    call_args.extend(extra_args.iter().cloned());
                    if fail_fast {
                        let result = interp.call_function(&fun, &call_args, &extra_named, env)?;
                        results.push(result);
                    } else {
                        match interp.call_function(&fun, &call_args, &extra_named, env) {
                            Ok(result) => results.push(result),
                            Err(_) => results.push(RValue::Null),
                        }
                    }
                }
                Ok::<(), RError>(())
            })?;
            simplify_apply_results(results)
        }
        _ => Err(RError::new(
            RErrorKind::Argument,
            format!(
                "MARGIN must be 1 (rows) or 2 (columns) — got {}. \
             Higher-dimensional margins are not yet supported.",
                margin
            ),
        )),
    }
}

/// Simplify apply() results: if all results are scalars, return a vector;
/// if all are equal-length vectors, return a matrix; otherwise return a list.
fn simplify_apply_results(results: Vec<RValue>) -> Result<RValue, RError> {
    if results.is_empty() {
        return Ok(RValue::List(RList::new(vec![])));
    }

    // Check if all results are scalar
    let all_scalar = results.iter().all(|r| r.length() == 1);
    if all_scalar {
        let first_type = results[0].type_name();
        let all_same = results.iter().all(|r| r.type_name() == first_type);
        if all_same {
            match first_type {
                "double" => {
                    let vals: Vec<Option<f64>> = results
                        .iter()
                        .filter_map(|r| {
                            r.as_vector()
                                .map(|v| v.to_doubles().into_iter().next().unwrap_or(None))
                        })
                        .collect();
                    return Ok(RValue::vec(Vector::Double(vals.into())));
                }
                "integer" => {
                    let vals: Vec<Option<i64>> = results
                        .iter()
                        .filter_map(|r| {
                            r.as_vector()
                                .map(|v| v.to_integers().into_iter().next().unwrap_or(None))
                        })
                        .collect();
                    return Ok(RValue::vec(Vector::Integer(vals.into())));
                }
                "character" => {
                    let vals: Vec<Option<String>> = results
                        .iter()
                        .filter_map(|r| {
                            r.as_vector()
                                .map(|v| v.to_characters().into_iter().next().unwrap_or(None))
                        })
                        .collect();
                    return Ok(RValue::vec(Vector::Character(vals.into())));
                }
                "logical" => {
                    let vals: Vec<Option<bool>> = results
                        .iter()
                        .filter_map(|r| {
                            r.as_vector()
                                .map(|v| v.to_logicals().into_iter().next().unwrap_or(None))
                        })
                        .collect();
                    return Ok(RValue::vec(Vector::Logical(vals.into())));
                }
                _ => {}
            }
        }
    }

    // Check if all results are equal-length vectors — return a matrix
    let first_len = results[0].length();
    let all_same_len = first_len > 1 && results.iter().all(|r| r.length() == first_len);
    if all_same_len {
        // Build a matrix: each result becomes a column (R's apply convention)
        let ncol = results.len();
        let nrow = first_len;
        let mut mat_data: Vec<Option<f64>> = Vec::with_capacity(nrow * ncol);
        for result in &results {
            if let Some(v) = result.as_vector() {
                mat_data.extend(v.to_doubles());
            }
        }
        let mut rv = RVector::from(Vector::Double(mat_data.into()));
        rv.set_attr(
            "class".to_string(),
            RValue::vec(Vector::Character(
                vec![Some("matrix".to_string()), Some("array".to_string())].into(),
            )),
        );
        rv.set_attr(
            "dim".to_string(),
            RValue::vec(Vector::Integer(
                vec![Some(i64::try_from(nrow)?), Some(i64::try_from(ncol)?)].into(),
            )),
        );
        return Ok(RValue::Vector(rv));
    }

    // Fall back to a list
    let values: Vec<(Option<String>, RValue)> = results.into_iter().map(|v| (None, v)).collect();
    Ok(RValue::List(RList::new(values)))
}

#[interpreter_builtin(name = "mapply", min_args = 2)]
fn interp_mapply(
    positional: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let env = context.env();
    // mapply(FUN, ..., MoreArgs = NULL, SIMPLIFY = TRUE, USE.NAMES = TRUE)
    let (fail_fast, extra_named) = extract_fail_fast(named);
    let fun = match_fun(
        positional.first().ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "argument 'FUN' is missing".to_string(),
            )
        })?,
        env,
    )?;

    let simplify = extra_named
        .iter()
        .find(|(n, _)| n == "SIMPLIFY")
        .and_then(|(_, v)| v.as_vector()?.as_logical_scalar())
        .unwrap_or(true);

    // Collect the input sequences (all positional args after FUN, excluding named)
    let seqs: Vec<Vec<RValue>> = positional[1..].iter().map(rvalue_to_items).collect();

    if seqs.is_empty() {
        return Ok(RValue::List(RList::new(vec![])));
    }

    // Find the longest sequence for recycling
    let max_len = seqs.iter().map(|s| s.len()).max().unwrap_or(0);

    let mut results: Vec<RValue> = Vec::with_capacity(max_len);

    context.with_interpreter(|interp| {
        for i in 0..max_len {
            let call_args: Vec<RValue> = seqs
                .iter()
                .map(|s| {
                    if s.is_empty() {
                        RValue::Null
                    } else {
                        s[i % s.len()].clone()
                    }
                })
                .collect();
            let result = if fail_fast {
                interp.call_function(&fun, &call_args, &[], env)?
            } else {
                interp
                    .call_function(&fun, &call_args, &[], env)
                    .unwrap_or(RValue::Null)
            };
            results.push(result);
        }
        Ok::<(), RError>(())
    })?;

    if simplify {
        let all_scalar = results.iter().all(|r| r.length() == 1);
        if all_scalar && !results.is_empty() {
            let first_type = results[0].type_name();
            let all_same = results.iter().all(|r| r.type_name() == first_type);
            if all_same {
                match first_type {
                    "double" => {
                        let vals: Vec<Option<f64>> = results
                            .iter()
                            .filter_map(|r| {
                                r.as_vector()
                                    .map(|v| v.to_doubles().into_iter().next().unwrap_or(None))
                            })
                            .collect();
                        return Ok(RValue::vec(Vector::Double(vals.into())));
                    }
                    "integer" => {
                        let vals: Vec<Option<i64>> = results
                            .iter()
                            .filter_map(|r| {
                                r.as_vector()
                                    .map(|v| v.to_integers().into_iter().next().unwrap_or(None))
                            })
                            .collect();
                        return Ok(RValue::vec(Vector::Integer(vals.into())));
                    }
                    "character" => {
                        let vals: Vec<Option<String>> = results
                            .iter()
                            .filter_map(|r| {
                                r.as_vector()
                                    .map(|v| v.to_characters().into_iter().next().unwrap_or(None))
                            })
                            .collect();
                        return Ok(RValue::vec(Vector::Character(vals.into())));
                    }
                    "logical" => {
                        let vals: Vec<Option<bool>> = results
                            .iter()
                            .filter_map(|r| {
                                r.as_vector()
                                    .map(|v| v.to_logicals().into_iter().next().unwrap_or(None))
                            })
                            .collect();
                        return Ok(RValue::vec(Vector::Logical(vals.into())));
                    }
                    _ => {}
                }
            }
        }
    }

    let values: Vec<(Option<String>, RValue)> = results.into_iter().map(|v| (None, v)).collect();
    Ok(RValue::List(RList::new(values)))
}

#[interpreter_builtin(name = "tapply", min_args = 3)]
fn interp_tapply(
    positional: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let env = context.env();
    // tapply(X, INDEX, FUN)
    let (fail_fast, extra_named) = extract_fail_fast(named);
    let x = positional
        .first()
        .ok_or_else(|| RError::new(RErrorKind::Argument, "argument 'X' is missing".to_string()))?;
    let index = positional.get(1).ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "argument 'INDEX' is missing".to_string(),
        )
    })?;
    let fun = match_fun(
        positional.get(2).ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "argument 'FUN' is missing".to_string(),
            )
        })?,
        env,
    )?;

    let x_items = rvalue_to_items(x);
    let index_items = rvalue_to_items(index);

    if x_items.len() != index_items.len() {
        return Err(RError::new(
            RErrorKind::Argument,
            format!(
                "arguments 'X' (length {}) and 'INDEX' (length {}) must have the same length",
                x_items.len(),
                index_items.len()
            ),
        ));
    }

    // Convert index values to string keys for grouping
    let index_keys: Vec<String> = index_items
        .iter()
        .map(|v| match v {
            RValue::Vector(rv) => rv
                .inner
                .as_character_scalar()
                .unwrap_or_else(|| format!("{}", v)),
            _ => format!("{}", v),
        })
        .collect();

    // Collect unique group names preserving first-seen order
    let mut group_names: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for key in &index_keys {
        if seen.insert(key.clone()) {
            group_names.push(key.clone());
        }
    }

    // Group X values by INDEX
    let mut groups: std::collections::HashMap<String, Vec<RValue>> =
        std::collections::HashMap::new();
    for (item, key) in x_items.into_iter().zip(index_keys.iter()) {
        groups.entry(key.clone()).or_default().push(item);
    }

    // Apply FUN to each group
    let mut result_entries: Vec<(Option<String>, RValue)> = Vec::with_capacity(group_names.len());

    context.with_interpreter(|interp| {
        for name in &group_names {
            let group = groups.remove(name).unwrap_or_default();
            let group_vec = combine_items_to_vector(&group);
            if fail_fast {
                let result = interp.call_function(&fun, &[group_vec], &extra_named, env)?;
                result_entries.push((Some(name.clone()), result));
            } else {
                match interp.call_function(&fun, &[group_vec], &extra_named, env) {
                    Ok(result) => result_entries.push((Some(name.clone()), result)),
                    Err(_) => result_entries.push((Some(name.clone()), RValue::Null)),
                }
            }
        }
        Ok::<(), RError>(())
    })?;

    // Try to simplify to a named vector if all results are scalar
    let all_scalar = result_entries.iter().all(|(_, v)| v.length() == 1);
    if all_scalar && !result_entries.is_empty() {
        let first_type = result_entries[0].1.type_name();
        let all_same = result_entries
            .iter()
            .all(|(_, v)| v.type_name() == first_type);
        if all_same {
            let names: Vec<Option<String>> =
                result_entries.iter().map(|(n, _)| n.clone()).collect();
            match first_type {
                "double" => {
                    let vals: Vec<Option<f64>> = result_entries
                        .iter()
                        .filter_map(|(_, r)| {
                            r.as_vector()
                                .map(|v| v.to_doubles().into_iter().next().unwrap_or(None))
                        })
                        .collect();
                    let mut rv = RVector::from(Vector::Double(vals.into()));
                    rv.set_attr(
                        "names".to_string(),
                        RValue::vec(Vector::Character(names.into())),
                    );
                    return Ok(RValue::Vector(rv));
                }
                "integer" => {
                    let vals: Vec<Option<i64>> = result_entries
                        .iter()
                        .filter_map(|(_, r)| {
                            r.as_vector()
                                .map(|v| v.to_integers().into_iter().next().unwrap_or(None))
                        })
                        .collect();
                    let mut rv = RVector::from(Vector::Integer(vals.into()));
                    rv.set_attr(
                        "names".to_string(),
                        RValue::vec(Vector::Character(names.into())),
                    );
                    return Ok(RValue::Vector(rv));
                }
                "character" => {
                    let vals: Vec<Option<String>> = result_entries
                        .iter()
                        .filter_map(|(_, r)| {
                            r.as_vector()
                                .map(|v| v.to_characters().into_iter().next().unwrap_or(None))
                        })
                        .collect();
                    let mut rv = RVector::from(Vector::Character(vals.into()));
                    rv.set_attr(
                        "names".to_string(),
                        RValue::vec(Vector::Character(names.into())),
                    );
                    return Ok(RValue::Vector(rv));
                }
                "logical" => {
                    let vals: Vec<Option<bool>> = result_entries
                        .iter()
                        .filter_map(|(_, r)| {
                            r.as_vector()
                                .map(|v| v.to_logicals().into_iter().next().unwrap_or(None))
                        })
                        .collect();
                    let mut rv = RVector::from(Vector::Logical(vals.into()));
                    rv.set_attr(
                        "names".to_string(),
                        RValue::vec(Vector::Character(names.into())),
                    );
                    return Ok(RValue::Vector(rv));
                }
                _ => {}
            }
        }
    }

    Ok(RValue::List(RList::new(result_entries)))
}

/// Combine a list of scalar RValues back into a single vector RValue.
fn combine_items_to_vector(items: &[RValue]) -> RValue {
    if items.is_empty() {
        return RValue::Null;
    }

    // Determine the type from the first element
    let first_type = items[0].type_name();
    let all_same = items.iter().all(|v| v.type_name() == first_type);

    if all_same {
        match first_type {
            "double" => {
                let vals: Vec<Option<f64>> = items
                    .iter()
                    .flat_map(|r| {
                        r.as_vector()
                            .map(|v| v.to_doubles())
                            .unwrap_or_else(|| vec![None])
                    })
                    .collect();
                RValue::vec(Vector::Double(vals.into()))
            }
            "integer" => {
                let vals: Vec<Option<i64>> = items
                    .iter()
                    .flat_map(|r| {
                        r.as_vector()
                            .map(|v| v.to_integers())
                            .unwrap_or_else(|| vec![None])
                    })
                    .collect();
                RValue::vec(Vector::Integer(vals.into()))
            }
            "character" => {
                let vals: Vec<Option<String>> = items
                    .iter()
                    .flat_map(|r| {
                        r.as_vector()
                            .map(|v| v.to_characters())
                            .unwrap_or_else(|| vec![None])
                    })
                    .collect();
                RValue::vec(Vector::Character(vals.into()))
            }
            "logical" => {
                let vals: Vec<Option<bool>> = items
                    .iter()
                    .flat_map(|r| {
                        r.as_vector()
                            .map(|v| v.to_logicals())
                            .unwrap_or_else(|| vec![None])
                    })
                    .collect();
                RValue::vec(Vector::Logical(vals.into()))
            }
            _ => {
                // Fall back to coercing to doubles
                let vals: Vec<Option<f64>> = items
                    .iter()
                    .flat_map(|r| {
                        r.as_vector()
                            .map(|v| v.to_doubles())
                            .unwrap_or_else(|| vec![None])
                    })
                    .collect();
                RValue::vec(Vector::Double(vals.into()))
            }
        }
    } else {
        // Mixed types: coerce to doubles (R's coercion hierarchy)
        let vals: Vec<Option<f64>> = items
            .iter()
            .flat_map(|r| {
                r.as_vector()
                    .map(|v| v.to_doubles())
                    .unwrap_or_else(|| vec![None])
            })
            .collect();
        RValue::vec(Vector::Double(vals.into()))
    }
}

#[interpreter_builtin(name = "by", min_args = 3)]
fn interp_by(
    positional: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let env = context.env();
    // by(data, INDICES, FUN) — similar to tapply but for data-frame-like objects.
    let (fail_fast, extra_named) = extract_fail_fast(named);
    // For vectors, delegate to tapply-like behavior.
    // For lists/data frames, split rows by INDICES and apply FUN to each subset.
    let data = positional.first().ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "argument 'data' is missing".to_string(),
        )
    })?;
    let indices = positional.get(1).ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "argument 'INDICES' is missing".to_string(),
        )
    })?;
    let fun = match_fun(
        positional.get(2).ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "argument 'FUN' is missing".to_string(),
            )
        })?,
        env,
    )?;

    // For atomic vectors, treat like tapply
    if matches!(data, RValue::Vector(_)) {
        let x_items = rvalue_to_items(data);
        let index_items = rvalue_to_items(indices);

        if x_items.len() != index_items.len() {
            return Err(RError::new(
                RErrorKind::Argument,
                format!(
                "arguments 'data' (length {}) and 'INDICES' (length {}) must have the same length",
                x_items.len(),
                index_items.len()
            ),
            ));
        }

        let index_keys: Vec<String> = index_items
            .iter()
            .map(|v| match v {
                RValue::Vector(rv) => rv
                    .inner
                    .as_character_scalar()
                    .unwrap_or_else(|| format!("{}", v)),
                _ => format!("{}", v),
            })
            .collect();

        let mut group_names: Vec<String> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for key in &index_keys {
            if seen.insert(key.clone()) {
                group_names.push(key.clone());
            }
        }

        let mut groups: std::collections::HashMap<String, Vec<RValue>> =
            std::collections::HashMap::new();
        for (item, key) in x_items.into_iter().zip(index_keys.iter()) {
            groups.entry(key.clone()).or_default().push(item);
        }

        let mut result_entries: Vec<(Option<String>, RValue)> =
            Vec::with_capacity(group_names.len());

        context.with_interpreter(|interp| {
            for name in &group_names {
                let group = groups.remove(name).unwrap_or_default();
                let group_vec = combine_items_to_vector(&group);
                if fail_fast {
                    let result = interp.call_function(&fun, &[group_vec], &extra_named, env)?;
                    result_entries.push((Some(name.clone()), result));
                } else {
                    match interp.call_function(&fun, &[group_vec], &extra_named, env) {
                        Ok(result) => result_entries.push((Some(name.clone()), result)),
                        Err(_) => result_entries.push((Some(name.clone()), RValue::Null)),
                    }
                }
            }
            Ok::<(), RError>(())
        })?;

        return Ok(RValue::List(RList::new(result_entries)));
    }

    // For lists (including data frames), split by INDICES and apply FUN
    if let RValue::List(list) = data {
        let index_items = rvalue_to_items(indices);

        // For a data frame, determine nrow from the first column
        let nrow = list.values.first().map(|(_, v)| v.length()).unwrap_or(0);

        if index_items.len() != nrow {
            return Err(RError::new(
                RErrorKind::Argument,
                format!(
                "arguments 'data' ({} rows) and 'INDICES' (length {}) must have the same length",
                nrow,
                index_items.len()
            ),
            ));
        }

        let index_keys: Vec<String> = index_items
            .iter()
            .map(|v| match v {
                RValue::Vector(rv) => rv
                    .inner
                    .as_character_scalar()
                    .unwrap_or_else(|| format!("{}", v)),
                _ => format!("{}", v),
            })
            .collect();

        let mut group_names: Vec<String> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for key in &index_keys {
            if seen.insert(key.clone()) {
                group_names.push(key.clone());
            }
        }

        // For each group, build a subset data frame and call FUN
        let mut result_entries: Vec<(Option<String>, RValue)> =
            Vec::with_capacity(group_names.len());

        context.with_interpreter(|interp| {
            for name in &group_names {
                // Find row indices belonging to this group
                let row_indices: Vec<usize> = index_keys
                    .iter()
                    .enumerate()
                    .filter(|(_, k)| k.as_str() == name)
                    .map(|(i, _)| i)
                    .collect();

                // Build a subset list (data frame) with only these rows
                let mut subset_cols: Vec<(Option<String>, RValue)> = Vec::new();
                for (col_name, col_val) in &list.values {
                    let col_items = rvalue_to_items(col_val);
                    let subset: Vec<RValue> = row_indices
                        .iter()
                        .filter_map(|&i| col_items.get(i).cloned())
                        .collect();
                    let subset_vec = combine_items_to_vector(&subset);
                    subset_cols.push((col_name.clone(), subset_vec));
                }

                let mut subset_list = RList::new(subset_cols);
                // Preserve data.frame class if the original had it
                if let Some(cls) = list.get_attr("class") {
                    subset_list.set_attr("class".to_string(), cls.clone());
                }
                // Set row.names for the subset
                let row_names: Vec<Option<i64>> =
                    (1..=i64::try_from(row_indices.len())?).map(Some).collect();
                subset_list.set_attr(
                    "row.names".to_string(),
                    RValue::vec(Vector::Integer(row_names.into())),
                );
                // Set names attribute
                if let Some(names) = list.get_attr("names") {
                    subset_list.set_attr("names".to_string(), names.clone());
                }

                let subset_val = RValue::List(subset_list);
                if fail_fast {
                    let result = interp.call_function(&fun, &[subset_val], &extra_named, env)?;
                    result_entries.push((Some(name.clone()), result));
                } else {
                    match interp.call_function(&fun, &[subset_val], &extra_named, env) {
                        Ok(result) => result_entries.push((Some(name.clone()), result)),
                        Err(_) => result_entries.push((Some(name.clone()), RValue::Null)),
                    }
                }
            }
            Ok::<(), RError>(())
        })?;

        return Ok(RValue::List(RList::new(result_entries)));
    }

    Err(RError::new(
        RErrorKind::Argument,
        "by() requires a vector, list, or data frame as 'data'".to_string(),
    ))
}
