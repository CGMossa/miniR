mod interp;
mod math;
mod pre_eval;
mod strings;
mod stubs;

use crate::interpreter::environment::Environment;
use crate::interpreter::value::*;
use crate::parser::ast::Arg;
use linkme::distributed_slice;
use newr_macros::builtin;

pub type BuiltinFn = fn(&[RValue], &[(String, RValue)]) -> Result<RValue, RError>;

pub type InterpreterBuiltinFn =
    fn(&[RValue], &[(String, RValue)], &Environment) -> Result<RValue, RError>;

pub type PreEvalBuiltinFn = fn(&[Arg], &Environment) -> Result<RValue, RError>;

#[distributed_slice]
pub static BUILTIN_REGISTRY: [(&str, BuiltinFn, usize)];

#[distributed_slice]
pub static INTERPRETER_BUILTIN_REGISTRY: [(&str, InterpreterBuiltinFn, usize)];

#[distributed_slice]
pub static PRE_EVAL_BUILTIN_REGISTRY: [(&str, PreEvalBuiltinFn, usize)];

/// Helper for unary math builtins: applies `f64 -> f64` element-wise.
#[inline]
pub fn math_unary_op(args: &[RValue], f: fn(f64) -> f64) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(v)) => {
            let result: Vec<Option<f64>> = v.to_doubles().iter().map(|x| x.map(f)).collect();
            Ok(RValue::vec(Vector::Double(result.into())))
        }
        _ => Err(RError::Argument(
            "non-numeric argument to mathematical function".to_string(),
        )),
    }
}

/// Placeholder for interpreter-level builtins — never actually called because
/// dispatch is intercepted by the interpreter/pre-eval registries.
fn placeholder_builtin(_args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    Err(RError::Other(
        "internal error: interpreter builtin not intercepted".to_string(),
    ))
}

pub fn register_builtins(env: &Environment) {
    // Auto-registered builtins (via #[builtin] + linkme, including noop stubs)
    for &(name, func, _min_args) in BUILTIN_REGISTRY {
        env.set(
            name.to_string(),
            RValue::Function(RFunction::Builtin {
                name: name.to_string(),
                func,
            }),
        );
    }

    // Interpreter-level builtins (intercepted at dispatch time)
    for &(name, _, _) in INTERPRETER_BUILTIN_REGISTRY {
        env.set(
            name.to_string(),
            RValue::Function(RFunction::Builtin {
                name: name.to_string(),
                func: placeholder_builtin,
            }),
        );
    }

    // Pre-eval builtins (intercepted before argument evaluation)
    for &(name, _, _) in PRE_EVAL_BUILTIN_REGISTRY {
        env.set(
            name.to_string(),
            RValue::Function(RFunction::Builtin {
                name: name.to_string(),
                func: placeholder_builtin,
            }),
        );
    }

    // Constants
    env.set(
        "pi".to_string(),
        RValue::vec(Vector::Double(vec![Some(std::f64::consts::PI)].into())),
    );
    env.set(
        "T".to_string(),
        RValue::vec(Vector::Logical(vec![Some(true)].into())),
    );
    env.set(
        "F".to_string(),
        RValue::vec(Vector::Logical(vec![Some(false)].into())),
    );
    env.set(
        "TRUE".to_string(),
        RValue::vec(Vector::Logical(vec![Some(true)].into())),
    );
    env.set(
        "FALSE".to_string(),
        RValue::vec(Vector::Logical(vec![Some(false)].into())),
    );
    env.set(
        "Inf".to_string(),
        RValue::vec(Vector::Double(vec![Some(f64::INFINITY)].into())),
    );
    env.set(
        "NaN".to_string(),
        RValue::vec(Vector::Double(vec![Some(f64::NAN)].into())),
    );
    env.set(
        "NA".to_string(),
        RValue::vec(Vector::Logical(vec![None].into())),
    );
    env.set(
        "NA_integer_".to_string(),
        RValue::vec(Vector::Integer(vec![None].into())),
    );
    env.set(
        "NA_real_".to_string(),
        RValue::vec(Vector::Double(vec![None].into())),
    );
    env.set(
        "NA_character_".to_string(),
        RValue::vec(Vector::Character(vec![None].into())),
    );
    env.set(
        "LETTERS".to_string(),
        RValue::vec(Vector::Character(
            (b'A'..=b'Z')
                .map(|c| Some(String::from(c as char)))
                .collect::<Vec<_>>()
                .into(),
        )),
    );
    env.set(
        "letters".to_string(),
        RValue::vec(Vector::Character(
            (b'a'..=b'z')
                .map(|c| Some(String::from(c as char)))
                .collect::<Vec<_>>()
                .into(),
        )),
    );
    env.set(
        ".Machine".to_string(),
        RValue::List(RList::new(vec![
            (
                Some("integer.max".to_string()),
                RValue::vec(Vector::Integer(vec![Some(i32::MAX as i64)].into())),
            ),
            (
                Some("double.eps".to_string()),
                RValue::vec(Vector::Double(vec![Some(f64::EPSILON)].into())),
            ),
            (
                Some("double.xmax".to_string()),
                RValue::vec(Vector::Double(vec![Some(f64::MAX)].into())),
            ),
            (
                Some("double.xmin".to_string()),
                RValue::vec(Vector::Double(vec![Some(f64::MIN_POSITIVE)].into())),
            ),
        ])),
    );
}

// === Builtin implementations ===

#[builtin]
pub fn builtin_c(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let mut all_values: Vec<RValue> = Vec::new();
    for arg in args {
        all_values.push(arg.clone());
    }
    for (_, val) in named {
        all_values.push(val.clone());
    }

    if all_values.is_empty() {
        return Ok(RValue::Null);
    }

    // Check if any are lists
    let has_list = all_values.iter().any(|v| matches!(v, RValue::List(_)));
    if has_list {
        let mut result = Vec::new();
        for val in &all_values {
            match val {
                RValue::List(l) => result.extend(l.values.clone()),
                RValue::Null => {}
                other => result.push((None, other.clone())),
            }
        }
        return Ok(RValue::List(RList::new(result)));
    }

    // Determine highest type
    let mut has_char = false;
    let mut has_double = false;
    let mut has_int = false;

    for val in &all_values {
        match val {
            RValue::Vector(rv) if matches!(rv.inner, Vector::Character(_)) => has_char = true,
            RValue::Vector(rv) if matches!(rv.inner, Vector::Double(_)) => has_double = true,
            RValue::Vector(rv) if matches!(rv.inner, Vector::Integer(_)) => has_int = true,
            RValue::Null => {}
            _ => {}
        }
    }

    if has_char {
        let mut result = Vec::new();
        for val in &all_values {
            match val {
                RValue::Vector(v) => result.extend(v.to_characters()),
                RValue::Null => {}
                _ => {}
            }
        }
        Ok(RValue::vec(Vector::Character(result.into())))
    } else if has_double {
        let mut result = Vec::new();
        for val in &all_values {
            match val {
                RValue::Vector(v) => result.extend(v.to_doubles()),
                RValue::Null => {}
                _ => {}
            }
        }
        Ok(RValue::vec(Vector::Double(result.into())))
    } else if has_int {
        let mut result = Vec::new();
        for val in &all_values {
            match val {
                RValue::Vector(v) => result.extend(v.to_integers()),
                RValue::Null => {}
                _ => {}
            }
        }
        Ok(RValue::vec(Vector::Integer(result.into())))
    } else {
        let mut result = Vec::new();
        for val in &all_values {
            match val {
                RValue::Vector(v) => result.extend(v.to_logicals()),
                RValue::Null => {}
                _ => {}
            }
        }
        Ok(RValue::vec(Vector::Logical(result.into())))
    }
}

#[builtin(min_args = 1)]
fn builtin_print(args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    if let Some(val) = args.first() {
        println!("{}", val);
    }
    Ok(args.first().cloned().unwrap_or(RValue::Null))
}

