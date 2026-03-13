mod conditions;
mod factors;
mod interp;
pub mod io;
pub mod math;
mod pre_eval;
#[cfg(feature = "random")]
mod random;
pub mod strings;
mod stubs;
pub mod system;
mod tables;

use crate::interpreter::environment::Environment;
use crate::interpreter::value::*;
use crate::parser::ast::Arg;
use linkme::distributed_slice;
use minir_macros::builtin;

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
        _ => Err(RError::new(
            RErrorKind::Argument,
            "non-numeric argument to mathematical function".to_string(),
        )),
    }
}

/// Placeholder for interpreter-level builtins — never actually called because
/// dispatch is intercepted by the interpreter/pre-eval registries.
fn placeholder_builtin(_args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    Err(RError::other(
        "internal error: interpreter builtin not intercepted",
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
                RValue::vec(Vector::Integer(vec![Some(i64::from(i32::MAX))].into())),
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

    // .Platform constant
    let os_type = if cfg!(unix) { "unix" } else { "windows" };
    let file_sep = if cfg!(windows) { "\\" } else { "/" };
    let path_sep = if cfg!(windows) { ";" } else { ":" };
    let dynlib_ext = if cfg!(target_os = "macos") {
        ".dylib"
    } else if cfg!(windows) {
        ".dll"
    } else {
        ".so"
    };
    env.set(
        ".Platform".to_string(),
        RValue::List(RList::new(vec![
            (
                Some("OS.type".to_string()),
                RValue::vec(Vector::Character(vec![Some(os_type.to_string())].into())),
            ),
            (
                Some("file.sep".to_string()),
                RValue::vec(Vector::Character(vec![Some(file_sep.to_string())].into())),
            ),
            (
                Some("path.sep".to_string()),
                RValue::vec(Vector::Character(vec![Some(path_sep.to_string())].into())),
            ),
            (
                Some("dynlib.ext".to_string()),
                RValue::vec(Vector::Character(vec![Some(dynlib_ext.to_string())].into())),
            ),
            (
                Some("pkgType".to_string()),
                RValue::vec(Vector::Character(vec![Some("source".to_string())].into())),
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

    // Determine highest type (raw < logical < integer < double < complex < character)
    let mut has_char = false;
    let mut has_complex = false;
    let mut has_double = false;
    let mut has_int = false;
    let mut has_logical = false;
    let mut has_raw = false;

    for val in &all_values {
        match val {
            RValue::Vector(rv) if matches!(rv.inner, Vector::Character(_)) => has_char = true,
            RValue::Vector(rv) if matches!(rv.inner, Vector::Complex(_)) => has_complex = true,
            RValue::Vector(rv) if matches!(rv.inner, Vector::Double(_)) => has_double = true,
            RValue::Vector(rv) if matches!(rv.inner, Vector::Integer(_)) => has_int = true,
            RValue::Vector(rv) if matches!(rv.inner, Vector::Logical(_)) => has_logical = true,
            RValue::Vector(rv) if matches!(rv.inner, Vector::Raw(_)) => has_raw = true,
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
    } else if has_complex {
        let mut result = Vec::new();
        for val in &all_values {
            match val {
                RValue::Vector(v) => result.extend(v.inner.to_complex()),
                RValue::Null => {}
                _ => {}
            }
        }
        Ok(RValue::vec(Vector::Complex(result.into())))
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
    } else if has_logical || !has_raw {
        let mut result = Vec::new();
        for val in &all_values {
            match val {
                RValue::Vector(v) => result.extend(v.to_logicals()),
                RValue::Null => {}
                _ => {}
            }
        }
        Ok(RValue::vec(Vector::Logical(result.into())))
    } else {
        // All raw
        let mut result = Vec::new();
        for val in &all_values {
            match val {
                RValue::Vector(v) => result.extend(v.inner.to_raw()),
                RValue::Null => {}
                _ => {}
            }
        }
        Ok(RValue::vec(Vector::Raw(result)))
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
                    Vector::Raw(vals) => vals.iter().map(|b| format!("{:02x}", b)).collect(),
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
                    Vector::Complex(vals) => vals
                        .iter()
                        .map(|x| x.map(format_r_complex).unwrap_or_else(|| "NA".to_string()))
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
    Ok(RValue::vec(Vector::Integer(
        vec![Some(i64::try_from(len)?)].into(),
    )))
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
                .map(|s| s.as_ref().map(|s| i64::try_from(s.len()).unwrap_or(0)))
                .collect();
            Ok(RValue::vec(Vector::Integer(result.into())))
        }
        _ => Ok(RValue::vec(Vector::Integer(vec![None].into()))),
    }
}

#[builtin(min_args = 1, names = ["is.ordered", "is.call", "is.symbol", "is.name", "is.expression", "is.pairlist"])]
fn builtin_is_null(args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    let r = matches!(args.first(), Some(RValue::Null));
    Ok(RValue::vec(Vector::Logical(vec![Some(r)].into())))
}

#[builtin(name = "is.environment", min_args = 1)]
fn builtin_is_environment(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let r = matches!(args.first(), Some(RValue::Environment(_)));
    Ok(RValue::vec(Vector::Logical(vec![Some(r)].into())))
}

#[builtin(name = "is.language", min_args = 1)]
fn builtin_is_language(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let r = matches!(args.first(), Some(RValue::Language(_)));
    Ok(RValue::vec(Vector::Logical(vec![Some(r)].into())))
}

#[builtin(min_args = 1)]
fn builtin_is_na(args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(v)) => {
            let result: Vec<Option<bool>> = match &v.inner {
                Vector::Raw(vals) => vals.iter().map(|_| Some(false)).collect(),
                Vector::Logical(vals) => vals.iter().map(|x| Some(x.is_none())).collect(),
                Vector::Integer(vals) => vals.iter().map(|x| Some(x.is_none())).collect(),
                Vector::Double(vals) => vals
                    .iter()
                    .map(|x| Some(x.is_none() || x.map(|f| f.is_nan()).unwrap_or(false)))
                    .collect(),
                Vector::Complex(vals) => vals.iter().map(|x| Some(x.is_none())).collect(),
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
        Some(RValue::List(l)) => Ok(list_names_value(l)),
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
            set_list_names(&mut l, &names_val);
            Ok(RValue::List(l))
        }
        other => Ok(other.cloned().unwrap_or(RValue::Null)),
    }
}

fn character_name_vector(values: Vec<Option<String>>) -> RValue {
    RValue::vec(Vector::Character(values.into()))
}

fn coerce_name_strings(value: &RValue) -> RValue {
    match value {
        RValue::Vector(rv) => match &rv.inner {
            Vector::Character(_) => value.clone(),
            Vector::Integer(values) => RValue::vec(Vector::Character(
                values
                    .iter()
                    .map(|value| value.map(|value| value.to_string()))
                    .collect::<Vec<_>>()
                    .into(),
            )),
            Vector::Double(values) => RValue::vec(Vector::Character(
                values
                    .iter()
                    .map(|value| value.map(format_r_double))
                    .collect::<Vec<_>>()
                    .into(),
            )),
            _ => RValue::Null,
        },
        _ => RValue::Null,
    }
}

fn coerce_name_values(value: &RValue) -> Option<Vec<Option<String>>> {
    let coerced = coerce_name_strings(value);
    coerced.as_vector().map(|values| values.to_characters())
}

fn list_names_value(list: &RList) -> RValue {
    if let Some(names_attr) = list.get_attr("names") {
        return coerce_name_strings(names_attr);
    }

    let names: Vec<Option<String>> = list.values.iter().map(|(name, _)| name.clone()).collect();
    if names.iter().all(|name| name.is_none()) {
        RValue::Null
    } else {
        character_name_vector(names)
    }
}

fn set_list_names(list: &mut RList, names_val: &RValue) {
    if let Some(mut names) = coerce_name_values(names_val) {
        names.resize(list.values.len(), None);
        for (entry, name) in list.values.iter_mut().zip(names.iter()) {
            entry.0 = name.clone();
        }
        list.set_attr("names".to_string(), character_name_vector(names));
        return;
    }

    if names_val.is_null() {
        for entry in &mut list.values {
            entry.0 = None;
        }
        list.attrs.as_mut().map(|attrs| attrs.remove("names"));
    }
}

fn data_frame_row_count(list: &RList) -> usize {
    list.get_attr("row.names")
        .map(RValue::length)
        .unwrap_or_else(|| {
            list.values
                .iter()
                .map(|(_, value)| value.length())
                .max()
                .unwrap_or(0)
        })
}

fn automatic_row_names_value(count: usize) -> Result<RValue, RError> {
    Ok(RValue::vec(Vector::Integer(
        (1..=i64::try_from(count)?)
            .map(Some)
            .collect::<Vec<_>>()
            .into(),
    )))
}

fn data_frame_dimnames_value(list: &RList) -> Result<RValue, RError> {
    let row_names = list
        .get_attr("row.names")
        .map(coerce_name_strings)
        .unwrap_or(automatic_row_names_value(data_frame_row_count(list))?);
    let col_names = list_names_value(list);
    Ok(RValue::List(RList::new(vec![
        (None, row_names),
        (None, col_names),
    ])))
}

fn set_data_frame_row_names(list: &mut RList, row_names: &RValue) -> Result<(), RError> {
    if row_names.is_null() {
        list.set_attr(
            "row.names".to_string(),
            automatic_row_names_value(data_frame_row_count(list))?,
        );
        return Ok(());
    }

    let Some(names) = coerce_name_values(row_names) else {
        return Err(RError::other(
            "row names supplied are of the wrong length".to_string(),
        ));
    };
    if names.len() != data_frame_row_count(list) {
        return Err(RError::other(
            "row names supplied are of the wrong length".to_string(),
        ));
    }
    list.set_attr("row.names".to_string(), character_name_vector(names));
    Ok(())
}

fn set_data_frame_col_names(list: &mut RList, col_names: &RValue) -> Result<(), RError> {
    if col_names.is_null() {
        set_list_names(list, col_names);
        return Ok(());
    }

    let Some(names) = coerce_name_values(col_names) else {
        return Err(RError::other(
            "'names' attribute [1] must be the same length as the vector [0]".to_string(),
        ));
    };
    if names.len() != list.values.len() {
        return Err(RError::other(format!(
            "'names' attribute [{}] must be the same length as the vector [{}]",
            names.len(),
            list.values.len()
        )));
    }
    set_list_names(list, &character_name_vector(names));
    Ok(())
}

fn set_data_frame_dimnames(list: &mut RList, dimnames: &RValue) -> Result<(), RError> {
    let RValue::List(values) = dimnames else {
        return Err(RError::other(
            "invalid 'dimnames' given for data frame".to_string(),
        ));
    };
    if values.values.len() != 2 {
        return Err(RError::other(
            "invalid 'dimnames' given for data frame".to_string(),
        ));
    }

    let row_names = &values.values[0].1;
    let col_names = &values.values[1].1;

    let Some(row_values) = coerce_name_values(row_names) else {
        return Err(RError::other(
            "invalid 'dimnames' given for data frame".to_string(),
        ));
    };
    let Some(col_values) = coerce_name_values(col_names) else {
        return Err(RError::other(
            "invalid 'dimnames' given for data frame".to_string(),
        ));
    };

    if row_values.len() != data_frame_row_count(list) || col_values.len() != list.values.len() {
        return Err(RError::other(
            "invalid 'dimnames' given for data frame".to_string(),
        ));
    }

    list.set_attr("row.names".to_string(), character_name_vector(row_values));
    set_list_names(list, &character_name_vector(col_values));
    list.attrs.as_mut().map(|attrs| attrs.remove("dimnames"));
    Ok(())
}

fn updated_dimnames_component(current: Option<&RValue>, index: usize, value: &RValue) -> RValue {
    let mut components = match current {
        Some(RValue::List(list)) => {
            let mut components: Vec<RValue> =
                list.values.iter().map(|(_, value)| value.clone()).collect();
            components.resize(2, RValue::Null);
            components
        }
        _ => vec![RValue::Null, RValue::Null],
    };

    if index < components.len() {
        components[index] = value.clone();
    }

    if components.iter().all(RValue::is_null) {
        RValue::Null
    } else {
        RValue::List(RList::new(
            components.into_iter().map(|value| (None, value)).collect(),
        ))
    }
}

#[builtin(name = "row.names", names = ["rownames"], min_args = 1)]
fn builtin_row_names(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::List(list)) => Ok(list
            .get_attr("row.names")
            .map(coerce_name_strings)
            .unwrap_or(RValue::Null)),
        Some(RValue::Vector(rv)) => {
            if let Some(RValue::List(dimnames)) = rv.get_attr("dimnames") {
                if let Some((_, row_names)) = dimnames.values.first() {
                    return Ok(coerce_name_strings(row_names));
                }
            }
            Ok(RValue::Null)
        }
        _ => Ok(RValue::Null),
    }
}

#[builtin(name = "colnames", min_args = 1)]
fn builtin_col_names(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(value @ RValue::List(list)) => {
            if has_class(value, "data.frame") {
                return Ok(list_names_value(list));
            }
            if let Some(RValue::List(dimnames)) = list.get_attr("dimnames") {
                if let Some((_, col_names)) = dimnames.values.get(1) {
                    return Ok(coerce_name_strings(col_names));
                }
            }
            Ok(RValue::Null)
        }
        Some(RValue::Vector(rv)) => {
            if let Some(RValue::List(dimnames)) = rv.get_attr("dimnames") {
                if let Some((_, col_names)) = dimnames.values.get(1) {
                    return Ok(coerce_name_strings(col_names));
                }
            }
            Ok(RValue::Null)
        }
        _ => Ok(RValue::Null),
    }
}

#[builtin(name = "rownames<-", names = ["row.names<-"], min_args = 2)]
fn builtin_row_names_set(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let row_names = args.get(1).cloned().unwrap_or(RValue::Null);
    match args.first() {
        Some(value @ RValue::List(list)) if has_class(value, "data.frame") => {
            let mut list = list.clone();
            set_data_frame_row_names(&mut list, &row_names)?;
            Ok(RValue::List(list))
        }
        Some(RValue::Vector(rv)) => {
            let mut rv = rv.clone();
            let dimnames = updated_dimnames_component(rv.get_attr("dimnames"), 0, &row_names);
            if dimnames.is_null() {
                rv.attrs.as_mut().map(|attrs| attrs.remove("dimnames"));
            } else {
                rv.set_attr("dimnames".to_string(), dimnames);
            }
            Ok(RValue::Vector(rv))
        }
        Some(RValue::List(list)) => {
            let mut list = list.clone();
            let dimnames = updated_dimnames_component(list.get_attr("dimnames"), 0, &row_names);
            if dimnames.is_null() {
                list.attrs.as_mut().map(|attrs| attrs.remove("dimnames"));
            } else {
                list.set_attr("dimnames".to_string(), dimnames);
            }
            Ok(RValue::List(list))
        }
        other => Ok(other.cloned().unwrap_or(RValue::Null)),
    }
}

#[builtin(name = "colnames<-", min_args = 2)]
fn builtin_col_names_set(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let col_names = args.get(1).cloned().unwrap_or(RValue::Null);
    match args.first() {
        Some(value @ RValue::List(list)) if has_class(value, "data.frame") => {
            let mut list = list.clone();
            set_data_frame_col_names(&mut list, &col_names)?;
            Ok(RValue::List(list))
        }
        Some(RValue::Vector(rv)) => {
            let mut rv = rv.clone();
            let dimnames = updated_dimnames_component(rv.get_attr("dimnames"), 1, &col_names);
            if dimnames.is_null() {
                rv.attrs.as_mut().map(|attrs| attrs.remove("dimnames"));
            } else {
                rv.set_attr("dimnames".to_string(), dimnames);
            }
            Ok(RValue::Vector(rv))
        }
        Some(RValue::List(list)) => {
            let mut list = list.clone();
            let dimnames = updated_dimnames_component(list.get_attr("dimnames"), 1, &col_names);
            if dimnames.is_null() {
                list.attrs.as_mut().map(|attrs| attrs.remove("dimnames"));
            } else {
                list.set_attr("dimnames".to_string(), dimnames);
            }
            Ok(RValue::List(list))
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
        Some(RValue::Language(lang)) => {
            let mut lang = lang.clone();
            if class_val.is_null() {
                lang.attrs.as_mut().map(|a| a.remove("class"));
            } else {
                lang.set_attr("class".to_string(), class_val);
            }
            Ok(RValue::Language(lang))
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
    if let Some(RValue::Language(lang)) = args.first() {
        if let Some(cls) = lang.get_attr("class") {
            return Ok(cls.clone());
        }
    }
    let c = match args.first() {
        Some(RValue::Vector(rv)) => match &rv.inner {
            Vector::Raw(_) => "raw",
            Vector::Logical(_) => "logical",
            Vector::Integer(_) => "integer",
            Vector::Double(_) => "numeric",
            Vector::Complex(_) => "complex",
            Vector::Character(_) => "character",
        },
        Some(RValue::List(_)) => "list",
        Some(RValue::Function(_)) => "function",
        Some(RValue::Language(lang)) => match &**lang {
            Expr::Symbol(_) => "name",
            _ => "call",
        },
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
            Vector::Raw(_) => "raw",
            Vector::Logical(_) => "logical",
            Vector::Integer(_) | Vector::Double(_) => "numeric",
            Vector::Complex(_) => "complex",
            Vector::Character(_) => "character",
        },
        Some(RValue::List(_)) => "list",
        Some(RValue::Function(_)) => "function",
        Some(RValue::Language(lang)) => match &**lang {
            Expr::Symbol(_) => "name",
            _ => "call",
        },
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
                        Vector::Raw(vals) => vals
                            .iter()
                            .take(10)
                            .map(|b| format!("{:02x}", b))
                            .collect::<Vec<_>>()
                            .join(" "),
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
                        Vector::Complex(vals) => vals
                            .iter()
                            .take(10)
                            .map(|x| match x {
                                Some(c) => format_r_complex(*c),
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
        return Err(RError::new(
            RErrorKind::Argument,
            "need 2 arguments".to_string(),
        ));
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
        return Err(RError::new(
            RErrorKind::Argument,
            "need 2 arguments".to_string(),
        ));
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
        return Err(RError::new(
            RErrorKind::Argument,
            "need 2 arguments".to_string(),
        ));
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
        .unwrap_or(0);
    let length = usize::try_from(length).unwrap_or(0);
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
                Vector::Raw(vals) => vals
                    .iter()
                    .map(|&x| (None, RValue::vec(Vector::Raw(vec![x]))))
                    .collect(),
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
                Vector::Complex(vals) => vals
                    .iter()
                    .map(|x| (None, RValue::vec(Vector::Complex(vec![*x].into()))))
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

#[builtin(min_args = 1)]
fn builtin_invisible(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    Ok(args.first().cloned().unwrap_or(RValue::Null))
}

#[builtin(min_args = 3)]
fn builtin_ifelse(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    if args.len() < 3 {
        return Err(RError::new(
            RErrorKind::Argument,
            "need 3 arguments".to_string(),
        ));
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
        return Err(RError::new(
            RErrorKind::Argument,
            "need 2 arguments".to_string(),
        ));
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
                    .map(|p| i64::try_from(p).map(|v| v + 1).unwrap_or(0))
            })
        })
        .collect();
    Ok(RValue::vec(Vector::Integer(result.into())))
}

#[builtin(min_args = 3)]
fn builtin_replace(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    if args.len() < 3 {
        return Err(RError::new(
            RErrorKind::Argument,
            "need 3 arguments".to_string(),
        ));
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
                    let idx = usize::try_from(*idx)? - 1;
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

// (file.path, file.exists, readLines, writeLines, read.csv, write.csv — io.rs)

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
                vec![Some("miniR (Rust)".to_string())].into(),
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
        return Err(RError::new(
            RErrorKind::Argument,
            "need 2 arguments".to_string(),
        ));
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
        return Err(RError::new(
            RErrorKind::Argument,
            "need 2 arguments".to_string(),
        ));
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
        return Err(RError::new(
            RErrorKind::Argument,
            "need 2 arguments".to_string(),
        ));
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
        .map(usize::try_from)
        .transpose()?
        .unwrap_or(0);
    Ok(RValue::vec(Vector::Double(vec![Some(0.0); n].into())))
}

#[builtin]
fn builtin_integer(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let n = args
        .first()
        .and_then(|v| v.as_vector()?.as_integer_scalar())
        .map(usize::try_from)
        .transpose()?
        .unwrap_or(0);
    Ok(RValue::vec(Vector::Integer(vec![Some(0); n].into())))
}

#[builtin]
fn builtin_logical(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let n = args
        .first()
        .and_then(|v| v.as_vector()?.as_integer_scalar())
        .map(usize::try_from)
        .transpose()?
        .unwrap_or(0);
    Ok(RValue::vec(Vector::Logical(vec![Some(false); n].into())))
}

#[builtin]
fn builtin_character(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let n = args
        .first()
        .and_then(|v| v.as_vector()?.as_integer_scalar())
        .map(usize::try_from)
        .transpose()?
        .unwrap_or(0);
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
    let dimnames = named
        .iter()
        .find(|(n, _)| n == "dimnames")
        .map(|(_, v)| v)
        .or(args.get(4));

    let data_vec = match &data {
        RValue::Vector(v) => v.to_doubles(),
        _ => vec![Some(f64::NAN)],
    };
    let data_len = data_vec.len();

    let (nrow, ncol) = match (nrow_arg, ncol_arg) {
        (Some(r), Some(c)) => (usize::try_from(r)?, usize::try_from(c)?),
        (Some(r), None) => {
            let r = usize::try_from(r)?;
            (r, if r > 0 { data_len.div_ceil(r) } else { 0 })
        }
        (None, Some(c)) => {
            let c = usize::try_from(c)?;
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

    let mut rv = RVector::from(Vector::Double(mat.into()));
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
    if let Some(dimnames) = dimnames {
        if !dimnames.is_null() {
            rv.set_attr("dimnames".to_string(), dimnames.clone());
        }
    }
    Ok(RValue::Vector(rv))
}

#[builtin(min_args = 1)]
fn builtin_dim(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(rv)) => Ok(rv.get_attr("dim").cloned().unwrap_or(RValue::Null)),
        Some(RValue::List(l)) => Ok(l.get_attr("dim").cloned().unwrap_or(RValue::Null)),
        _ => Ok(RValue::Null),
    }
}

#[builtin(name = "dim<-", min_args = 2)]
fn builtin_dim_set(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let dim_val = args.get(1).cloned().unwrap_or(RValue::Null);
    match args.first() {
        Some(RValue::Vector(rv)) => {
            let mut rv = rv.clone();
            if dim_val.is_null() {
                rv.attrs.as_mut().map(|a| a.remove("dim"));
                rv.attrs.as_mut().map(|a| a.remove("class"));
            } else {
                rv.set_attr("dim".to_string(), dim_val);
                rv.set_attr(
                    "class".to_string(),
                    RValue::vec(Vector::Character(
                        vec![Some("matrix".to_string()), Some("array".to_string())].into(),
                    )),
                );
            }
            Ok(RValue::Vector(rv))
        }
        other => Ok(other.cloned().unwrap_or(RValue::Null)),
    }
}

#[builtin(min_args = 1)]
fn builtin_nrow(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(rv)) => {
            if let Some(dims) = get_dim_ints(rv.get_attr("dim")) {
                if !dims.is_empty() {
                    return Ok(RValue::vec(Vector::Integer(vec![dims[0]].into())));
                }
            }
            Ok(RValue::Null)
        }
        Some(RValue::List(l)) => {
            if let Some(dims) = get_dim_ints(l.get_attr("dim")) {
                if !dims.is_empty() {
                    return Ok(RValue::vec(Vector::Integer(vec![dims[0]].into())));
                }
            }
            if let Some(rn) = l.get_attr("row.names") {
                return Ok(RValue::vec(Vector::Integer(
                    vec![Some(i64::try_from(rn.length())?)].into(),
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
        Some(RValue::Vector(rv)) => {
            if let Some(dims) = get_dim_ints(rv.get_attr("dim")) {
                if dims.len() >= 2 {
                    return Ok(RValue::vec(Vector::Integer(vec![dims[1]].into())));
                }
            }
            Ok(RValue::Null)
        }
        Some(RValue::List(l)) => {
            if let Some(dims) = get_dim_ints(l.get_attr("dim")) {
                if dims.len() >= 2 {
                    return Ok(RValue::vec(Vector::Integer(vec![dims[1]].into())));
                }
            }
            if has_class(args.first().unwrap(), "data.frame") {
                return Ok(RValue::vec(Vector::Integer(
                    vec![Some(i64::try_from(l.values.len())?)].into(),
                )));
            }
            Ok(RValue::Null)
        }
        _ => Ok(RValue::Null),
    }
}

#[builtin(name = "NROW", min_args = 1)]
fn builtin_nrow_safe(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(rv)) => {
            if let Some(dims) = get_dim_ints(rv.get_attr("dim")) {
                if !dims.is_empty() {
                    return Ok(RValue::vec(Vector::Integer(vec![dims[0]].into())));
                }
            }
            Ok(RValue::vec(Vector::Integer(
                vec![Some(i64::try_from(rv.len())?)].into(),
            )))
        }
        Some(RValue::List(l)) => {
            if let Some(dims) = get_dim_ints(l.get_attr("dim")) {
                if !dims.is_empty() {
                    return Ok(RValue::vec(Vector::Integer(vec![dims[0]].into())));
                }
            }
            // Data frame: nrow = length of first column
            if has_class(args.first().unwrap(), "data.frame") {
                if let Some(rn) = l.get_attr("row.names") {
                    return Ok(RValue::vec(Vector::Integer(
                        vec![Some(i64::try_from(rn.length())?)].into(),
                    )));
                }
                let n = l.values.first().map(|(_, v)| v.length()).unwrap_or(0);
                return Ok(RValue::vec(Vector::Integer(
                    vec![Some(i64::try_from(n)?)].into(),
                )));
            }
            Ok(RValue::vec(Vector::Integer(
                vec![Some(i64::try_from(l.values.len())?)].into(),
            )))
        }
        Some(RValue::Null) => Ok(RValue::vec(Vector::Integer(vec![Some(0)].into()))),
        _ => Ok(RValue::vec(Vector::Integer(vec![Some(1)].into()))),
    }
}

#[builtin(name = "NCOL", min_args = 1)]
fn builtin_ncol_safe(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(rv)) => {
            if let Some(dims) = get_dim_ints(rv.get_attr("dim")) {
                if dims.len() >= 2 {
                    return Ok(RValue::vec(Vector::Integer(vec![dims[1]].into())));
                }
            }
            Ok(RValue::vec(Vector::Integer(vec![Some(1)].into())))
        }
        Some(RValue::List(l)) => {
            if let Some(dims) = get_dim_ints(l.get_attr("dim")) {
                if dims.len() >= 2 {
                    return Ok(RValue::vec(Vector::Integer(vec![dims[1]].into())));
                }
            }
            if has_class(args.first().unwrap(), "data.frame") {
                return Ok(RValue::vec(Vector::Integer(
                    vec![Some(i64::try_from(l.values.len())?)].into(),
                )));
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
        Some(RValue::Vector(rv)) => {
            let dims = match get_dim_ints(rv.get_attr("dim")) {
                Some(d) if d.len() >= 2 => (
                    usize::try_from(d[0].unwrap_or(0))?,
                    usize::try_from(d[1].unwrap_or(0))?,
                ),
                _ => return Ok(args[0].clone()),
            };
            let (nrow, ncol) = dims;
            let data = rv.to_doubles();
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
            let mut result = RVector::from(Vector::Double(transposed.into()));
            result.set_attr(
                "class".to_string(),
                RValue::vec(Vector::Character(
                    vec![Some("matrix".to_string()), Some("array".to_string())].into(),
                )),
            );
            result.set_attr(
                "dim".to_string(),
                RValue::vec(Vector::Integer(
                    vec![Some(i64::try_from(ncol)?), Some(i64::try_from(nrow)?)].into(),
                )),
            );
            Ok(RValue::Vector(result))
        }
        _ => Ok(args.first().cloned().unwrap_or(RValue::Null)),
    }
}

#[builtin(min_args = 1)]
fn builtin_unname(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(rv)) => {
            let mut rv = rv.clone();
            rv.attrs.as_mut().map(|a| a.remove("names"));
            Ok(RValue::Vector(rv))
        }
        Some(RValue::List(l)) => {
            let mut l = l.clone();
            for entry in &mut l.values {
                entry.0 = None;
            }
            l.attrs.as_mut().map(|a| a.remove("names"));
            Ok(RValue::List(l))
        }
        other => Ok(other.cloned().unwrap_or(RValue::Null)),
    }
}

#[builtin(min_args = 1)]
fn builtin_dimnames(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(value @ RValue::List(list)) if has_class(value, "data.frame") => {
            data_frame_dimnames_value(list)
        }
        Some(RValue::Vector(rv)) => Ok(rv.get_attr("dimnames").cloned().unwrap_or(RValue::Null)),
        Some(RValue::List(l)) => Ok(l.get_attr("dimnames").cloned().unwrap_or(RValue::Null)),
        _ => Ok(RValue::Null),
    }
}

#[builtin(name = "dimnames<-", min_args = 2)]
fn builtin_dimnames_set(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let dimnames_val = args.get(1).cloned().unwrap_or(RValue::Null);
    match args.first() {
        Some(RValue::Vector(rv)) => {
            let mut rv = rv.clone();
            if dimnames_val.is_null() {
                rv.attrs.as_mut().map(|a| a.remove("dimnames"));
            } else {
                rv.set_attr("dimnames".to_string(), dimnames_val);
            }
            Ok(RValue::Vector(rv))
        }
        Some(value @ RValue::List(l)) if has_class(value, "data.frame") => {
            let mut l = l.clone();
            set_data_frame_dimnames(&mut l, &dimnames_val)?;
            Ok(RValue::List(l))
        }
        Some(RValue::List(l)) => {
            let mut l = l.clone();
            if dimnames_val.is_null() {
                l.attrs.as_mut().map(|a| a.remove("dimnames"));
            } else {
                l.set_attr("dimnames".to_string(), dimnames_val);
            }
            Ok(RValue::List(l))
        }
        other => Ok(other.cloned().unwrap_or(RValue::Null)),
    }
}

#[builtin]
fn builtin_array(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    // array(data = NA, dim = length(data), dimnames = NULL)
    let data = args
        .first()
        .cloned()
        .unwrap_or(RValue::vec(Vector::Logical(vec![None].into())));
    let dim_arg = named
        .iter()
        .find(|(n, _)| n == "dim")
        .map(|(_, v)| v)
        .or(args.get(1));
    let dimnames_arg = named
        .iter()
        .find(|(n, _)| n == "dimnames")
        .map(|(_, v)| v)
        .or(args.get(2));

    let data_vec = match &data {
        RValue::Vector(v) => v.to_doubles(),
        RValue::Null => vec![],
        _ => vec![Some(f64::NAN)],
    };

    // Parse dim: can be a single integer or a vector of integers
    let dims: Vec<usize> = match dim_arg {
        Some(val) => {
            let ints = match val.as_vector() {
                Some(v) => v.to_integers(),
                None => {
                    return Err(RError::new(
                        RErrorKind::Argument,
                        "'dim' must be a numeric vector".to_string(),
                    ))
                }
            };
            ints.iter()
                .map(|x| usize::try_from(x.unwrap_or(0)))
                .collect::<Result<Vec<_>, _>>()?
        }
        None => vec![data_vec.len()],
    };

    // Calculate total elements
    let total: usize = dims.iter().product();

    // Recycle data to fill the array
    let mut mat = Vec::with_capacity(total);
    if data_vec.is_empty() {
        mat.resize(total, None);
    } else {
        for i in 0..total {
            mat.push(data_vec[i % data_vec.len()]);
        }
    }

    let mut rv = RVector::from(Vector::Double(mat.into()));

    // Set class: arrays with 2 dims get "matrix" + "array", others just "array"
    if dims.len() == 2 {
        rv.set_attr(
            "class".to_string(),
            RValue::vec(Vector::Character(
                vec![Some("matrix".to_string()), Some("array".to_string())].into(),
            )),
        );
    } else {
        rv.set_attr(
            "class".to_string(),
            RValue::vec(Vector::Character(vec![Some("array".to_string())].into())),
        );
    }

    // Set dim attribute
    rv.set_attr(
        "dim".to_string(),
        RValue::vec(Vector::Integer(
            dims.iter()
                .map(|&d| i64::try_from(d).map(Some))
                .collect::<Result<Vec<_>, _>>()?
                .into(),
        )),
    );

    // Set dimnames if provided
    if let Some(dn) = dimnames_arg {
        if !dn.is_null() {
            rv.set_attr("dimnames".to_string(), dn.clone());
        }
    }

    Ok(RValue::Vector(rv))
}

#[builtin(min_args = 1)]
fn builtin_rbind(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    if args.is_empty() {
        return Ok(RValue::Null);
    }

    // Collect all inputs as (data, nrow, ncol)
    let mut inputs: Vec<(Vec<Option<f64>>, usize, usize)> = Vec::new();
    for arg in args {
        match arg {
            RValue::Vector(rv) => {
                let data = rv.to_doubles();
                if let Some(dims) = get_dim_ints(rv.get_attr("dim")) {
                    if dims.len() >= 2 {
                        let nr = usize::try_from(dims[0].unwrap_or(0))?;
                        let nc = usize::try_from(dims[1].unwrap_or(0))?;
                        inputs.push((data, nr, nc));
                        continue;
                    }
                }
                // Plain vector becomes a 1-row matrix
                let len = data.len();
                inputs.push((data, 1, len));
            }
            RValue::Null => continue,
            _ => {
                return Err(RError::new(
                    RErrorKind::Argument,
                    "cannot rbind non-vector/matrix arguments".to_string(),
                ))
            }
        }
    }

    if inputs.is_empty() {
        return Ok(RValue::Null);
    }

    // All inputs must have the same number of columns (after recycling)
    let max_ncol = inputs.iter().map(|(_, _, nc)| *nc).max().unwrap_or(0);
    if max_ncol == 0 {
        return Ok(RValue::Null);
    }

    // Check column compatibility
    for (_, _, nc) in &inputs {
        if *nc != max_ncol && max_ncol % nc != 0 && nc % max_ncol != 0 {
            return Err(RError::new(
                RErrorKind::Argument,
                "number of columns of arguments do not match".to_string(),
            ));
        }
    }

    // Total rows
    let total_nrow: usize = inputs.iter().map(|(_, nr, _)| *nr).sum();

    // Build result column-major: for each column j, concatenate rows from all inputs
    let mut result = Vec::with_capacity(total_nrow * max_ncol);
    for j in 0..max_ncol {
        for (data, nr, nc) in &inputs {
            let actual_j = j % nc;
            for i in 0..*nr {
                // Column-major index: col * nrow + row
                let idx = actual_j * nr + i;
                result.push(if idx < data.len() { data[idx] } else { None });
            }
        }
    }

    let mut rv = RVector::from(Vector::Double(result.into()));
    rv.set_attr(
        "class".to_string(),
        RValue::vec(Vector::Character(
            vec![Some("matrix".to_string()), Some("array".to_string())].into(),
        )),
    );
    rv.set_attr(
        "dim".to_string(),
        RValue::vec(Vector::Integer(
            vec![
                Some(i64::try_from(total_nrow)?),
                Some(i64::try_from(max_ncol)?),
            ]
            .into(),
        )),
    );
    Ok(RValue::Vector(rv))
}

#[builtin(min_args = 1)]
fn builtin_cbind(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    if args.is_empty() {
        return Ok(RValue::Null);
    }

    // Collect all inputs as (data, nrow, ncol)
    let mut inputs: Vec<(Vec<Option<f64>>, usize, usize)> = Vec::new();
    for arg in args {
        match arg {
            RValue::Vector(rv) => {
                let data = rv.to_doubles();
                if let Some(dims) = get_dim_ints(rv.get_attr("dim")) {
                    if dims.len() >= 2 {
                        let nr = usize::try_from(dims[0].unwrap_or(0))?;
                        let nc = usize::try_from(dims[1].unwrap_or(0))?;
                        inputs.push((data, nr, nc));
                        continue;
                    }
                }
                // Plain vector becomes a 1-column matrix
                let len = data.len();
                inputs.push((data, len, 1));
            }
            RValue::Null => continue,
            _ => {
                return Err(RError::new(
                    RErrorKind::Argument,
                    "cannot cbind non-vector/matrix arguments".to_string(),
                ))
            }
        }
    }

    if inputs.is_empty() {
        return Ok(RValue::Null);
    }

    // All inputs must have the same number of rows (after recycling)
    let max_nrow = inputs.iter().map(|(_, nr, _)| *nr).max().unwrap_or(0);
    if max_nrow == 0 {
        return Ok(RValue::Null);
    }

    // Check row compatibility
    for (_, nr, _) in &inputs {
        if *nr != max_nrow && max_nrow % nr != 0 && nr % max_nrow != 0 {
            return Err(RError::new(
                RErrorKind::Argument,
                "number of rows of arguments do not match".to_string(),
            ));
        }
    }

    // Total columns
    let total_ncol: usize = inputs.iter().map(|(_, _, nc)| *nc).sum();

    // Build result column-major: for each input, append its columns (recycling rows)
    let mut result = Vec::with_capacity(max_nrow * total_ncol);
    for (data, nr, nc) in &inputs {
        for j in 0..*nc {
            for i in 0..max_nrow {
                // Recycle: wrap row index within the input's actual nrow
                let actual_i = i % nr;
                let idx = j * nr + actual_i;
                result.push(if idx < data.len() { data[idx] } else { None });
            }
        }
    }

    let mut rv = RVector::from(Vector::Double(result.into()));
    rv.set_attr(
        "class".to_string(),
        RValue::vec(Vector::Character(
            vec![Some("matrix".to_string()), Some("array".to_string())].into(),
        )),
    );
    rv.set_attr(
        "dim".to_string(),
        RValue::vec(Vector::Integer(
            vec![
                Some(i64::try_from(max_nrow)?),
                Some(i64::try_from(total_ncol)?),
            ]
            .into(),
        )),
    );
    Ok(RValue::Vector(rv))
}

#[builtin(min_args = 2)]
fn builtin_attr(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let which = args
        .get(1)
        .and_then(|v| v.as_vector())
        .and_then(|v| v.as_character_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "'which' must be a character string".to_string(),
            )
        })?;
    match args.first() {
        Some(RValue::Vector(rv)) => Ok(rv.get_attr(&which).cloned().unwrap_or(RValue::Null)),
        Some(RValue::List(l)) => Ok(l.get_attr(&which).cloned().unwrap_or(RValue::Null)),
        Some(RValue::Language(lang)) => Ok(lang.get_attr(&which).cloned().unwrap_or(RValue::Null)),
        _ => Ok(RValue::Null),
    }
}

#[builtin(name = "attr<-", min_args = 3)]
fn builtin_attr_set(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let which = args
        .get(1)
        .and_then(|v| v.as_vector())
        .and_then(|v| v.as_character_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "'which' must be a character string".to_string(),
            )
        })?;
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
        Some(RValue::Language(lang)) => {
            let mut lang = lang.clone();
            if value.is_null() {
                lang.attrs.as_mut().map(|a| a.remove(&which));
            } else {
                lang.set_attr(which, value);
            }
            Ok(RValue::Language(lang))
        }
        other => Ok(other.cloned().unwrap_or(RValue::Null)),
    }
}

