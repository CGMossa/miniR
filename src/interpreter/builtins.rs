mod args;
mod coercion;
#[cfg(feature = "collections")]
pub mod collections;
mod conditions;
pub mod connections;
#[cfg(feature = "datetime")]
mod datetime;
mod factors;
mod graphics;
mod interp;
#[cfg(feature = "io")]
pub mod io;
pub mod math;
mod pre_eval;
#[cfg(feature = "random")]
mod random;
mod s4;
mod stats;
pub mod strings;
mod stubs;
pub mod system;
mod tables;
mod types;

use unicode_width::UnicodeWidthStr;

use crate::interpreter::environment::Environment;
use crate::interpreter::value::*;
use crate::interpreter::BuiltinContext;
use crate::parser::ast::Arg;
use itertools::Itertools;
use linkme::distributed_slice;
use minir_macros::{builtin, interpreter_builtin};

pub use crate::interpreter::value::{
    BuiltinDescriptor, BuiltinFn, BuiltinImplementation, InterpreterBuiltinFn, PreEvalBuiltinFn,
};
pub(crate) use args::CallArgs;

#[distributed_slice]
pub static BUILTIN_REGISTRY: [BuiltinDescriptor];

fn register_builtin_binding(env: &Environment, binding_name: &str, descriptor: BuiltinDescriptor) {
    env.set(
        binding_name.to_string(),
        RValue::Function(RFunction::Builtin {
            name: binding_name.to_string(),
            implementation: descriptor.implementation,
            min_args: descriptor.min_args,
            max_args: descriptor.max_args,
        }),
    );
}

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

/// Look up a builtin descriptor by R name or alias.
pub fn find_builtin(name: &str) -> Option<&'static BuiltinDescriptor> {
    BUILTIN_REGISTRY
        .iter()
        .find(|d| d.name == name || d.aliases.contains(&name))
}

/// Format a builtin's doc string for display.
/// Convention: first line = title, rest = description/params.
pub fn format_help(descriptor: &BuiltinDescriptor) -> String {
    let mut out = String::new();
    out.push_str(&format!("{}\n", descriptor.name));
    out.push_str(&"─".repeat(descriptor.name.len().max(20)));
    out.push('\n');

    if descriptor.doc.is_empty() {
        out.push_str(&format!(
            "  .Primitive(\"{}\")  [{} arg{}]\n",
            descriptor.name,
            descriptor.min_args,
            if descriptor.min_args == 1 { "" } else { "s" }
        ));
    } else {
        for line in descriptor.doc.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("@param ") {
                if let Some((param, desc)) = rest.split_once(' ') {
                    out.push_str(&format!("  {:<12} {}\n", param, desc));
                } else {
                    out.push_str(&format!("  {}\n", rest));
                }
            } else if let Some(ret) = line.strip_prefix("@return ") {
                out.push_str(&format!("\nReturns: {ret}\n"));
            } else if !line.is_empty() {
                out.push_str(&format!("{}\n", line));
            }
        }
    }

    if !descriptor.aliases.is_empty() {
        out.push_str(&format!("\nAliases: {}\n", descriptor.aliases.join(", ")));
    }
    out
}

/// Extract parameter names from a doc string's `@param name ...` lines.
fn extract_param_names_from_doc(doc: &str) -> Vec<String> {
    doc.lines()
        .filter_map(|line| {
            let line = line.trim();
            line.strip_prefix("@param ")
                .and_then(|rest| rest.split_whitespace().next().map(|name| name.to_string()))
        })
        .collect()
}

