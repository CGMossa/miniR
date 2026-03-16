//! Interpreter-level builtins — functions that receive `BuiltinContext` so
//! they can call back into the active interpreter without direct TLS lookups.
//! Each is auto-registered via `#[interpreter_builtin]`.

use super::CallArgs;
use crate::interpreter::environment::Environment;
use crate::interpreter::value::*;
use crate::interpreter::BuiltinContext;
use crate::parser::ast::{Arg, BinaryOp, Expr, Param, UnaryOp};
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

/// Print a value to stdout (S3 generic).
///
/// @param x the value to print
/// @return x, invisibly
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

/// Format a value as a character string (S3 generic).
///
/// @param x the value to format
/// @return character string representation
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

/// Apply a function over a vector or list, simplifying the result.
///
/// @param X vector or list to iterate over
/// @param FUN function to apply to each element
/// @return simplified vector or list of results
#[interpreter_builtin(name = "sapply", min_args = 2)]
fn interp_sapply(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    eval_apply(args, named, true, context)
}

/// Apply a function over a vector or list, returning a list.
///
/// @param X vector or list to iterate over
/// @param FUN function to apply to each element
/// @return list of results
#[interpreter_builtin(name = "lapply", min_args = 2)]
fn interp_lapply(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    eval_apply(args, named, false, context)
}

/// Apply a function over a vector or list with a type-checked return template.
///
/// @param X vector or list to iterate over
/// @param FUN function to apply to each element
/// @param FUN.VALUE template value specifying the expected return type
/// @return simplified vector matching FUN.VALUE type
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

/// Call a function with arguments supplied as a list.
///
/// Named elements in the list are passed as named arguments to the function.
/// The `quote` parameter is accepted but currently ignored (all args are already
/// evaluated when passed via a list).
///
/// @param what function or character string naming the function
/// @param args list of arguments to pass to the function
/// @param quote logical: whether to quote arguments (default FALSE, accepted but ignored)
/// @return the result of the function call
#[interpreter_builtin(name = "do.call", min_args = 2)]
fn interp_do_call(
    positional: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    // Filter out the `quote` and `envir` named args (they are for do.call itself)
    let extra_named: Vec<(String, RValue)> = named
        .iter()
        .filter(|(n, _)| n != "quote" && n != "envir")
        .cloned()
        .collect();

    let env = context.env();
    if positional.len() >= 2 {
        let f = match_fun(&positional[0], env)?;
        return context.with_interpreter(|interp| match &positional[1] {
            RValue::List(l) => {
                // Separate named and positional args from the list
                let mut pos_args: Vec<RValue> = Vec::new();
                let mut named_args: Vec<(String, RValue)> = extra_named;
                for (name, val) in &l.values {
                    match name {
                        Some(n) if !n.is_empty() => named_args.push((n.clone(), val.clone())),
                        _ => pos_args.push(val.clone()),
                    }
                }
                interp
                    .call_function(&f, &pos_args, &named_args, env)
                    .map_err(RError::from)
            }
            _ => interp
                .call_function(&f, &positional[1..], &extra_named, env)
                .map_err(RError::from),
        });
    }
    Err(RError::new(
        RErrorKind::Argument,
        "do.call requires at least 2 arguments".to_string(),
    ))
}

/// Create a vectorized version of a function (stub: returns FUN unchanged).
///
/// @param FUN function to vectorize
/// @return the function (currently a no-op pass-through)
#[interpreter_builtin(name = "Vectorize", min_args = 1)]
fn interp_vectorize(
    positional: &[RValue],
    _named: &[(String, RValue)],
    _context: &BuiltinContext,
) -> Result<RValue, RError> {
    Ok(positional.first().cloned().unwrap_or(RValue::Null))
}

/// Reduce a vector or list to a single value by applying a binary function.
///
/// @param f binary function taking two arguments
/// @param x vector or list to reduce
/// @param init optional initial value for the accumulator
/// @param accumulate if TRUE, return all intermediate results
/// @return the final accumulated value, or a list of intermediate values if accumulate=TRUE
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

/// Select elements of a vector or list for which a predicate returns TRUE.
///
/// @param f predicate function returning a logical scalar
/// @param x vector or list to filter
/// @return elements of x for which f returns TRUE
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

/// Apply a function to corresponding elements of multiple vectors or lists.
///
/// @param f function to apply
/// @param ... vectors or lists to map over in parallel
/// @return list of results
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