#[builtin(min_args = 1)]
fn builtin_attributes(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let attrs = match args.first() {
        Some(RValue::Vector(rv)) => rv.attrs.as_deref(),
        Some(RValue::List(l)) => l.attrs.as_deref(),
        Some(RValue::Language(lang)) => lang.attrs.as_deref(),
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
        RValue::Language(mut lang) => {
            for (name, value) in named {
                lang.set_attr(name.clone(), value.clone());
            }
            Ok(RValue::Language(lang))
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
                    Vector::Raw(_) => vec!["raw".to_string()],
                    Vector::Logical(_) => vec!["logical".to_string()],
                    Vector::Integer(_) => vec!["integer".to_string()],
                    Vector::Double(_) => vec!["numeric".to_string()],
                    Vector::Complex(_) => vec!["complex".to_string()],
                    Vector::Character(_) => vec!["character".to_string()],
                }
            }
        }
        Some(RValue::Function(_)) => vec!["function".to_string()],
        Some(RValue::Language(lang)) => lang.class().unwrap_or_default(),
        _ => vec![],
    };

    let result = what
        .iter()
        .any(|w| w.as_ref().is_some_and(|w| classes.iter().any(|c| c == w)));
    Ok(RValue::vec(Vector::Logical(vec![Some(result)].into())))
}

