mod args;
mod coercion;
#[cfg(feature = "collections")]
pub mod collections;
mod conditions;
pub mod connections;
mod dataframes;
#[cfg(feature = "datetime")]
mod datetime;
#[cfg(any(feature = "digest", feature = "blake3"))]
mod digest;
mod factors;
mod graphics;
mod interp;
#[cfg(feature = "io")]
pub mod io;
#[cfg(feature = "json")]
mod json;
pub mod math;
#[cfg(feature = "tls")]
mod net;
mod pre_eval;
#[cfg(feature = "progress")]
pub mod progress;
#[cfg(feature = "random")]
mod random;
mod s4;
pub mod serialize;
mod stats;
pub mod strings;
mod stubs;
pub mod system;
mod tables;
#[cfg(feature = "tables")]
mod tables_display;
#[cfg(feature = "toml")]
mod toml;
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

/// Look up a builtin by namespace::name (e.g. "stats::cor").
pub fn find_builtin_ns(namespace: &str, name: &str) -> Option<&'static BuiltinDescriptor> {
    BUILTIN_REGISTRY
        .iter()
        .find(|d| d.namespace == namespace && (d.name == name || d.aliases.contains(&name)))
}

/// Format a builtin's doc string for display.
/// Convention: first line = title, rest = description/params.
pub fn format_help(descriptor: &BuiltinDescriptor) -> String {
    let mut out = String::new();
    let header = format!("{}::{}", descriptor.namespace, descriptor.name);
    out.push_str(&format!("{header}\n"));
    out.push_str(&"─".repeat(header.len().max(20)));
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

    // Primitive operators as callable functions (e.g. lapply(x, `[[`, "name"))
    register_operator_builtins(env);
}