#[builtin]
fn builtin_cat(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let sep = named
        .iter()
        .find(|(n, _)| n == "sep")
        .and_then(|(_, v)| v.as_vector()?.as_character_scalar())
        .unwrap_or_else(|| " ".to_string());

    let parts: Vec<String> = args
        .iter()
        .map(|arg| match arg {
            RValue::Vector(v) => {
                let elems: Vec<String> = match &v.inner {
                    Vector::Character(vals) => vals
                        .iter()
                        .map(|x| x.clone().unwrap_or_else(|| "NA".to_string()))
                        .collect(),
                    Vector::Double(vals) => vals
                        .iter()
                        .map(|x| x.map(format_r_double).unwrap_or_else(|| "NA".to_string()))
                        .collect(),
                    Vector::Integer(vals) => vals
                        .iter()
                        .map(|x| x.map(|i| i.to_string()).unwrap_or_else(|| "NA".to_string()))
                        .collect(),
                    Vector::Logical(vals) => vals
                        .iter()
                        .map(|x| match x {
                            Some(true) => "TRUE".to_string(),
                            Some(false) => "FALSE".to_string(),
                            None => "NA".to_string(),
                        })
                        .collect(),
                };
                elems.join(&sep)
            }
            RValue::Null => "".to_string(),
            other => format!("{}", other),
        })
        .collect();

    print!("{}", parts.join(&sep));
    Ok(RValue::Null)
}

#[builtin]
fn builtin_paste(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let sep = named
        .iter()
        .find(|(n, _)| n == "sep")
        .and_then(|(_, v)| v.as_vector()?.as_character_scalar())
        .unwrap_or_else(|| " ".to_string());
    let collapse = named
        .iter()
        .find(|(n, _)| n == "collapse")
        .and_then(|(_, v)| v.as_vector()?.as_character_scalar());

    // Convert each arg to character vector
    let char_vecs: Vec<Vec<String>> = args
        .iter()
        .map(|arg| match arg {
            RValue::Vector(v) => v
                .to_characters()
                .into_iter()
                .map(|s| s.unwrap_or_else(|| "NA".to_string()))
                .collect(),
            RValue::Null => vec![],
            other => vec![format!("{}", other)],
        })
        .collect();

    if char_vecs.is_empty() {
        return Ok(RValue::vec(Vector::Character(
            vec![Some(String::new())].into(),
        )));
    }

    // Recycle to max length
    let max_len = char_vecs.iter().map(|v| v.len()).max().unwrap_or(0);
    if max_len == 0 {
        return Ok(RValue::vec(Vector::Character(vec![].into())));
    }

    let result: Vec<Option<String>> = (0..max_len)
        .map(|i| {
            let parts: Vec<&str> = char_vecs
                .iter()
                .filter(|v| !v.is_empty())
                .map(|v| v[i % v.len()].as_str())
                .collect();
            Some(parts.join(&sep))
        })
        .collect();

    match collapse {
        Some(col) => {
            let collapsed: String = result
                .iter()
                .filter_map(|s| s.as_ref())
                .cloned()
                .collect::<Vec<_>>()
                .join(&col);
            Ok(RValue::vec(Vector::Character(vec![Some(collapsed)].into())))
        }
        None => Ok(RValue::vec(Vector::Character(result.into()))),
    }
}

#[builtin]
fn builtin_paste0(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let mut new_named = named.to_vec();
    if !new_named.iter().any(|(n, _)| n == "sep") {
        new_named.push((
            "sep".to_string(),
            RValue::vec(Vector::Character(vec![Some(String::new())].into())),
        ));
    }
    builtin_paste(args, &new_named)
}

#[builtin(min_args = 1)]
fn builtin_length(args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    let len = args.first().map(|v| v.length()).unwrap_or(0);
    Ok(RValue::vec(Vector::Integer(vec![Some(len as i64)].into())))
}

#[builtin(min_args = 1)]
fn builtin_nchar(args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(rv)) if matches!(rv.inner, Vector::Character(_)) => {
            let Vector::Character(vals) = &rv.inner else {
                unreachable!()
            };
            let result: Vec<Option<i64>> = vals
                .iter()
                .map(|s| s.as_ref().map(|s| s.len() as i64))
                .collect();
            Ok(RValue::vec(Vector::Integer(result.into())))
        }
        _ => Ok(RValue::vec(Vector::Integer(vec![None].into()))),
    }
}

#[builtin(min_args = 1, names = ["is.ordered", "is.call", "is.symbol", "is.name", "is.expression", "is.pairlist", "is.environment"])]
fn builtin_is_null(args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    let r = matches!(args.first(), Some(RValue::Null));
    Ok(RValue::vec(Vector::Logical(vec![Some(r)].into())))
}

#[builtin(min_args = 1)]
fn builtin_is_na(args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(v)) => {
            let result: Vec<Option<bool>> = match &v.inner {
                Vector::Logical(vals) => vals.iter().map(|x| Some(x.is_none())).collect(),
                Vector::Integer(vals) => vals.iter().map(|x| Some(x.is_none())).collect(),
                Vector::Double(vals) => vals
                    .iter()
                    .map(|x| Some(x.is_none() || x.map(|f| f.is_nan()).unwrap_or(false)))
                    .collect(),
                Vector::Character(vals) => vals.iter().map(|x| Some(x.is_none())).collect(),
            };
            Ok(RValue::vec(Vector::Logical(result.into())))
        }
        _ => Ok(RValue::vec(Vector::Logical(vec![Some(false)].into()))),
    }
}

#[builtin(min_args = 1)]
fn builtin_is_numeric(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let r = matches!(
        args.first(),
        Some(RValue::Vector(rv)) if matches!(rv.inner, Vector::Double(_) | Vector::Integer(_))
    );
    Ok(RValue::vec(Vector::Logical(vec![Some(r)].into())))
}

#[builtin(min_args = 1)]
fn builtin_is_character(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let r = matches!(args.first(), Some(RValue::Vector(rv)) if matches!(rv.inner, Vector::Character(_)));
    Ok(RValue::vec(Vector::Logical(vec![Some(r)].into())))
}

#[builtin(min_args = 1)]
fn builtin_is_logical(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let r =
        matches!(args.first(), Some(RValue::Vector(rv)) if matches!(rv.inner, Vector::Logical(_)));
    Ok(RValue::vec(Vector::Logical(vec![Some(r)].into())))
}

#[builtin(min_args = 1)]
fn builtin_is_integer(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let r =
        matches!(args.first(), Some(RValue::Vector(rv)) if matches!(rv.inner, Vector::Integer(_)));
    Ok(RValue::vec(Vector::Logical(vec![Some(r)].into())))
}

#[builtin(min_args = 1)]
fn builtin_is_double(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let r =
        matches!(args.first(), Some(RValue::Vector(rv)) if matches!(rv.inner, Vector::Double(_)));
    Ok(RValue::vec(Vector::Logical(vec![Some(r)].into())))
}

#[builtin(min_args = 1, names = ["is.primitive"])]
fn builtin_is_function(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let r = matches!(args.first(), Some(RValue::Function(_)));
    Ok(RValue::vec(Vector::Logical(vec![Some(r)].into())))
}

#[builtin(min_args = 1)]
fn builtin_is_vector(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let r = matches!(args.first(), Some(RValue::Vector(_)));
    Ok(RValue::vec(Vector::Logical(vec![Some(r)].into())))
}

#[builtin(min_args = 1)]
fn builtin_is_list(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let r = matches!(args.first(), Some(RValue::List(_)));
    Ok(RValue::vec(Vector::Logical(vec![Some(r)].into())))
}

#[builtin(min_args = 1, names = ["as.double"])]
fn builtin_as_numeric(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(v)) => Ok(RValue::vec(Vector::Double(v.to_doubles().into()))),
        Some(RValue::Null) => Ok(RValue::vec(Vector::Double(vec![].into()))),
        _ => Ok(RValue::vec(Vector::Double(vec![None].into()))),
    }
}

#[builtin(min_args = 1)]
fn builtin_as_integer(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(v)) => Ok(RValue::vec(Vector::Integer(v.to_integers().into()))),
        Some(RValue::Null) => Ok(RValue::vec(Vector::Integer(vec![].into()))),
        _ => Ok(RValue::vec(Vector::Integer(vec![None].into()))),
    }
}

