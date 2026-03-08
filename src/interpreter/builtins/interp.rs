//! Interpreter-level builtins — functions that need `&mut Interpreter` and/or
//! `&Environment` access. Each is auto-registered via `#[interpreter_builtin]`.

use crate::interpreter::environment::Environment;
use crate::interpreter::value::*;
use crate::interpreter::Interpreter;
use newr_macros::interpreter_builtin;

#[interpreter_builtin(name = "sapply", min_args = 2)]
fn interp_sapply(
    interp: &mut Interpreter,
    args: &[RValue],
    named: &[(String, RValue)],
    _env: &Environment,
) -> Result<RValue, RError> {
    eval_apply(interp, args, named, true)
}

#[interpreter_builtin(name = "lapply", min_args = 2)]
fn interp_lapply(
    interp: &mut Interpreter,
    args: &[RValue],
    named: &[(String, RValue)],
    _env: &Environment,
) -> Result<RValue, RError> {
    eval_apply(interp, args, named, false)
}

#[interpreter_builtin(name = "vapply", min_args = 3)]
fn interp_vapply(
    interp: &mut Interpreter,
    args: &[RValue],
    named: &[(String, RValue)],
    _env: &Environment,
) -> Result<RValue, RError> {
    eval_apply(interp, args, named, true)
}

fn eval_apply(
    interp: &mut Interpreter,
    positional: &[RValue],
    _named: &[(String, RValue)],
    simplify: bool,
) -> Result<RValue, RError> {
    if positional.len() < 2 {
        return Err(RError::Argument(
            "need at least 2 arguments for apply".to_string(),
        ));
    }
    let x = &positional[0];
    let f = &positional[1];

    let items: Vec<RValue> = match x {
        RValue::Vector(v) => match v {
            Vector::Double(vals) => vals
                .iter()
                .map(|x| RValue::Vector(Vector::Double(vec![*x])))
                .collect(),
            Vector::Integer(vals) => vals
                .iter()
                .map(|x| RValue::Vector(Vector::Integer(vec![*x])))
                .collect(),
            Vector::Character(vals) => vals
                .iter()
                .map(|x| RValue::Vector(Vector::Character(vec![x.clone()])))
                .collect(),
            Vector::Logical(vals) => vals
                .iter()
                .map(|x| RValue::Vector(Vector::Logical(vec![*x])))
                .collect(),
        },
        RValue::List(l) => l.values.iter().map(|(_, v)| v.clone()).collect(),
        _ => vec![x.clone()],
    };

    let env = &interp.global_env.clone();
    let mut results: Vec<RValue> = Vec::new();
    for item in &items {
        let result = interp.call_function(f, std::slice::from_ref(item), &[], env)?;
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
                        return Ok(RValue::Vector(Vector::Double(vals)));
                    }
                    "integer" => {
                        let vals: Vec<Option<i64>> = results
                            .iter()
                            .filter_map(|r| {
                                r.as_vector()
                                    .map(|v| v.to_integers().into_iter().next().unwrap_or(None))
                            })
                            .collect();
                        return Ok(RValue::Vector(Vector::Integer(vals)));
                    }
                    "character" => {
                        let vals: Vec<Option<String>> = results
                            .iter()
                            .filter_map(|r| {
                                r.as_vector()
                                    .map(|v| v.to_characters().into_iter().next().unwrap_or(None))
                            })
                            .collect();
                        return Ok(RValue::Vector(Vector::Character(vals)));
                    }
                    "logical" => {
                        let vals: Vec<Option<bool>> = results
                            .iter()
                            .filter_map(|r| {
                                r.as_vector()
                                    .map(|v| v.to_logicals().into_iter().next().unwrap_or(None))
                            })
                            .collect();
                        return Ok(RValue::Vector(Vector::Logical(vals)));
                    }
                    _ => {}
                }
            }
        }
    }

    let values: Vec<(Option<String>, RValue)> = results.into_iter().map(|v| (None, v)).collect();
    Ok(RValue::List(RList::new(values)))
}

#[interpreter_builtin(name = "do.call", min_args = 2)]
fn interp_do_call(
    interp: &mut Interpreter,
    positional: &[RValue],
    named: &[(String, RValue)],
    env: &Environment,
) -> Result<RValue, RError> {
    if positional.len() >= 2 {
        let f = &positional[0];
        match &positional[1] {
            RValue::List(l) => {
                let args: Vec<RValue> = l.values.iter().map(|(_, v)| v.clone()).collect();
                return interp.call_function(f, &args, named, env);
            }
            _ => return interp.call_function(f, &positional[1..], named, env),
        }
    }
    Err(RError::Argument(
        "do.call requires at least 2 arguments".to_string(),
    ))
}