/// Register R's primitive operators as first-class callable functions.
///
/// In R, operators like `[[`, `[`, `+`, `-` etc. can be passed as function
/// arguments using backtick-quoting: `lapply(x, \`[[\`, "name")`.
fn register_operator_builtins(env: &Environment) {
    use crate::interpreter::value::*;

    // `[[` — extract single element
    env.set(
        "[[".to_string(),
        RValue::Function(RFunction::Builtin {
            name: "[[".to_string(),
            implementation: BuiltinImplementation::Eager(|args, _| {
                if args.len() < 2 {
                    return Err(RError::new(
                        RErrorKind::Argument,
                        "`[[` requires 2 arguments".to_string(),
                    ));
                }
                let obj = &args[0];
                let idx = &args[1];
                match obj {
                    RValue::List(list) => match idx {
                        RValue::Vector(rv) if matches!(rv.inner, Vector::Character(_)) => {
                            let name = rv.inner.as_character_scalar().unwrap_or_default();
                            for (n, v) in &list.values {
                                if n.as_deref() == Some(&name) {
                                    return Ok(v.clone());
                                }
                            }
                            Ok(RValue::Null)
                        }
                        RValue::Vector(v) => {
                            let i = v.as_integer_scalar().unwrap_or(0) as usize;
                            if i > 0 && i <= list.values.len() {
                                Ok(list.values[i - 1].1.clone())
                            } else {
                                Ok(RValue::Null)
                            }
                        }
                        _ => Ok(RValue::Null),
                    },
                    RValue::Vector(v) => match idx {
                        RValue::Vector(idx_rv)
                            if matches!(idx_rv.inner, Vector::Character(_)) =>
                        {
                            let name = idx_rv.inner.as_character_scalar().unwrap_or_default();
                            if let Some(names_attr) = v.get_attr("names") {
                                if let Some(names_vec) = names_attr.as_vector() {
                                    let name_strs = names_vec.to_characters();
                                    for (j, n) in name_strs.iter().enumerate() {
                                        if n.as_deref() == Some(name.as_str()) && j < v.len() {
                                            return Ok(
                                                crate::interpreter::indexing::extract_vector_element(
                                                    v, j,
                                                ),
                                            );
                                        }
                                    }
                                }
                            }
                            Ok(RValue::Null)
                        }
                        RValue::Vector(idx_rv) => {
                            let i = idx_rv.as_integer_scalar().unwrap_or(0) as usize;
                            if i > 0 && i <= v.len() {
                                Ok(crate::interpreter::indexing::extract_vector_element(v, i - 1))
                            } else {
                                Ok(RValue::Null)
                            }
                        }
                        _ => Ok(RValue::Null),
                    },
                    _ => Err(RError::other("object of type 'closure' is not subsettable".to_string())),
                }
            }),
            min_args: 2,
            max_args: None,
        }),
    );

    // `[` — subset
    env.set(
        "[".to_string(),
        RValue::Function(RFunction::Builtin {
            name: "[".to_string(),
            implementation: BuiltinImplementation::Eager(|args, _| {
                if args.is_empty() {
                    return Err(RError::new(
                        RErrorKind::Argument,
                        "`[` requires at least 1 argument".to_string(),
                    ));
                }
                if args.len() == 1 {
                    return Ok(args[0].clone());
                }
                // Delegate to the indexing machinery via a simplified path
                // For now, support the common case: vector[index]
                match (&args[0], &args[1]) {
                    (RValue::Vector(v), RValue::Vector(idx)) => {
                        let i = idx.as_integer_scalar().unwrap_or(0) as usize;
                        if i > 0 && i <= v.len() {
                            Ok(crate::interpreter::indexing::extract_vector_element(
                                v,
                                i - 1,
                            ))
                        } else {
                            Ok(RValue::Null)
                        }
                    }
                    (RValue::List(list), RValue::Vector(idx)) => {
                        let i = idx.as_integer_scalar().unwrap_or(0) as usize;
                        if i > 0 && i <= list.values.len() {
                            let (name, val) = &list.values[i - 1];
                            Ok(RValue::List(RList::new(vec![(name.clone(), val.clone())])))
                        } else {
                            Ok(RValue::Null)
                        }
                    }
                    _ => Ok(RValue::Null),
                }
            }),
            min_args: 1,
            max_args: None,
        }),
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
#[interpreter_builtin(min_args = 1)]
fn interp_help(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let name = match args.first() {
        Some(RValue::Vector(rv)) => rv.as_character_scalar().unwrap_or_default(),
        Some(RValue::Function(RFunction::Builtin { name, .. })) => name.clone(),
        _ => String::new(),
    };
    if name.is_empty() {
        return Ok(RValue::Null);
    }

    // Check if it's a namespace name (e.g. ?base, ?stats, ?utils)
    if BUILTIN_REGISTRY.iter().any(|d| d.namespace == name) {
        let mut fns: Vec<&str> = BUILTIN_REGISTRY
            .iter()
            .filter(|d| d.namespace == name)
            .map(|d| d.name)
            .collect();
        fns.sort();
        fns.dedup();
        println!("Package '{name}'");
        println!("{}", "─".repeat(20 + name.len()));
        println!();
        println!("{} functions:", fns.len());
        println!();
        let max_width = fns.iter().map(|f| f.len()).max().unwrap_or(10) + 2;
        let cols = 80 / max_width.max(1);
        for (i, f) in fns.iter().enumerate() {
            print!("{:<width$}", f, width = max_width);
            if (i + 1) % cols == 0 {
                println!();
            }
        }
        if !fns.len().is_multiple_of(cols) {
            println!();
        }
        println!();
        println!("Use ?{name}::name for help on a specific function.");
        return Ok(RValue::Null);
    }

    // 1. Check Rd help index (package man/ pages) first
    let rd_result = {
        let index = context.interpreter().rd_help_index.borrow();
        if let Some((ns, n)) = name.split_once("::") {
            index.lookup_in_package(n, ns).map(|e| e.doc.format_text())
        } else {
            index.lookup(&name).first().map(|e| e.doc.format_text())
        }
    };
    if let Some(text) = rd_result {
        context.write(&text);
        context.write("\n");
        return Ok(RValue::Null);
    }

    // 2. Fall back to builtin registry (rustdoc-based help)
    let descriptor = if let Some((ns, n)) = name.split_once("::") {
        find_builtin_ns(ns, n)
    } else {
        find_builtin(&name)
    };
    match descriptor {
        Some(d) => {
            context.write(&format!("{}\n", format_help(d)));
            Ok(RValue::Null)
        }
        None => {
            context.write(&format!("No documentation for '{name}'\n"));
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
#[interpreter_builtin(name = "cat")]
fn builtin_cat(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
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
            let path = context.interpreter().resolve_path(path);
            let mut f = OpenOptions::new()
                .write(true)
                .create(true)
                .append(append)
                .truncate(!append)
                .open(&path)
                .map_err(|e| {
                    RError::other(format!("cannot open file '{}': {}", path.display(), e))
                })?;
            f.write_all(output.as_bytes()).map_err(|e| {
                RError::other(format!("error writing to '{}': {}", path.display(), e))
            })?;
        }
        _ => {
            context.write(&output);
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
/// @param type one of "bytes", "chars", "width", or "graphemes" (default "chars")
/// @return integer vector of string lengths
#[builtin(min_args = 1)]
fn builtin_nchar(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    use unicode_segmentation::UnicodeSegmentation;

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
                            "graphemes" => s.graphemes(true).count(),
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
                rv.attrs.as_mut().map(|a| a.shift_remove("names"));
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
        list.attrs.as_mut().map(|attrs| attrs.shift_remove("names"));
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
    list.attrs
        .as_mut()
        .map(|attrs| attrs.shift_remove("dimnames"));
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
                rv.attrs
                    .as_mut()
                    .map(|attrs| attrs.shift_remove("dimnames"));
            } else {
                rv.set_attr("dimnames".to_string(), dimnames);
            }
            Ok(RValue::Vector(rv))
        }
        Some(RValue::List(list)) => {
            let mut list = list.clone();
            let dimnames = updated_dimnames_component(list.get_attr("dimnames"), 0, &row_names);
            if dimnames.is_null() {
                list.attrs
                    .as_mut()
                    .map(|attrs| attrs.shift_remove("dimnames"));
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
                rv.attrs
                    .as_mut()
                    .map(|attrs| attrs.shift_remove("dimnames"));
            } else {
                rv.set_attr("dimnames".to_string(), dimnames);
            }
            Ok(RValue::Vector(rv))
        }
        Some(RValue::List(list)) => {
            let mut list = list.clone();
            let dimnames = updated_dimnames_component(list.get_attr("dimnames"), 1, &col_names);
            if dimnames.is_null() {
                list.attrs
                    .as_mut()
                    .map(|attrs| attrs.shift_remove("dimnames"));
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
                rv.attrs.as_mut().map(|a| a.shift_remove("class"));
            } else {
                rv.set_attr("class".to_string(), class_val);
            }
            Ok(RValue::Vector(rv))
        }
        Some(RValue::List(l)) => {
            let mut l = l.clone();
            if class_val.is_null() {
                l.attrs.as_mut().map(|a| a.shift_remove("class"));
            } else {
                l.set_attr("class".to_string(), class_val);
            }
            Ok(RValue::List(l))
        }
        Some(RValue::Language(lang)) => {
            let mut lang = lang.clone();
            if class_val.is_null() {
                lang.attrs.as_mut().map(|a| a.shift_remove("class"));
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
/// For data.frames, shows a structured view with column types aligned
/// using the `tabled` crate.
///
/// @param x object to inspect
/// @return NULL (invisibly)
#[interpreter_builtin(min_args = 1)]
fn interp_str(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    match args.first() {
        Some(val) => {
            // Check for data.frame first — use tabled-based structured display
            #[cfg(feature = "tables")]
            if let Some(output) = tables_display::str_data_frame(val) {
                context.write(&output);
                return Ok(RValue::Null);
            }

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
                    context.write(&format!(" {} [1:{}] {}\n", type_name, len, preview));
                }
                RValue::List(l) => {
                    context.write(&format!("List of {}\n", l.values.len()));
                    for (i, (name, elem)) in l.values.iter().enumerate() {
                        let label = name.clone().unwrap_or_else(|| format!("[[{}]]", i + 1));
                        let child = str_format_element(elem, 1);
                        context.write(&format!(" $ {:<13}:{}\n", label, child));
                    }
                }
                RValue::Null => context.write(" NULL\n"),
                _ => context.write(&format!(" {}\n", val)),
            }
            Ok(RValue::Null)
        }
        None => Ok(RValue::Null),
    }
}

/// Format one element for str() output (used for list elements).
/// Returns a one-line summary: " type [1:len] preview" for vectors,
/// "List of N" for nested lists, etc.
fn str_format_element(val: &RValue, indent: usize) -> String {
    let prefix = " ".repeat(indent);
    match val {
        RValue::Null => format!("{prefix} NULL"),
        RValue::Vector(v) => {
            let len = v.len();
            let type_name = v.type_name();
            let preview = str_vector_preview(v);
            if len == 1 {
                format!("{prefix} {type_name} {preview}")
            } else {
                let dots = if len > 10 { " ..." } else { "" };
                format!("{prefix} {type_name} [1:{len}] {preview}{dots}")
            }
        }
        RValue::List(l) => {
            let mut out = format!("{prefix}List of {}", l.values.len());
            for (i, (name, elem)) in l.values.iter().enumerate() {
                let label = name.clone().unwrap_or_else(|| format!("[[{}]]", i + 1));
                let child = str_format_element(elem, indent + 1);
                out.push_str(&format!("\n{prefix} $ {label:<13}:{child}"));
            }
            out
        }
        RValue::Function(_) => format!("{prefix}function (...)"),
        RValue::Environment(_) => format!("{prefix}<environment>"),
        RValue::Language(_) => format!("{prefix} language ..."),
    }
}

/// Format a vector element preview for str() output (first 10 elements).
fn str_vector_preview(v: &RVector) -> String {
    let len = v.inner.len().min(10);
    let elems: Vec<String> = (0..len)
        .map(|i| match &v.inner {
            Vector::Raw(vals) => format!("{:02x}", vals[i]),
            Vector::Logical(vals) => match vals[i] {
                Some(true) => "TRUE".to_string(),
                Some(false) => "FALSE".to_string(),
                None => "NA".to_string(),
            },
            Vector::Integer(vals) => match vals[i] {
                Some(n) => n.to_string(),
                None => "NA".to_string(),
            },
            Vector::Double(vals) => match vals[i] {
                Some(f) => format_r_double(f),
                None => "NA".to_string(),
            },
            Vector::Complex(vals) => match vals[i] {
                Some(c) => format_r_complex(c),
                None => "NA".to_string(),
            },
            Vector::Character(vals) => match &vals[i] {
                Some(s) => format!("\"{}\"", s),
                None => "NA".to_string(),
            },
        })
        .collect();
    elems.join(" ")
}

/// Test if two objects are exactly identical.
///
/// Performs deep structural comparison: type, length, element values,
/// and attributes must all match. NaN == NaN is TRUE (unlike `==`).
/// NA == NA is TRUE. Lists are compared recursively.
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
    let result = r_identical(&args[0], &args[1]);
    Ok(RValue::vec(Vector::Logical(vec![Some(result)].into())))
}

/// Deep structural comparison of two R values.
///
/// In R, `identical()` treats NaN == NaN as TRUE and NA == NA as TRUE,
/// unlike the `==` operator. Attributes must also match.
fn r_identical(a: &RValue, b: &RValue) -> bool {
    match (a, b) {
        (RValue::Null, RValue::Null) => true,
        (RValue::Vector(va), RValue::Vector(vb)) => {
            vectors_identical(&va.inner, &vb.inner) && attrs_identical(&va.attrs, &vb.attrs)
        }
        (RValue::List(la), RValue::List(lb)) => {
            if la.values.len() != lb.values.len() {
                return false;
            }
            for ((na, va), (nb, vb)) in la.values.iter().zip(lb.values.iter()) {
                if na != nb {
                    return false;
                }
                if !r_identical(va, vb) {
                    return false;
                }
            }
            attrs_identical(&la.attrs, &lb.attrs)
        }
        (RValue::Function(fa), RValue::Function(fb)) => match (fa, fb) {
            (RFunction::Builtin { name: na, .. }, RFunction::Builtin { name: nb, .. }) => na == nb,
            (
                RFunction::Closure {
                    params: pa,
                    body: ba,
                    ..
                },
                RFunction::Closure {
                    params: pb,
                    body: bb,
                    ..
                },
            ) => {
                format!("{:?}", pa) == format!("{:?}", pb)
                    && format!("{:?}", ba) == format!("{:?}", bb)
            }
            _ => false,
        },
        (RValue::Environment(ea), RValue::Environment(eb)) => {
            // Environments are identical only if they are the same Rc (pointer equality)
            ea.ptr_eq(eb)
        }
        (RValue::Language(la), RValue::Language(lb)) => {
            format!("{:?}", la.inner) == format!("{:?}", lb.inner)
                && attrs_identical(&la.attrs, &lb.attrs)
        }
        _ => false,
    }
}

/// Compare two attribute maps for identical-ness.
fn attrs_identical(a: &Option<Box<Attributes>>, b: &Option<Box<Attributes>>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(aa), Some(bb)) => {
            if aa.len() != bb.len() {
                return false;
            }
            for (key, val_a) in aa.iter() {
                match bb.get(key) {
                    Some(val_b) => {
                        if !r_identical(val_a, val_b) {
                            return false;
                        }
                    }
                    None => return false,
                }
            }
            true
        }
        _ => false,
    }
}

/// Element-wise comparison of two atomic vectors.
///
/// Both vectors must have the same type and length, and every element must
/// be bitwise-identical (NaN == NaN is TRUE, NA == NA is TRUE).
fn vectors_identical(a: &Vector, b: &Vector) -> bool {
    match (a, b) {
        (Vector::Logical(va), Vector::Logical(vb)) => va.0 == vb.0,
        (Vector::Integer(va), Vector::Integer(vb)) => va.0 == vb.0,
        (Vector::Character(va), Vector::Character(vb)) => va.0 == vb.0,
        (Vector::Raw(va), Vector::Raw(vb)) => va == vb,
        (Vector::Double(va), Vector::Double(vb)) => {
            if va.len() != vb.len() {
                return false;
            }
            va.iter().zip(vb.iter()).all(|(x, y)| match (x, y) {
                (None, None) => true,
                (Some(fx), Some(fy)) => fx.to_bits() == fy.to_bits(),
                _ => false,
            })
        }
        (Vector::Complex(va), Vector::Complex(vb)) => {
            if va.len() != vb.len() {
                return false;
            }
            va.iter().zip(vb.iter()).all(|(x, y)| match (x, y) {
                (None, None) => true,
                (Some(cx), Some(cy)) => {
                    cx.re.to_bits() == cy.re.to_bits() && cx.im.to_bits() == cy.im.to_bits()
                }
                _ => false,
            })
        }
        _ => false, // different types are never identical
    }
}

/// Test near-equality of two objects within a tolerance.
///
/// Returns TRUE if the objects are nearly equal, or a character vector of
/// strings describing the differences. Matches R's `all.equal()` semantics:
/// numeric comparison uses mean relative/absolute difference with a
/// configurable tolerance.
///
/// @param target first object
/// @param current second object
/// @param tolerance maximum allowed difference (default: 1.5e-8)
/// @param check.attributes if TRUE, also compare attributes
/// @param check.names if TRUE, compare names attributes
/// @return TRUE if equal, or character string(s) describing the difference(s)
#[builtin(min_args = 2)]
fn builtin_all_equal(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let tolerance = named
        .iter()
        .find(|(n, _)| n == "tolerance")
        .and_then(|(_, v)| v.as_vector()?.as_double_scalar())
        .unwrap_or(1.5e-8);

    let check_attributes = named
        .iter()
        .find(|(n, _)| n == "check.attributes")
        .and_then(|(_, v)| v.as_vector()?.as_logical_scalar())
        .unwrap_or(true);

    let check_names = named
        .iter()
        .find(|(n, _)| n == "check.names")
        .and_then(|(_, v)| v.as_vector()?.as_logical_scalar())
        .unwrap_or(true);

    if args.len() < 2 {
        return Err(RError::new(
            RErrorKind::Argument,
            "need 2 arguments".to_string(),
        ));
    }

    let mut diffs = Vec::new();
    all_equal_recurse(
        &args[0],
        &args[1],
        tolerance,
        check_attributes,
        check_names,
        "",
        &mut diffs,
    );

    if diffs.is_empty() {
        Ok(RValue::vec(Vector::Logical(vec![Some(true)].into())))
    } else {
        let msgs: Vec<Option<String>> = diffs.into_iter().map(Some).collect();
        Ok(RValue::vec(Vector::Character(msgs.into())))
    }
}

/// Recursively compare two R values, collecting difference messages.
fn all_equal_recurse(
    target: &RValue,
    current: &RValue,
    tolerance: f64,
    check_attributes: bool,
    check_names: bool,
    prefix: &str,
    diffs: &mut Vec<String>,
) {
    match (target, current) {
        // NULL == NULL
        (RValue::Null, RValue::Null) => {}

        // Type mismatch involving NULL
        (RValue::Null, _) | (_, RValue::Null) => {
            diffs.push(format!(
                "{}target is {}, current is {}",
                prefix,
                target.type_name(),
                current.type_name()
            ));
        }

        // Both vectors
        (RValue::Vector(v1), RValue::Vector(v2)) => {
            all_equal_vectors(
                v1,
                v2,
                tolerance,
                check_attributes,
                check_names,
                prefix,
                diffs,
            );
        }

        // Both lists
        (RValue::List(l1), RValue::List(l2)) => {
            all_equal_lists(
                l1,
                l2,
                tolerance,
                check_attributes,
                check_names,
                prefix,
                diffs,
            );
        }

        // Mismatched types
        _ => {
            diffs.push(format!(
                "{}target is {}, current is {}",
                prefix,
                target.type_name(),
                current.type_name()
            ));
        }
    }
}

/// Compare two vectors, collecting difference messages.
fn all_equal_vectors(
    v1: &RVector,
    v2: &RVector,
    tolerance: f64,
    check_attributes: bool,
    check_names: bool,
    prefix: &str,
    diffs: &mut Vec<String>,
) {
    let t1 = v1.inner.type_name();
    let t2 = v2.inner.type_name();

    // Numeric types (double, integer, logical) can be compared numerically
    let is_numeric = |t: &str| matches!(t, "double" | "integer" | "logical");

    if t1 == "character" && t2 == "character" {
        all_equal_character(v1, v2, prefix, diffs);
    } else if is_numeric(t1) && is_numeric(t2) {
        all_equal_numeric(v1, v2, tolerance, prefix, diffs);
    } else if t1 != t2 {
        diffs.push(format!("{}target is {}, current is {}", prefix, t1, t2));
        return;
    } else {
        // Same non-numeric, non-character type (complex, raw) — coerce to doubles
        all_equal_numeric(v1, v2, tolerance, prefix, diffs);
    }

    // Check attributes if requested
    if check_attributes {
        all_equal_attrs(
            v1.attrs.as_deref(),
            v2.attrs.as_deref(),
            check_names,
            prefix,
            diffs,
        );
    } else if check_names {
        // Even with check.attributes=FALSE, check.names=TRUE checks names
        all_equal_names_attr(v1.attrs.as_deref(), v2.attrs.as_deref(), prefix, diffs);
    }
}

/// Compare numeric vectors using R's mean relative/absolute difference.
fn all_equal_numeric(
    v1: &RVector,
    v2: &RVector,
    tolerance: f64,
    prefix: &str,
    diffs: &mut Vec<String>,
) {
    let d1 = v1.to_doubles();
    let d2 = v2.to_doubles();

    if d1.len() != d2.len() {
        diffs.push(format!(
            "{}Lengths ({}, {}) differ",
            prefix,
            d1.len(),
            d2.len()
        ));
        return;
    }

    if d1.is_empty() {
        return;
    }

    // Compute mean absolute difference and mean absolute target (R's algorithm)
    let mut sum_abs_diff = 0.0;
    let mut sum_abs_target = 0.0;
    let mut count = 0usize;

    for (a, b) in d1.iter().zip(d2.iter()) {
        match (a, b) {
            (Some(a), Some(b)) => {
                sum_abs_diff += (a - b).abs();
                sum_abs_target += a.abs();
                count += 1;
            }
            (None, None) => {} // both NA — equal
            _ => {
                // One is NA, the other is not
                count += 1;
                sum_abs_diff += f64::INFINITY;
            }
        }
    }

    if count == 0 {
        return;
    }

    let mean_abs_diff = sum_abs_diff / count as f64;
    let mean_abs_target = sum_abs_target / count as f64;

    // R uses relative difference when mean(abs(target)) > tolerance,
    // otherwise absolute difference
    if mean_abs_target.is_finite() && mean_abs_target > tolerance {
        let relative_diff = mean_abs_diff / mean_abs_target;
        if relative_diff > tolerance {
            diffs.push(format!(
                "{}Mean relative difference: {}",
                prefix, relative_diff
            ));
        }
    } else if mean_abs_diff > tolerance {
        diffs.push(format!(
            "{}Mean absolute difference: {}",
            prefix, mean_abs_diff
        ));
    }
}

/// Compare character vectors element-wise.
fn all_equal_character(v1: &RVector, v2: &RVector, prefix: &str, diffs: &mut Vec<String>) {
    let c1 = v1.to_characters();
    let c2 = v2.to_characters();

    if c1.len() != c2.len() {
        diffs.push(format!(
            "{}Lengths ({}, {}) differ",
            prefix,
            c1.len(),
            c2.len()
        ));
        return;
    }

    let mut mismatches = 0usize;
    for (a, b) in c1.iter().zip(c2.iter()) {
        if a != b {
            mismatches += 1;
        }
    }

    if mismatches > 0 {
        diffs.push(format!(
            "{}{} string mismatch{}",
            prefix,
            mismatches,
            if mismatches == 1 { "" } else { "es" }
        ));
    }
}

/// Compare two lists recursively.
fn all_equal_lists(
    l1: &RList,
    l2: &RList,
    tolerance: f64,
    check_attributes: bool,
    check_names: bool,
    prefix: &str,
    diffs: &mut Vec<String>,
) {
    if l1.values.len() != l2.values.len() {
        diffs.push(format!(
            "{}Lengths ({}, {}) differ",
            prefix,
            l1.values.len(),
            l2.values.len()
        ));
        return;
    }

    // Check names if requested
    if check_names {
        let names1: Vec<Option<&str>> = l1.values.iter().map(|(n, _)| n.as_deref()).collect();
        let names2: Vec<Option<&str>> = l2.values.iter().map(|(n, _)| n.as_deref()).collect();
        if names1 != names2 {
            diffs.push(format!("{}Component names differ", prefix));
        }
    }

    // Recursively compare elements
    for (i, ((_, v1), (name, v2))) in l1.values.iter().zip(l2.values.iter()).enumerate() {
        let elem_prefix = match name {
            Some(n) => format!("Component \"{}\": ", n),
            None => format!("Component {}: ", i + 1),
        };
        all_equal_recurse(
            v1,
            v2,
            tolerance,
            check_attributes,
            check_names,
            &elem_prefix,
            diffs,
        );
    }

    // Check list-level attributes
    if check_attributes {
        all_equal_attrs(
            l1.attrs.as_deref(),
            l2.attrs.as_deref(),
            check_names,
            prefix,
            diffs,
        );
    }
}

/// Compare attributes of two objects.
fn all_equal_attrs(
    attrs1: Option<&Attributes>,
    attrs2: Option<&Attributes>,
    check_names: bool,
    prefix: &str,
    diffs: &mut Vec<String>,
) {
    let empty = indexmap::IndexMap::new();
    let a1 = attrs1.unwrap_or(&empty);
    let a2 = attrs2.unwrap_or(&empty);

    let mut keys1: Vec<&str> = a1.keys().map(|s| s.as_str()).collect();
    let mut keys2: Vec<&str> = a2.keys().map(|s| s.as_str()).collect();

    if !check_names {
        keys1.retain(|k| *k != "names");
        keys2.retain(|k| *k != "names");
    }

    keys1.sort();
    keys2.sort();

    if keys1 != keys2 {
        diffs.push(format!("{}Attributes differ", prefix));
        return;
    }

    for key in &keys1 {
        if let (Some(v1), Some(v2)) = (a1.get(*key), a2.get(*key)) {
            let attr_prefix = format!("{}Attributes: <{}> - ", prefix, key);
            all_equal_recurse(v1, v2, 1.5e-8, true, true, &attr_prefix, diffs);
        }
    }
}

/// Check only the "names" attribute.
fn all_equal_names_attr(
    attrs1: Option<&Attributes>,
    attrs2: Option<&Attributes>,
    prefix: &str,
    diffs: &mut Vec<String>,
) {
    let n1 = attrs1.and_then(|a| a.get("names"));
    let n2 = attrs2.and_then(|a| a.get("names"));

    match (n1, n2) {
        (None, None) => {}
        (Some(_), None) | (None, Some(_)) => {
            diffs.push(format!("{}Names differ", prefix));
        }
        (Some(v1), Some(v2)) => {
            let attr_prefix = format!("{}Names: ", prefix);
            all_equal_recurse(v1, v2, 1.5e-8, false, false, &attr_prefix, diffs);
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

/// Vectorized exclusive OR.
///
/// Computes element-wise XOR of two logical vectors, recycling the shorter.
/// Returns NA where either input is NA.
///
/// @param x first logical vector
/// @param y second logical vector
/// @return logical vector
#[builtin(min_args = 2)]
fn builtin_xor(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    if args.len() < 2 {
        return Err(RError::new(
            RErrorKind::Argument,
            "need 2 arguments".to_string(),
        ));
    }
    let a_vec = match &args[0] {
        RValue::Vector(v) => v.to_logicals(),
        _ => {
            return Err(RError::new(
                RErrorKind::Argument,
                "argument 'x' must be coercible to logical".to_string(),
            ))
        }
    };
    let b_vec = match &args[1] {
        RValue::Vector(v) => v.to_logicals(),
        _ => {
            return Err(RError::new(
                RErrorKind::Argument,
                "argument 'y' must be coercible to logical".to_string(),
            ))
        }
    };
    let len = a_vec.len().max(b_vec.len());
    let result: Vec<Option<bool>> = (0..len)
        .map(|i| {
            let a = a_vec[i % a_vec.len()];
            let b = b_vec[i % b_vec.len()];
            match (a, b) {
                (Some(a), Some(b)) => Some(a ^ b),
                _ => None,
            }
        })
        .collect();
    Ok(RValue::vec(Vector::Logical(result.into())))
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

/// Create a data frame from all combinations of the supplied vectors.
///
/// Each argument is a vector of values; the result is a data frame with one
/// row for every combination. The first factor varies fastest, matching R's
/// `expand.grid()` semantics.
///
/// Uses `itertools::multi_cartesian_product` internally (with reversed input
/// order so the first argument cycles fastest).
///
/// @param ... vectors whose Cartesian product forms the rows
/// @return data.frame with `prod(lengths)` rows
#[builtin(name = "expand.grid")]
fn builtin_expand_grid(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    // Collect all input vectors with their names
    let mut inputs: Vec<(String, Vec<RValue>)> = Vec::new();
    let mut unnamed_idx = 0usize;

    // Positional args first, then named — preserving call order
    // (R interleaves positional and named in the order they appear,
    // but the builtin dispatch separates them. We number unnamed ones
    // sequentially as Var1, Var2, … and named ones keep their name.)
    for arg in args {
        unnamed_idx += 1;
        let name = format!("Var{unnamed_idx}");
        let items = vector_to_items(arg);
        if items.is_empty() {
            return Ok(empty_expand_grid());
        }
        inputs.push((name, items));
    }
    for (name, val) in named {
        if name == "stringsAsFactors" || name == "KEEP.OUT.ATTRS" {
            continue; // control args — skip
        }
        let items = vector_to_items(val);
        if items.is_empty() {
            return Ok(empty_expand_grid());
        }
        inputs.push((name.clone(), items));
    }

    if inputs.is_empty() {
        return Ok(empty_expand_grid());
    }

    // Total number of rows = product of all vector lengths
    let nrow: usize = inputs.iter().map(|(_, v)| v.len()).product();

    // Build columns using modular arithmetic.
    // The first input varies fastest: column k repeats each element
    // `repeat_each` times and the whole sequence `repeat_whole` times.
    //
    //   repeat_each  = product of lengths of inputs 0..(k-1)
    //   repeat_whole = nrow / (len_k * repeat_each)
    let mut columns: Vec<(Option<String>, RValue)> = Vec::with_capacity(inputs.len());
    let mut repeat_each: usize = 1;

    for (col_name, items) in &inputs {
        let len_k = items.len();
        let mut col_values: Vec<RValue> = Vec::with_capacity(nrow);

        // Pattern: repeat the whole sequence (nrow / (len_k * repeat_each)) times,
        // and within each cycle, repeat each element `repeat_each` times.
        let cycle_len = len_k * repeat_each;
        let n_cycles = nrow / cycle_len;

        for _ in 0..n_cycles {
            for item in items {
                for _ in 0..repeat_each {
                    col_values.push(item.clone());
                }
            }
        }

        // Combine the column items into a typed vector
        let col_vec = combine_expand_grid_column(&col_values);
        columns.push((Some(col_name.clone()), col_vec));

        repeat_each *= len_k;
    }

    // Build the data frame
    let col_names: Vec<Option<String>> = columns.iter().map(|(n, _)| n.clone()).collect();
    let mut result = RList::new(columns);
    result.set_attr(
        "class".to_string(),
        RValue::vec(Vector::Character(
            vec![Some("data.frame".to_string())].into(),
        )),
    );
    result.set_attr(
        "names".to_string(),
        RValue::vec(Vector::Character(col_names.into())),
    );
    let row_names: Vec<Option<i64>> = (1..=i64::try_from(nrow)?).map(Some).collect();
    result.set_attr(
        "row.names".to_string(),
        RValue::vec(Vector::Integer(row_names.into())),
    );

    Ok(RValue::List(result))
}

/// Return an empty data.frame for expand.grid with zero-length inputs.
fn empty_expand_grid() -> RValue {
    let mut result = RList::new(Vec::new());
    result.set_attr(
        "class".to_string(),
        RValue::vec(Vector::Character(
            vec![Some("data.frame".to_string())].into(),
        )),
    );
    result.set_attr(
        "names".to_string(),
        RValue::vec(Vector::Character(Vec::<Option<String>>::new().into())),
    );
    result.set_attr(
        "row.names".to_string(),
        RValue::vec(Vector::Integer(Vec::<Option<i64>>::new().into())),
    );
    RValue::List(result)
}

/// Extract scalar items from an RValue for expand.grid column construction.
fn vector_to_items(x: &RValue) -> Vec<RValue> {
    match x {
        RValue::Vector(rv) => match &rv.inner {
            Vector::Double(vals) => vals
                .iter()
                .map(|v| RValue::vec(Vector::Double(vec![*v].into())))
                .collect(),
            Vector::Integer(vals) => vals
                .iter()
                .map(|v| RValue::vec(Vector::Integer(vec![*v].into())))
                .collect(),
            Vector::Logical(vals) => vals
                .iter()
                .map(|v| RValue::vec(Vector::Logical(vec![*v].into())))
                .collect(),
            Vector::Character(vals) => vals
                .iter()
                .map(|v| RValue::vec(Vector::Character(vec![v.clone()].into())))
                .collect(),
            Vector::Complex(vals) => vals
                .iter()
                .map(|v| RValue::vec(Vector::Complex(vec![*v].into())))
                .collect(),
            Vector::Raw(vals) => vals
                .iter()
                .map(|v| RValue::vec(Vector::Raw(vec![*v])))
                .collect(),
        },
        RValue::List(l) => l.values.iter().map(|(_, v)| v.clone()).collect(),
        RValue::Null => Vec::new(),
        other => vec![other.clone()],
    }
}

/// Combine a column of scalar RValues back into a single typed vector.
fn combine_expand_grid_column(items: &[RValue]) -> RValue {
    if items.is_empty() {
        return RValue::Null;
    }

    // Determine the type from the first element
    let first_type = items[0].type_name();
    let all_same = items.iter().all(|i| i.type_name() == first_type);

    if all_same {
        match first_type {
            "double" => {
                let vals: Vec<Option<f64>> = items
                    .iter()
                    .map(|i| {
                        i.as_vector()
                            .and_then(|v| v.to_doubles().into_iter().next())
                            .flatten()
                    })
                    .collect();
                RValue::vec(Vector::Double(vals.into()))
            }
            "integer" => {
                let vals: Vec<Option<i64>> = items
                    .iter()
                    .map(|i| {
                        i.as_vector()
                            .and_then(|v| v.to_integers().into_iter().next())
                            .flatten()
                    })
                    .collect();
                RValue::vec(Vector::Integer(vals.into()))
            }
            "logical" => {
                let vals: Vec<Option<bool>> = items
                    .iter()
                    .map(|i| {
                        i.as_vector()
                            .and_then(|v| v.to_logicals().into_iter().next())
                            .flatten()
                    })
                    .collect();
                RValue::vec(Vector::Logical(vals.into()))
            }
            "character" => {
                let vals: Vec<Option<String>> = items
                    .iter()
                    .map(|i| {
                        i.as_vector()
                            .and_then(|v| v.to_characters().into_iter().next())
                            .flatten()
                    })
                    .collect();
                RValue::vec(Vector::Character(vals.into()))
            }
            _ => {
                // Fall back to list
                let entries: Vec<(Option<String>, RValue)> =
                    items.iter().map(|v| (None, v.clone())).collect();
                RValue::List(RList::new(entries))
            }
        }
    } else {
        // Mixed types — coerce to character
        let vals: Vec<Option<String>> = items
            .iter()
            .map(|i| {
                i.as_vector()
                    .and_then(|v| v.to_characters().into_iter().next())
                    .flatten()
            })
            .collect();
        RValue::vec(Vector::Character(vals.into()))
    }
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
/// When `recursive = TRUE` (default), recursively flattens all nested lists
/// into a single atomic vector using the same coercion rules as `c()`.
/// When `recursive = FALSE`, flattens only one level of list nesting.
///
/// @param x list to flatten
/// @param recursive whether to flatten recursively (default: TRUE)
/// @return atomic vector
#[builtin(min_args = 1)]
fn builtin_unlist(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let recursive = named
        .iter()
        .find(|(k, _)| k == "recursive")
        .and_then(|(_, v)| v.as_vector()?.as_logical_scalar())
        .unwrap_or(true);

    match args.first() {
        Some(RValue::List(l)) => {
            let mut all_vals = Vec::new();
            if recursive {
                collect_list_elements_recursive(l, &mut all_vals);
            } else {
                // Flatten one level only: extract elements from sub-lists
                // but don't recurse deeper
                for (_, v) in &l.values {
                    match v {
                        RValue::List(inner) => {
                            for (_, elem) in &inner.values {
                                all_vals.push(elem.clone());
                            }
                        }
                        other => all_vals.push(other.clone()),
                    }
                }
            }
            builtin_c(&all_vals, &[])
        }
        Some(other) => Ok(other.clone()),
        None => Ok(RValue::Null),
    }
}

/// Recursively collect all non-list elements from a list and its nested sublists.
fn collect_list_elements_recursive(list: &RList, out: &mut Vec<RValue>) {
    for (_, v) in &list.values {
        match v {
            RValue::List(inner) => collect_list_elements_recursive(inner, out),
            other => out.push(other.clone()),
        }
    }
}

// invisible() is implemented as an interpreter_builtin in interp.rs
// so it can set the interpreter's visibility flag.

/// Vectorized conditional: for each element of test, select the corresponding
/// element from yes (when TRUE) or no (when FALSE). yes and no are recycled
/// to the length of test.
///
/// @param test logical vector
/// @param yes values to use where test is TRUE (recycled)
/// @param no values to use where test is FALSE (recycled)
/// @return vector of same length as test, with elements drawn from yes or no
#[builtin(min_args = 3)]
fn builtin_ifelse(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    if args.len() < 3 {
        return Err(RError::new(
            RErrorKind::Argument,
            "need 3 arguments".to_string(),
        ));
    }
    let test_vec = args[0]
        .as_vector()
        .ok_or_else(|| RError::new(RErrorKind::Argument, "test must be a vector".to_string()))?;
    let test_logicals = test_vec.to_logicals();
    let n = test_logicals.len();

    let yes_vec = args[1]
        .as_vector()
        .ok_or_else(|| RError::new(RErrorKind::Argument, "yes must be a vector".to_string()))?;
    let no_vec = args[2]
        .as_vector()
        .ok_or_else(|| RError::new(RErrorKind::Argument, "no must be a vector".to_string()))?;

    // Determine result type based on R's coercion hierarchy
    let is_character =
        matches!(yes_vec, Vector::Character(_)) || matches!(no_vec, Vector::Character(_));
    let is_logical = matches!(yes_vec, Vector::Logical(_)) && matches!(no_vec, Vector::Logical(_));
    let is_integer = !is_character
        && (matches!(yes_vec, Vector::Integer(_) | Vector::Logical(_)))
        && (matches!(no_vec, Vector::Integer(_) | Vector::Logical(_)));

    if is_character {
        let yes_chars = yes_vec.to_characters();
        let no_chars = no_vec.to_characters();
        let result: Vec<Option<String>> = (0..n)
            .map(|i| match test_logicals[i] {
                Some(true) => yes_chars[i % yes_chars.len()].clone(),
                Some(false) => no_chars[i % no_chars.len()].clone(),
                None => None,
            })
            .collect();
        Ok(RValue::vec(Vector::Character(result.into())))
    } else if is_logical {
        let yes_bools = yes_vec.to_logicals();
        let no_bools = no_vec.to_logicals();
        let result: Vec<Option<bool>> = (0..n)
            .map(|i| match test_logicals[i] {
                Some(true) => yes_bools[i % yes_bools.len()],
                Some(false) => no_bools[i % no_bools.len()],
                None => None,
            })
            .collect();
        Ok(RValue::vec(Vector::Logical(result.into())))
    } else if is_integer {
        let yes_ints = yes_vec.to_integers();
        let no_ints = no_vec.to_integers();
        let result: Vec<Option<i64>> = (0..n)
            .map(|i| match test_logicals[i] {
                Some(true) => yes_ints[i % yes_ints.len()],
                Some(false) => no_ints[i % no_ints.len()],
                None => None,
            })
            .collect();
        Ok(RValue::vec(Vector::Integer(result.into())))
    } else {
        let yes_doubles = yes_vec.to_doubles();
        let no_doubles = no_vec.to_doubles();
        let result: Vec<Option<f64>> = (0..n)
            .map(|i| match test_logicals[i] {
                Some(true) => yes_doubles[i % yes_doubles.len()],
                Some(false) => no_doubles[i % no_doubles.len()],
                None => None,
            })
            .collect();
        Ok(RValue::vec(Vector::Double(result.into())))
    }
}

/// Find positions of first matches of x in table.
///
/// For each element of x, returns the position of its first exact match in table,
/// or NA if no match is found. Supports case-insensitive matching via `ignore.case`.
///
/// @param x values to look up
/// @param table values to match against
/// @param ignore.case logical; if TRUE, comparison is case-insensitive (Unicode-aware)
/// @return integer vector of match positions (1-indexed)
#[builtin(min_args = 2)]
fn builtin_match(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    if args.len() < 2 {
        return Err(RError::new(
            RErrorKind::Argument,
            "need 2 arguments".to_string(),
        ));
    }

    let ignore_case = named
        .iter()
        .find(|(k, _)| k == "ignore.case")
        .and_then(|(_, v)| match v {
            RValue::Vector(rv) => rv.inner.as_logical_scalar(),
            _ => None,
        })
        .unwrap_or(false);

    let x = match &args[0] {
        RValue::Vector(v) => v.to_characters(),
        _ => return Ok(RValue::vec(Vector::Integer(vec![None].into()))),
    };
    let table = match &args[1] {
        RValue::Vector(v) => v.to_characters(),
        _ => return Ok(RValue::vec(Vector::Integer(vec![None].into()))),
    };

    let result: Vec<Option<i64>> = if ignore_case {
        x.iter()
            .map(|xi| {
                xi.as_ref().and_then(|xi| {
                    let key = unicase::UniCase::new(xi.as_str());
                    table
                        .iter()
                        .position(|t| t.as_deref().map(unicase::UniCase::new) == Some(key))
                        .map(|p| i64::try_from(p).map(|v| v + 1).unwrap_or(0))
                })
            })
            .collect()
    } else {
        x.iter()
            .map(|xi| {
                xi.as_ref().and_then(|xi| {
                    table
                        .iter()
                        .position(|t| t.as_ref() == Some(xi))
                        .map(|p| i64::try_from(p).map(|v| v + 1).unwrap_or(0))
                })
            })
            .collect()
    };
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
/// Preserves the type of the input vector. Replacement values are coerced
/// to the input type and recycled if shorter than the index vector.
///
/// @param x vector to modify
/// @param list indices at which to replace (1-indexed)
/// @param values replacement values (recycled if shorter)
/// @return modified vector with same type as x
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
            let indices = args[1]
                .as_vector()
                .map(|v| v.to_integers())
                .unwrap_or_default();
            let vals_vec = args[2]
                .as_vector()
                .cloned()
                .unwrap_or(Vector::Logical(vec![None].into()));

            match &v.inner {
                Vector::Character(_) => {
                    let mut data = v.to_characters();
                    let vals = vals_vec.to_characters();
                    if vals.is_empty() {
                        return Ok(RValue::vec(Vector::Character(data.into())));
                    }
                    for (i, idx) in indices.iter().enumerate() {
                        if let Some(idx) = idx {
                            let idx = usize::try_from(*idx)? - 1;
                            if idx < data.len() {
                                data[idx] = vals[i % vals.len()].clone();
                            }
                        }
                    }
                    Ok(RValue::vec(Vector::Character(data.into())))
                }
                Vector::Integer(_) => {
                    let mut data = v.to_integers();
                    let vals = vals_vec.to_integers();
                    if vals.is_empty() {
                        return Ok(RValue::vec(Vector::Integer(data.into())));
                    }
                    for (i, idx) in indices.iter().enumerate() {
                        if let Some(idx) = idx {
                            let idx = usize::try_from(*idx)? - 1;
                            if idx < data.len() {
                                data[idx] = vals[i % vals.len()];
                            }
                        }
                    }
                    Ok(RValue::vec(Vector::Integer(data.into())))
                }
                Vector::Logical(_) => {
                    let mut data = v.to_logicals();
                    let vals = vals_vec.to_logicals();
                    if vals.is_empty() {
                        return Ok(RValue::vec(Vector::Logical(data.into())));
                    }
                    for (i, idx) in indices.iter().enumerate() {
                        if let Some(idx) = idx {
                            let idx = usize::try_from(*idx)? - 1;
                            if idx < data.len() {
                                data[idx] = vals[i % vals.len()];
                            }
                        }
                    }
                    Ok(RValue::vec(Vector::Logical(data.into())))
                }
                Vector::Complex(_) => {
                    let mut data = v.to_complex();
                    let vals = vals_vec.to_complex();
                    if vals.is_empty() {
                        return Ok(RValue::vec(Vector::Complex(data.into())));
                    }
                    for (i, idx) in indices.iter().enumerate() {
                        if let Some(idx) = idx {
                            let idx = usize::try_from(*idx)? - 1;
                            if idx < data.len() {
                                data[idx] = vals[i % vals.len()];
                            }
                        }
                    }
                    Ok(RValue::vec(Vector::Complex(data.into())))
                }
                _ => {
                    // Double and Raw fall through to double replacement
                    let mut data = v.to_doubles();
                    let vals = vals_vec.to_doubles();
                    if vals.is_empty() {
                        return Ok(RValue::vec(Vector::Double(data.into())));
                    }
                    for (i, idx) in indices.iter().enumerate() {
                        if let Some(idx) = idx {
                            let idx = usize::try_from(*idx)? - 1;
                            if idx < data.len() {
                                data[idx] = vals[i % vals.len()];
                            }
                        }
                    }
                    Ok(RValue::vec(Vector::Double(data.into())))
                }
            }
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
#[interpreter_builtin]
fn interp_readline(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let prompt = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();
    context.write(&prompt);
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

/// Load and attach a package.
///
/// Searches .libPaths() for the package directory, parses DESCRIPTION and
/// NAMESPACE, creates a namespace environment, sources all R/*.R files,
/// registers exports, and attaches the package to the search path.
///
/// @param package name of the package to load (character or symbol)
/// @return the package name (invisibly), or errors if package not found
#[interpreter_builtin(name = "library", min_args = 1)]
fn interp_library(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    // Extract package name — R accepts both quoted strings and bare symbols
    let pkg = extract_package_name(args, named)?;

    context.with_interpreter(|interp| {
        // Load namespace (creates env, sources R files, etc.)
        interp.load_namespace(&pkg)?;
        // Attach to search path
        interp.attach_package(&pkg)?;
        Ok(RValue::vec(Vector::Character(vec![Some(pkg)].into())))
    })
}

/// Load a package if available, returning TRUE/FALSE instead of erroring.
///
/// Like library(), but returns FALSE instead of raising an error if the
/// package is not found. When quietly = TRUE, suppresses the warning message.
///
/// @param package name of the package to load
/// @param quietly logical: suppress messages?
/// @return logical: TRUE if package was loaded, FALSE otherwise
#[interpreter_builtin(name = "require", min_args = 1)]
fn interp_require(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let pkg = extract_package_name(args, named)?;
    let quietly = named
        .iter()
        .find(|(n, _)| n == "quietly")
        .and_then(|(_, v)| v.as_vector()?.as_logical_scalar())
        .unwrap_or(false);

    let result = context.with_interpreter(|interp| match interp.load_namespace(&pkg) {
        Ok(_) => {
            let _ = interp.attach_package(&pkg);
            Ok(RValue::vec(Vector::Logical(vec![Some(true)].into())))
        }
        Err(_) => Ok(RValue::vec(Vector::Logical(vec![Some(false)].into()))),
    });
    if let Ok(RValue::Vector(rv)) = &result {
        if rv.as_logical_scalar() == Some(false) && !quietly {
            context.write_err(&format!(
                "Warning message:\nthere is no package called '{}'\n",
                pkg
            ));
        }
    }
    result
}

/// Extract the package name from library/require arguments.
///
/// R accepts both `library("pkg")` and `library(pkg)` (unquoted symbol).
/// Since we receive post-evaluation values, bare symbols have already been
/// resolved. The name must come as a character scalar.
fn extract_package_name(args: &[RValue], named: &[(String, RValue)]) -> Result<String, RError> {
    // Check named 'package' argument first
    let val = named
        .iter()
        .find(|(n, _)| n == "package")
        .map(|(_, v)| v)
        .or_else(|| args.first());

    match val {
        Some(v) => v
            .as_vector()
            .and_then(|v| v.as_character_scalar())
            .ok_or_else(|| {
                RError::new(
                    RErrorKind::Argument,
                    "invalid package name — expected a character string".to_string(),
                )
            }),
        None => Err(RError::new(
            RErrorKind::Argument,
            "argument 'package' is missing".to_string(),
        )),
    }
}

/// Load a package namespace without attaching it to the search path.
///
/// Creates the namespace environment, sources all R files, and registers
/// exports, but does not add the package to the search path. This is the
/// mechanism underlying `library()` and `::` resolution.
///
/// @param package character scalar: the package name
/// @return the namespace environment
#[interpreter_builtin(name = "loadNamespace", min_args = 1)]
fn interp_load_namespace(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let pkg = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| RError::new(RErrorKind::Argument, "invalid package name".to_string()))?;

    let env = context.with_interpreter(|interp| interp.load_namespace(&pkg))?;
    Ok(RValue::Environment(env))
}

/// Check if a namespace can be loaded, returning TRUE/FALSE.
///
/// Like loadNamespace(), but returns TRUE/FALSE instead of raising an error.
/// Useful for conditional logic that depends on package availability.
///
/// @param package character scalar: the package name
/// @param quietly logical: suppress messages? (default TRUE)
/// @return logical: TRUE if the namespace could be loaded
#[interpreter_builtin(name = "requireNamespace", min_args = 1)]
fn interp_require_namespace(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let pkg = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| RError::new(RErrorKind::Argument, "invalid package name".to_string()))?;

    let quietly = named
        .iter()
        .find(|(n, _)| n == "quietly")
        .and_then(|(_, v)| v.as_vector()?.as_logical_scalar())
        .unwrap_or(true);

    let result = context.with_interpreter(|interp| interp.load_namespace(&pkg));
    match result {
        Ok(_) => Ok(RValue::vec(Vector::Logical(vec![Some(true)].into()))),
        Err(_) => {
            if !quietly {
                context.write_err(&format!(
                    "Warning message:\nthere is no package called '{}'\n",
                    pkg
                ));
            }
            Ok(RValue::vec(Vector::Logical(vec![Some(false)].into())))
        }
    }
}

/// Detach a package from the search path.
///
/// @param name the search path entry to detach (e.g. "package:dplyr")
/// @return NULL (invisibly)
#[interpreter_builtin(name = "detach", min_args = 1)]
fn interp_detach(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let name = named
        .iter()
        .find(|(n, _)| n == "name")
        .map(|(_, v)| v)
        .or_else(|| args.first())
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| RError::new(RErrorKind::Argument, "invalid 'name' argument".to_string()))?;

    // If user said just "dplyr", prefix with "package:"
    let entry_name = if name.starts_with("package:") {
        name
    } else {
        format!("package:{}", name)
    };

    context.with_interpreter(|interp| interp.detach_package(&entry_name))?;
    Ok(RValue::Null)
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
                rv.attrs.as_mut().map(|a| a.shift_remove("dim"));
                rv.attrs.as_mut().map(|a| a.shift_remove("class"));
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
            rv.attrs.as_mut().map(|a| a.shift_remove("names"));
            rv.attrs.as_mut().map(|a| a.shift_remove("dimnames"));
            Ok(RValue::Vector(rv))
        }
        Some(RValue::List(l)) => {
            let mut l = l.clone();
            for entry in &mut l.values {
                entry.0 = None;
            }
            l.attrs.as_mut().map(|a| a.shift_remove("names"));
            l.attrs.as_mut().map(|a| a.shift_remove("dimnames"));
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
                rv.attrs.as_mut().map(|a| a.shift_remove("dimnames"));
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
                l.attrs.as_mut().map(|a| a.shift_remove("dimnames"));
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

/// Bind vectors, matrices, or data frames by rows.
///
/// Combines arguments row-wise. If any argument is a data frame, produces
/// a data frame result by matching columns by name. Otherwise produces a matrix.
///
/// @param ... vectors, matrices, or data frames to bind
/// @return matrix or data frame
#[builtin(min_args = 1)]
fn builtin_rbind(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    if args.is_empty() {
        return Ok(RValue::Null);
    }

    // Check if any argument is a data frame — if so, use data frame rbind
    let has_df = args
        .iter()
        .any(|a| matches!(a, RValue::List(_)) && has_class(a, "data.frame"));
    if has_df {
        return rbind_data_frames(args);
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

/// Row-bind data frames by matching column names.
fn rbind_data_frames(args: &[RValue]) -> Result<RValue, RError> {
    // Collect all unique column names in order
    let mut all_col_names: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut dfs: Vec<&RList> = Vec::new();

    for arg in args {
        match arg {
            RValue::List(list) if has_class(arg, "data.frame") => {
                let names = dataframes::df_col_names(list);
                for name in names.into_iter().flatten() {
                    if seen.insert(name.clone()) {
                        all_col_names.push(name);
                    }
                }
                dfs.push(list);
            }
            RValue::Null => continue,
            _ => {
                return Err(RError::new(
                    RErrorKind::Argument,
                    "rbind() with data frames requires all arguments to be data frames".to_string(),
                ))
            }
        }
    }

    if dfs.is_empty() {
        return Ok(RValue::Null);
    }

    let total_nrow: usize = dfs.iter().map(|df| dataframes::df_nrow(df)).sum();

    // For each column, concatenate values from all data frames
    let mut output_columns: Vec<(Option<String>, RValue)> = Vec::new();
    for col_name in &all_col_names {
        let mut parts: Vec<RValue> = Vec::new();
        for df in &dfs {
            let col_idx = dataframes::df_col_index(df, col_name);
            let nrow = dataframes::df_nrow(df);
            if let Some(idx) = col_idx {
                parts.push(df.values[idx].1.clone());
            } else {
                // Fill with NA for missing columns
                parts.push(RValue::vec(Vector::Logical(vec![None; nrow].into())));
            }
        }
        let combined = concat_column_values(&parts)?;
        output_columns.push((Some(col_name.clone()), combined));
    }

    dataframes::build_data_frame(output_columns, total_nrow)
}

/// Concatenate column values from multiple data frames into a single column.
fn concat_column_values(parts: &[RValue]) -> Result<RValue, RError> {
    // Determine the "widest" type
    let mut has_character = false;
    let mut has_double = false;
    let mut has_integer = false;
    let mut has_logical = false;

    for part in parts {
        if let RValue::Vector(rv) = part {
            match &rv.inner {
                Vector::Character(_) => has_character = true,
                Vector::Double(_) => has_double = true,
                Vector::Integer(_) => has_integer = true,
                Vector::Logical(_) => has_logical = true,
                _ => {}
            }
        }
    }

    if has_character {
        let mut result: Vec<Option<String>> = Vec::new();
        for part in parts {
            if let RValue::Vector(rv) = part {
                result.extend(rv.inner.to_characters());
            }
        }
        Ok(RValue::vec(Vector::Character(result.into())))
    } else if has_double {
        let mut result: Vec<Option<f64>> = Vec::new();
        for part in parts {
            if let RValue::Vector(rv) = part {
                result.extend(rv.to_doubles());
            }
        }
        Ok(RValue::vec(Vector::Double(result.into())))
    } else if has_integer {
        let mut result: Vec<Option<i64>> = Vec::new();
        for part in parts {
            if let RValue::Vector(rv) = part {
                result.extend(rv.inner.to_integers());
            }
        }
        Ok(RValue::vec(Vector::Integer(result.into())))
    } else if has_logical {
        let mut result: Vec<Option<bool>> = Vec::new();
        for part in parts {
            if let RValue::Vector(rv) = part {
                result.extend(rv.inner.to_logicals());
            }
        }
        Ok(RValue::vec(Vector::Logical(result.into())))
    } else {
        Ok(RValue::Null)
    }
}

/// Bind vectors, matrices, or data frames by columns.
///
/// Combines arguments column-wise. If any argument is a data frame, produces
/// a data frame result. Otherwise produces a matrix.
///
/// @param ... vectors, matrices, or data frames to bind
/// @return matrix or data frame
#[builtin(min_args = 1)]
fn builtin_cbind(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    if args.is_empty() {
        return Ok(RValue::Null);
    }

    // Check if any argument is a data frame — if so, use data frame cbind
    let has_df = args
        .iter()
        .any(|a| matches!(a, RValue::List(_)) && has_class(a, "data.frame"));
    if has_df {
        return cbind_data_frames(args);
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

/// Column-bind data frames.
fn cbind_data_frames(args: &[RValue]) -> Result<RValue, RError> {
    let mut output_columns: Vec<(Option<String>, RValue)> = Vec::new();
    let mut target_nrow: Option<usize> = None;

    for arg in args {
        match arg {
            RValue::List(list) if has_class(arg, "data.frame") => {
                let nrow = dataframes::df_nrow(list);
                if let Some(expected) = target_nrow {
                    if nrow != expected {
                        return Err(RError::new(
                            RErrorKind::Argument,
                            format!(
                                "cbind() arguments have different row counts: {} vs {}",
                                expected, nrow
                            ),
                        ));
                    }
                } else {
                    target_nrow = Some(nrow);
                }
                for (name, val) in &list.values {
                    output_columns.push((name.clone(), val.clone()));
                }
            }
            RValue::Vector(rv) => {
                let len = rv.inner.len();
                if let Some(expected) = target_nrow {
                    if len != expected && len != 1 {
                        return Err(RError::new(
                            RErrorKind::Argument,
                            format!(
                                "cbind() arguments have different row counts: {} vs {}",
                                expected, len
                            ),
                        ));
                    }
                } else {
                    target_nrow = Some(len);
                }
                // Use names attr as column name if available
                let col_name = rv
                    .get_attr("names")
                    .and_then(|v| v.as_vector()?.as_character_scalar());
                output_columns.push((col_name, RValue::Vector(rv.clone())));
            }
            RValue::Null => continue,
            _ => {
                return Err(RError::new(
                    RErrorKind::Argument,
                    "cbind() requires data frames or vectors".to_string(),
                ))
            }
        }
    }

    let nrow = target_nrow.unwrap_or(0);
    dataframes::build_data_frame(output_columns, nrow)
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
                rv.attrs.as_mut().map(|a| a.shift_remove(&which));
            } else {
                rv.set_attr(which, value);
            }
            Ok(RValue::Vector(rv))
        }
        Some(RValue::List(l)) => {
            let mut l = l.clone();
            if value.is_null() {
                l.attrs.as_mut().map(|a| a.shift_remove(&which));
            } else {
                l.set_attr(which, value);
            }
            Ok(RValue::List(l))
        }
        Some(RValue::Language(lang)) => {
            let mut lang = lang.clone();
            if value.is_null() {
                lang.attrs.as_mut().map(|a| a.shift_remove(&which));
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
/// @param hash logical; if TRUE, use a hashed environment (always TRUE in miniR since we use HashMap)
/// @param parent parent environment (or NULL for an empty environment)
/// @param size integer; initial size hint (ignored — HashMap handles resizing)
/// @return new environment
#[builtin(name = "new.env")]
fn builtin_new_env(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    // R signature: new.env(hash = TRUE, parent = parent.frame(), size = 29L)
    // Positional order: hash, parent, size
    // We accept all three but only parent affects behavior.

    let named_hash = named.iter().find(|(n, _)| n == "hash").map(|(_, v)| v);
    let named_parent = named.iter().find(|(n, _)| n == "parent").map(|(_, v)| v);
    let named_size = named.iter().find(|(n, _)| n == "size").map(|(_, v)| v);

    // Resolve positional args (hash, parent, size) skipping those provided as named
    let mut positional_iter = args.iter();
    let _hash_val = named_hash.or_else(|| positional_iter.next());
    // hash is always TRUE in miniR — we use HashMap regardless
    let parent_val = named_parent.or_else(|| positional_iter.next());
    let _size_val = named_size.or_else(|| positional_iter.next());
    // size is a no-op — HashMap resizes dynamically

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

/// Set the parent (enclosing) environment.
///
/// This is a replacement function: `parent.env(e) <- value` sets the parent
/// of environment `e` to `value`.
///
/// @param env environment whose parent to change
/// @param value new parent environment
/// @return the modified environment (invisibly)
#[builtin(name = "parent.env<-", min_args = 2)]
fn builtin_parent_env_set(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let env = match args.first() {
        Some(RValue::Environment(e)) => e.clone(),
        _ => {
            return Err(RError::new(
                RErrorKind::Argument,
                "not an environment".to_string(),
            ))
        }
    };
    let new_parent = match args.get(1) {
        Some(RValue::Environment(p)) => Some(p.clone()),
        Some(RValue::Null) => None,
        _ => {
            return Err(RError::new(
                RErrorKind::Argument,
                "'value' must be an environment or NULL".to_string(),
            ))
        }
    };
    env.set_parent(new_parent);
    Ok(RValue::Environment(env))
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
            rv.attrs.as_mut().map(|a| a.shift_remove("class"));
            Ok(RValue::Vector(rv))
        }
        Some(RValue::List(l)) => {
            let mut l = l.clone();
            l.attrs.as_mut().map(|a| a.shift_remove("class"));
            Ok(RValue::List(l))
        }
        Some(RValue::Language(lang)) => {
            let mut lang = lang.clone();
            lang.attrs.as_mut().map(|a| a.shift_remove("class"));
            Ok(RValue::Language(lang))
        }
        other => Ok(other.cloned().unwrap_or(RValue::Null)),
    }
}

/// Match an argument to a set of candidate values.
///
/// Performs exact and then partial matching against the choices vector.
/// When called without explicit `choices`, looks up the calling function's
/// formals to determine valid choices (R's standard behavior for
/// `match.arg(arg)` inside a function with `arg = c("a","b","c")`).
///
/// @param arg character scalar (or vector if several.ok=TRUE) to match
/// @param choices character vector of allowed values
/// @param several.ok logical; if TRUE, allow arg to have length > 1
/// @return the matched value(s)
#[builtin(name = "match.arg", min_args = 1)]
fn builtin_match_arg(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let arg = args.first().cloned().unwrap_or(RValue::Null);
    let choices = args
        .get(1)
        .or_else(|| named.iter().find(|(n, _)| n == "choices").map(|(_, v)| v));

    let several_ok = args
        .get(2)
        .or_else(|| {
            named
                .iter()
                .find(|(n, _)| n == "several.ok")
                .map(|(_, v)| v)
        })
        .and_then(|v| match v {
            RValue::Vector(rv) => rv.as_logical_scalar(),
            _ => None,
        })
        .unwrap_or(false);

    let choices_vec = match choices {
        Some(RValue::Vector(rv)) if matches!(rv.inner, Vector::Character(_)) => {
            let Vector::Character(v) = &rv.inner else {
                unreachable!()
            };
            v.iter().filter_map(|s| s.clone()).collect::<Vec<_>>()
        }
        Some(RValue::Null) | None => {
            // No explicit choices provided — use the arg value as both the
            // choices and the indicator. In R, match.arg(arg) without choices
            // uses the formal default; when the user doesn't override the
            // default, arg IS the full choices vector. If arg is a character
            // vector with length > 1, treat it as the choices and return the
            // first element (the default). If it's length 1, accept it as-is.
            match &arg {
                RValue::Vector(rv) if matches!(rv.inner, Vector::Character(_)) => {
                    let Vector::Character(v) = &rv.inner else {
                        unreachable!()
                    };
                    let strings: Vec<String> =
                        v.iter().filter_map(|s| s.clone()).collect::<Vec<_>>();
                    if strings.len() > 1 && !several_ok {
                        // User didn't override the default — return first choice
                        return Ok(RValue::vec(Vector::Character(
                            vec![Some(strings[0].clone())].into(),
                        )));
                    } else if strings.len() == 1 {
                        // User provided a single value — accept it
                        return Ok(arg);
                    } else if strings.is_empty() {
                        return Ok(arg);
                    }
                    // several.ok = TRUE with multi-element arg
                    return Ok(arg);
                }
                RValue::Null => {
                    // NULL arg with no choices — can't do anything
                    return Ok(RValue::Null);
                }
                _ => return Ok(arg),
            }
        }
        _ => return Ok(arg),
    };

    if choices_vec.is_empty() {
        return Ok(arg);
    }

    // Extract the argument strings
    let arg_strings: Vec<Option<String>> = match &arg {
        RValue::Vector(rv) => match &rv.inner {
            Vector::Character(v) => v.iter().cloned().collect(),
            _ => vec![None],
        },
        RValue::Null => vec![],
        _ => vec![None],
    };

    // NULL or empty arg: return first choice (R behavior)
    if arg_strings.is_empty() {
        return Ok(RValue::vec(Vector::Character(
            vec![Some(choices_vec[0].clone())].into(),
        )));
    }

    // If arg equals choices (user didn't override default), return first choice
    if !several_ok && arg_strings.len() > 1 && arg_strings.len() == choices_vec.len() {
        let all_match = arg_strings
            .iter()
            .zip(choices_vec.iter())
            .all(|(a, c)| a.as_deref() == Some(c.as_str()));
        if all_match {
            return Ok(RValue::vec(Vector::Character(
                vec![Some(choices_vec[0].clone())].into(),
            )));
        }
    }

    // If several.ok is FALSE, arg must be length 1
    if !several_ok && arg_strings.len() > 1 {
        return Err(RError::new(
            RErrorKind::Argument,
            format!(
                "'arg' should be one of {}, not a vector of length {}",
                choices_vec.iter().map(|c| format!("'{c}'")).join(", "),
                arg_strings.len()
            ),
        ));
    }

    // Match each element
    let mut matched = Vec::with_capacity(arg_strings.len());
    for s_opt in &arg_strings {
        let s = match s_opt {
            Some(s) => s,
            None => {
                return Err(RError::new(
                    RErrorKind::Argument,
                    "'arg' should be one of the choices".to_string(),
                ))
            }
        };

        // Exact match first
        if choices_vec.contains(s) {
            matched.push(Some(s.clone()));
            continue;
        }
        // Partial match
        let partial: Vec<&String> = choices_vec
            .iter()
            .filter(|c| c.starts_with(s.as_str()))
            .collect();
        match partial.len() {
            1 => matched.push(Some(partial[0].clone())),
            0 => {
                return Err(RError::new(
                    RErrorKind::Argument,
                    format!(
                        "'arg' should be one of {}",
                        choices_vec.iter().map(|c| format!("'{c}'")).join(", ")
                    ),
                ))
            }
            _ => {
                return Err(RError::new(
                    RErrorKind::Argument,
                    format!(
                        "'arg' should be one of {}",
                        choices_vec.iter().map(|c| format!("'{c}'")).join(", ")
                    ),
                ))
            }
        }
    }

    Ok(RValue::vec(Vector::Character(matched.into())))
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

// `Recall(...)` is an interpreter builtin — see builtins/interp.rs

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
#[interpreter_builtin]
fn interp_debug(
    _args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    context
        .write_err("debug() is not supported in miniR — interactive debugging is not available.\n");
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
#[interpreter_builtin]
fn interp_browser(
    _args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    context.write_err(
        "browser() is not supported in miniR — interactive debugging is not available.\n",
    );
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