pub fn register_builtins(env: &Environment) {
    for descriptor in BUILTIN_REGISTRY {
        register_builtin_binding(env, descriptor.name, *descriptor);
        for &alias in descriptor.aliases {
            register_builtin_binding(env, alias, *descriptor);
        }
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
                Some("double.neg.eps".to_string()),
                RValue::vec(Vector::Double(vec![Some(f64::EPSILON / 2.0)].into())),
            ),
            (
                Some("double.xmax".to_string()),
                RValue::vec(Vector::Double(vec![Some(f64::MAX)].into())),
            ),
            (
                Some("double.xmin".to_string()),
                RValue::vec(Vector::Double(vec![Some(f64::MIN_POSITIVE)].into())),
            ),
            (
                Some("double.digits".to_string()),
                RValue::vec(Vector::Integer(vec![Some(53)].into())),
            ),
            (
                Some("double.max.exp".to_string()),
                RValue::vec(Vector::Integer(vec![Some(1024)].into())),
            ),
            (
                Some("double.min.exp".to_string()),
                RValue::vec(Vector::Integer(vec![Some(-1021)].into())),
            ),
            (
                Some("sizeof.long".to_string()),
                RValue::vec(Vector::Integer(
                    vec![Some(
                        i64::try_from(std::mem::size_of::<std::ffi::c_long>()).unwrap_or(8),
                    )]
                    .into(),
                )),
            ),
            (
                Some("sizeof.longlong".to_string()),
                RValue::vec(Vector::Integer(
                    vec![Some(
                        i64::try_from(std::mem::size_of::<std::ffi::c_longlong>()).unwrap_or(8),
                    )]
                    .into(),
                )),
            ),
            (
                Some("sizeof.pointer".to_string()),
                RValue::vec(Vector::Integer(
                    vec![Some(
                        i64::try_from(std::mem::size_of::<*const u8>()).unwrap_or(8),
                    )]
                    .into(),
                )),
            ),
            (
                Some("longdouble.eps".to_string()),
                RValue::vec(Vector::Double(vec![Some(f64::EPSILON)].into())),
            ),
            (
                Some("longdouble.max".to_string()),
                RValue::vec(Vector::Double(vec![Some(f64::MAX)].into())),
            ),
            (
                Some("longdouble.digits".to_string()),
                RValue::vec(Vector::Integer(vec![Some(53)].into())),
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

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::BUILTIN_REGISTRY;

    #[test]
    fn builtin_registry_names_are_unique() {
        let mut names = HashSet::new();

        for descriptor in BUILTIN_REGISTRY {
            assert!(
                names.insert(descriptor.name),
                "duplicate builtin name or alias in registry: {}",
                descriptor.name
            );
            for &alias in descriptor.aliases {
                assert!(
                    names.insert(alias),
                    "duplicate builtin name or alias in registry: {}",
                    alias
                );
            }
        }
    }
}

// === Builtin implementations ===

/// Combine values into a vector or list.
///
/// Coerces all arguments to a common type and concatenates them.
/// Named arguments become element names on the result.
///
/// @param ... values to combine
/// @return vector or list containing all input elements
#[builtin]
pub fn builtin_c(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    // Collect (optional_name, value) pairs
    let mut all_entries: Vec<(Option<String>, RValue)> = Vec::new();
    for arg in args {
        all_entries.push((None, arg.clone()));
    }
    for (name, val) in named {
        all_entries.push((Some(name.clone()), val.clone()));
    }

    if all_entries.is_empty() {
        return Ok(RValue::Null);
    }

    let all_values: Vec<RValue> = all_entries.iter().map(|(_, v)| v.clone()).collect();

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

    let vec_result = if has_char {
        let mut result = Vec::new();
        for val in &all_values {
            match val {
                RValue::Vector(v) => result.extend(v.to_characters()),
                RValue::Null => {}
                _ => {}
            }
        }
        RValue::vec(Vector::Character(result.into()))
    } else if has_complex {
        let mut result = Vec::new();
        for val in &all_values {
            match val {
                RValue::Vector(v) => result.extend(v.inner.to_complex()),
                RValue::Null => {}
                _ => {}
            }
        }
        RValue::vec(Vector::Complex(result.into()))
    } else if has_double {
        let mut result = Vec::new();
        for val in &all_values {
            match val {
                RValue::Vector(v) => result.extend(v.to_doubles()),
                RValue::Null => {}
                _ => {}
            }
        }
        RValue::vec(Vector::Double(result.into()))
    } else if has_int {
        let mut result = Vec::new();
        for val in &all_values {
            match val {
                RValue::Vector(v) => result.extend(v.to_integers()),
                RValue::Null => {}
                _ => {}
            }
        }
        RValue::vec(Vector::Integer(result.into()))
    } else if has_logical || !has_raw {
        let mut result = Vec::new();
        for val in &all_values {
            match val {
                RValue::Vector(v) => result.extend(v.to_logicals()),
                RValue::Null => {}
                _ => {}
            }
        }
        RValue::vec(Vector::Logical(result.into()))
    } else {
        let mut result = Vec::new();
        for val in &all_values {
            match val {
                RValue::Vector(v) => result.extend(v.inner.to_raw()),
                RValue::Null => {}
                _ => {}
            }
        }
        RValue::vec(Vector::Raw(result))
    };

    // Collect names from named arguments and existing names attributes
    let result = vec_result;
    let names = collect_c_names(&all_entries);
    if names.iter().any(|n| n.is_some()) {
        match result {
            RValue::Vector(mut rv) => {
                rv.set_attr(
                    "names".to_string(),
                    RValue::vec(Vector::Character(names.into())),
                );
                Ok(RValue::Vector(rv))
            }
            other => Ok(other),
        }
    } else {
        Ok(result)
    }
}

/// Collect element names for c(): named arguments provide names for scalars,
/// existing names attributes provide names for vector elements.
fn collect_c_names(entries: &[(Option<String>, RValue)]) -> Vec<Option<String>> {
    let mut names = Vec::new();
    for (arg_name, val) in entries {
        match val {
            RValue::Vector(rv) => {
                let len = rv.inner.len();
                // Check for existing names attribute
                let existing_names = rv
                    .get_attr("names")
                    .and_then(|v| v.as_vector())
                    .map(|v| v.to_characters());

                if let Some(ref enames) = existing_names {
                    for i in 0..len {
                        names.push(enames.get(i).cloned().flatten());
                    }
                } else if len == 1 {
                    // Scalar: use the argument name if present
                    names.push(arg_name.clone());
                } else {
                    // Multi-element unnamed vector
                    for _ in 0..len {
                        names.push(None);
                    }
                }
            }
            RValue::Null => {}
            _ => names.push(arg_name.clone()),
        }
    }
    names
}

/// Display help for a function.
///
/// @param topic name of the function to look up
#[builtin(min_args = 1)]
fn builtin_help(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let name = match args.first() {
        Some(RValue::Vector(rv)) => rv.as_character_scalar().unwrap_or_default(),
        Some(RValue::Function(RFunction::Builtin { name, .. })) => name.clone(),
        _ => String::new(),
    };
    if name.is_empty() {
        return Ok(RValue::Null);
    }
    match find_builtin(&name) {
        Some(descriptor) => {
            println!("{}", format_help(descriptor));
            Ok(RValue::Null)
        }
        None => {
            println!("No documentation for '{name}'");
            Ok(RValue::Null)
        }
    }
}

// print() is in interp.rs (S3-dispatching interpreter builtin)

/// Concatenate and print objects to stdout.
///
/// Converts each argument to character and writes them separated by `sep`.
/// Unlike `print()`, does not add a trailing newline unless the output contains one.
/// When `file` is specified, writes to that file instead of stdout. When `append`
/// is TRUE, appends to the file rather than overwriting.
///
/// @param ... objects to concatenate and print
/// @param sep separator string between elements (default: " ")
/// @param file a connection or file name to write to (default: "" meaning stdout)
/// @param append logical; if TRUE, append to file (default: FALSE)
/// @return NULL (invisibly)
#[builtin]
fn builtin_cat(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let sep = named
        .iter()
        .find(|(n, _)| n == "sep")
        .and_then(|(_, v)| v.as_vector()?.as_character_scalar())
        .unwrap_or_else(|| " ".to_string());

    let file = named
        .iter()
        .find(|(n, _)| n == "file")
        .and_then(|(_, v)| v.as_vector()?.as_character_scalar());

    let append = named
        .iter()
        .find(|(n, _)| n == "append")
        .and_then(|(_, v)| v.as_vector()?.as_logical_scalar())
        .unwrap_or(false);

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

    let output = parts.join(&sep);

    match file {
        Some(ref path) if !path.is_empty() => {
            use std::fs::OpenOptions;
            use std::io::Write;
            let mut f = OpenOptions::new()
                .write(true)
                .create(true)
                .append(append)
                .truncate(!append)
                .open(path)
                .map_err(|e| RError::other(format!("cannot open file '{}': {}", path, e)))?;
            f.write_all(output.as_bytes())
                .map_err(|e| RError::other(format!("error writing to '{}': {}", path, e)))?;
        }
        _ => {
            print!("{}", output);
        }
    }

    Ok(RValue::Null)
}

/// Concatenate strings with a separator.
///
/// Converts arguments to character, recycles to the longest length,
/// and joins corresponding elements with `sep`. Optionally collapses
/// the result into a single string using `collapse`.
///
/// @param ... objects to paste together
/// @param sep separator between arguments (default: " ")
/// @param collapse optional string to join result elements
/// @return character vector (length 1 if collapse is set)
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
            let collapsed: String = result.iter().filter_map(|s| s.as_ref()).join(&col);
            Ok(RValue::vec(Vector::Character(vec![Some(collapsed)].into())))
        }
        None => Ok(RValue::vec(Vector::Character(result.into()))),
    }
}

/// Concatenate strings with no separator.
///
/// Equivalent to `paste(..., sep = "")`.
///
/// @param ... objects to paste together
/// @param collapse optional string to join result elements
/// @return character vector
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

/// Get the length of an object.
///
/// @param x object to measure
/// @return integer scalar
#[builtin(min_args = 1)]
fn builtin_length(args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    let len = args.first().map(|v| v.length()).unwrap_or(0);
    Ok(RValue::vec(Vector::Integer(
        vec![Some(i64::try_from(len)?)].into(),
    )))
}

/// Count the number of characters in each element of a character vector.
///
/// @param x character vector
/// @param type one of "bytes", "chars", or "width" (default "chars")
/// @return integer vector of string lengths
#[builtin(min_args = 1)]
fn builtin_nchar(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    // Extract the "type" named argument (default: "chars")
    let nchar_type = named
        .iter()
        .find(|(k, _)| k == "type")
        .and_then(|(_, v)| match v {
            RValue::Vector(rv) => rv.inner.as_character_scalar(),
            _ => None,
        })
        .unwrap_or_else(|| "chars".to_string());

    match args.first() {
        Some(RValue::Vector(rv)) if matches!(rv.inner, Vector::Character(_)) => {
            let Vector::Character(vals) = &rv.inner else {
                unreachable!()
            };
            let result: Vec<Option<i64>> = vals
                .iter()
                .map(|s| {
                    s.as_ref().map(|s| {
                        let n = match nchar_type.as_str() {
                            "bytes" => s.len(),
                            "width" => UnicodeWidthStr::width(s.as_str()),
                            _ => s.chars().count(), // "chars" or default
                        };
                        i64::try_from(n).unwrap_or(0)
                    })
                })
                .collect();
            Ok(RValue::vec(Vector::Integer(result.into())))
        }
        _ => Ok(RValue::vec(Vector::Integer(vec![None].into()))),
    }
}

/// Get the names attribute of an object.
///
/// @param x object whose names to retrieve
/// @return character vector of names, or NULL if unnamed
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

/// Set the names attribute of an object.
///
/// @param x object to modify
/// @param value character vector of new names, or NULL to remove
/// @return the modified object
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

struct BindInput {
    data: Vec<Option<f64>>,
    nrow: usize,
    ncol: usize,
    row_names: Option<Vec<Option<String>>>,
    col_names: Option<Vec<Option<String>>>,
}

