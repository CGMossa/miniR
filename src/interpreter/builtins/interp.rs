//! Interpreter-level builtins — functions that need `&Environment` access and
//! call back into the interpreter. Each is auto-registered via `#[interpreter_builtin]`.
//! The interpreter is accessed via the thread-local `with_interpreter()`.

use crate::interpreter::environment::Environment;
use crate::interpreter::value::*;
use crate::interpreter::with_interpreter;
use crate::parser::ast::{BinaryOp, UnaryOp};
use newr_macros::interpreter_builtin;

/// Resolve a function specification: accepts an RValue::Function directly,
/// or a string naming a function to look up in the environment.
/// Equivalent to R's match.fun().
fn match_fun(f: &RValue, env: &Environment) -> Result<RValue, RError> {
    match f {
        RValue::Function(_) => Ok(f.clone()),
        RValue::Vector(Vector::Character(s)) => {
            let name = s
                .first()
                .and_then(|x| x.as_ref())
                .ok_or_else(|| RError::Argument("not a valid function name".to_string()))?;
            env.get_function(name)
                .ok_or_else(|| RError::Other(format!("could not find function '{}'", name)))
        }
        _ => Err(RError::Argument(
            "FUN is not a function and not a string naming a function".to_string(),
        )),
    }
}

#[interpreter_builtin(name = "sapply", min_args = 2)]
fn interp_sapply(
    args: &[RValue],
    named: &[(String, RValue)],
    env: &Environment,
) -> Result<RValue, RError> {
    eval_apply(args, named, true, env)
}

#[interpreter_builtin(name = "lapply", min_args = 2)]
fn interp_lapply(
    args: &[RValue],
    named: &[(String, RValue)],
    env: &Environment,
) -> Result<RValue, RError> {
    eval_apply(args, named, false, env)
}

#[interpreter_builtin(name = "vapply", min_args = 3)]
fn interp_vapply(
    args: &[RValue],
    named: &[(String, RValue)],
    env: &Environment,
) -> Result<RValue, RError> {
    eval_apply(args, named, true, env)
}