#[builtin(min_args = 1)]
fn builtin_as_character(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(v)) => Ok(RValue::vec(Vector::Character(v.to_characters().into()))),
        Some(RValue::Null) => Ok(RValue::vec(Vector::Character(vec![].into()))),
        _ => Ok(RValue::vec(Vector::Character(vec![None].into()))),
    }
}

#[builtin(min_args = 1)]
fn builtin_as_logical(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(v)) => Ok(RValue::vec(Vector::Logical(v.to_logicals().into()))),
        Some(RValue::Null) => Ok(RValue::vec(Vector::Logical(vec![].into()))),
        _ => Ok(RValue::vec(Vector::Logical(vec![None].into()))),
    }
}

#[builtin(min_args = 1)]
fn builtin_names(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(rv)) => match rv.get_attr("names") {
            Some(v) => Ok(v.clone()),
            None => Ok(RValue::Null),
        },
        Some(RValue::List(l)) => {
            if let Some(names_attr) = l.get_attr("names") {
                return Ok(names_attr.clone());
            }
            let names: Vec<Option<String>> = l.values.iter().map(|(n, _)| n.clone()).collect();
            if names.iter().all(|n| n.is_none()) {
                Ok(RValue::Null)
            } else {
                Ok(RValue::vec(Vector::Character(names.into())))
            }
        }
        _ => Ok(RValue::Null),
    }
}

#[builtin(name = "names<-", min_args = 2)]
fn builtin_names_set(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let names_val = args.get(1).cloned().unwrap_or(RValue::Null);
    match args.first() {
        Some(RValue::Vector(rv)) => {
            let mut rv = rv.clone();
            if names_val.is_null() {
                rv.attrs.as_mut().map(|a| a.remove("names"));
            } else {
                rv.set_attr("names".to_string(), names_val);
            }
            Ok(RValue::Vector(rv))
        }
        Some(RValue::List(l)) => {
            let mut l = l.clone();
            if let Some(names_vec) = names_val.as_vector() {
                let names = names_vec.to_characters();
                for (i, name) in names.iter().enumerate() {
                    if i < l.values.len() {
                        l.values[i].0 = name.clone();
                    }
                }
            } else if names_val.is_null() {
                for entry in &mut l.values {
                    entry.0 = None;
                }
            }
            Ok(RValue::List(l))
        }
        other => Ok(other.cloned().unwrap_or(RValue::Null)),
    }
}

#[builtin(name = "class<-", min_args = 2)]
fn builtin_class_set(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let class_val = args.get(1).cloned().unwrap_or(RValue::Null);
    match args.first() {
        Some(RValue::Vector(rv)) => {
            let mut rv = rv.clone();
            if class_val.is_null() {
                rv.attrs.as_mut().map(|a| a.remove("class"));
            } else {
                rv.set_attr("class".to_string(), class_val);
            }
            Ok(RValue::Vector(rv))
        }
        Some(RValue::List(l)) => {
            let mut l = l.clone();
            if class_val.is_null() {
                l.attrs.as_mut().map(|a| a.remove("class"));
            } else {
                l.set_attr("class".to_string(), class_val);
            }
            Ok(RValue::List(l))
        }
        other => Ok(other.cloned().unwrap_or(RValue::Null)),
    }
}

#[builtin(min_args = 1)]
fn builtin_typeof(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let t = args.first().map(|v| v.type_name()).unwrap_or("NULL");
    Ok(RValue::vec(Vector::Character(
        vec![Some(t.to_string())].into(),
    )))
}

#[builtin(min_args = 1)]
fn builtin_class(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    // Check for explicit class attribute on vectors
    if let Some(RValue::Vector(rv)) = args.first() {
        if let Some(cls) = rv.get_attr("class") {
            return Ok(cls.clone());
        }
    }
    // Check for explicit class attribute on lists
    if let Some(RValue::List(l)) = args.first() {
        if let Some(cls) = l.get_attr("class") {
            return Ok(cls.clone());
        }
    }
    let c = match args.first() {
        Some(RValue::Vector(rv)) => match &rv.inner {
            Vector::Logical(_) => "logical",
            Vector::Integer(_) => "integer",
            Vector::Double(_) => "numeric",
            Vector::Character(_) => "character",
        },
        Some(RValue::List(_)) => "list",
        Some(RValue::Function(_)) => "function",
        Some(RValue::Null) => "NULL",
        _ => "NULL",
    };
    Ok(RValue::vec(Vector::Character(
        vec![Some(c.to_string())].into(),
    )))
}

#[builtin(min_args = 1)]
fn builtin_mode(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let m = match args.first() {
        Some(RValue::Vector(rv)) => match &rv.inner {
            Vector::Logical(_) => "logical",
            Vector::Integer(_) | Vector::Double(_) => "numeric",
            Vector::Character(_) => "character",
        },
        Some(RValue::List(_)) => "list",
        Some(RValue::Function(_)) => "function",
        Some(RValue::Null) => "NULL",
        _ => "NULL",
    };
    Ok(RValue::vec(Vector::Character(
        vec![Some(m.to_string())].into(),
    )))
}

#[builtin(min_args = 1)]
fn builtin_str(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(val) => {
            match val {
                RValue::Vector(v) => {
                    let len = v.len();
                    let type_name = v.type_name();
                    let preview: String = match &v.inner {
                        Vector::Double(vals) => vals
                            .iter()
                            .take(10)
                            .map(|x| match x {
                                Some(f) => format_r_double(*f),
                                None => "NA".to_string(),
                            })
                            .collect::<Vec<_>>()
                            .join(" "),
                        Vector::Integer(vals) => vals
                            .iter()
                            .take(10)
                            .map(|x| match x {
                                Some(i) => i.to_string(),
                                None => "NA".to_string(),
                            })
                            .collect::<Vec<_>>()
                            .join(" "),
                        Vector::Logical(vals) => vals
                            .iter()
                            .take(10)
                            .map(|x| match x {
                                Some(true) => "TRUE".to_string(),
                                Some(false) => "FALSE".to_string(),
                                None => "NA".to_string(),
                            })
                            .collect::<Vec<_>>()
                            .join(" "),
                        Vector::Character(vals) => vals
                            .iter()
                            .take(10)
                            .map(|x| match x {
                                Some(s) => format!("\"{}\"", s),
                                None => "NA".to_string(),
                            })
                            .collect::<Vec<_>>()
                            .join(" "),
                    };
                    println!(" {} [1:{}] {}", type_name, len, preview);
                }
                RValue::List(l) => println!("List of {}", l.values.len()),
                RValue::Null => println!(" NULL"),
                _ => println!(" {}", val),
            }
            Ok(RValue::Null)
        }
        None => Ok(RValue::Null),
    }
}

#[builtin(min_args = 2)]
fn builtin_identical(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    if args.len() < 2 {
        return Err(RError::Argument("need 2 arguments".to_string()));
    }
    let result = format!("{:?}", args[0]) == format!("{:?}", args[1]);
    Ok(RValue::vec(Vector::Logical(vec![Some(result)].into())))
}

#[builtin(min_args = 2)]
fn builtin_all_equal(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let tolerance = named
        .iter()
        .find(|(n, _)| n == "tolerance")
        .and_then(|(_, v)| v.as_vector()?.as_double_scalar())
        .unwrap_or(1.5e-8);

    if args.len() < 2 {
        return Err(RError::Argument("need 2 arguments".to_string()));
    }
    match (&args[0], &args[1]) {
        (RValue::Vector(v1), RValue::Vector(v2)) => {
            let d1 = v1.to_doubles();
            let d2 = v2.to_doubles();
            if d1.len() != d2.len() {
                return Ok(RValue::vec(Vector::Character(
                    vec![Some(format!("lengths ({}, {}) differ", d1.len(), d2.len()))].into(),
                )));
            }
            for (a, b) in d1.iter().zip(d2.iter()) {
                match (a, b) {
                    (Some(a), Some(b)) if (a - b).abs() > tolerance => {
                        return Ok(RValue::vec(Vector::Character(
                            vec![Some(format!("Mean relative difference: {}", (a - b).abs()))]
                                .into(),
                        )));
                    }
                    _ => {}
                }
            }
            Ok(RValue::vec(Vector::Logical(vec![Some(true)].into())))
        }
        _ => {
            let result = format!("{:?}", args[0]) == format!("{:?}", args[1]);
            Ok(RValue::vec(Vector::Logical(vec![Some(result)].into())))
        }
    }
}