fn dimnames_component(dimnames: Option<&RValue>, index: usize) -> Option<Vec<Option<String>>> {
    let Some(RValue::List(list)) = dimnames else {
        return None;
    };
    list.values
        .get(index)
        .and_then(|(_, value)| coerce_name_values(value))
}

fn bind_dimnames_value(
    row_names: Vec<Option<String>>,
    col_names: Vec<Option<String>>,
) -> Option<RValue> {
    let has_row_names = row_names.iter().any(|name| name.is_some());
    let has_col_names = col_names.iter().any(|name| name.is_some());
    if !has_row_names && !has_col_names {
        return None;
    }

    Some(RValue::List(RList::new(vec![
        (None, character_name_vector(row_names)),
        (None, character_name_vector(col_names)),
    ])))
}

/// Get the row names of a data frame or matrix.
///
/// @param x data frame, matrix, or object with dimnames
/// @return character vector of row names, or NULL
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

/// Get the column names of a data frame or matrix.
///
/// @param x data frame, matrix, or object with dimnames
/// @return character vector of column names, or NULL
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

/// Set the row names of a data frame or matrix.
///
/// @param x data frame, matrix, or object with dimnames
/// @param value character vector of new row names, or NULL
/// @return the modified object
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

/// Set the column names of a data frame or matrix.
///
/// @param x data frame, matrix, or object with dimnames
/// @param value character vector of new column names, or NULL
/// @return the modified object
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

/// Set the class attribute of an object.
///
/// @param x object to modify
/// @param value character vector of class names, or NULL to remove
/// @return the modified object
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

/// Get the internal type of an object.
///
/// Returns the low-level type name (e.g., "double", "integer", "character",
/// "logical", "list", "closure", "builtin", "NULL").
///
/// @param x object to inspect
/// @return character scalar
#[builtin(min_args = 1)]
fn builtin_typeof(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let t = args.first().map(|v| v.type_name()).unwrap_or("NULL");
    Ok(RValue::vec(Vector::Character(
        vec![Some(t.to_string())].into(),
    )))
}

/// Get the class of an object.
///
/// Returns the explicit class attribute if set, otherwise the implicit
/// class based on the object's type (e.g., "numeric", "character", "list").
///
/// @param x object to inspect
/// @return character vector of class names
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

/// Get the mode (storage type) of an object.
///
/// Similar to `typeof()` but maps integer and double to "numeric".
///
/// @param x object to inspect
/// @return character scalar
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