/// Select one of several alternatives based on an expression value.
///
/// @param EXPR expression to evaluate; character for named matching, integer for positional
/// @param ... named alternatives (for character EXPR) or positional alternatives (for integer)
/// @return the value of the matched alternative, or NULL if none match
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

/// Look up a variable by name in an environment.
///
/// @param x character string giving the variable name
/// @param envir environment in which to look up the variable (default: calling environment)
/// @return the value bound to the name
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

/// Assign a value to a variable name in an environment.
///
/// @param x character string giving the variable name
/// @param value the value to assign
/// @param envir environment in which to make the assignment (default: calling environment)
/// @return the assigned value, invisibly
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

/// Test whether a variable exists in an environment.
///
/// @param x character string giving the variable name
/// @param envir environment to search in (default: calling environment)
/// @return TRUE if the variable exists, FALSE otherwise
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

/// Read and evaluate an R source file.
///
/// @param file path to the R source file
/// @return the result of evaluating the last expression in the file
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

/// Read and evaluate an R source file in a specified environment.
///
/// Like `source()`, but evaluates the expressions in the given environment
/// rather than the global environment. This is useful for loading code into
/// a specific namespace or local environment.
///
/// @param file path to the R source file
/// @param envir environment in which to evaluate (default: base environment)
/// @return the result of evaluating the last expression in the file (invisibly)
#[interpreter_builtin(name = "sys.source", min_args = 1)]
fn interp_sys_source(
    positional: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let path = positional
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| RError::new(RErrorKind::Argument, "invalid 'file' argument".to_string()))?;

    // Get environment from named 'envir' argument or second positional
    let env = named
        .iter()
        .find(|(n, _)| n == "envir")
        .map(|(_, v)| v)
        .or_else(|| positional.get(1));

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

    match env {
        Some(RValue::Environment(target_env)) => context
            .with_interpreter(|interp| interp.eval_in(&ast, target_env).map_err(RError::from)),
        _ => context.with_interpreter(|interp| interp.eval(&ast).map_err(RError::from)),
    }
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

/// Addition operator as a function (unary positive or binary addition).
///
/// @param e1 first operand (or sole operand for unary +)
/// @param e2 second operand (optional)
/// @return sum of e1 and e2, or e1 unchanged for unary +
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

/// Subtraction operator as a function (unary negation or binary subtraction).
///
/// @param e1 first operand (or sole operand for unary -)
/// @param e2 second operand (optional)
/// @return difference of e1 and e2, or negation of e1 for unary -
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

/// Multiplication operator as a function.
///
/// @param e1 first operand
/// @param e2 second operand
/// @return product of e1 and e2
#[interpreter_builtin(name = "*", min_args = 2)]
fn interp_op_mul(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    eval_binop(BinaryOp::Mul, args, context)
}

/// Division operator as a function.
///
/// @param e1 numerator
/// @param e2 denominator
/// @return quotient of e1 and e2
#[interpreter_builtin(name = "/", min_args = 2)]
fn interp_op_div(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    eval_binop(BinaryOp::Div, args, context)
}

/// Exponentiation operator as a function.
///
/// @param e1 base
/// @param e2 exponent
/// @return e1 raised to the power of e2
#[interpreter_builtin(name = "^", min_args = 2)]
fn interp_op_pow(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    eval_binop(BinaryOp::Pow, args, context)
}

/// Modulo operator as a function.
///
/// @param e1 dividend
/// @param e2 divisor
/// @return remainder of e1 divided by e2
#[interpreter_builtin(name = "%%", min_args = 2)]
fn interp_op_mod(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    eval_binop(BinaryOp::Mod, args, context)
}

/// Integer division operator as a function.
///
/// @param e1 dividend
/// @param e2 divisor
/// @return integer quotient of e1 divided by e2
#[interpreter_builtin(name = "%/%", min_args = 2)]
fn interp_op_intdiv(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    eval_binop(BinaryOp::IntDiv, args, context)
}

/// Equality comparison operator as a function.
///
/// @param e1 first operand
/// @param e2 second operand
/// @return logical vector indicating element-wise equality
#[interpreter_builtin(name = "==", min_args = 2)]
fn interp_op_eq(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    eval_binop(BinaryOp::Eq, args, context)
}

/// Inequality comparison operator as a function.
///
/// @param e1 first operand
/// @param e2 second operand
/// @return logical vector indicating element-wise inequality
#[interpreter_builtin(name = "!=", min_args = 2)]
fn interp_op_ne(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    eval_binop(BinaryOp::Ne, args, context)
}