#[interpreter_builtin(name = "Vectorize", min_args = 1)]
fn interp_vectorize(
    _interp: &mut Interpreter,
    positional: &[RValue],
    _named: &[(String, RValue)],
    _env: &Environment,
) -> Result<RValue, RError> {
    Ok(positional.first().cloned().unwrap_or(RValue::Null))
}

#[interpreter_builtin(name = "Reduce", min_args = 2)]
fn interp_reduce(
    interp: &mut Interpreter,
    positional: &[RValue],
    named: &[(String, RValue)],
    env: &Environment,
) -> Result<RValue, RError> {
    if positional.len() < 2 {
        return Err(RError::Argument(
            "Reduce requires at least 2 arguments".to_string(),
        ));
    }
    let f = &positional[0];
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

    for item in items.iter().skip(start) {
        acc = interp.call_function(f, &[acc, item.clone()], &[], env)?;
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
}

#[interpreter_builtin(name = "Filter", min_args = 2)]
fn interp_filter(
    interp: &mut Interpreter,
    positional: &[RValue],
    _named: &[(String, RValue)],
    env: &Environment,
) -> Result<RValue, RError> {
    if positional.len() < 2 {
        return Err(RError::Argument("Filter requires 2 arguments".to_string()));
    }
    let f = &positional[0];
    let x = &positional[1];

    let items: Vec<RValue> = rvalue_to_items(x);

    let mut results = Vec::new();
    for item in &items {
        let keep = interp.call_function(f, std::slice::from_ref(item), &[], env)?;
        if keep
            .as_vector()
            .and_then(|v| v.as_logical_scalar())
            .unwrap_or(false)
        {
            results.push(item.clone());
        }
    }

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
    interp: &mut Interpreter,
    positional: &[RValue],
    _named: &[(String, RValue)],
    env: &Environment,
) -> Result<RValue, RError> {
    if positional.len() < 2 {
        return Err(RError::Argument(
            "Map requires at least 2 arguments".to_string(),
        ));
    }
    let f = &positional[0];

    let seqs: Vec<Vec<RValue>> = positional[1..].iter().map(rvalue_to_items).collect();

    let max_len = seqs.iter().map(|s| s.len()).max().unwrap_or(0);
    let mut results = Vec::new();

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
        let result = interp.call_function(f, &call_args, &[], env)?;
        results.push((None, result));
    }

    Ok(RValue::List(RList::new(results)))
}

#[interpreter_builtin(name = "switch", min_args = 1)]
fn interp_switch(
    _interp: &mut Interpreter,
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
    _interp: &mut Interpreter,
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
    _interp: &mut Interpreter,
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
    _interp: &mut Interpreter,
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
    Ok(RValue::Vector(Vector::Logical(vec![Some(found)])))
}

#[interpreter_builtin(name = "source", min_args = 1)]
fn interp_source(
    interp: &mut Interpreter,
    positional: &[RValue],
    _named: &[(String, RValue)],
    _env: &Environment,
) -> Result<RValue, RError> {
    let path = positional
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| RError::Argument("invalid 'file' argument".to_string()))?;
    let source = std::fs::read_to_string(&path)
        .map_err(|e| RError::Other(format!("cannot open file '{}': {}", path, e)))?;
    let ast = crate::parser::parse_program(&source)
        .map_err(|e| RError::Other(format!("parse error in '{}': {}", path, e)))?;
    interp.eval(&ast)
}

#[interpreter_builtin(name = "system.time", min_args = 1)]
fn interp_system_time(
    _interp: &mut Interpreter,
    positional: &[RValue],
    _named: &[(String, RValue)],
    _env: &Environment,
) -> Result<RValue, RError> {
    let start = std::time::Instant::now();
    let _result = positional.first().cloned().unwrap_or(RValue::Null);
    let elapsed = start.elapsed().as_secs_f64();
    Ok(RValue::Vector(Vector::Double(vec![
        Some(elapsed),
        Some(0.0),
        Some(elapsed),
    ])))
}

/// Convert an RValue to a Vec of individual items (for apply/map/filter/reduce).
fn rvalue_to_items(x: &RValue) -> Vec<RValue> {
    match x {
        RValue::Vector(v) => match v {
            Vector::Double(vals) => vals
                .iter()
                .map(|x| RValue::Vector(Vector::Double(vec![*x])))
                .collect(),
            Vector::Integer(vals) => vals
                .iter()
                .map(|x| RValue::Vector(Vector::Integer(vec![*x])))
                .collect(),
            Vector::Character(vals) => vals
                .iter()
                .map(|x| RValue::Vector(Vector::Character(vec![x.clone()])))
                .collect(),
            Vector::Logical(vals) => vals
                .iter()
                .map(|x| RValue::Vector(Vector::Logical(vec![*x])))
                .collect(),
        },
        RValue::List(l) => l.values.iter().map(|(_, v)| v.clone()).collect(),
        _ => vec![x.clone()],
    }
}