/// Display the compact internal structure of an object.
///
/// Prints type, length, and a preview of the first few elements.
///
/// @param x object to inspect
/// @return NULL (invisibly)
#[builtin(min_args = 1)]
fn builtin_str(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(val) => {
            match val {
                RValue::Vector(v) => {
                    let len = v.len();
                    let type_name = v.type_name();
                    let preview: String = match &v.inner {
                        Vector::Raw(vals) => {
                            vals.iter().take(10).map(|b| format!("{:02x}", b)).join(" ")
                        }
                        Vector::Double(vals) => vals
                            .iter()
                            .take(10)
                            .map(|x| match x {
                                Some(f) => format_r_double(*f),
                                None => "NA".to_string(),
                            })
                            .join(" "),
                        Vector::Integer(vals) => vals
                            .iter()
                            .take(10)
                            .map(|x| match x {
                                Some(i) => i.to_string(),
                                None => "NA".to_string(),
                            })
                            .join(" "),
                        Vector::Logical(vals) => vals
                            .iter()
                            .take(10)
                            .map(|x| match x {
                                Some(true) => "TRUE".to_string(),
                                Some(false) => "FALSE".to_string(),
                                None => "NA".to_string(),
                            })
                            .join(" "),
                        Vector::Complex(vals) => vals
                            .iter()
                            .take(10)
                            .map(|x| match x {
                                Some(c) => format_r_complex(*c),
                                None => "NA".to_string(),
                            })
                            .join(" "),
                        Vector::Character(vals) => vals
                            .iter()
                            .take(10)
                            .map(|x| match x {
                                Some(s) => format!("\"{}\"", s),
                                None => "NA".to_string(),
                            })
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

/// Test if two objects are exactly identical.
///
/// @param x first object
/// @param y second object
/// @return logical scalar
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

/// Test near-equality of two objects within a tolerance.
///
/// For numeric vectors, checks that all corresponding elements differ by
/// at most `tolerance`. Returns TRUE or a descriptive character string.
///
/// @param target first object
/// @param current second object
/// @param tolerance maximum allowed difference (default: 1.5e-8)
/// @return TRUE if equal, or character string describing the difference
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

/// Test if any values are TRUE.
///
/// @param ... logical vectors to test
/// @param na.rm if TRUE, remove NA values before testing
/// @return logical scalar
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

/// Test if all values are TRUE.
///
/// @param ... logical vectors to test
/// @param na.rm if TRUE, remove NA values before testing
/// @return logical scalar
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

/// Exclusive OR of two logical values.
///
/// @param x first logical value
/// @param y second logical value
/// @return logical scalar
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

/// Construct a list from the given arguments.
///
/// Named arguments become named elements of the list.
///
/// @param ... values to include in the list
/// @return list
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

/// Create a vector of a given mode and length.
///
/// @param mode type of vector ("numeric", "integer", "character", "logical", "list")
/// @param length number of elements (default: 0)
/// @return vector initialized with default values for the mode
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

/// Flatten a list into an atomic vector.
///
/// Recursively combines list elements using the same coercion rules as `c()`.
///
/// @param x list to flatten
/// @return atomic vector
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

/// Return a value invisibly (suppresses auto-printing).
///
/// @param x value to return
/// @return x (invisibly)
#[builtin(min_args = 1)]
fn builtin_invisible(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    Ok(args.first().cloned().unwrap_or(RValue::Null))
}

/// Vectorized conditional: select elements from yes or no based on test.
///
/// @param test logical condition
/// @param yes value to use when test is TRUE
/// @param no value to use when test is FALSE
/// @return value from yes or no depending on test
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

/// Find positions of first matches of x in table.
///
/// For each element of x, returns the position of its first exact match in table,
/// or NA if no match is found.
///
/// @param x values to look up
/// @param table values to match against
/// @return integer vector of match positions (1-indexed)
#[builtin(min_args = 2)]
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

/// Partial string matching — find unique partial matches of x in table.
///
/// For each element of x, returns the position of its unique partial match in table
/// (i.e. the table element that starts with x). Returns NA if no match or if
/// multiple table entries match (ambiguous). Exact matches are preferred.
///
/// @param x values to look up (partial strings)
/// @param table values to match against
/// @return integer vector of match positions (1-indexed), or NA for no/ambiguous match
#[builtin(min_args = 2)]
fn builtin_pmatch(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let x = match args.first() {
        Some(RValue::Vector(v)) => v.to_characters(),
        _ => return Ok(RValue::vec(Vector::Integer(vec![None].into()))),
    };
    let table = match args.get(1) {
        Some(RValue::Vector(v)) => v.to_characters(),
        _ => return Ok(RValue::vec(Vector::Integer(vec![None].into()))),
    };

    let result: Vec<Option<i64>> = x
        .iter()
        .map(|xi| {
            xi.as_ref().and_then(|xi| {
                // First try exact match
                if let Some(pos) = table.iter().position(|t| t.as_deref() == Some(xi.as_str())) {
                    return Some(i64::try_from(pos).map(|v| v + 1).unwrap_or(0));
                }
                // Then try unique partial match (prefix)
                let matches: Vec<usize> = table
                    .iter()
                    .enumerate()
                    .filter(|(_, t)| {
                        t.as_ref()
                            .map(|t| t.starts_with(xi.as_str()))
                            .unwrap_or(false)
                    })
                    .map(|(i, _)| i)
                    .collect();
                if matches.len() == 1 {
                    Some(i64::try_from(matches[0]).map(|v| v + 1).unwrap_or(0))
                } else {
                    None // no match or ambiguous
                }
            })
        })
        .collect();
    Ok(RValue::vec(Vector::Integer(result.into())))
}

/// Character partial matching — like pmatch but returns 0 for ambiguous matches.
///
/// For each element of x, returns the position of its unique partial match in table.
/// Returns NA for no match, and 0 for ambiguous matches (multiple partial matches).
///
/// @param x values to look up
/// @param table values to match against
/// @return integer vector of match positions (1-indexed), 0 for ambiguous, NA for no match
#[builtin(min_args = 2)]
fn builtin_charmatch(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let x = match args.first() {
        Some(RValue::Vector(v)) => v.to_characters(),
        _ => return Ok(RValue::vec(Vector::Integer(vec![None].into()))),
    };
    let table = match args.get(1) {
        Some(RValue::Vector(v)) => v.to_characters(),
        _ => return Ok(RValue::vec(Vector::Integer(vec![None].into()))),
    };

    let result: Vec<Option<i64>> = x
        .iter()
        .map(|xi| {
            xi.as_ref().and_then(|xi| {
                // First check for exact match
                if let Some(pos) = table.iter().position(|t| t.as_deref() == Some(xi.as_str())) {
                    return Some(i64::try_from(pos).map(|v| v + 1).unwrap_or(0));
                }
                // Then try partial match (prefix)
                let matches: Vec<usize> = table
                    .iter()
                    .enumerate()
                    .filter(|(_, t)| {
                        t.as_ref()
                            .map(|t| t.starts_with(xi.as_str()))
                            .unwrap_or(false)
                    })
                    .map(|(i, _)| i)
                    .collect();
                match matches.len() {
                    0 => None,                                                        // no match -> NA
                    1 => Some(i64::try_from(matches[0]).map(|v| v + 1).unwrap_or(0)), // unique match
                    _ => Some(0), // ambiguous -> 0
                }
            })
        })
        .collect();
    Ok(RValue::vec(Vector::Integer(result.into())))
}

/// Replace values in a vector at specified indices.
///
/// @param x vector to modify
/// @param list indices at which to replace (1-indexed)
/// @param values replacement values (recycled if shorter)
/// @return modified vector
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

// options() and getOption() are interpreter builtins in interp.rs

// Sys.time() is in datetime.rs; proc.time() is in system.rs

/// Read a line of user input from stdin.
///
/// @param prompt string to display before reading input
/// @return character scalar of the input (without trailing newline)
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

/// Get an environment variable from the interpreter's environment.
///
/// @param x name of the environment variable
/// @return character scalar with the variable's value, or "" if unset
#[interpreter_builtin(name = "Sys.getenv")]
fn interp_sys_getenv(
    args: &[RValue],
    _: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let name = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();
    let val = context
        .with_interpreter(|interp| interp.get_env_var(&name))
        .unwrap_or_default();
    Ok(RValue::vec(Vector::Character(vec![Some(val)].into())))
}

// (file.path, file.exists, readLines, writeLines, read.csv, write.csv — io.rs)

/// Load a package (stub).
///
/// Prints a warning that the package is not available and returns FALSE.
/// Also aliased as `library`.
///
/// @param package name of the package to load
/// @return logical FALSE
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

/// Get the version of the R implementation.
///
/// Returns a list with major, minor, language, and engine fields.
///
/// @return named list of version information
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

/// Canonical string key for set operations — works across numeric and character types.
fn set_key(val: &Option<String>) -> String {
    match val {
        Some(s) => s.clone(),
        None => "NA".to_string(),
    }
}

/// Determine whether set operations should work in numeric or character mode.
/// Returns the result type based on input types: numeric inputs stay numeric.
fn set_op_extract(
    x: &RValue,
    y: &RValue,
) -> (
    Vec<Option<String>>,
    Vec<Option<String>>,
    bool, /* numeric */
) {
    let x_vec = x.as_vector();
    let y_vec = y.as_vector();
    let is_numeric = matches!(
        (x_vec, y_vec),
        (
            Some(Vector::Integer(_) | Vector::Double(_)),
            Some(Vector::Integer(_) | Vector::Double(_))
        )
    );
    let xc = x_vec.map(|v| v.to_characters()).unwrap_or_default();
    let yc = y_vec.map(|v| v.to_characters()).unwrap_or_default();
    (xc, yc, is_numeric)
}

/// Reconstruct a numeric vector from character representations produced by set operations.
fn set_result_numeric(chars: Vec<Option<String>>) -> RValue {
    // Try integer first, fall back to double
    let as_ints: Option<Vec<Option<i64>>> = chars
        .iter()
        .map(|c| match c {
            None => Some(None),
            Some(s) => s.parse::<i64>().ok().map(Some),
        })
        .collect();
    if let Some(ints) = as_ints {
        return RValue::vec(Vector::Integer(ints.into()));
    }
    let doubles: Vec<Option<f64>> = chars
        .iter()
        .map(|c| c.as_ref().and_then(|s| s.parse::<f64>().ok()))
        .collect();
    RValue::vec(Vector::Double(doubles.into()))
}

/// Set difference: elements in x that are not in y.
///
/// Works on character, integer, and double vectors. For numeric inputs,
/// returns a numeric vector; for character inputs, returns a character vector.
///
/// @param x vector
/// @param y vector
/// @return vector of elements in x but not y
#[builtin(min_args = 2)]
fn builtin_setdiff(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let (x, y, numeric) = set_op_extract(&args[0], &args[1]);
    let y_keys: std::collections::HashSet<String> = y.iter().map(set_key).collect();
    let result: Vec<Option<String>> = x
        .into_iter()
        .filter(|xi| !y_keys.contains(&set_key(xi)))
        .collect();
    if numeric {
        Ok(set_result_numeric(result))
    } else {
        Ok(RValue::vec(Vector::Character(result.into())))
    }
}

/// Set intersection: elements present in both x and y.
///
/// Works on character, integer, and double vectors. For numeric inputs,
/// returns a numeric vector; for character inputs, returns a character vector.
///
/// @param x vector
/// @param y vector
/// @return vector of common elements
#[builtin(min_args = 2)]
fn builtin_intersect(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let (x, y, numeric) = set_op_extract(&args[0], &args[1]);
    let y_keys: std::collections::HashSet<String> = y.iter().map(set_key).collect();
    let result: Vec<Option<String>> = x
        .into_iter()
        .filter(|xi| y_keys.contains(&set_key(xi)))
        .collect();
    if numeric {
        Ok(set_result_numeric(result))
    } else {
        Ok(RValue::vec(Vector::Character(result.into())))
    }
}

/// Set union: unique elements from both x and y.
///
/// Works on character, integer, and double vectors. For numeric inputs,
/// returns a numeric vector; for character inputs, returns a character vector.
///
/// @param x vector
/// @param y vector
/// @return vector of all unique elements
#[builtin(min_args = 2)]
fn builtin_union(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let (x, y, numeric) = set_op_extract(&args[0], &args[1]);
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();
    for xi in x.into_iter().chain(y) {
        let key = set_key(&xi);
        if seen.insert(key) {
            result.push(xi);
        }
    }
    if numeric {
        Ok(set_result_numeric(result))
    } else {
        Ok(RValue::vec(Vector::Character(result.into())))
    }
}

/// Bin continuous values into intervals, returning a factor with interval labels.
///
/// @param x numeric vector of values to bin
/// @param breaks numeric vector of cut points (must be sorted, length >= 2)
/// @param labels optional character vector of labels (length = length(breaks) - 1)
/// @param include.lowest if TRUE, the lowest break is inclusive on the left
/// @param right if TRUE (default), intervals are (a,b]; if FALSE, [a,b)
/// @return integer vector with factor class and interval-label levels
#[builtin(min_args = 2)]
fn builtin_cut(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let ca = CallArgs::new(args, named);

    let x_vals = ca
        .value("x", 0)
        .and_then(|v| v.as_vector().map(|v| v.to_doubles()))
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "'x' must be a numeric vector".to_string(),
            )
        })?;
    let breaks = ca
        .value("breaks", 1)
        .and_then(|v| v.as_vector().map(|v| v.to_doubles()))
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "'breaks' must be a numeric vector".to_string(),
            )
        })?;

    let breaks: Vec<f64> = breaks.into_iter().flatten().collect();
    if breaks.len() < 2 {
        return Err(RError::new(
            RErrorKind::Argument,
            "'breaks' must have at least 2 values".to_string(),
        ));
    }

    let right = ca.logical_flag("right", 4, true);
    let include_lowest = ca.logical_flag("include.lowest", 5, false);

    let custom_labels: Option<Vec<String>> = ca.value("labels", 2).and_then(|v| {
        v.as_vector().and_then(|vec| {
            let chars = vec.to_characters();
            chars.into_iter().collect::<Option<Vec<String>>>()
        })
    });

    let n_intervals = breaks.len() - 1;

    // Generate default interval labels
    let labels: Vec<String> = if let Some(ref cl) = custom_labels {
        if cl.len() != n_intervals {
            return Err(RError::new(
                RErrorKind::Argument,
                format!(
                    "lengths of 'breaks' and 'labels' differ: {} breaks produce {} intervals but {} labels given",
                    breaks.len(),
                    n_intervals,
                    cl.len()
                ),
            ));
        }
        cl.clone()
    } else {
        (0..n_intervals)
            .map(|i| {
                let lo = breaks[i];
                let hi = breaks[i + 1];
                if right {
                    format!("({},{}]", format_r_double_cut(lo), format_r_double_cut(hi))
                } else {
                    format!("[{},{})", format_r_double_cut(lo), format_r_double_cut(hi))
                }
            })
            .collect()
    };

    // Bin each value
    let codes: Vec<Option<i64>> = x_vals
        .iter()
        .map(|x| {
            let val = match x {
                Some(v) if v.is_finite() => *v,
                _ => return None, // NA or non-finite -> NA
            };
            for i in 0..n_intervals {
                let lo = breaks[i];
                let hi = breaks[i + 1];
                let in_bin = if right {
                    let lower_ok = if include_lowest && i == 0 {
                        val >= lo
                    } else {
                        val > lo
                    };
                    lower_ok && val <= hi
                } else {
                    let upper_ok = if include_lowest && i == n_intervals - 1 {
                        val <= hi
                    } else {
                        val < hi
                    };
                    val >= lo && upper_ok
                };
                if in_bin {
                    return Some(i64::try_from(i + 1).unwrap_or(0));
                }
            }
            None // outside all breaks
        })
        .collect();

    let mut rv = RVector::from(Vector::Integer(codes.into()));
    rv.set_attr(
        "levels".to_string(),
        RValue::vec(Vector::Character(
            labels.into_iter().map(Some).collect::<Vec<_>>().into(),
        )),
    );
    rv.set_attr(
        "class".to_string(),
        RValue::vec(Vector::Character(vec![Some("factor".to_string())].into())),
    );
    Ok(RValue::Vector(rv))
}