/// Less-than comparison operator as a function.
///
/// @param e1 first operand
/// @param e2 second operand
/// @return logical vector indicating element-wise less-than
#[interpreter_builtin(name = "<", min_args = 2)]
fn interp_op_lt(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    eval_binop(BinaryOp::Lt, args, context)
}

/// Greater-than comparison operator as a function.
///
/// @param e1 first operand
/// @param e2 second operand
/// @return logical vector indicating element-wise greater-than
#[interpreter_builtin(name = ">", min_args = 2)]
fn interp_op_gt(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    eval_binop(BinaryOp::Gt, args, context)
}

/// Less-than-or-equal comparison operator as a function.
///
/// @param e1 first operand
/// @param e2 second operand
/// @return logical vector indicating element-wise less-than-or-equal
#[interpreter_builtin(name = "<=", min_args = 2)]
fn interp_op_le(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    eval_binop(BinaryOp::Le, args, context)
}

/// Greater-than-or-equal comparison operator as a function.
///
/// @param e1 first operand
/// @param e2 second operand
/// @return logical vector indicating element-wise greater-than-or-equal
#[interpreter_builtin(name = ">=", min_args = 2)]
fn interp_op_ge(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    eval_binop(BinaryOp::Ge, args, context)
}

/// Element-wise logical AND operator as a function.
///
/// @param e1 first logical operand
/// @param e2 second logical operand
/// @return logical vector of element-wise AND results
#[interpreter_builtin(name = "&", min_args = 2)]
fn interp_op_and(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    eval_binop(BinaryOp::And, args, context)
}

/// Element-wise logical OR operator as a function.
///
/// @param e1 first logical operand
/// @param e2 second logical operand
/// @return logical vector of element-wise OR results
#[interpreter_builtin(name = "|", min_args = 2)]
fn interp_op_or(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    eval_binop(BinaryOp::Or, args, context)
}

/// Logical NOT operator as a function.
///
/// @param x logical operand
/// @return logical vector of negated values
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

/// Invoke the next method in an S3 method dispatch chain.
///
/// @param generic character string naming the generic (optional, inferred from context)
/// @param object the object being dispatched on (optional, inferred from context)
/// @return the result of calling the next method
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

/// Get or query the environment of a function.
///
/// @param fun function whose environment to return (optional; returns calling env if omitted)
/// @return the environment of fun, or the calling environment if no argument given
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

/// Coerce a value to an environment.
///
/// @param x integer (search path position), string (environment name), or environment
/// @return the corresponding environment
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

/// Return the global environment.
///
/// @return the global environment
#[interpreter_builtin(name = "globalenv", max_args = 0)]
fn interp_globalenv(
    _positional: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    context.with_interpreter(|interp| Ok(RValue::Environment(interp.global_env.clone())))
}

/// Return the base environment.
///
/// @return the base environment (parent of the global environment)
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

/// Return the empty environment (has no parent and no bindings).
///
/// @return the empty environment
#[interpreter_builtin(name = "emptyenv", max_args = 0)]
fn interp_emptyenv(
    _positional: &[RValue],
    _named: &[(String, RValue)],
    _context: &BuiltinContext,
) -> Result<RValue, RError> {
    Ok(RValue::Environment(Environment::new_empty()))
}

/// Get the call expression of a frame on the call stack.
///
/// @param which frame number (0 = current, positive = counting from bottom)
/// @return the call as a language object, or NULL
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

/// Get the function of a frame on the call stack.
///
/// @param which frame number (0 = current, positive = counting from bottom)
/// @return the function object for the given frame
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

/// Get the environment of a frame on the call stack.
///
/// @param which frame number (0 = global env, positive = counting from bottom)
/// @return the environment for the given frame
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

/// Get the list of all calls on the call stack.
///
/// @return list of call language objects for all active frames
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

/// Get the list of all environments on the call stack.
///
/// @return list of environments for all active frames
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

/// Get the parent frame indices for all frames on the call stack.
///
/// @return integer vector of parent frame indices
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

/// Get the on.exit expression for the current frame.
///
/// @return the on.exit expression as a language object, or NULL if none
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

/// Get the number of frames on the call stack.
///
/// @return integer giving the current stack depth
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

/// Get the number of arguments supplied to the current function call.
///
/// @return integer giving the number of supplied arguments
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

/// Get the environment of the parent (calling) frame.
///
/// @param n number of generations to go back (default 1)
/// @return the environment of the n-th parent frame
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

/// List the names of objects in an environment.
///
/// @param envir environment to list (default: calling environment)
/// @return character vector of variable names
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