/// Extract integer dim values from a dim attribute
pub(crate) fn get_dim_ints(dim_attr: Option<&RValue>) -> Option<Vec<Option<i64>>> {
    match dim_attr {
        Some(RValue::Vector(rv)) => match &rv.inner {
            Vector::Integer(dims) => Some(dims.0.clone()),
            _ => None,
        },
        _ => None,
    }
}

fn has_class(val: &RValue, class_name: &str) -> bool {
    let class_attr = match val {
        RValue::Vector(rv) => rv.get_attr("class"),
        RValue::List(l) => l.get_attr("class"),
        RValue::Language(lang) => lang.get_attr("class"),
        _ => None,
    };
    if let Some(RValue::Vector(rv)) = class_attr {
        if let Vector::Character(cls) = &rv.inner {
            return cls.iter().any(|c| c.as_deref() == Some(class_name));
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
        // Check class attribute
        if has_class(v, "matrix") {
            return true;
        }
        // A matrix is any object with a dim attribute of length 2
        let dim_attr = match v {
            RValue::Vector(rv) => rv.get_attr("dim"),
            RValue::List(l) => l.get_attr("dim"),
            _ => None,
        };
        get_dim_ints(dim_attr).is_some_and(|d| d.len() == 2)
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
        return Err(RError::new(
            RErrorKind::Argument,
            "need 2 arguments".to_string(),
        ));
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
    let row_names: Vec<Option<i64>> = (1..=i64::try_from(max_len)?).map(Some).collect();
    list.set_attr(
        "row.names".to_string(),
        RValue::vec(Vector::Integer(row_names.into())),
    );
    Ok(RValue::List(list))
}

#[builtin(name = "environment")]
fn builtin_environment(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Function(RFunction::Closure { env, .. })) => {
            Ok(RValue::Environment(env.clone()))
        }
        Some(_) => Ok(RValue::Null),
        // No args: should return the current env, but we don't have it here
        // This is handled by the interpreter builtin for the no-arg case
        None => Ok(RValue::Null),
    }
}