/// Format a double for cut() labels — avoid trailing zeros but keep precision.
fn format_r_double_cut(x: f64) -> String {
    if x == x.floor() && x.abs() < 1e15 {
        format!("{}", x as i64)
    } else {
        format!("{}", x)
    }
}

/// Find the interval index containing each element of x.
///
/// For a sorted vector `vec` of length N, returns an integer vector of the same
/// length as `x`, where each value is in 0..N indicating which interval the
/// corresponding x value falls into (0 = before vec[0], N = after vec[N-1]).
///
/// @param x numeric vector
/// @param vec sorted numeric vector of break points
/// @return integer vector of interval indices
#[builtin(name = "findInterval", min_args = 2)]
fn builtin_find_interval(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let ca = CallArgs::new(args, named);

    let x_vals = ca
        .value("x", 0)
        .and_then(|v| v.as_vector().map(|v| v.to_doubles()))
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "'x' must be a numeric vector".to_string(),
            )
        })?;
    let vec_vals: Vec<f64> = ca
        .value("vec", 1)
        .and_then(|v| v.as_vector().map(|v| v.to_doubles()))
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "'vec' must be a numeric vector".to_string(),
            )
        })?
        .into_iter()
        .flatten()
        .collect();

    let result: Vec<Option<i64>> = x_vals
        .iter()
        .map(|x| {
            let val = (*x)?;
            // Binary search: find largest index i where vec_vals[i] <= val
            let mut lo = 0usize;
            let mut hi = vec_vals.len();
            while lo < hi {
                let mid = lo + (hi - lo) / 2;
                if vec_vals[mid] <= val {
                    lo = mid + 1;
                } else {
                    hi = mid;
                }
            }
            Some(i64::try_from(lo).unwrap_or(0))
        })
        .collect();
    Ok(RValue::vec(Vector::Integer(result.into())))
}

/// Identify duplicate elements in a vector.
///
/// Returns TRUE for elements that have already appeared earlier in the vector.
///
/// @param x vector to check
/// @return logical vector of the same length
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

/// Get the interpreter's current working directory.
///
/// @return character scalar with the absolute path
#[interpreter_builtin]
fn interp_getwd(
    _args: &[RValue],
    _: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let cwd =
        context.with_interpreter(|interp| interp.get_working_dir().to_string_lossy().to_string());
    Ok(RValue::vec(Vector::Character(vec![Some(cwd)].into())))
}

/// Create a double vector of zeros with the given length.
///
/// Also aliased as `double`.
///
/// @param length number of elements (default: 0)
/// @return double vector initialized to 0.0
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

/// Create an integer vector of zeros with the given length.
///
/// @param length number of elements (default: 0)
/// @return integer vector initialized to 0
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

/// Create a logical vector of FALSE values with the given length.
///
/// @param length number of elements (default: 0)
/// @return logical vector initialized to FALSE
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

/// Create a character vector of empty strings with the given length.
///
/// @param length number of elements (default: 0)
/// @return character vector initialized to ""
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

/// Create a matrix from the given data.
///
/// Fills column-by-column by default. Use `byrow = TRUE` for row-major fill.
///
/// @param data vector of values to fill the matrix
/// @param nrow number of rows
/// @param ncol number of columns
/// @param byrow if TRUE, fill by rows instead of columns
/// @param dimnames list of row and column name vectors
/// @return matrix (vector with dim and class attributes)
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

    let data_inner = match &data {
        RValue::Vector(v) => &v.inner,
        _ => {
            return Err(RError::new(
                RErrorKind::Type,
                "data must be a vector".to_string(),
            ));
        }
    };
    let data_len = data_inner.len();

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
    // Build index mapping: source index for each element in the result
    let indices: Vec<usize> = if byrow {
        (0..nrow)
            .flat_map(|i| (0..ncol).map(move |j| (i * ncol + j) % data_len))
            .collect()
    } else {
        (0..total).map(|idx| idx % data_len).collect()
    };

    let mat_vec = data_inner.select_indices(&indices);
    let mut rv = RVector::from(mat_vec);
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

/// Get the dimensions of a matrix, array, or data frame.
///
/// @param x object to query
/// @return integer vector of dimensions, or NULL if none
#[builtin(min_args = 1)]
fn builtin_dim(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(rv)) => Ok(rv.get_attr("dim").cloned().unwrap_or(RValue::Null)),
        Some(value @ RValue::List(l)) if has_class(value, "data.frame") => {
            Ok(RValue::vec(Vector::Integer(
                vec![
                    Some(i64::try_from(data_frame_row_count(l))?),
                    Some(i64::try_from(l.values.len())?),
                ]
                .into(),
            )))
        }
        Some(RValue::List(l)) => Ok(l.get_attr("dim").cloned().unwrap_or(RValue::Null)),
        _ => Ok(RValue::Null),
    }
}