/// Evaluate an expression in a specified environment.
///
/// @param expr expression to evaluate (language object or character string)
/// @param envir environment in which to evaluate (default: calling environment)
/// @return the result of evaluating expr
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

/// Parse R source text into a language object.
///
/// @param text character string containing R code to parse
/// @return a language object representing the parsed expression
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

/// Apply a function over rows or columns of a matrix.
///
/// @param X matrix or array
/// @param MARGIN 1 for rows, 2 for columns
/// @param FUN function to apply
/// @param ... additional arguments passed to FUN
/// @return vector, matrix, or list of results
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

/// Apply a function to corresponding elements of multiple vectors.
///
/// @param FUN function to apply
/// @param ... vectors to iterate over in parallel
/// @param SIMPLIFY if TRUE, simplify the result to a vector or matrix
/// @return simplified vector or list of results
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

/// Apply a function to groups of values defined by a factor/index.
///
/// @param X vector of values to split into groups
/// @param INDEX factor or vector defining the groups
/// @param FUN function to apply to each group
/// @return named vector or list of per-group results
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

/// Apply a function to subsets of a data frame or vector split by a grouping factor.
///
/// @param data data frame or vector to split
/// @param INDICES factor or vector defining the groups
/// @param FUN function to apply to each subset
/// @return list of per-group results
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

/// Summarize an object (S3 generic).
///
/// Dispatches to summary.lm, summary.data.frame, etc. when a method exists.
/// Falls back to printing the object's structure.
///
/// @param object the object to summarize
/// @return a summary of the object
#[interpreter_builtin(min_args = 1)]
fn interp_summary(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    // Try S3 dispatch (summary.lm, summary.data.frame, etc.)
    if let Some(result) = try_s3_dispatch("summary", args, named, context)? {
        return Ok(result);
    }
    // Default: for vectors, compute basic summary statistics
    match args.first() {
        Some(RValue::Vector(rv)) => {
            let doubles = rv.to_doubles();
            let vals: Vec<f64> = doubles.into_iter().flatten().collect();
            if vals.is_empty() {
                return Ok(RValue::Null);
            }
            let min = vals.iter().copied().fold(f64::INFINITY, f64::min);
            let max = vals.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            let sum: f64 = vals.iter().sum();
            let mean = sum / vals.len() as f64;
            let mut sorted = vals;
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let median = if sorted.len().is_multiple_of(2) {
                (sorted[sorted.len() / 2 - 1] + sorted[sorted.len() / 2]) / 2.0
            } else {
                sorted[sorted.len() / 2]
            };

            let mut result_rv = RVector::from(Vector::Double(
                vec![Some(min), Some(median), Some(mean), Some(max)].into(),
            ));
            result_rv.set_attr(
                "names".to_string(),
                RValue::vec(Vector::Character(
                    vec![
                        Some("Min.".to_string()),
                        Some("Median".to_string()),
                        Some("Mean".to_string()),
                        Some("Max.".to_string()),
                    ]
                    .into(),
                )),
            );
            Ok(RValue::Vector(result_rv))
        }
        Some(other) => Ok(other.clone()),
        None => Ok(RValue::Null),
    }
}

// region: reg.finalizer

/// Register a function to be called when an environment is garbage collected,
/// or at interpreter exit if `onexit = TRUE`.
///
/// Since miniR uses Rc-based environments (no tracing GC), finalizers with
/// `onexit = FALSE` are accepted silently but will never fire. When
/// `onexit = TRUE`, the finalizer is stored on the Interpreter and executed
/// during its Drop.
///
/// @param e an environment to attach the finalizer to
/// @param f a function of one argument (the environment) to call
/// @param onexit logical; if TRUE, run the finalizer at interpreter exit
/// @return NULL, invisibly
#[interpreter_builtin(name = "reg.finalizer", min_args = 2, max_args = 3)]
fn interp_reg_finalizer(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);

    // e — must be an environment
    let e = call_args.value("e", 0).ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "reg.finalizer() requires an environment as its first argument".to_string(),
        )
    })?;
    if !matches!(e, RValue::Environment(_)) {
        return Err(RError::new(
            RErrorKind::Argument,
            "reg.finalizer() requires an environment as its first argument".to_string(),
        ));
    }

    // f — must be a function
    let f = call_args.value("f", 1).ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "reg.finalizer() requires a function as its second argument".to_string(),
        )
    })?;
    let f = match_fun(f, context.env())?;

    // onexit — logical, default FALSE
    let onexit = call_args.logical_flag("onexit", 2, false);

    if onexit {
        context.with_interpreter(|interp| {
            interp.finalizers.borrow_mut().push(f);
        });
    }
    // When onexit is FALSE, we accept silently — no GC means it won't fire,
    // but it shouldn't error either.

    Ok(RValue::Null)
}