#[builtin]
fn builtin_any(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let na_rm = named
        .iter()
        .find(|(n, _)| n == "na.rm")
        .and_then(|(_, v)| v.as_vector()?.as_logical_scalar())
        .unwrap_or(false);

    for arg in args {
        if let Some(v) = arg.as_vector() {
            for l in v.to_logicals() {
                match l {
                    Some(true) => return Ok(RValue::vec(Vector::Logical(vec![Some(true)].into()))),
                    None if !na_rm => return Ok(RValue::vec(Vector::Logical(vec![None].into()))),
                    _ => {}
                }
            }
        }
    }
    Ok(RValue::vec(Vector::Logical(vec![Some(false)].into())))
}

#[builtin]
fn builtin_all(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let na_rm = named
        .iter()
        .find(|(n, _)| n == "na.rm")
        .and_then(|(_, v)| v.as_vector()?.as_logical_scalar())
        .unwrap_or(false);

    for arg in args {
        if let Some(v) = arg.as_vector() {
            for l in v.to_logicals() {
                match l {
                    Some(false) => {
                        return Ok(RValue::vec(Vector::Logical(vec![Some(false)].into())))
                    }
                    None if !na_rm => return Ok(RValue::vec(Vector::Logical(vec![None].into()))),
                    _ => {}
                }
            }
        }
    }
    Ok(RValue::vec(Vector::Logical(vec![Some(true)].into())))
}

#[builtin(min_args = 2)]
fn builtin_xor(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    if args.len() < 2 {
        return Err(RError::Argument("need 2 arguments".to_string()));
    }
    let a = args[0].as_vector().and_then(|v| v.as_logical_scalar());
    let b = args[1].as_vector().and_then(|v| v.as_logical_scalar());
    match (a, b) {
        (Some(a), Some(b)) => Ok(RValue::vec(Vector::Logical(vec![Some(a ^ b)].into()))),
        _ => Ok(RValue::vec(Vector::Logical(vec![None].into()))),
    }
}

#[builtin]
fn builtin_list(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let mut values: Vec<(Option<String>, RValue)> = Vec::new();
    for arg in args {
        values.push((None, arg.clone()));
    }
    for (name, val) in named {
        values.push((Some(name.clone()), val.clone()));
    }
    Ok(RValue::List(RList::new(values)))
}

#[builtin(min_args = 1)]
fn builtin_vector(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let mode = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_else(|| "logical".to_string());
    let length = args
        .get(1)
        .and_then(|v| v.as_vector()?.as_integer_scalar())
        .unwrap_or(0) as usize;
    match mode.as_str() {
        "numeric" | "double" => Ok(RValue::vec(Vector::Double(vec![Some(0.0); length].into()))),
        "integer" => Ok(RValue::vec(Vector::Integer(vec![Some(0); length].into()))),
        "character" => Ok(RValue::vec(Vector::Character(
            vec![Some(String::new()); length].into(),
        ))),
        "logical" => Ok(RValue::vec(Vector::Logical(
            vec![Some(false); length].into(),
        ))),
        "list" => Ok(RValue::List(RList::new(vec![(None, RValue::Null); length]))),
        _ => Ok(RValue::vec(Vector::Logical(
            vec![Some(false); length].into(),
        ))),
    }
}

#[builtin(min_args = 1)]
fn builtin_as_list(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::List(l)) => Ok(RValue::List(l.clone())),
        Some(RValue::Vector(v)) => {
            let values: Vec<(Option<String>, RValue)> = match &v.inner {
                Vector::Double(vals) => vals
                    .iter()
                    .map(|x| (None, RValue::vec(Vector::Double(vec![*x].into()))))
                    .collect(),
                Vector::Integer(vals) => vals
                    .iter()
                    .map(|x| (None, RValue::vec(Vector::Integer(vec![*x].into()))))
                    .collect(),
                Vector::Logical(vals) => vals
                    .iter()
                    .map(|x| (None, RValue::vec(Vector::Logical(vec![*x].into()))))
                    .collect(),
                Vector::Character(vals) => vals
                    .iter()
                    .map(|x| (None, RValue::vec(Vector::Character(vec![x.clone()].into()))))
                    .collect(),
            };
            Ok(RValue::List(RList::new(values)))
        }
        Some(RValue::Null) => Ok(RValue::List(RList::new(vec![]))),
        _ => Ok(RValue::List(RList::new(vec![]))),
    }
}

#[builtin(min_args = 1)]
fn builtin_unlist(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::List(l)) => {
            let mut all_vals = Vec::new();
            for (_, v) in &l.values {
                all_vals.push(v.clone());
            }
            builtin_c(&all_vals, &[])
        }
        Some(other) => Ok(other.clone()),
        None => Ok(RValue::Null),
    }
}

#[builtin]
fn builtin_stop(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let msg = args
        .iter()
        .map(|v| match v {
            RValue::Vector(vec) => vec.as_character_scalar().unwrap_or_default(),
            other => format!("{}", other),
        })
        .collect::<Vec<_>>()
        .join("");
    Err(RError::Other(msg))
}

#[builtin]
fn builtin_warning(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let msg = args
        .iter()
        .map(|v| match v {
            RValue::Vector(vec) => vec.as_character_scalar().unwrap_or_default(),
            other => format!("{}", other),
        })
        .collect::<Vec<_>>()
        .join("");
    eprintln!("Warning message:\n{}", msg);
    Ok(RValue::Null)
}

#[builtin]
fn builtin_message(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let msg = args
        .iter()
        .map(|v| match v {
            RValue::Vector(vec) => vec.as_character_scalar().unwrap_or_default(),
            other => format!("{}", other),
        })
        .collect::<Vec<_>>()
        .join("");
    eprintln!("{}", msg);
    Ok(RValue::Null)
}

#[builtin(min_args = 1)]
fn builtin_invisible(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    Ok(args.first().cloned().unwrap_or(RValue::Null))
}

#[builtin(min_args = 3)]
fn builtin_ifelse(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    if args.len() < 3 {
        return Err(RError::Argument("need 3 arguments".to_string()));
    }
    let test = args[0].as_vector().and_then(|v| v.as_logical_scalar());
    match test {
        Some(true) => Ok(args[1].clone()),
        Some(false) => Ok(args[2].clone()),
        None => Ok(RValue::vec(Vector::Logical(vec![None].into()))),
    }
}

#[builtin(min_args = 2, names = ["pmatch", "charmatch"])]
fn builtin_match(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    if args.len() < 2 {
        return Err(RError::Argument("need 2 arguments".to_string()));
    }
    let x = match &args[0] {
        RValue::Vector(v) => v.to_characters(),
        _ => return Ok(RValue::vec(Vector::Integer(vec![None].into()))),
    };
    let table = match &args[1] {
        RValue::Vector(v) => v.to_characters(),
        _ => return Ok(RValue::vec(Vector::Integer(vec![None].into()))),
    };

    let result: Vec<Option<i64>> = x
        .iter()
        .map(|xi| {
            xi.as_ref().and_then(|xi| {
                table
                    .iter()
                    .position(|t| t.as_ref() == Some(xi))
                    .map(|p| p as i64 + 1)
            })
        })
        .collect();
    Ok(RValue::vec(Vector::Integer(result.into())))
}

#[builtin(min_args = 3)]
fn builtin_replace(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    if args.len() < 3 {
        return Err(RError::Argument("need 3 arguments".to_string()));
    }
    match &args[0] {
        RValue::Vector(v) => {
            let mut doubles = v.to_doubles();
            let indices = args[1]
                .as_vector()
                .map(|v| v.to_integers())
                .unwrap_or_default();
            let values = args[2]
                .as_vector()
                .map(|v| v.to_doubles())
                .unwrap_or_default();
            for (i, idx) in indices.iter().enumerate() {
                if let Some(idx) = idx {
                    let idx = *idx as usize - 1;
                    if idx < doubles.len() {
                        doubles[idx] = values
                            .get(i % values.len())
                            .copied()
                            .flatten()
                            .map(Some)
                            .unwrap_or(None);
                    }
                }
            }
            Ok(RValue::vec(Vector::Double(doubles.into())))
        }
        _ => Ok(args[0].clone()),
    }
}