/// Set the dimensions of an object, converting it to a matrix or array.
///
/// @param x vector to reshape
/// @param value integer vector of new dimensions, or NULL to remove
/// @return the reshaped object
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

/// Get the number of rows of a matrix, array, or data frame.
///
/// @param x object to query
/// @return integer scalar, or NULL for objects without dimensions
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

/// Get the number of columns of a matrix, array, or data frame.
///
/// @param x object to query
/// @return integer scalar, or NULL for objects without dimensions
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

/// Get the number of rows, treating non-matrix vectors as 1-column matrices.
///
/// Unlike `nrow()`, returns the vector length instead of NULL for plain vectors.
///
/// @param x object to query
/// @return integer scalar
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

/// Get the number of columns, treating non-matrix vectors as 1-column.
///
/// Unlike `ncol()`, returns 1 instead of NULL for plain vectors.
///
/// @param x object to query
/// @return integer scalar
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

// t() is in math.rs (type-preserving implementation)

/// Remove names and dimnames from an object.
///
/// @param obj object to strip names from
/// @return the object with names and dimnames attributes removed
#[builtin(min_args = 1)]
fn builtin_unname(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(rv)) => {
            let mut rv = rv.clone();
            rv.attrs.as_mut().map(|a| a.remove("names"));
            rv.attrs.as_mut().map(|a| a.remove("dimnames"));
            Ok(RValue::Vector(rv))
        }
        Some(RValue::List(l)) => {
            let mut l = l.clone();
            for entry in &mut l.values {
                entry.0 = None;
            }
            l.attrs.as_mut().map(|a| a.remove("names"));
            l.attrs.as_mut().map(|a| a.remove("dimnames"));
            Ok(RValue::List(l))
        }
        other => Ok(other.cloned().unwrap_or(RValue::Null)),
    }
}

/// Get the dimension names of a matrix, array, or data frame.
///
/// @param x object to query
/// @return list of character vectors (one per dimension), or NULL
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

/// Set the dimension names of a matrix, array, or data frame.
///
/// @param x object to modify
/// @param value list of character vectors (one per dimension), or NULL
/// @return the modified object
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

/// Create a multi-dimensional array.
///
/// @param data vector of values to fill the array (recycled if needed)
/// @param dim integer vector of dimensions
/// @param dimnames list of dimension name vectors
/// @return array (vector with dim and class attributes)
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

/// Transpose or permute the dimensions of an array.
///
/// For 2D arrays (matrices), this is equivalent to `t()`. For higher-dimensional
/// arrays, `perm` specifies the new ordering of dimensions. By default, `perm`
/// reverses the dimensions (i.e. for a 3D array with dims (a,b,c), the default
/// permutation is c(3,2,1) giving dims (c,b,a)).
///
/// @param a an array (vector with dim attribute)
/// @param perm integer vector specifying the new dimension order (1-based)
/// @return array with permuted dimensions
#[builtin(min_args = 1)]
fn builtin_aperm(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let x = args.first().ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "aperm() requires an array argument".to_string(),
        )
    })?;

    let rv = match x {
        RValue::Vector(rv) => rv,
        _ => {
            return Err(RError::new(
                RErrorKind::Type,
                "aperm() requires an array (a vector with dim attribute)".to_string(),
            ))
        }
    };

    // Extract dimensions
    let dims: Vec<usize> = match get_dim_ints(rv.get_attr("dim")) {
        Some(dim_ints) => dim_ints
            .iter()
            .map(|x| usize::try_from(x.unwrap_or(0)))
            .collect::<Result<Vec<_>, _>>()?,
        None => {
            return Err(RError::new(
                RErrorKind::Argument,
                "aperm() requires an array with a 'dim' attribute".to_string(),
            ))
        }
    };

    let ndim = dims.len();

    // Parse perm argument: either named or positional
    let perm_arg = named
        .iter()
        .find(|(n, _)| n == "perm")
        .map(|(_, v)| v)
        .or(args.get(1));

    // perm is 1-based in R, convert to 0-based
    let perm: Vec<usize> = match perm_arg {
        Some(val) => {
            let ints = match val.as_vector() {
                Some(v) => v.to_integers(),
                None => {
                    return Err(RError::new(
                        RErrorKind::Argument,
                        "'perm' must be a numeric vector".to_string(),
                    ))
                }
            };
            if ints.len() != ndim {
                return Err(RError::new(
                    RErrorKind::Argument,
                    format!(
                        "'perm' must have length {} (matching the number of dimensions), got {}",
                        ndim,
                        ints.len()
                    ),
                ));
            }
            let mut p = Vec::with_capacity(ndim);
            for &v in &ints {
                let idx = usize::try_from(v.unwrap_or(0))?;
                if idx < 1 || idx > ndim {
                    return Err(RError::new(
                        RErrorKind::Argument,
                        format!("perm values must be between 1 and {}, got {}", ndim, idx),
                    ));
                }
                p.push(idx - 1);
            }
            p
        }
        None => {
            // Default: reverse dimensions
            (0..ndim).rev().collect()
        }
    };

    // Validate perm is a valid permutation (all dimensions appear exactly once)
    let mut seen = vec![false; ndim];
    for &p in &perm {
        if seen[p] {
            return Err(RError::new(
                RErrorKind::Argument,
                "perm must be a permutation of 1:n where n is the number of dimensions".to_string(),
            ));
        }
        seen[p] = true;
    }

    let total: usize = dims.iter().product();
    let data = rv.to_doubles();

    // Compute new dimensions
    let new_dims: Vec<usize> = perm.iter().map(|&p| dims[p]).collect();

    // Compute strides for original array (column-major / Fortran order)
    let mut old_strides = vec![1usize; ndim];
    for d in 1..ndim {
        old_strides[d] = old_strides[d - 1] * dims[d - 1];
    }

    // Compute strides for new array
    let mut new_strides = vec![1usize; ndim];
    for d in 1..ndim {
        new_strides[d] = new_strides[d - 1] * new_dims[d - 1];
    }

    // Permute: for each position in the new array, compute the corresponding
    // position in the old array
    let mut result = vec![None; total];
    for (new_flat, slot) in result.iter_mut().enumerate() {
        // Decompose new_flat into new multi-index
        let mut remainder = new_flat;
        let mut new_idx = vec![0usize; ndim];
        for d in (0..ndim).rev() {
            new_idx[d] = remainder / new_strides[d];
            remainder %= new_strides[d];
        }

        // Map to old multi-index via inverse permutation
        let mut old_flat = 0;
        for d in 0..ndim {
            old_flat += new_idx[d] * old_strides[perm[d]];
        }

        *slot = data[old_flat];
    }

    let mut out = RVector::from(Vector::Double(result.into()));

    // Set dim attribute
    out.set_attr(
        "dim".to_string(),
        RValue::vec(Vector::Integer(
            new_dims
                .iter()
                .map(|&d| i64::try_from(d).map(Some))
                .collect::<Result<Vec<_>, _>>()?
                .into(),
        )),
    );

    // Set class attribute
    if new_dims.len() == 2 {
        out.set_attr(
            "class".to_string(),
            RValue::vec(Vector::Character(
                vec![Some("matrix".to_string()), Some("array".to_string())].into(),
            )),
        );
    } else {
        out.set_attr(
            "class".to_string(),
            RValue::vec(Vector::Character(vec![Some("array".to_string())].into())),
        );
    }

    // Permute dimnames if present
    if let Some(RValue::List(dimnames_list)) = rv.get_attr("dimnames") {
        let mut new_dimnames_vals = Vec::with_capacity(ndim);
        for &p in &perm {
            if p < dimnames_list.values.len() {
                new_dimnames_vals.push(dimnames_list.values[p].clone());
            } else {
                new_dimnames_vals.push((None, RValue::Null));
            }
        }
        out.set_attr(
            "dimnames".to_string(),
            RValue::List(RList::new(new_dimnames_vals)),
        );
    }

    Ok(RValue::Vector(out))
}