// endregion

// region: options

/// Get or set global options.
///
/// With no arguments, returns all current options as a named list.
/// With character arguments, returns the named options.
/// With name=value pairs, sets those options and returns the previous values.
///
/// @param ... option names to query, or name=value pairs to set
/// @return list of (previous) option values
#[interpreter_builtin]
fn interp_options(
    positional: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    context.with_interpreter(|interp| {
        let mut result: Vec<(Option<String>, RValue)> = Vec::new();

        // If no arguments, return all options
        if positional.is_empty() && named.is_empty() {
            let opts = interp.options.borrow();
            let mut entries: Vec<_> = opts.iter().collect();
            entries.sort_by_key(|(k, _)| (*k).clone());
            for (k, v) in entries {
                result.push((Some(k.clone()), v.clone()));
            }
            return Ok(RValue::List(RList::new(result)));
        }

        // Process positional args — character strings are queries
        for arg in positional {
            if let Some(name) = arg.as_vector().and_then(|v| v.as_character_scalar()) {
                let val = interp
                    .options
                    .borrow()
                    .get(&name)
                    .cloned()
                    .unwrap_or(RValue::Null);
                result.push((Some(name), val));
            } else if let RValue::List(list) = arg {
                // Setting options from a list (e.g. options(old_opts))
                for (opt_name, val) in &list.values {
                    if let Some(opt_name) = opt_name {
                        let prev = interp
                            .options
                            .borrow()
                            .get(opt_name.as_str())
                            .cloned()
                            .unwrap_or(RValue::Null);
                        interp
                            .options
                            .borrow_mut()
                            .insert(opt_name.clone(), val.clone());
                        result.push((Some(opt_name.clone()), prev));
                    }
                }
            }
        }

        // Process named args — these are set operations
        for (name, val) in named {
            let prev = interp
                .options
                .borrow()
                .get(name)
                .cloned()
                .unwrap_or(RValue::Null);
            interp
                .options
                .borrow_mut()
                .insert(name.clone(), val.clone());
            result.push((Some(name.clone()), prev));
        }

        Ok(RValue::List(RList::new(result)))
    })
}

/// Get the value of a named global option.
///
/// @param name character string — the option name
/// @param default value to return if the option is not set (default NULL)
/// @return the option value, or default if not set
#[interpreter_builtin(name = "getOption", min_args = 1)]
fn interp_get_option(
    positional: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let name = positional
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "getOption() requires a character string as its first argument".to_string(),
            )
        })?;
    let default = positional.get(1).cloned().unwrap_or(RValue::Null);

    context.with_interpreter(|interp| {
        Ok(interp
            .options
            .borrow()
            .get(&name)
            .cloned()
            .unwrap_or(default))
    })
}

// endregion

// region: match.call, Find, Position, Negate, rapply