#[builtin]
fn builtin_options(_args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    Ok(RValue::List(RList::new(vec![])))
}

#[builtin(name = "getOption", min_args = 1)]
fn builtin_get_option(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let name = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();
    match name.as_str() {
        "digits" => Ok(RValue::vec(Vector::Integer(vec![Some(7)].into()))),
        "warn" => Ok(RValue::vec(Vector::Integer(vec![Some(0)].into()))),
        "OutDec" => Ok(RValue::vec(Vector::Character(
            vec![Some(".".to_string())].into(),
        ))),
        _ => Ok(RValue::Null),
    }
}

#[builtin(name = "Sys.time")]
fn builtin_sys_time(_args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    Ok(RValue::vec(Vector::Double(vec![Some(secs)].into())))
}

#[builtin]
fn builtin_proc_time(_args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    // R returns c(user.self, sys.self, elapsed) — we approximate with wall time
    Ok(RValue::vec(Vector::Double(
        vec![Some(secs), Some(0.0), Some(secs)].into(),
    )))
}

#[builtin(name = "Sys.sleep", min_args = 1)]
fn builtin_sys_sleep(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    if let Some(secs) = args.first().and_then(|v| v.as_vector()?.as_double_scalar()) {
        std::thread::sleep(std::time::Duration::from_secs_f64(secs));
    }
    Ok(RValue::Null)
}

#[builtin]
fn builtin_readline(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let prompt = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();
    print!("{}", prompt);
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).ok();
    Ok(RValue::vec(Vector::Character(
        vec![Some(input.trim_end().to_string())].into(),
    )))
}

#[builtin(name = "Sys.getenv")]
fn builtin_sys_getenv(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let name = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();
    let val = std::env::var(&name).unwrap_or_default();
    Ok(RValue::vec(Vector::Character(vec![Some(val)].into())))
}

#[builtin]
fn builtin_file_path(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let sep = named
        .iter()
        .find(|(n, _)| n == "fsep")
        .and_then(|(_, v)| v.as_vector()?.as_character_scalar())
        .unwrap_or_else(|| "/".to_string());

    let parts: Vec<String> = args
        .iter()
        .filter_map(|v| v.as_vector()?.as_character_scalar())
        .collect();
    Ok(RValue::vec(Vector::Character(
        vec![Some(parts.join(&sep))].into(),
    )))
}

#[builtin(name = "file.exists", min_args = 1)]
fn builtin_file_exists(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let results: Vec<Option<bool>> = args
        .iter()
        .map(|arg| {
            let path = arg
                .as_vector()
                .and_then(|v| v.as_character_scalar())
                .unwrap_or_default();
            Some(std::path::Path::new(&path).exists())
        })
        .collect();
    Ok(RValue::vec(Vector::Logical(results.into())))
}

#[builtin(name = "readLines", min_args = 1)]
fn builtin_read_lines(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let path = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| RError::Argument("invalid 'con' argument".to_string()))?;
    let n = named
        .iter()
        .find(|(n, _)| n == "n")
        .or_else(|| named.iter().find(|(n, _)| n == "n"))
        .and_then(|(_, v)| v.as_vector()?.as_integer_scalar())
        .unwrap_or(-1);

    let content = std::fs::read_to_string(&path)
        .map_err(|e| RError::Other(format!("cannot open connection: {}", e)))?;
    let lines: Vec<Option<String>> = if n < 0 {
        content.lines().map(|l| Some(l.to_string())).collect()
    } else {
        content
            .lines()
            .take(n as usize)
            .map(|l| Some(l.to_string()))
            .collect()
    };
    Ok(RValue::vec(Vector::Character(lines.into())))
}

#[builtin(name = "writeLines", min_args = 1)]
fn builtin_write_lines(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let text = args
        .first()
        .and_then(|v| v.as_vector())
        .map(|v| v.to_characters())
        .unwrap_or_default();
    let con = args
        .get(1)
        .or_else(|| named.iter().find(|(n, _)| n == "con").map(|(_, v)| v))
        .and_then(|v| v.as_vector()?.as_character_scalar());
    let sep = named
        .iter()
        .find(|(n, _)| n == "sep")
        .and_then(|(_, v)| v.as_vector()?.as_character_scalar())
        .unwrap_or_else(|| "\n".to_string());

    let output: String = text
        .iter()
        .map(|s| s.clone().unwrap_or_else(|| "NA".to_string()))
        .collect::<Vec<_>>()
        .join(&sep);

    match con {
        Some(path) => {
            std::fs::write(&path, format!("{}{}", output, sep))
                .map_err(|e| RError::Other(format!("cannot open connection: {}", e)))?;
        }
        None => {
            // Write to stdout
            println!("{}", output);
        }
    }
    Ok(RValue::Null)
}

#[builtin(name = "require", min_args = 1, names = ["library"])]
fn builtin_require_stub(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let pkg = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();
    eprintln!(
        "Warning: package '{}' is not available in this R implementation",
        pkg
    );
    Ok(RValue::vec(Vector::Logical(vec![Some(false)].into())))
}

#[builtin(name = "R.Version", names = ["version"])]
fn builtin_r_version(_args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    Ok(RValue::List(RList::new(vec![
        (
            Some("major".to_string()),
            RValue::vec(Vector::Character(vec![Some("0".to_string())].into())),
        ),
        (
            Some("minor".to_string()),
            RValue::vec(Vector::Character(vec![Some("1.0".to_string())].into())),
        ),
        (
            Some("language".to_string()),
            RValue::vec(Vector::Character(vec![Some("R".to_string())].into())),
        ),
        (
            Some("engine".to_string()),
            RValue::vec(Vector::Character(
                vec![Some("newr (Rust)".to_string())].into(),
            )),
        ),
    ])))
}

#[builtin(min_args = 1)]
fn builtin_is_recursive(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let r = matches!(args.first(), Some(RValue::List(_)));
    Ok(RValue::vec(Vector::Logical(vec![Some(r)].into())))
}

#[builtin(min_args = 1)]
fn builtin_is_atomic(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let r = matches!(args.first(), Some(RValue::Vector(_) | RValue::Null));
    Ok(RValue::vec(Vector::Logical(vec![Some(r)].into())))
}

#[builtin(min_args = 1)]
fn builtin_is_finite(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(v)) => {
            let result: Vec<Option<bool>> = v
                .to_doubles()
                .iter()
                .map(|x| Some(x.map(|f| f.is_finite()).unwrap_or(false)))
                .collect();
            Ok(RValue::vec(Vector::Logical(result.into())))
        }
        _ => Ok(RValue::vec(Vector::Logical(vec![Some(false)].into()))),
    }
}

#[builtin(min_args = 1)]
fn builtin_is_infinite(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(v)) => {
            let result: Vec<Option<bool>> = v
                .to_doubles()
                .iter()
                .map(|x| Some(x.map(|f| f.is_infinite()).unwrap_or(false)))
                .collect();
            Ok(RValue::vec(Vector::Logical(result.into())))
        }
        _ => Ok(RValue::vec(Vector::Logical(vec![Some(false)].into()))),
    }
}

#[builtin(min_args = 1)]
fn builtin_is_nan(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(v)) => {
            let result: Vec<Option<bool>> = v
                .to_doubles()
                .iter()
                .map(|x| Some(x.map(|f| f.is_nan()).unwrap_or(false)))
                .collect();
            Ok(RValue::vec(Vector::Logical(result.into())))
        }
        _ => Ok(RValue::vec(Vector::Logical(vec![Some(false)].into()))),
    }
}

#[builtin(min_args = 2)]
fn builtin_setdiff(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    if args.len() < 2 {
        return Err(RError::Argument("need 2 arguments".to_string()));
    }
    let x = args[0]
        .as_vector()
        .map(|v| v.to_characters())
        .unwrap_or_default();
    let y = args[1]
        .as_vector()
        .map(|v| v.to_characters())
        .unwrap_or_default();
    let result: Vec<Option<String>> = x.into_iter().filter(|xi| !y.contains(xi)).collect();
    Ok(RValue::vec(Vector::Character(result.into())))
}