/// Bind vectors or matrices by rows.
///
/// Combines arguments row-wise into a matrix. Vectors become single rows.
///
/// @param ... vectors or matrices to bind
/// @return matrix
#[builtin(min_args = 1)]
fn builtin_rbind(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    if args.is_empty() {
        return Ok(RValue::Null);
    }

    let mut inputs = Vec::new();
    for arg in args {
        match arg {
            RValue::Vector(rv) => {
                let data = rv.to_doubles();
                if let Some(dims) = get_dim_ints(rv.get_attr("dim")) {
                    if dims.len() >= 2 {
                        let nr = usize::try_from(dims[0].unwrap_or(0))?;
                        let nc = usize::try_from(dims[1].unwrap_or(0))?;
                        inputs.push(BindInput {
                            data,
                            nrow: nr,
                            ncol: nc,
                            row_names: dimnames_component(rv.get_attr("dimnames"), 0),
                            col_names: dimnames_component(rv.get_attr("dimnames"), 1),
                        });
                        continue;
                    }
                }
                // Plain vector becomes a 1-row matrix
                let len = data.len();
                inputs.push(BindInput {
                    data,
                    nrow: 1,
                    ncol: len,
                    row_names: None,
                    col_names: rv.get_attr("names").and_then(coerce_name_values),
                });
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
    let max_ncol = inputs.iter().map(|input| input.ncol).max().unwrap_or(0);
    if max_ncol == 0 {
        return Ok(RValue::Null);
    }

    // Check column compatibility
    for input in &inputs {
        if input.ncol != max_ncol && max_ncol % input.ncol != 0 && input.ncol % max_ncol != 0 {
            return Err(RError::new(
                RErrorKind::Argument,
                "number of columns of arguments do not match".to_string(),
            ));
        }
    }

    // Total rows
    let total_nrow: usize = inputs.iter().map(|input| input.nrow).sum();

    // Build result column-major: for each column j, concatenate rows from all inputs
    let mut result = Vec::with_capacity(total_nrow * max_ncol);
    for j in 0..max_ncol {
        for input in &inputs {
            let actual_j = j % input.ncol;
            for i in 0..input.nrow {
                // Column-major index: col * nrow + row
                let idx = actual_j * input.nrow + i;
                result.push(if idx < input.data.len() {
                    input.data[idx]
                } else {
                    None
                });
            }
        }
    }

    let mut row_names = Vec::new();
    let has_any_row_names = inputs.iter().any(|input| input.row_names.is_some());
    if has_any_row_names {
        for input in &inputs {
            if let Some(names) = &input.row_names {
                row_names.extend(
                    (0..input.nrow)
                        .map(|idx| names.get(idx).cloned().unwrap_or(None))
                        .collect::<Vec<_>>(),
                );
            } else {
                row_names.extend(std::iter::repeat_n(None, input.nrow));
            }
        }
    }

    let mut col_names = Vec::new();
    if let Some(source_names) = inputs.iter().find_map(|input| input.col_names.clone()) {
        col_names.extend(
            (0..max_ncol)
                .map(|idx| {
                    source_names
                        .get(idx % source_names.len())
                        .cloned()
                        .unwrap_or(None)
                })
                .collect::<Vec<_>>(),
        );
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
    if let Some(dimnames) = bind_dimnames_value(row_names, col_names) {
        rv.set_attr("dimnames".to_string(), dimnames);
    }
    Ok(RValue::Vector(rv))
}

/// Bind vectors or matrices by columns.
///
/// Combines arguments column-wise into a matrix. Vectors become single columns.
///
/// @param ... vectors or matrices to bind
/// @return matrix
#[builtin(min_args = 1)]
fn builtin_cbind(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    if args.is_empty() {
        return Ok(RValue::Null);
    }

    let mut inputs = Vec::new();
    for arg in args {
        match arg {
            RValue::Vector(rv) => {
                let data = rv.to_doubles();
                if let Some(dims) = get_dim_ints(rv.get_attr("dim")) {
                    if dims.len() >= 2 {
                        let nr = usize::try_from(dims[0].unwrap_or(0))?;
                        let nc = usize::try_from(dims[1].unwrap_or(0))?;
                        inputs.push(BindInput {
                            data,
                            nrow: nr,
                            ncol: nc,
                            row_names: dimnames_component(rv.get_attr("dimnames"), 0),
                            col_names: dimnames_component(rv.get_attr("dimnames"), 1),
                        });
                        continue;
                    }
                }
                // Plain vector becomes a 1-column matrix
                let len = data.len();
                inputs.push(BindInput {
                    data,
                    nrow: len,
                    ncol: 1,
                    row_names: rv.get_attr("names").and_then(coerce_name_values),
                    col_names: None,
                });
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
    let max_nrow = inputs.iter().map(|input| input.nrow).max().unwrap_or(0);
    if max_nrow == 0 {
        return Ok(RValue::Null);
    }

    // Check row compatibility
    for input in &inputs {
        if input.nrow != max_nrow && max_nrow % input.nrow != 0 && input.nrow % max_nrow != 0 {
            return Err(RError::new(
                RErrorKind::Argument,
                "number of rows of arguments do not match".to_string(),
            ));
        }
    }

    // Total columns
    let total_ncol: usize = inputs.iter().map(|input| input.ncol).sum();

    // Build result column-major: for each input, append its columns (recycling rows)
    let mut result = Vec::with_capacity(max_nrow * total_ncol);
    for input in &inputs {
        for j in 0..input.ncol {
            for i in 0..max_nrow {
                // Recycle: wrap row index within the input's actual nrow
                let actual_i = i % input.nrow;
                let idx = j * input.nrow + actual_i;
                result.push(if idx < input.data.len() {
                    input.data[idx]
                } else {
                    None
                });
            }
        }
    }

    let row_names =
        if let Some(source_names) = inputs.iter().find_map(|input| input.row_names.clone()) {
            (0..max_nrow)
                .map(|idx| {
                    source_names
                        .get(idx % source_names.len())
                        .cloned()
                        .unwrap_or(None)
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };

    let has_any_col_names = inputs.iter().any(|input| input.col_names.is_some());
    let mut col_names = Vec::new();
    if has_any_col_names {
        for input in &inputs {
            if let Some(names) = &input.col_names {
                col_names.extend(
                    (0..input.ncol)
                        .map(|idx| names.get(idx).cloned().unwrap_or(None))
                        .collect::<Vec<_>>(),
                );
            } else {
                col_names.extend(std::iter::repeat_n(None, input.ncol));
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
    if let Some(dimnames) = bind_dimnames_value(row_names, col_names) {
        rv.set_attr("dimnames".to_string(), dimnames);
    }
    Ok(RValue::Vector(rv))
}

/// Get a single attribute of an object.
///
/// @param x object to inspect
/// @param which name of the attribute to retrieve
/// @return the attribute value, or NULL if not set
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

/// Set a single attribute on an object.
///
/// @param x object to modify
/// @param which name of the attribute to set
/// @param value new attribute value, or NULL to remove it
/// @return the modified object
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

/// Get all attributes of an object as a named list.
///
/// @param x object to inspect
/// @return named list of attributes, or NULL if none
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

/// Set attributes on an object in a single call.
///
/// Named arguments become attributes on the object. The special names
/// ".Names" and "names" set the names attribute.
///
/// @param .Data object to modify
/// @param ... name=value pairs to set as attributes
/// @return the modified object
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

/// Test if an object inherits from one or more classes.
///
/// Checks the object's class vector for any matches with the given class names.
///
/// @param x object to test
/// @param what character vector of class names to check
/// @return logical scalar
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

pub(crate) fn has_class(val: &RValue, class_name: &str) -> bool {
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

// `environment()` is an interpreter builtin in interp.rs (needs BuiltinContext for no-arg case)

/// Create a new environment.
///
/// @param parent parent environment (or NULL for an empty environment)
/// @return new environment
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

/// Get the name of an environment.
///
/// @param env environment to query
/// @return character scalar (empty string for anonymous environments)
#[builtin(name = "environmentName", min_args = 1)]
fn builtin_environment_name(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let name = match args.first() {
        Some(RValue::Environment(e)) => e.name().unwrap_or_default(),
        _ => String::new(),
    };
    Ok(RValue::vec(Vector::Character(vec![Some(name)].into())))
}

/// Get the parent (enclosing) environment of an environment.
///
/// @param env environment to query
/// @return the parent environment, or NULL for the empty environment
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

/// Assert that all arguments are TRUE, stopping with an error otherwise.
///
/// Checks each argument in order. If any element is FALSE or NA,
/// raises an error identifying the failing argument.
///
/// @param ... logical conditions that must all be TRUE
/// @return NULL (invisibly) if all conditions pass
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

/// Remove the class attribute from an object.
///
/// @param x object to unclass
/// @return the object with its class attribute removed
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

/// Match an argument to a set of candidate values.
///
/// Performs exact and then partial matching against the choices vector.
/// If arg is NULL, returns the first choice (the default).
///
/// @param arg character scalar to match
/// @param choices character vector of allowed values
/// @return the matched value
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
                        choices_vec.iter().map(|c| format!("'{}'", c)).join(", ")
                    ),
                )),
                _ => Err(RError::new(
                    RErrorKind::Argument,
                    format!(
                        "'arg' should be one of {}",
                        choices_vec.iter().map(|c| format!("'{}'", c)).join(", ")
                    ),
                )),
            }
        }
    }
}

/// Terminate the R session.
///
/// Also aliased as `quit`.
#[builtin(names = ["quit"])]
fn builtin_q(_args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    std::process::exit(0);
}

// === Metaprogramming builtins ===

use crate::parser::ast::Expr;

/// `formals(fn)` — return the formal parameter list of a function as a named list.
///
/// For closures, returns param names with defaults. For trait-based builtins
/// with `@param` docs, returns the parameter names. Otherwise returns NULL.
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
                        None => RValue::Null,
                    };
                    (Some(name), value)
                })
                .collect();
            Ok(RValue::List(RList::new(entries)))
        }
        Some(RValue::Function(RFunction::Builtin { name, .. })) => {
            if let Some(descriptor) = find_builtin(name) {
                let param_names = extract_param_names_from_doc(descriptor.doc);
                if !param_names.is_empty() {
                    let entries: Vec<(Option<String>, RValue)> = param_names
                        .into_iter()
                        .map(|n| (Some(n), RValue::Null))
                        .collect();
                    return Ok(RValue::List(RList::new(entries)));
                }
            }
            Ok(RValue::Null)
        }
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

// region: locale, gc, debugging stubs

/// Return locale-specific numeric formatting conventions.
///
/// Returns a named character vector with C locale defaults (miniR does not
/// support locale switching).
///
/// @return named character vector of locale conventions
#[builtin(name = "Sys.localeconv")]
fn builtin_sys_localeconv(_args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let names = vec![
        "decimal_point",
        "thousands_sep",
        "grouping",
        "int_curr_symbol",
        "currency_symbol",
        "mon_decimal_point",
        "mon_thousands_sep",
        "mon_grouping",
        "positive_sign",
        "negative_sign",
        "int_frac_digits",
        "frac_digits",
        "p_cs_precedes",
        "p_sep_by_space",
        "n_cs_precedes",
        "n_sep_by_space",
        "p_sign_posn",
        "n_sign_posn",
    ];
    let values: Vec<Option<String>> = vec![
        Some(".".to_string()),   // decimal_point
        Some(String::new()),     // thousands_sep
        Some(String::new()),     // grouping
        Some(String::new()),     // int_curr_symbol
        Some(String::new()),     // currency_symbol
        Some(String::new()),     // mon_decimal_point
        Some(String::new()),     // mon_thousands_sep
        Some(String::new()),     // mon_grouping
        Some(String::new()),     // positive_sign
        Some(String::new()),     // negative_sign
        Some("127".to_string()), // int_frac_digits (CHAR_MAX)
        Some("127".to_string()), // frac_digits
        Some("127".to_string()), // p_cs_precedes
        Some("127".to_string()), // p_sep_by_space
        Some("127".to_string()), // n_cs_precedes
        Some("127".to_string()), // n_sep_by_space
        Some("127".to_string()), // p_sign_posn
        Some("127".to_string()), // n_sign_posn
    ];
    let mut rv = RVector::from(Vector::Character(values.into()));
    rv.set_attr(
        "names".to_string(),
        RValue::vec(Vector::Character(
            names
                .into_iter()
                .map(|s| Some(s.to_string()))
                .collect::<Vec<_>>()
                .into(),
        )),
    );
    Ok(RValue::Vector(rv))
}

/// Trigger garbage collection (stub).
///
/// miniR uses Rust's ownership model for memory management, so there is no
/// garbage collector to invoke. Returns invisible NULL.
///
/// @param verbose logical; if TRUE, print GC info (ignored)
/// @param reset logical; if TRUE, reset max memory stats (ignored)
/// @param full logical; if TRUE, do a full collection (ignored)
/// @return invisible NULL
#[builtin]
fn builtin_gc(_args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    // Return a minimal named double matrix matching R's gc() output shape:
    // 2 rows (Ncells, Vcells) x 6 cols (used, gc trigger, max used, ...)
    // For simplicity, return zeros.
    Ok(RValue::Null)
}

/// Set or query verbose GC reporting (stub).
///
/// miniR does not have a garbage collector. Always returns FALSE.
///
/// @param verbose logical (ignored)
/// @return previous setting (always FALSE)
#[builtin]
fn builtin_gcinfo(_args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    Ok(RValue::vec(Vector::Logical(vec![Some(false)].into())))
}

/// Print the call stack of the last error (stub).
///
/// miniR does not yet maintain a full traceback. Returns invisible NULL.
///
/// @param x number of calls to display (ignored)
/// @return invisible NULL
#[builtin(names = ["traceBack"])]
fn builtin_traceback(_args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    Ok(RValue::Null)
}

/// Set a function for debug-mode single-stepping (stub).
///
/// miniR does not support interactive debugging. Prints a message.
///
/// @param fun function to debug
/// @return invisible NULL
#[builtin]
fn builtin_debug(_args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    eprintln!("debug() is not supported in miniR — interactive debugging is not available.");
    Ok(RValue::Null)
}

/// Remove debug-mode single-stepping from a function (stub).
///
/// @param fun function to undebug
/// @return invisible NULL
#[builtin]
fn builtin_undebug(_args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    Ok(RValue::Null)
}

/// Query whether a function has debug-mode enabled (stub).
///
/// @param fun function to query
/// @return always FALSE
#[builtin(name = "isdebugged")]
fn builtin_isdebugged(_args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    Ok(RValue::vec(Vector::Logical(vec![Some(false)].into())))
}

/// Enter the browser for interactive debugging (stub).
///
/// miniR does not support interactive debugging. Prints a message.
///
/// @return invisible NULL
#[builtin]
fn builtin_browser(_args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    eprintln!("browser() is not supported in miniR — interactive debugging is not available.");
    Ok(RValue::Null)
}

// endregion

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