/// Return the call expression with arguments matched to formal parameters.
///
/// Reconstructs the call as if all arguments were named according to the
/// function's formal parameter list. Useful for programming on the language.
///
/// @param definition the function whose formals to match against (default: parent function)
/// @param call the call to match (default: parent's call)
/// @return language object with matched arguments
#[interpreter_builtin(name = "match.call")]
fn interp_match_call(
    _positional: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    context.with_interpreter(|interp| {
        let frame = interp
            .current_call_frame()
            .ok_or_else(|| RError::other("match.call() must be called from within a function"))?;

        // Get the formals from the function
        let params: Vec<Param> = match &frame.function {
            RValue::Function(RFunction::Closure { params, .. }) => params.clone(),
            _ => Vec::new(),
        };

        // Get the original call expression
        let call_expr = frame
            .call
            .ok_or_else(|| RError::other("match.call() requires a call expression on the stack"))?;

        // Extract the function name from the call
        let func_expr = match &call_expr {
            Expr::Call { func, .. } => (**func).clone(),
            _ => return Ok(RValue::Language(Language::new(call_expr))),
        };

        // Reconstruct with matched argument names
        let positional = &frame.supplied_positional;
        let named = &frame.supplied_named;

        // Simplified 3-pass matching to figure out which positional maps to which formal
        let formal_names: Vec<&str> = params
            .iter()
            .filter(|p| !p.is_dots)
            .map(|p| p.name.as_str())
            .collect();

        let mut named_to_formal: std::collections::HashMap<usize, &str> =
            std::collections::HashMap::new();
        let mut matched_formals: std::collections::HashSet<&str> = std::collections::HashSet::new();

        // Pass 1: exact name match
        for (i, (arg_name, _)) in named.iter().enumerate() {
            if let Some(&formal) = formal_names.iter().find(|&&f| f == arg_name) {
                if !matched_formals.contains(formal) {
                    matched_formals.insert(formal);
                    named_to_formal.insert(i, formal);
                }
            }
        }

        // Pass 2: partial match
        for (i, (arg_name, _)) in named.iter().enumerate() {
            if named_to_formal.contains_key(&i) {
                continue;
            }
            let candidates: Vec<&str> = formal_names
                .iter()
                .filter(|&&f| !matched_formals.contains(f) && f.starts_with(arg_name.as_str()))
                .copied()
                .collect();
            if candidates.len() == 1 {
                matched_formals.insert(candidates[0]);
                named_to_formal.insert(i, candidates[0]);
            }
        }

        // Build reverse map
        let formal_to_named: std::collections::HashMap<&str, usize> = named_to_formal
            .iter()
            .map(|(&idx, &formal)| (formal, idx))
            .collect();

        // Reconstruct args in formal order
        let mut result_args: Vec<Arg> = Vec::new();
        let mut pos_idx = 0usize;

        for param in &params {
            if param.is_dots {
                // Collect remaining positional
                while pos_idx < positional.len() {
                    result_args.push(Arg {
                        name: None,
                        value: Some(rvalue_to_expr(&positional[pos_idx])),
                    });
                    pos_idx += 1;
                }
                // Collect unmatched named
                for (i, (name, val)) in named.iter().enumerate() {
                    if !named_to_formal.contains_key(&i) {
                        result_args.push(Arg {
                            name: Some(name.clone()),
                            value: Some(rvalue_to_expr(val)),
                        });
                    }
                }
                continue;
            }

            if let Some(&named_idx) = formal_to_named.get(param.name.as_str()) {
                result_args.push(Arg {
                    name: Some(param.name.clone()),
                    value: Some(rvalue_to_expr(&named[named_idx].1)),
                });
            } else if pos_idx < positional.len() {
                result_args.push(Arg {
                    name: Some(param.name.clone()),
                    value: Some(rvalue_to_expr(&positional[pos_idx])),
                });
                pos_idx += 1;
            }
            // Skip unmatched formals with defaults
        }

        let matched_call = Expr::Call {
            func: Box::new(func_expr),
            args: result_args,
        };
        Ok(RValue::Language(Language::new(matched_call)))
    })
}

/// Convert an RValue to an Expr for use in match.call() reconstructed calls.
fn rvalue_to_expr(val: &RValue) -> Expr {
    match val {
        RValue::Null => Expr::Null,
        RValue::Vector(rv) => match &rv.inner {
            Vector::Double(d) if d.len() == 1 => match d[0] {
                Some(v) if v.is_infinite() && v > 0.0 => Expr::Inf,
                Some(v) if v.is_nan() => Expr::NaN,
                Some(v) => Expr::Double(v),
                None => Expr::Na(crate::parser::ast::NaType::Real),
            },
            Vector::Integer(i) if i.len() == 1 => match i[0] {
                Some(v) => Expr::Integer(v),
                None => Expr::Na(crate::parser::ast::NaType::Integer),
            },
            Vector::Logical(l) if l.len() == 1 => match l[0] {
                Some(v) => Expr::Bool(v),
                None => Expr::Na(crate::parser::ast::NaType::Logical),
            },
            Vector::Character(c) if c.len() == 1 => match &c[0] {
                Some(v) => Expr::String(v.clone()),
                None => Expr::Na(crate::parser::ast::NaType::Character),
            },
            _ => Expr::Symbol(format!("{}", val)),
        },
        RValue::Language(lang) => (*lang.inner).clone(),
        _ => Expr::Symbol(format!("{}", val)),
    }
}