#[builtin(name = "new.env")]
fn builtin_new_env(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let parent_val = named
        .iter()
        .find(|(n, _)| n == "parent")
        .map(|(_, v)| v)
        .or_else(|| args.first());
    let parent = parent_val.and_then(|v| {
        if let RValue::Environment(e) = v {
            Some(e.clone())
        } else {
            None
        }
    });
    match parent {
        Some(p) => Ok(RValue::Environment(Environment::new_child(&p))),
        None => Ok(RValue::Environment(Environment::new_empty())),
    }
}

#[builtin(name = "environmentName", min_args = 1)]
fn builtin_environment_name(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let name = match args.first() {
        Some(RValue::Environment(e)) => e.name().unwrap_or_default(),
        _ => String::new(),
    };
    Ok(RValue::vec(Vector::Character(vec![Some(name)].into())))
}

#[builtin(name = "parent.env", min_args = 1)]
fn builtin_parent_env(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Environment(e)) => match e.parent() {
            Some(p) => Ok(RValue::Environment(p)),
            None => Ok(RValue::Null),
        },
        _ => Err(RError::new(
            RErrorKind::Argument,
            "not an environment".to_string(),
        )),
    }
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
                            return Err(RError::other(format!(
                                "not all are TRUE (element {} of argument {})",
                                j + 1,
                                i + 1
                            )));
                        }
                        None => {
                            return Err(RError::other(format!(
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
                        return Err(RError::other(format!("argument {} is not TRUE", i + 1)));
                    }
                }
            }
            _ => {
                return Err(RError::other(format!(
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
        Some(RValue::Vector(v)) => {
            let mut v = v.clone();
            v.attrs = None;
            Ok(RValue::Vector(v))
        }
        Some(RValue::List(items)) => {
            let mut items = items.clone();
            items.attrs = None;
            Ok(RValue::List(items))
        }
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
        Some(RValue::Language(lang)) => {
            let mut lang = lang.clone();
            lang.attrs.as_mut().map(|a| a.remove("class"));
            Ok(RValue::Language(lang))
        }
        other => Ok(other.cloned().unwrap_or(RValue::Null)),
    }
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
                0 => Err(RError::new(
                    RErrorKind::Argument,
                    format!(
                        "'arg' should be one of {}",
                        choices_vec
                            .iter()
                            .map(|c| format!("'{}'", c))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                )),
                _ => Err(RError::new(
                    RErrorKind::Argument,
                    format!(
                        "'arg' should be one of {}",
                        choices_vec
                            .iter()
                            .map(|c| format!("'{}'", c))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                )),
            }
        }
    }
}

#[builtin(names = ["quit"])]
fn builtin_q(_args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    std::process::exit(0);
}

// === Metaprogramming builtins ===

use crate::parser::ast::Expr;

/// `formals(fn)` — return the formal parameter list of a function as a named list.
/// For closures, returns param names with defaults (if any). For builtins, returns NULL.
#[builtin(min_args = 1)]
fn builtin_formals(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Function(RFunction::Closure { params, .. })) => {
            if params.is_empty() {
                return Ok(RValue::Null);
            }
            let entries: Vec<(Option<String>, RValue)> = params
                .iter()
                .map(|p| {
                    let name = if p.is_dots {
                        "...".to_string()
                    } else {
                        p.name.clone()
                    };
                    let value = match &p.default {
                        Some(expr) => RValue::Language(Language::new(expr.clone())),
                        None => {
                            if p.is_dots {
                                // ... has no default — represent as empty symbol
                                RValue::Null
                            } else {
                                // Missing default — represent as empty symbol (R uses missing)
                                RValue::Null
                            }
                        }
                    };
                    (Some(name), value)
                })
                .collect();
            Ok(RValue::List(RList::new(entries)))
        }
        Some(RValue::Function(RFunction::Builtin { .. })) => Ok(RValue::Null),
        _ => Err(RError::new(
            RErrorKind::Argument,
            "'fn' is not a function — formals() requires a function argument".to_string(),
        )),
    }
}