#[builtin(min_args = 2)]
fn builtin_intersect(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    if args.len() < 2 {
        return Err(RError::Argument("need 2 arguments".to_string()));
    }
    let x = args[0]
        .as_vector()
        .map(|v| v.to_characters())
        .unwrap_or_default();
    let y = args[1]
        .as_vector()
        .map(|v| v.to_characters())
        .unwrap_or_default();
    let result: Vec<Option<String>> = x.into_iter().filter(|xi| y.contains(xi)).collect();
    Ok(RValue::vec(Vector::Character(result.into())))
}

#[builtin(min_args = 2)]
fn builtin_union(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    if args.len() < 2 {
        return Err(RError::Argument("need 2 arguments".to_string()));
    }
    let x = args[0]
        .as_vector()
        .map(|v| v.to_characters())
        .unwrap_or_default();
    let y = args[1]
        .as_vector()
        .map(|v| v.to_characters())
        .unwrap_or_default();
    let mut result = x;
    for yi in y {
        if !result.contains(&yi) {
            result.push(yi);
        }
    }
    Ok(RValue::vec(Vector::Character(result.into())))
}

#[builtin(min_args = 1)]
fn builtin_duplicated(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(v)) => {
            let chars = v.to_characters();
            let mut seen = Vec::new();
            let result: Vec<Option<bool>> = chars
                .iter()
                .map(|x| {
                    let key = format!("{:?}", x);
                    if seen.contains(&key) {
                        Some(true)
                    } else {
                        seen.push(key);
                        Some(false)
                    }
                })
                .collect();
            Ok(RValue::vec(Vector::Logical(result.into())))
        }
        _ => Ok(RValue::vec(Vector::Logical(vec![].into()))),
    }
}

#[builtin]
fn builtin_getwd(_args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    Ok(RValue::vec(Vector::Character(vec![Some(cwd)].into())))
}

#[builtin(names = ["double"])]
fn builtin_numeric(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let n = args
        .first()
        .and_then(|v| v.as_vector()?.as_integer_scalar())
        .unwrap_or(0) as usize;
    Ok(RValue::vec(Vector::Double(vec![Some(0.0); n].into())))
}

#[builtin]
fn builtin_integer(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let n = args
        .first()
        .and_then(|v| v.as_vector()?.as_integer_scalar())
        .unwrap_or(0) as usize;
    Ok(RValue::vec(Vector::Integer(vec![Some(0); n].into())))
}

#[builtin]
fn builtin_logical(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let n = args
        .first()
        .and_then(|v| v.as_vector()?.as_integer_scalar())
        .unwrap_or(0) as usize;
    Ok(RValue::vec(Vector::Logical(vec![Some(false); n].into())))
}

#[builtin]
fn builtin_character(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let n = args
        .first()
        .and_then(|v| v.as_vector()?.as_integer_scalar())
        .unwrap_or(0) as usize;
    Ok(RValue::vec(Vector::Character(
        vec![Some(String::new()); n].into(),
    )))
}

#[builtin]
fn builtin_matrix(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let data = args
        .first()
        .cloned()
        .unwrap_or(RValue::vec(Vector::Double(vec![Some(f64::NAN)].into())));
    let nrow_arg = named
        .iter()
        .find(|(n, _)| n == "nrow")
        .map(|(_, v)| v)
        .or(args.get(1))
        .and_then(|v| v.as_vector()?.as_integer_scalar());
    let ncol_arg = named
        .iter()
        .find(|(n, _)| n == "ncol")
        .map(|(_, v)| v)
        .or(args.get(2))
        .and_then(|v| v.as_vector()?.as_integer_scalar());
    let byrow = named
        .iter()
        .find(|(n, _)| n == "byrow")
        .map(|(_, v)| v)
        .or(args.get(3))
        .and_then(|v| v.as_vector()?.as_logical_scalar())
        .unwrap_or(false);

    let data_vec = match &data {
        RValue::Vector(v) => v.to_doubles(),
        _ => vec![Some(f64::NAN)],
    };
    let data_len = data_vec.len();

    let (nrow, ncol) = match (nrow_arg, ncol_arg) {
        (Some(r), Some(c)) => (r as usize, c as usize),
        (Some(r), None) => {
            let r = r as usize;
            (r, if r > 0 { data_len.div_ceil(r) } else { 0 })
        }
        (None, Some(c)) => {
            let c = c as usize;
            (if c > 0 { data_len.div_ceil(c) } else { 0 }, c)
        }
        (None, None) => (data_len, 1),
    };

    let total = nrow * ncol;
    let mut mat = Vec::with_capacity(total);
    if byrow {
        for i in 0..nrow {
            for j in 0..ncol {
                let idx = (i * ncol + j) % data_len;
                mat.push(data_vec[idx]);
            }
        }
    } else {
        for idx in 0..total {
            mat.push(data_vec[idx % data_len]);
        }
    }

    let mut list = RList::new(vec![(None, RValue::vec(Vector::Double(mat.into())))]);
    list.set_attr(
        "class".to_string(),
        RValue::vec(Vector::Character(
            vec![Some("matrix".to_string()), Some("array".to_string())].into(),
        )),
    );
    list.set_attr(
        "dim".to_string(),
        RValue::vec(Vector::Integer(
            vec![Some(nrow as i64), Some(ncol as i64)].into(),
        )),
    );
    Ok(RValue::List(list))
}

#[builtin(min_args = 1)]
fn builtin_dim(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::List(l)) => Ok(l.get_attr("dim").cloned().unwrap_or(RValue::Null)),
        _ => Ok(RValue::Null),
    }
}

#[builtin(min_args = 1)]
fn builtin_nrow(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::List(l)) => {
            if let Some(RValue::Vector(rv)) = l.get_attr("dim") {
                if let Vector::Integer(dims) = &rv.inner {
                    if !dims.is_empty() {
                        return Ok(RValue::vec(Vector::Integer(vec![dims[0]].into())));
                    }
                }
            }
            if let Some(rn) = l.get_attr("row.names") {
                return Ok(RValue::vec(Vector::Integer(
                    vec![Some(rn.length() as i64)].into(),
                )));
            }
            Ok(RValue::Null)
        }
        _ => Ok(RValue::Null),
    }
}

#[builtin(min_args = 1)]
fn builtin_ncol(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::List(l)) => {
            if let Some(RValue::Vector(rv)) = l.get_attr("dim") {
                if let Vector::Integer(dims) = &rv.inner {
                    if dims.len() >= 2 {
                        return Ok(RValue::vec(Vector::Integer(vec![dims[1]].into())));
                    }
                }
            }
            if has_class(args.first().unwrap(), "data.frame") {
                return Ok(RValue::vec(Vector::Integer(
                    vec![Some(l.values.len() as i64)].into(),
                )));
            }
            Ok(RValue::Null)
        }
        _ => Ok(RValue::Null),
    }
}

#[builtin(name = "nrow", min_args = 1, names = ["NROW"])]
fn builtin_nrow_safe(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::List(l)) => {
            if let Some(RValue::Vector(rv)) = l.get_attr("dim") {
                if let Vector::Integer(dims) = &rv.inner {
                    if !dims.is_empty() {
                        return Ok(RValue::vec(Vector::Integer(vec![dims[0]].into())));
                    }
                }
            }
            Ok(RValue::vec(Vector::Integer(
                vec![Some(l.values.len() as i64)].into(),
            )))
        }
        Some(RValue::Vector(v)) => Ok(RValue::vec(Vector::Integer(
            vec![Some(v.len() as i64)].into(),
        ))),
        Some(RValue::Null) => Ok(RValue::vec(Vector::Integer(vec![Some(0)].into()))),
        _ => Ok(RValue::vec(Vector::Integer(vec![Some(1)].into()))),
    }
}

#[builtin(name = "ncol", min_args = 1, names = ["NCOL"])]
fn builtin_ncol_safe(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::List(l)) => {
            if let Some(RValue::Vector(rv)) = l.get_attr("dim") {
                if let Vector::Integer(dims) = &rv.inner {
                    if dims.len() >= 2 {
                        return Ok(RValue::vec(Vector::Integer(vec![dims[1]].into())));
                    }
                }
            }
            Ok(RValue::vec(Vector::Integer(vec![Some(1)].into())))
        }
        Some(RValue::Null) => Ok(RValue::vec(Vector::Integer(vec![Some(0)].into()))),
        _ => Ok(RValue::vec(Vector::Integer(vec![Some(1)].into()))),
    }
}