/// Find the first element of a vector for which a predicate returns TRUE.
///
/// @param f predicate function returning a logical scalar
/// @param x vector or list to search
/// @param right if TRUE, search from right to left
/// @return the first matching element, or NULL if none found
#[interpreter_builtin(name = "Find", min_args = 2)]
fn interp_find(
    positional: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    if positional.len() < 2 {
        return Err(RError::new(
            RErrorKind::Argument,
            "Find requires 2 arguments: f and x".to_string(),
        ));
    }
    let env = context.env();
    let f = match_fun(&positional[0], env)?;
    let x = &positional[1];

    let right = named
        .iter()
        .find(|(n, _)| n == "right")
        .and_then(|(_, v)| v.as_vector()?.as_logical_scalar())
        .unwrap_or(false);

    let items: Vec<RValue> = rvalue_to_items(x);

    context.with_interpreter(|interp| {
        let iter: Box<dyn Iterator<Item = &RValue>> = if right {
            Box::new(items.iter().rev())
        } else {
            Box::new(items.iter())
        };

        for item in iter {
            let result = interp.call_function(&f, std::slice::from_ref(item), &[], env)?;
            if result
                .as_vector()
                .and_then(|v| v.as_logical_scalar())
                .unwrap_or(false)
            {
                return Ok(item.clone());
            }
        }
        Ok(RValue::Null)
    })
}

/// Find the position (1-based index) of the first element where a predicate is TRUE.
///
/// @param f predicate function returning a logical scalar
/// @param x vector or list to search
/// @param right if TRUE, search from right to left
/// @return scalar integer position, or NULL if none found
#[interpreter_builtin(name = "Position", min_args = 2)]
fn interp_position(
    positional: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    if positional.len() < 2 {
        return Err(RError::new(
            RErrorKind::Argument,
            "Position requires 2 arguments: f and x".to_string(),
        ));
    }
    let env = context.env();
    let f = match_fun(&positional[0], env)?;
    let x = &positional[1];

    let right = named
        .iter()
        .find(|(n, _)| n == "right")
        .and_then(|(_, v)| v.as_vector()?.as_logical_scalar())
        .unwrap_or(false);

    let items: Vec<RValue> = rvalue_to_items(x);

    context.with_interpreter(|interp| {
        let indices: Box<dyn Iterator<Item = usize>> = if right {
            Box::new((0..items.len()).rev())
        } else {
            Box::new(0..items.len())
        };

        for i in indices {
            let result = interp.call_function(&f, std::slice::from_ref(&items[i]), &[], env)?;
            if result
                .as_vector()
                .and_then(|v| v.as_logical_scalar())
                .unwrap_or(false)
            {
                let pos = i64::try_from(i + 1).map_err(RError::from)?;
                return Ok(RValue::vec(Vector::Integer(vec![Some(pos)].into())));
            }
        }
        Ok(RValue::Null)
    })
}

/// Negate a predicate function, returning a new function that returns the
/// logical complement of the original.
///
/// @param f predicate function
/// @return a new closure that calls f and negates the result
#[interpreter_builtin(name = "Negate", min_args = 1)]
fn interp_negate(
    positional: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let env = context.env();
    let f = match_fun(&positional[0], env)?;

    // Create an environment that captures the original function
    let closure_env = Environment::new_child(env);
    closure_env.set(".negate_f".to_string(), f);

    // Build: function(...) !.negate_f(...)
    let body = Expr::UnaryOp {
        op: UnaryOp::Not,
        operand: Box::new(Expr::Call {
            func: Box::new(Expr::Symbol(".negate_f".to_string())),
            args: vec![Arg {
                name: None,
                value: Some(Expr::Dots),
            }],
        }),
    };

    Ok(RValue::Function(RFunction::Closure {
        params: vec![Param {
            name: "...".to_string(),
            default: None,
            is_dots: true,
        }],
        body,
        env: closure_env,
    }))
}

/// Recursively apply a function to all non-list elements of a (nested) list.
///
/// @param object a list (possibly nested)
/// @param f function to apply to non-list elements
/// @param how one of "unlist" (default), "replace", or "list"
/// @return depends on `how`: "unlist" returns a flat vector, "replace" returns a list
///   with the same structure, "list" returns a flat list of results
#[interpreter_builtin(name = "rapply", min_args = 2)]
fn interp_rapply(
    positional: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    if positional.len() < 2 {
        return Err(RError::new(
            RErrorKind::Argument,
            "rapply requires at least 2 arguments: object and f".to_string(),
        ));
    }
    let env = context.env();
    let object = &positional[0];
    let f = match_fun(&positional[1], env)?;

    let how = named
        .iter()
        .find(|(n, _)| n == "how")
        .and_then(|(_, v)| v.as_vector()?.as_character_scalar())
        .or_else(|| {
            positional
                .get(2)
                .and_then(|v| v.as_vector()?.as_character_scalar())
        })
        .unwrap_or_else(|| "unlist".to_string());

    context.with_interpreter(|interp| match how.as_str() {
        "replace" => rapply_replace(interp, object, &f, env),
        "list" => {
            let mut results = Vec::new();
            rapply_collect(interp, object, &f, env, &mut results)?;
            Ok(RValue::List(RList::new(
                results.into_iter().map(|v| (None, v)).collect(),
            )))
        }
        _ => {
            // "unlist" (default)
            let mut results = Vec::new();
            rapply_collect(interp, object, &f, env, &mut results)?;
            if results.is_empty() {
                return Ok(RValue::Null);
            }
            // Try to simplify to a vector via c()
            crate::interpreter::builtins::builtin_c(&results, &[])
        }
    })
}