/// `body(fn)` — return the body of a function as a Language object.
/// For closures, returns the body expression. For builtins, returns NULL.
#[builtin(min_args = 1)]
fn builtin_body(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Function(RFunction::Closure { body, .. })) => {
            Ok(RValue::Language(Language::new(body.clone())))
        }
        Some(RValue::Function(RFunction::Builtin { .. })) => Ok(RValue::Null),
        _ => Err(RError::new(
            RErrorKind::Argument,
            "'fn' is not a function — body() requires a function argument".to_string(),
        )),
    }
}

/// `args(fn)` — return the formals of a function (simplified: same as formals).
/// In GNU R, args() returns a function with the same formals but NULL body.
/// We simplify to just returning formals, which covers all practical uses.
#[builtin(min_args = 1)]
fn builtin_args(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    builtin_formals(args, named)
}

/// `call(name, ...)` — construct an unevaluated function call expression.
/// `call("f", 1, 2)` returns the language object `f(1, 2)`.
#[builtin(name = "call", min_args = 1)]
fn builtin_call(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let func_name = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "first argument must be a character string naming the function to call".to_string(),
            )
        })?;

    // Build Arg list from remaining positional args + named args
    let mut call_args: Vec<crate::parser::ast::Arg> = Vec::new();

    for val in args.iter().skip(1) {
        call_args.push(Arg {
            name: None,
            value: Some(rvalue_to_expr(val)),
        });
    }

    for (name, val) in named {
        call_args.push(Arg {
            name: Some(name.clone()),
            value: Some(rvalue_to_expr(val)),
        });
    }

    let expr = Expr::Call {
        func: Box::new(Expr::Symbol(func_name)),
        args: call_args,
    };

    Ok(RValue::Language(Language::new(expr)))
}

/// `UseMethod()` is intercepted directly by the evaluator so it can unwind the
/// current generic frame instead of returning like an ordinary builtin.
#[builtin(name = "UseMethod", min_args = 1)]
fn builtin_use_method(_args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    Err(RError::other(
        "internal error: UseMethod() should be intercepted during evaluation",
    ))
}

// `expression()` is a pre-eval builtin — see builtins/pre_eval.rs

/// `Recall(...)` — recursive self-call. Requires a call stack to know the current
/// function. Not yet implemented since we don't track a call stack.
#[builtin(name = "Recall")]
fn builtin_recall(_args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    Err(RError::other(
        "Recall() is not yet available — it requires call stack tracking, which is not yet implemented. \
         As a workaround, give your function a name and call it directly for recursion.",
    ))
}

/// Convert an RValue back to an AST expression (for call/expression construction).
fn rvalue_to_expr(val: &RValue) -> Expr {
    match val {
        RValue::Language(expr) => *expr.inner.clone(),
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
        _ => Expr::Symbol(format!("{}", val)),
    }
}