#[builtin(min_args = 1)]
fn builtin_t(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::List(l)) => {
            let dims = if let Some(RValue::Vector(rv)) = l.get_attr("dim") {
                if let Vector::Integer(d) = &rv.inner {
                    if d.len() >= 2 {
                        (d[0].unwrap_or(0) as usize, d[1].unwrap_or(0) as usize)
                    } else {
                        return Ok(args[0].clone());
                    }
                } else {
                    return Ok(args[0].clone());
                }
            } else {
                return Ok(args[0].clone());
            };
            let (nrow, ncol) = dims;
            let data = if let Some((_, RValue::Vector(rv))) = l.values.first() {
                if let Vector::Double(v) = &rv.inner {
                    v.clone()
                } else {
                    return Ok(args[0].clone());
                }
            } else {
                return Ok(args[0].clone());
            };
            let mut transposed = vec![Some(0.0f64); nrow * ncol];
            for i in 0..nrow {
                for j in 0..ncol {
                    transposed[j * nrow + i] = data
                        .get(i + j * nrow)
                        .copied()
                        .flatten()
                        .map(Some)
                        .unwrap_or(None);
                }
            }
            let mut result =
                RList::new(vec![(None, RValue::vec(Vector::Double(transposed.into())))]);
            result.set_attr(
                "class".to_string(),
                RValue::vec(Vector::Character(
                    vec![Some("matrix".to_string()), Some("array".to_string())].into(),
                )),
            );
            result.set_attr(
                "dim".to_string(),
                RValue::vec(Vector::Integer(
                    vec![Some(ncol as i64), Some(nrow as i64)].into(),
                )),
            );
            Ok(RValue::List(result))
        }
        _ => Ok(args.first().cloned().unwrap_or(RValue::Null)),
    }
}

#[builtin(min_args = 2)]
fn builtin_attr(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let which = args
        .get(1)
        .and_then(|v| v.as_vector())
        .and_then(|v| v.as_character_scalar())
        .ok_or_else(|| RError::Argument("'which' must be a character string".to_string()))?;
    match args.first() {
        Some(RValue::Vector(rv)) => Ok(rv.get_attr(&which).cloned().unwrap_or(RValue::Null)),
        Some(RValue::List(l)) => Ok(l.get_attr(&which).cloned().unwrap_or(RValue::Null)),
        _ => Ok(RValue::Null),
    }
}

#[builtin(name = "attr<-", min_args = 3)]
fn builtin_attr_set(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let which = args
        .get(1)
        .and_then(|v| v.as_vector())
        .and_then(|v| v.as_character_scalar())
        .ok_or_else(|| RError::Argument("'which' must be a character string".to_string()))?;
    let value = args.get(2).cloned().unwrap_or(RValue::Null);
    match args.first() {
        Some(RValue::Vector(rv)) => {
            let mut rv = rv.clone();
            if value.is_null() {
                rv.attrs.as_mut().map(|a| a.remove(&which));
            } else {
                rv.set_attr(which, value);
            }
            Ok(RValue::Vector(rv))
        }
        Some(RValue::List(l)) => {
            let mut l = l.clone();
            if value.is_null() {
                l.attrs.as_mut().map(|a| a.remove(&which));
            } else {
                l.set_attr(which, value);
            }
            Ok(RValue::List(l))
        }
        other => Ok(other.cloned().unwrap_or(RValue::Null)),
    }
}

#[builtin(min_args = 1)]
fn builtin_attributes(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let attrs = match args.first() {
        Some(RValue::Vector(rv)) => rv.attrs.as_deref(),
        Some(RValue::List(l)) => l.attrs.as_deref(),
        _ => None,
    };
    match attrs {
        Some(a) if !a.is_empty() => {
            let values: Vec<(Option<String>, RValue)> = a
                .iter()
                .map(|(k, v)| (Some(k.clone()), v.clone()))
                .collect();
            Ok(RValue::List(RList::new(values)))
        }
        _ => Ok(RValue::Null),
    }
}

#[builtin(min_args = 1)]
fn builtin_structure(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let base = args.first().cloned().unwrap_or(RValue::Null);
    if named.is_empty() {
        return Ok(base);
    }
    match base {
        RValue::List(mut l) => {
            for (name, value) in named {
                if name == ".Names" || name == "names" {
                    if let RValue::Vector(rv) = value {
                        if let Vector::Character(names) = &rv.inner {
                            for (i, n) in names.iter().enumerate() {
                                if i < l.values.len() {
                                    l.values[i].0 = n.clone();
                                }
                            }
                        }
                    }
                } else {
                    l.set_attr(name.clone(), value.clone());
                }
            }
            Ok(RValue::List(l))
        }
        RValue::Vector(mut rv) => {
            for (name, value) in named {
                if name == ".Names" || name == "names" {
                    rv.set_attr("names".to_string(), value.clone());
                } else {
                    rv.set_attr(name.clone(), value.clone());
                }
            }
            Ok(RValue::Vector(rv))
        }
        other => Ok(other),
    }
}

#[builtin(min_args = 2)]
fn builtin_inherits(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let what = args
        .get(1)
        .and_then(|v| v.as_vector())
        .map(|v| v.to_characters())
        .unwrap_or_default();

    let classes = match args.first() {
        Some(RValue::List(l)) => {
            if let Some(RValue::Vector(rv)) = l.get_attr("class") {
                if let Vector::Character(cls) = &rv.inner {
                    cls.iter().filter_map(|s| s.clone()).collect::<Vec<_>>()
                } else {
                    vec!["list".to_string()]
                }
            } else {
                vec!["list".to_string()]
            }
        }
        Some(RValue::Vector(rv)) => {
            if let Some(cls) = rv.class() {
                cls
            } else {
                match &rv.inner {
                    Vector::Logical(_) => vec!["logical".to_string()],
                    Vector::Integer(_) => vec!["integer".to_string()],
                    Vector::Double(_) => vec!["numeric".to_string()],
                    Vector::Character(_) => vec!["character".to_string()],
                }
            }
        }
        Some(RValue::Function(_)) => vec!["function".to_string()],
        _ => vec![],
    };

    let result = what
        .iter()
        .any(|w| w.as_ref().is_some_and(|w| classes.iter().any(|c| c == w)));
    Ok(RValue::vec(Vector::Logical(vec![Some(result)].into())))
}

fn has_class(val: &RValue, class_name: &str) -> bool {
    if let RValue::List(l) = val {
        if let Some(RValue::Vector(rv)) = l.get_attr("class") {
            if let Vector::Character(cls) = &rv.inner {
                return cls.iter().any(|c| c.as_deref() == Some(class_name));
            }
        }
    }
    false
}

#[builtin(min_args = 1)]
fn builtin_is_factor(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let r = args.first().is_some_and(|v| has_class(v, "factor"));
    Ok(RValue::vec(Vector::Logical(vec![Some(r)].into())))
}

#[builtin(min_args = 1)]
fn builtin_is_data_frame(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let r = args.first().is_some_and(|v| has_class(v, "data.frame"));
    Ok(RValue::vec(Vector::Logical(vec![Some(r)].into())))
}

#[builtin(min_args = 1)]
fn builtin_is_matrix(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let r = args.first().is_some_and(|v| {
        if has_class(v, "matrix") {
            return true;
        }
        // A matrix is any object with a dim attribute of length 2
        if let RValue::List(l) = v {
            if let Some(RValue::Vector(rv)) = l.get_attr("dim") {
                if let Vector::Integer(dims) = &rv.inner {
                    return dims.len() == 2;
                }
            }
        }
        false
    });
    Ok(RValue::vec(Vector::Logical(vec![Some(r)].into())))
}

#[builtin(min_args = 1)]
fn builtin_is_array(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let r = args.first().is_some_and(|v| has_class(v, "array"));
    Ok(RValue::vec(Vector::Logical(vec![Some(r)].into())))
}