/// Helper: collect results of applying f to all leaf (non-list) elements.
fn rapply_collect(
    interp: &crate::interpreter::Interpreter,
    x: &RValue,
    f: &RValue,
    env: &Environment,
    out: &mut Vec<RValue>,
) -> Result<(), RError> {
    match x {
        RValue::List(list) => {
            for (_, val) in &list.values {
                rapply_collect(interp, val, f, env, out)?;
            }
        }
        _ => {
            let result = interp
                .call_function(f, std::slice::from_ref(x), &[], env)
                .map_err(RError::from)?;
            out.push(result);
        }
    }
    Ok(())
}

/// Helper: recursively apply f, preserving list structure ("replace" mode).
fn rapply_replace(
    interp: &crate::interpreter::Interpreter,
    x: &RValue,
    f: &RValue,
    env: &Environment,
) -> Result<RValue, RError> {
    match x {
        RValue::List(list) => {
            let new_vals: Vec<(Option<String>, RValue)> = list
                .values
                .iter()
                .map(|(name, val)| {
                    let new_val = rapply_replace(interp, val, f, env)?;
                    Ok((name.clone(), new_val))
                })
                .collect::<Result<Vec<_>, RError>>()?;
            Ok(RValue::List(RList::new(new_vals)))
        }
        _ => Ok(interp
            .call_function(f, std::slice::from_ref(x), &[], env)
            .map_err(RError::from)?),
    }
}

// endregion

// region: Recall and parent.env<-

/// Recursive self-call: re-invoke the currently executing function with new arguments.
///
/// Recall looks up the call stack to find the enclosing user-defined function
/// and calls it again with the supplied arguments.
///
/// @param ... arguments to pass to the re-invoked function
/// @return the result of re-calling the current function
#[interpreter_builtin(name = "Recall")]
fn interp_recall(
    positional: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    context.with_interpreter(|interp| {
        // Walk the call stack to find the nearest user-defined (closure) function
        let frames = interp.call_stack.borrow();
        let closure_frame = frames.iter().rev().find(|frame| {
            matches!(
                &frame.function,
                RValue::Function(RFunction::Closure { .. })
            )
        });
        match closure_frame {
            Some(frame) => {
                let func = frame.function.clone();
                let env = frame.env.clone();
                drop(frames); // release borrow before calling
                interp
                    .call_function(&func, positional, named, &env)
                    .map_err(RError::from)
            }
            None => {
                drop(frames);
                Err(RError::other(
                    "Recall() called from outside a function — there is no enclosing function to re-invoke"
                        .to_string(),
                ))
            }
        }
    })
}

/// Set the parent environment of an environment.
///
/// This is the replacement function for `parent.env(e)`.
/// Usage: `parent.env(e) <- value`
///
/// @param e environment whose parent to set
/// @param value the new parent environment
/// @return the modified environment (invisibly)
#[interpreter_builtin(name = "parent.env<-", min_args = 2)]
fn interp_parent_env_assign(
    positional: &[RValue],
    _named: &[(String, RValue)],
    _context: &BuiltinContext,
) -> Result<RValue, RError> {
    let env = match positional.first() {
        Some(RValue::Environment(e)) => e,
        _ => {
            return Err(RError::new(
                RErrorKind::Argument,
                "`parent.env<-` requires an environment as its first argument".to_string(),
            ))
        }
    };
    let new_parent = match positional.get(1) {
        Some(RValue::Environment(e)) => Some(e.clone()),
        Some(RValue::Null) => None,
        _ => {
            return Err(RError::new(
                RErrorKind::Argument,
                "`parent.env<-` requires an environment (or NULL) as the replacement value"
                    .to_string(),
            ))
        }
    };
    env.set_parent(new_parent);
    Ok(RValue::Environment(env.clone()))
}

// endregion