fn eval_apply(
    positional: &[RValue],
    _named: &[(String, RValue)],
    simplify: bool,
    env: &Environment,
) -> Result<RValue, RError> {
    if positional.len() < 2 {
        return Err(RError::Argument(
            "need at least 2 arguments for apply".to_string(),
        ));
    }
    let x = &positional[0];
    let f = match_fun(&positional[1], env)?;

    let items: Vec<RValue> = match x {
        RValue::Vector(v) => match v {
            Vector::Double(vals) => vals
                .iter()
                .map(|x| RValue::Vector(Vector::Double(vec![*x].into())))
                .collect(),
            Vector::Integer(vals) => vals
                .iter()
                .map(|x| RValue::Vector(Vector::Integer(vec![*x].into())))
                .collect(),
            Vector::Character(vals) => vals
                .iter()
                .map(|x| RValue::Vector(Vector::Character(vec![x.clone()].into())))
                .collect(),
            Vector::Logical(vals) => vals
                .iter()
                .map(|x| RValue::Vector(Vector::Logical(vec![*x].into())))
                .collect(),
        },
        RValue::List(l) => l.values.iter().map(|(_, v)| v.clone()).collect(),
        _ => vec![x.clone()],
    };

    with_interpreter(|interp| {
        let mut results: Vec<RValue> = Vec::new();
        for item in &items {
            let result = interp.call_function(&f, std::slice::from_ref(item), &[], env)?;
            results.push(result);
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
                            return Ok(RValue::Vector(Vector::Double(vals.into())));
                        }
                        "integer" => {
                            let vals: Vec<Option<i64>> = results
                                .iter()
                                .filter_map(|r| {
                                    r.as_vector()
                                        .map(|v| v.to_integers().into_iter().next().unwrap_or(None))
                                })
                                .collect();
                            return Ok(RValue::Vector(Vector::Integer(vals.into())));
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
                            return Ok(RValue::Vector(Vector::Character(vals.into())));
                        }
                        "logical" => {
                            let vals: Vec<Option<bool>> = results
                                .iter()
                                .filter_map(|r| {
                                    r.as_vector()
                                        .map(|v| v.to_logicals().into_iter().next().unwrap_or(None))
                                })
                                .collect();
                            return Ok(RValue::Vector(Vector::Logical(vals.into())));
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
    env: &Environment,
) -> Result<RValue, RError> {
    if positional.len() >= 2 {
        let f = match_fun(&positional[0], env)?;
        return with_interpreter(|interp| match &positional[1] {
            RValue::List(l) => {
                let args: Vec<RValue> = l.values.iter().map(|(_, v)| v.clone()).collect();
                interp.call_function(&f, &args, named, env)
            }
            _ => interp.call_function(&f, &positional[1..], named, env),
        });
    }
    Err(RError::Argument(
        "do.call requires at least 2 arguments".to_string(),
    ))
}

#[interpreter_builtin(name = "Vectorize", min_args = 1)]
fn interp_vectorize(
    positional: &[RValue],
    _named: &[(String, RValue)],
    _env: &Environment,
) -> Result<RValue, RError> {
    Ok(positional.first().cloned().unwrap_or(RValue::Null))
}

#[interpreter_builtin(name = "Reduce", min_args = 2)]
fn interp_reduce(
    positional: &[RValue],
    named: &[(String, RValue)],
    env: &Environment,
) -> Result<RValue, RError> {
    if positional.len() < 2 {
        return Err(RError::Argument(
            "Reduce requires at least 2 arguments".to_string(),
        ));
    }
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

    with_interpreter(|interp| {
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
    _named: &[(String, RValue)],
    env: &Environment,
) -> Result<RValue, RError> {
    if positional.len() < 2 {
        return Err(RError::Argument("Filter requires 2 arguments".to_string()));
    }
    let f = match_fun(&positional[0], env)?;
    let x = &positional[1];

    let items: Vec<RValue> = rvalue_to_items(x);

    let mut results = Vec::new();
    with_interpreter(|interp| {
        for item in &items {
            let keep = interp.call_function(&f, std::slice::from_ref(item), &[], env)?;
            if keep
                .as_vector()
                .and_then(|v| v.as_logical_scalar())
                .unwrap_or(false)
            {
                results.push(item.clone());
            }
        }
        Ok(())
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
    _named: &[(String, RValue)],
    env: &Environment,
) -> Result<RValue, RError> {
    if positional.len() < 2 {
        return Err(RError::Argument(
            "Map requires at least 2 arguments".to_string(),
        ));
    }
    let f = match_fun(&positional[0], env)?;

    let seqs: Vec<Vec<RValue>> = positional[1..].iter().map(rvalue_to_items).collect();

    let max_len = seqs.iter().map(|s| s.len()).max().unwrap_or(0);
    let mut results = Vec::new();

    with_interpreter(|interp| {
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
            let result = interp.call_function(&f, &call_args, &[], env)?;
            results.push((None, result));
        }
        Ok(())
    })?;

    Ok(RValue::List(RList::new(results)))
}

#[interpreter_builtin(name = "switch", min_args = 1)]
fn interp_switch(
    positional: &[RValue],
    named: &[(String, RValue)],
    _env: &Environment,
) -> Result<RValue, RError> {
    let expr = positional
        .first()
        .ok_or_else(|| RError::Argument("'EXPR' is missing".to_string()))?;

    let is_character = matches!(expr, RValue::Vector(Vector::Character(_)));

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
                    .get((i - 1) as usize)
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
    env: &Environment,
) -> Result<RValue, RError> {
    let name = positional
        .first()
        .and_then(|v| {
            if let RValue::Vector(vec) = v {
                vec.as_character_scalar()
            } else {
                None
            }
        })
        .ok_or_else(|| RError::Argument("invalid first argument".to_string()))?;
    let _envir = named.iter().find(|(n, _)| n == "envir").map(|(_, v)| v);
    env.get(&name)
        .ok_or_else(|| RError::Other(format!("object '{}' not found", name)))
}

#[interpreter_builtin(name = "assign", min_args = 2)]
fn interp_assign(
    positional: &[RValue],
    named: &[(String, RValue)],
    env: &Environment,
) -> Result<RValue, RError> {
    let name = positional
        .first()
        .and_then(|v| {
            if let RValue::Vector(vec) = v {
                vec.as_character_scalar()
            } else {
                None
            }
        })
        .ok_or_else(|| RError::Argument("invalid first argument".to_string()))?;
    let value = positional
        .get(1)
        .or_else(|| named.iter().find(|(n, _)| n == "value").map(|(_, v)| v))
        .cloned()
        .unwrap_or(RValue::Null);
    env.set(name, value.clone());
    Ok(value)
}

#[interpreter_builtin(name = "exists", min_args = 1)]
fn interp_exists(
    positional: &[RValue],
    _named: &[(String, RValue)],
    env: &Environment,
) -> Result<RValue, RError> {
    let name = positional
        .first()
        .and_then(|v| {
            if let RValue::Vector(vec) = v {
                vec.as_character_scalar()
            } else {
                None
            }
        })
        .unwrap_or_default();
    let found = env.get(&name).is_some();
    Ok(RValue::Vector(Vector::Logical(vec![Some(found)].into())))
}

#[interpreter_builtin(name = "source", min_args = 1)]
fn interp_source(
    positional: &[RValue],
    _named: &[(String, RValue)],
    _env: &Environment,
) -> Result<RValue, RError> {
    let path = positional
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| RError::Argument("invalid 'file' argument".to_string()))?;
    let source = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::InvalidData => {
            let bytes = std::fs::read(&path)
                .map_err(|e2| RError::Other(format!("cannot open file '{}': {}", path, e2)))?;
            String::from_utf8_lossy(&bytes).into_owned()
        }
        Err(e) => return Err(RError::Other(format!("cannot open file '{}': {}", path, e))),
    };
    let ast = crate::parser::parse_program(&source)
        .map_err(|e| RError::Other(format!("parse error in '{}': {}", path, e)))?;
    with_interpreter(|interp| interp.eval(&ast))
}

#[interpreter_builtin(name = "system.time", min_args = 1)]
fn interp_system_time(
    positional: &[RValue],
    _named: &[(String, RValue)],
    _env: &Environment,
) -> Result<RValue, RError> {
    let start = std::time::Instant::now();
    let _result = positional.first().cloned().unwrap_or(RValue::Null);
    let elapsed = start.elapsed().as_secs_f64();
    Ok(RValue::Vector(Vector::Double(
        vec![Some(elapsed), Some(0.0), Some(elapsed)].into(),
    )))
}

// --- Operator builtins: R operators as first-class functions ---
// These allow `Reduce("+", 1:10)`, `sapply(x, "-")`, `do.call("*", list(3,4))`, etc.

fn eval_binop(op: BinaryOp, args: &[RValue]) -> Result<RValue, RError> {
    let left = args.first().cloned().unwrap_or(RValue::Null);
    let right = args.get(1).cloned().unwrap_or(RValue::Null);
    with_interpreter(|interp| interp.eval_binary(op, &left, &right))
}

#[interpreter_builtin(name = "+", min_args = 1)]
fn interp_op_add(
    args: &[RValue],
    _named: &[(String, RValue)],
    _env: &Environment,
) -> Result<RValue, RError> {
    if args.len() == 1 {
        with_interpreter(|interp| interp.eval_unary(UnaryOp::Pos, &args[0]))
    } else {
        eval_binop(BinaryOp::Add, args)
    }
}

#[interpreter_builtin(name = "-", min_args = 1)]
fn interp_op_sub(
    args: &[RValue],
    _named: &[(String, RValue)],
    _env: &Environment,
) -> Result<RValue, RError> {
    if args.len() == 1 {
        with_interpreter(|interp| interp.eval_unary(UnaryOp::Neg, &args[0]))
    } else {
        eval_binop(BinaryOp::Sub, args)
    }
}

#[interpreter_builtin(name = "*", min_args = 2)]
fn interp_op_mul(
    args: &[RValue],
    _named: &[(String, RValue)],
    _env: &Environment,
) -> Result<RValue, RError> {
    eval_binop(BinaryOp::Mul, args)
}

#[interpreter_builtin(name = "/", min_args = 2)]
fn interp_op_div(
    args: &[RValue],
    _named: &[(String, RValue)],
    _env: &Environment,
) -> Result<RValue, RError> {
    eval_binop(BinaryOp::Div, args)
}

#[interpreter_builtin(name = "^", min_args = 2)]
fn interp_op_pow(
    args: &[RValue],
    _named: &[(String, RValue)],
    _env: &Environment,
) -> Result<RValue, RError> {
    eval_binop(BinaryOp::Pow, args)
}

#[interpreter_builtin(name = "%%", min_args = 2)]
fn interp_op_mod(
    args: &[RValue],
    _named: &[(String, RValue)],
    _env: &Environment,
) -> Result<RValue, RError> {
    eval_binop(BinaryOp::Mod, args)
}

#[interpreter_builtin(name = "%/%", min_args = 2)]
fn interp_op_intdiv(
    args: &[RValue],
    _named: &[(String, RValue)],
    _env: &Environment,
) -> Result<RValue, RError> {
    eval_binop(BinaryOp::IntDiv, args)
}

#[interpreter_builtin(name = "==", min_args = 2)]
fn interp_op_eq(
    args: &[RValue],
    _named: &[(String, RValue)],
    _env: &Environment,
) -> Result<RValue, RError> {
    eval_binop(BinaryOp::Eq, args)
}

#[interpreter_builtin(name = "!=", min_args = 2)]
fn interp_op_ne(
    args: &[RValue],
    _named: &[(String, RValue)],
    _env: &Environment,
) -> Result<RValue, RError> {
    eval_binop(BinaryOp::Ne, args)
}

#[interpreter_builtin(name = "<", min_args = 2)]
fn interp_op_lt(
    args: &[RValue],
    _named: &[(String, RValue)],
    _env: &Environment,
) -> Result<RValue, RError> {
    eval_binop(BinaryOp::Lt, args)
}

#[interpreter_builtin(name = ">", min_args = 2)]
fn interp_op_gt(
    args: &[RValue],
    _named: &[(String, RValue)],
    _env: &Environment,
) -> Result<RValue, RError> {
    eval_binop(BinaryOp::Gt, args)
}

#[interpreter_builtin(name = "<=", min_args = 2)]
fn interp_op_le(
    args: &[RValue],
    _named: &[(String, RValue)],
    _env: &Environment,
) -> Result<RValue, RError> {
    eval_binop(BinaryOp::Le, args)
}

#[interpreter_builtin(name = ">=", min_args = 2)]
fn interp_op_ge(
    args: &[RValue],
    _named: &[(String, RValue)],
    _env: &Environment,
) -> Result<RValue, RError> {
    eval_binop(BinaryOp::Ge, args)
}

#[interpreter_builtin(name = "&", min_args = 2)]
fn interp_op_and(
    args: &[RValue],
    _named: &[(String, RValue)],
    _env: &Environment,
) -> Result<RValue, RError> {
    eval_binop(BinaryOp::And, args)
}

#[interpreter_builtin(name = "|", min_args = 2)]
fn interp_op_or(
    args: &[RValue],
    _named: &[(String, RValue)],
    _env: &Environment,
) -> Result<RValue, RError> {
    eval_binop(BinaryOp::Or, args)
}

#[interpreter_builtin(name = "!", min_args = 1)]
fn interp_op_not(
    args: &[RValue],
    _named: &[(String, RValue)],
    _env: &Environment,
) -> Result<RValue, RError> {
    with_interpreter(|interp| interp.eval_unary(UnaryOp::Not, &args[0]))
}

/// Convert an RValue to a Vec of individual items (for apply/map/filter/reduce).
fn rvalue_to_items(x: &RValue) -> Vec<RValue> {
    match x {
        RValue::Vector(v) => match v {
            Vector::Double(vals) => vals
                .iter()
                .map(|x| RValue::Vector(Vector::Double(vec![*x].into())))
                .collect(),
            Vector::Integer(vals) => vals
                .iter()
                .map(|x| RValue::Vector(Vector::Integer(vec![*x].into())))
                .collect(),
            Vector::Character(vals) => vals
                .iter()
                .map(|x| RValue::Vector(Vector::Character(vec![x.clone()].into())))
                .collect(),
            Vector::Logical(vals) => vals
                .iter()
                .map(|x| RValue::Vector(Vector::Logical(vec![*x].into())))
                .collect(),
        },
        RValue::List(l) => l.values.iter().map(|(_, v)| v.clone()).collect(),
        _ => vec![x.clone()],
    }
}