#[builtin(min_args = 2)]
fn builtin_is_element(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    if args.len() < 2 {
        return Err(RError::Argument("need 2 arguments".to_string()));
    }
    let x = match &args[0] {
        RValue::Vector(v) => v.to_characters(),
        _ => return Ok(RValue::vec(Vector::Logical(vec![Some(false)].into()))),
    };
    let table = match &args[1] {
        RValue::Vector(v) => v.to_characters(),
        _ => return Ok(RValue::vec(Vector::Logical(vec![Some(false)].into()))),
    };
    let result: Vec<Option<bool>> = x
        .iter()
        .map(|xi| {
            Some(
                xi.as_ref()
                    .is_some_and(|xi| table.iter().any(|t| t.as_ref() == Some(xi))),
            )
        })
        .collect();
    Ok(RValue::vec(Vector::Logical(result.into())))
}

#[builtin]
fn builtin_data_frame(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let mut columns: Vec<(Option<String>, RValue)> = Vec::new();
    let mut max_len = 0usize;

    for (name, value) in named {
        let len = value.length();
        if len > max_len {
            max_len = len;
        }
        columns.push((Some(name.clone()), value.clone()));
    }
    for (i, arg) in args.iter().enumerate() {
        let len = arg.length();
        if len > max_len {
            max_len = len;
        }
        columns.push((Some(format!("V{}", i + 1)), arg.clone()));
    }

    let col_names: Vec<Option<String>> = columns.iter().map(|(n, _)| n.clone()).collect();

    let mut list = RList::new(columns);
    list.set_attr(
        "class".to_string(),
        RValue::vec(Vector::Character(
            vec![Some("data.frame".to_string())].into(),
        )),
    );
    list.set_attr(
        "names".to_string(),
        RValue::vec(Vector::Character(col_names.into())),
    );
    let row_names: Vec<Option<i64>> = (1..=max_len as i64).map(Some).collect();
    list.set_attr(
        "row.names".to_string(),
        RValue::vec(Vector::Integer(row_names.into())),
    );
    Ok(RValue::List(list))
}

#[builtin(name = "environment")]
fn builtin_environment_stub(_args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    Ok(RValue::Null)
}

#[builtin(name = "new.env")]
fn builtin_new_env_stub(_args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    Ok(RValue::Environment(Environment::new_global()))
}

#[builtin(name = "globalenv", names = ["baseenv", "emptyenv", "parent.env"])]
fn builtin_globalenv_stub(_args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    Ok(RValue::Null)
}

#[builtin(name = "nargs", names = ["sys.nframe", "sys.function"])]
fn builtin_nargs_stub(_args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    Ok(RValue::vec(Vector::Integer(vec![Some(0)].into())))
}

#[builtin(name = "isTRUE", min_args = 1)]
fn builtin_is_true(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let result = match args.first() {
        Some(RValue::Vector(rv)) if matches!(rv.inner, Vector::Logical(_)) => {
            let Vector::Logical(v) = &rv.inner else {
                unreachable!()
            };
            v.len() == 1 && v[0] == Some(true)
        }
        _ => false,
    };
    Ok(RValue::vec(Vector::Logical(vec![Some(result)].into())))
}

#[builtin(name = "isFALSE", min_args = 1)]
fn builtin_is_false(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let result = match args.first() {
        Some(RValue::Vector(rv)) if matches!(rv.inner, Vector::Logical(_)) => {
            let Vector::Logical(v) = &rv.inner else {
                unreachable!()
            };
            v.len() == 1 && v[0] == Some(false)
        }
        _ => false,
    };
    Ok(RValue::vec(Vector::Logical(vec![Some(result)].into())))
}

#[builtin]
fn builtin_stopifnot(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    for (i, arg) in args.iter().enumerate() {
        match arg {
            RValue::Vector(rv) if matches!(rv.inner, Vector::Logical(_)) => {
                let Vector::Logical(v) = &rv.inner else {
                    unreachable!()
                };
                for (j, val) in v.iter().enumerate() {
                    match val {
                        Some(true) => {}
                        Some(false) => {
                            return Err(RError::Other(format!(
                                "not all are TRUE (element {} of argument {})",
                                j + 1,
                                i + 1
                            )));
                        }
                        None => {
                            return Err(RError::Other(format!(
                                "missing value where TRUE/FALSE needed (argument {})",
                                i + 1
                            )));
                        }
                    }
                }
            }
            RValue::Vector(v) => {
                if let Some(b) = v.as_logical_scalar() {
                    if !b {
                        return Err(RError::Other(format!("argument {} is not TRUE", i + 1)));
                    }
                }
            }
            _ => {
                return Err(RError::Other(format!(
                    "argument {} is not a logical value",
                    i + 1
                )));
            }
        }
    }
    Ok(RValue::Null)
}

#[builtin(min_args = 1)]
fn builtin_as_vector(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(v)) => Ok(RValue::Vector(v.clone())),
        Some(RValue::List(items)) => Ok(RValue::List(items.clone())),
        Some(RValue::Null) => Ok(RValue::Null),
        _ => Ok(args.first().cloned().unwrap_or(RValue::Null)),
    }
}

#[builtin(min_args = 1)]
fn builtin_unclass(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(rv)) => {
            let mut rv = rv.clone();
            rv.attrs.as_mut().map(|a| a.remove("class"));
            Ok(RValue::Vector(rv))
        }
        Some(RValue::List(l)) => {
            let mut l = l.clone();
            l.attrs.as_mut().map(|a| a.remove("class"));
            Ok(RValue::List(l))
        }
        other => Ok(other.cloned().unwrap_or(RValue::Null)),
    }
}

#[builtin(name = "missing", min_args = 1)]
fn builtin_missing_stub(_args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    Ok(RValue::vec(Vector::Logical(vec![Some(false)].into())))
}

#[builtin(name = "match.arg", min_args = 1)]
fn builtin_match_arg(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let arg = args.first().cloned().unwrap_or(RValue::Null);
    let choices = args
        .get(1)
        .or_else(|| named.iter().find(|(n, _)| n == "choices").map(|(_, v)| v));

    let arg_str = match &arg {
        RValue::Vector(v) => v.as_character_scalar(),
        RValue::Null => None,
        _ => None,
    };

    let choices_vec = match choices {
        Some(RValue::Vector(rv)) if matches!(rv.inner, Vector::Character(_)) => {
            let Vector::Character(v) = &rv.inner else {
                unreachable!()
            };
            v.iter().filter_map(|s| s.clone()).collect::<Vec<_>>()
        }
        Some(RValue::Null) | None => {
            // No choices provided — return arg as-is (R would use formals, we can't)
            return Ok(arg);
        }
        _ => return Ok(arg),
    };

    if choices_vec.is_empty() {
        return Ok(arg);
    }

    match arg_str {
        None => {
            // NULL arg: return first choice (R behavior)
            Ok(RValue::vec(Vector::Character(
                vec![Some(choices_vec[0].clone())].into(),
            )))
        }
        Some(ref s) => {
            // Exact match first
            if choices_vec.contains(s) {
                return Ok(RValue::vec(Vector::Character(vec![Some(s.clone())].into())));
            }
            // Partial match
            let matches: Vec<&String> = choices_vec
                .iter()
                .filter(|c| c.starts_with(s.as_str()))
                .collect();
            match matches.len() {
                1 => Ok(RValue::vec(Vector::Character(
                    vec![Some(matches[0].clone())].into(),
                ))),
                0 => Err(RError::Argument(format!(
                    "'arg' should be one of {}",
                    choices_vec
                        .iter()
                        .map(|c| format!("'{}'", c))
                        .collect::<Vec<_>>()
                        .join(", ")
                ))),
                _ => Err(RError::Argument(format!(
                    "'arg' should be one of {}",
                    choices_vec
                        .iter()
                        .map(|c| format!("'{}'", c))
                        .collect::<Vec<_>>()
                        .join(", ")
                ))),
            }
        }
    }
}

#[builtin(name = "sys.call")]
fn builtin_sys_call_stub(_args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    Ok(RValue::Null)
}

#[builtin(names = ["quit"])]
fn builtin_q(_args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    std::process::exit(0);
}
