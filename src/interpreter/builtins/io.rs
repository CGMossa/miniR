//! File I/O builtins — reading and writing data files (CSV, table, lines, scan)
//! and file system utilities (file.path, file.exists).

use std::collections::HashSet;

use super::CallArgs;
use crate::interpreter::environment::Environment;
use crate::interpreter::value::*;
use crate::interpreter::BuiltinContext;
use crate::interpreter::Interpreter;
use crate::parser::ast::{Arg, Expr};
use crate::parser::parse_program;
use derive_more::{Display, Error};
use itertools::Itertools;
use minir_macros::{builtin, interpreter_builtin, pre_eval_builtin};

const MINIR_RDS_HEADER: &str = "miniRDS1\n";
const MINIR_WORKSPACE_CLASS: &str = "miniR.workspace";

// region: IoError

/// Structured error type for file I/O operations.
#[derive(Debug, Display, Error)]
pub enum IoError {
    #[display("cannot open file '{}': {}", path, source)]
    CannotOpen {
        path: String,
        source: std::io::Error,
    },
    #[display("cannot write to file '{}': {}", path, source)]
    WriteFailed {
        path: String,
        source: std::io::Error,
    },
    #[display("error reading CSV {}: {}", context, source)]
    CsvRead { context: String, source: csv::Error },
    #[display("error writing CSV: {}", source)]
    CsvWrite {
        #[error(source)]
        source: csv::Error,
    },
    #[display("cannot open connection: {}", source)]
    Connection {
        #[error(source)]
        source: std::io::Error,
    },
    #[display("unsupported value in saveRDS(): {}", details)]
    UnsupportedSerialization { details: String },
}

impl From<IoError> for RError {
    fn from(e: IoError) -> Self {
        RError::from_source(RErrorKind::Other, e)
    }
}

// endregion

fn escape_r_string(value: &str) -> String {
    let mut escaped = String::new();
    for ch in value.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

fn syntactic_attr_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '.') {
        return false;
    }
    chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '.' || ch == '_')
}

fn serialize_complex(value: num_complex::Complex64) -> String {
    if value.im < 0.0 {
        format!(
            "{}{}i",
            format_r_double(value.re),
            format_r_double(value.im)
        )
    } else {
        format!(
            "{}+{}i",
            format_r_double(value.re),
            format_r_double(value.im)
        )
    }
}

fn serialize_vector(value: &Vector) -> String {
    match value {
        Vector::Raw(values) if values.is_empty() => "raw(0)".to_string(),
        Vector::Raw(values) => format!(
            "as.raw(c({}))",
            values.iter().map(|value| value.to_string()).join(", ")
        ),
        Vector::Logical(values) if values.is_empty() => "logical(0)".to_string(),
        Vector::Logical(values) => format!(
            "c({})",
            values
                .iter()
                .map(|value| match value {
                    Some(true) => "TRUE".to_string(),
                    Some(false) => "FALSE".to_string(),
                    None => "NA".to_string(),
                })
                .join(", ")
        ),
        Vector::Integer(values) if values.is_empty() => "integer(0)".to_string(),
        Vector::Integer(values) => format!(
            "c({})",
            values
                .iter()
                .map(|value| match value {
                    Some(value) => format!("{value}L"),
                    None => "NA_integer_".to_string(),
                })
                .join(", ")
        ),
        Vector::Double(values) if values.is_empty() => "numeric(0)".to_string(),
        Vector::Double(values) => format!(
            "c({})",
            values
                .iter()
                .map(|value| match value {
                    Some(value) => format_r_double(*value),
                    None => "NA_real_".to_string(),
                })
                .join(", ")
        ),
        Vector::Complex(values) if values.is_empty() => "complex(0)".to_string(),
        Vector::Complex(values) => format!(
            "c({})",
            values
                .iter()
                .map(|value| match value {
                    Some(value) => serialize_complex(*value),
                    None => "NA_complex_".to_string(),
                })
                .join(", ")
        ),
        Vector::Character(values) if values.is_empty() => "character(0)".to_string(),
        Vector::Character(values) => format!(
            "c({})",
            values
                .iter()
                .map(|value| match value {
                    Some(value) => format!("\"{}\"", escape_r_string(value)),
                    None => "NA_character_".to_string(),
                })
                .join(", ")
        ),
    }
}

fn serialize_attr_pairs(
    attrs: Option<&std::collections::HashMap<String, RValue>>,
    synthetic_names: Option<Vec<Option<String>>>,
) -> Result<Vec<(String, String)>, RError> {
    let mut pairs = Vec::new();
    let mut seen = HashSet::new();

    if let Some(names) = synthetic_names {
        if names.iter().any(|name| name.is_some()) {
            pairs.push((
                "names".to_string(),
                serialize_rvalue(&RValue::vec(Vector::Character(names.into())))?,
            ));
            seen.insert("names".to_string());
        }
    }

    if let Some(attrs) = attrs {
        for key in attrs.keys().sorted() {
            if seen.contains(key) {
                continue;
            }
            if !syntactic_attr_name(key) {
                return Err(IoError::UnsupportedSerialization {
                    details: format!("attribute '{}' is not yet serializable", key),
                }
                .into());
            }
            pairs.push((key.clone(), serialize_rvalue(&attrs[key])?));
        }
    }

    Ok(pairs)
}

fn serialize_with_attrs(base: String, attrs: Vec<(String, String)>) -> String {
    if attrs.is_empty() {
        return base;
    }
    let attr_args = attrs
        .into_iter()
        .map(|(name, value)| format!("{name} = {value}"))
        .join(", ");
    format!("structure({base}, {attr_args})")
}

fn serialize_rvalue(value: &RValue) -> Result<String, RError> {
    match value {
        RValue::Null => Ok("NULL".to_string()),
        RValue::Vector(rv) => Ok(serialize_with_attrs(
            serialize_vector(&rv.inner),
            serialize_attr_pairs(rv.attrs.as_deref(), None)?,
        )),
        RValue::List(list) => {
            let base = format!(
                "list({})",
                list.values
                    .iter()
                    .map(|(_, value)| serialize_rvalue(value))
                    .collect::<Result<Vec<_>, _>>()?
                    .join(", ")
            );
            let synthetic_names = if list.get_attr("names").is_none() {
                Some(
                    list.values
                        .iter()
                        .map(|(name, _)| name.clone())
                        .collect::<Vec<_>>(),
                )
            } else {
                None
            };
            Ok(serialize_with_attrs(
                base,
                serialize_attr_pairs(list.attrs.as_deref(), synthetic_names)?,
            ))
        }
        RValue::Language(expr) => {
            let base = format!("quote({})", deparse_expr(expr));
            Ok(serialize_with_attrs(
                base,
                serialize_attr_pairs(expr.attrs.as_deref(), None)?,
            ))
        }
        RValue::Function(_) => Err(IoError::UnsupportedSerialization {
            details: "functions are not yet serializable".to_string(),
        }
        .into()),
        RValue::Environment(_) => Err(IoError::UnsupportedSerialization {
            details: "environments are not yet serializable".to_string(),
        }
        .into()),
    }
}

// region: miniRDS helpers

fn write_minirds(path: &str, value: &RValue) -> Result<(), RError> {
    let serialized = serialize_rvalue(value)?;
    std::fs::write(path, format!("{MINIR_RDS_HEADER}{serialized}\n")).map_err(|source| {
        IoError::WriteFailed {
            path: path.to_string(),
            source,
        }
        .into()
    })
}

fn read_minirds(
    path: &str,
    reader_name: &str,
    writer_name: &str,
    interp: &Interpreter,
) -> Result<RValue, RError> {
    let content = std::fs::read_to_string(path).map_err(|source| IoError::CannotOpen {
        path: path.to_string(),
        source,
    })?;

    let body = content.strip_prefix(MINIR_RDS_HEADER).ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            format!(
                "unsupported {reader_name}() format in '{}': miniR currently reads only miniRDS text files written by {writer_name}()",
                path,
            ),
        )
    })?;

    let ast =
        parse_program(body).map_err(|err| RError::new(RErrorKind::Parse, format!("{err}")))?;

    let base = interp
        .global_env
        .parent()
        .unwrap_or_else(|| interp.global_env.clone());
    let eval_env = Environment::new_child(&base);
    interp.eval_in(&ast, &eval_env).map_err(RError::from)
}

// endregion

// region: workspace helpers

fn workspace_class_value() -> RValue {
    RValue::vec(Vector::Character(
        vec![Some(MINIR_WORKSPACE_CLASS.to_string())].into(),
    ))
}

fn is_workspace_value(value: &RValue) -> bool {
    let RValue::List(list) = value else {
        return false;
    };

    list.get_attr("class")
        .and_then(|value| value.as_vector())
        .map(|values| {
            values
                .to_characters()
                .iter()
                .flatten()
                .any(|class_name| class_name == MINIR_WORKSPACE_CLASS)
        })
        .unwrap_or(false)
}

fn workspace_binding_names(list: &RList) -> Result<Vec<String>, RError> {
    if let Some(names_attr) = list.get_attr("names") {
        let values = names_attr
            .as_vector()
            .ok_or_else(|| {
                RError::new(
                    RErrorKind::Argument,
                    "invalid workspace file: 'names' attribute is not a character vector"
                        .to_string(),
                )
            })?
            .to_characters();

        if values.len() != list.values.len() {
            return Err(RError::new(
                RErrorKind::Argument,
                "invalid workspace file: binding names do not match saved values".to_string(),
            ));
        }

        return values
            .into_iter()
            .map(|name| {
                name.ok_or_else(|| {
                    RError::new(
                        RErrorKind::Argument,
                        "invalid workspace file: every saved object needs a name".to_string(),
                    )
                })
            })
            .collect();
    }

    list.values
        .iter()
        .map(|(name, _)| {
            name.clone().ok_or_else(|| {
                RError::new(
                    RErrorKind::Argument,
                    "invalid workspace file: every saved object needs a name".to_string(),
                )
            })
        })
        .collect()
}

fn eval_arg_value(arg: &Arg, env: &Environment, interp: &Interpreter) -> Result<RValue, RError> {
    let expr = arg.value.as_ref().ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "argument is missing a value".to_string(),
        )
    })?;
    interp.eval_in(expr, env).map_err(RError::from)
}

fn push_save_name(
    names: &mut Vec<String>,
    seen: &mut HashSet<String>,
    name: String,
) -> Result<(), RError> {
    if !seen.insert(name.clone()) {
        return Err(RError::new(
            RErrorKind::Argument,
            format!("duplicate object name '{}' in save()", name),
        ));
    }

    names.push(name);
    Ok(())
}

fn workspace_file_arg(
    args: &[Arg],
    env: &Environment,
    interp: &Interpreter,
) -> Result<String, RError> {
    args.iter()
        .find(|arg| arg.name.as_deref() == Some("file"))
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "save() requires a named 'file' argument".to_string(),
            )
        })
        .and_then(|arg| {
            eval_arg_value(arg, env, interp)?
                .as_vector()
                .and_then(|value| value.as_character_scalar())
                .ok_or_else(|| {
                    RError::new(RErrorKind::Argument, "invalid 'file' argument".to_string())
                })
        })
}

fn workspace_target_env(
    args: &[Arg],
    env: &Environment,
    interp: &Interpreter,
) -> Result<Environment, RError> {
    match args.iter().find(|arg| arg.name.as_deref() == Some("envir")) {
        Some(arg) => match eval_arg_value(arg, env, interp)? {
            RValue::Environment(target_env) => Ok(target_env),
            _ => Err(RError::new(
                RErrorKind::Argument,
                "invalid 'envir' argument".to_string(),
            )),
        },
        None => Ok(env.clone()),
    }
}

fn workspace_requested_names(
    args: &[Arg],
    env: &Environment,
    interp: &Interpreter,
) -> Result<Vec<String>, RError> {
    let mut names = Vec::new();
    let mut seen = HashSet::new();

    for arg in args {
        match arg.name.as_deref() {
            None => match arg.value.as_ref() {
                Some(Expr::Symbol(name)) | Some(Expr::String(name)) => {
                    push_save_name(&mut names, &mut seen, name.clone())?;
                }
                Some(_) => {
                    return Err(RError::new(
                        RErrorKind::Argument,
                        "save() positional arguments must be bare names; use list = c(...) for computed names".to_string(),
                    ));
                }
                None => {
                    return Err(RError::new(
                        RErrorKind::Argument,
                        "save() received an empty argument".to_string(),
                    ));
                }
            },
            Some("file" | "envir") => {}
            Some("list") => {
                let listed = eval_arg_value(arg, env, interp)?;
                if listed.is_null() {
                    continue;
                }

                let listed_names = listed
                    .as_vector()
                    .ok_or_else(|| {
                        RError::new(
                            RErrorKind::Argument,
                            "invalid 'list' argument in save(): expected a character vector of object names"
                                .to_string(),
                        )
                    })?
                    .to_characters();

                for name in listed_names {
                    push_save_name(
                        &mut names,
                        &mut seen,
                        name.ok_or_else(|| {
                            RError::new(
                                RErrorKind::Argument,
                                "invalid 'list' argument in save(): object names cannot be NA"
                                    .to_string(),
                            )
                        })?,
                    )?;
                }
            }
            Some("ascii" | "compress" | "version" | "precheck" | "eval.promises" | "safe") => {}
            Some(name) => {
                return Err(RError::new(
                    RErrorKind::Argument,
                    format!("unsupported argument '{}' in save()", name),
                ));
            }
        }
    }

    if names.is_empty() {
        return Err(RError::new(
            RErrorKind::Argument,
            "save() needs at least one object name".to_string(),
        ));
    }

    Ok(names)
}

// endregion

fn read_rds_path(args: &[RValue], named: &[(String, RValue)]) -> Result<String, RError> {
    CallArgs::new(args, named).string("file", 0)
}

/// Read a single R object from a miniRDS file.
///
/// @param file character scalar: path to the .rds file
/// @return the deserialized R value
#[interpreter_builtin(name = "readRDS", min_args = 1)]
fn interp_read_rds(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let path = read_rds_path(args, named)?;
    read_minirds(&path, "readRDS", "saveRDS", context.interpreter())
}

/// Serialize a single R object to a miniRDS file.
///
/// @param object any R value to serialize
/// @param file character scalar: path to write the .rds file
/// @return NULL (invisibly)
#[builtin(name = "saveRDS", min_args = 2)]
fn builtin_save_rds(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let object = call_args.value("object", 0).ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "argument 'object' is missing".to_string(),
        )
    })?;
    let path = call_args.string("file", 1)?;

    write_minirds(&path, object)?;
    Ok(RValue::Null)
}

/// Load a workspace file (saved with save()) into an environment.
///
/// @param file character scalar: path to the workspace file
/// @param envir environment to load bindings into (default: calling environment)
/// @return character vector of names of loaded objects
#[interpreter_builtin(name = "load", min_args = 1)]
fn interp_load(
    positional: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let path = read_rds_path(positional, named)?;
    let env = context.env();
    let target_env = named
        .iter()
        .find(|(name, _)| name == "envir")
        .map(|(_, value)| value)
        .or_else(|| positional.get(1))
        .map(|value| match value {
            RValue::Environment(target_env) => Ok(target_env.clone()),
            _ => Err(RError::new(
                RErrorKind::Argument,
                "invalid 'envir' argument".to_string(),
            )),
        })
        .transpose()?
        .unwrap_or_else(|| env.clone());

    let value = read_minirds(&path, "load", "save", context.interpreter())?;
    if !is_workspace_value(&value) {
        return Err(RError::new(
            RErrorKind::Argument,
            format!(
                "unsupported load() format in '{}': miniR currently loads only workspace files written by save()",
                path
            ),
        ));
    }

    let RValue::List(list) = value else {
        unreachable!();
    };
    let names = workspace_binding_names(&list)?;
    let loaded_names: Vec<Option<String>> = names.iter().cloned().map(Some).collect();

    for (name, (_, saved_value)) in names.into_iter().zip(list.values.into_iter()) {
        target_env.set(name, saved_value);
    }

    Ok(RValue::vec(Vector::Character(loaded_names.into())))
}

/// Save named R objects to a workspace file in miniRDS format.
///
/// @param ... bare names of objects to save
/// @param list character vector of additional object names
/// @param file character scalar: path to write the workspace file
/// @param envir environment to look up objects in (default: calling environment)
/// @return NULL (invisibly)
#[pre_eval_builtin(name = "save")]
fn pre_eval_save(
    args: &[Arg],
    env: &Environment,
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let interp = context.interpreter();
    let path = workspace_file_arg(args, env, interp)?;
    let target_env = workspace_target_env(args, env, interp)?;
    let requested_names = workspace_requested_names(args, env, interp)?;

    let mut values = Vec::with_capacity(requested_names.len());
    for name in requested_names {
        let value = target_env.get(&name).ok_or_else(|| {
            RError::new(
                RErrorKind::Name,
                format!("object '{}' not found in save()", name),
            )
        })?;
        values.push((Some(name), value));
    }

    let mut workspace = RList::new(values);
    workspace.set_attr("class".to_string(), workspace_class_value());
    write_minirds(&path, &RValue::List(workspace))?;
    Ok(RValue::Null)
}

/// Construct a platform-independent file path from components.
///
/// @param ... character scalars: path components to join
/// @param fsep character scalar: path separator (default "/")
/// @return character scalar containing the joined path
#[builtin]
fn builtin_file_path(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let sep = CallArgs::new(args, named)
        .named_string("fsep")
        .unwrap_or_else(|| "/".to_string());

    let parts: Vec<String> = args
        .iter()
        .filter_map(|v| v.as_vector()?.as_character_scalar())
        .collect();
    Ok(RValue::vec(Vector::Character(
        vec![Some(parts.join(&sep))].into(),
    )))
}

/// Test whether files exist at the given paths.
///
/// @param ... character scalars: file paths to check
/// @return logical vector indicating existence of each file
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

/// Read a CSV file into a data frame.
///
/// @param file character scalar: path to the CSV file
/// @param header logical: does the file have a header row? (default TRUE)
/// @param sep character scalar: field separator (default ",")
/// @return data.frame with columns coerced to numeric where possible
#[builtin(name = "read.csv", min_args = 1)]
fn builtin_read_csv(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let path = call_args.string("file", 0)?;

    let header = call_args.logical_flag("header", usize::MAX, true);

    let sep = call_args
        .named_string("sep")
        .and_then(|s| s.bytes().next())
        .unwrap_or(b',');

    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(header)
        .delimiter(sep)
        .from_path(&path)
        .map_err(|source| IoError::CsvRead {
            context: format!("opening '{}'", path),
            source,
        })?;

    let col_names: Vec<String> = if header {
        rdr.headers()
            .map_err(|source| IoError::CsvRead {
                context: "headers".to_string(),
                source,
            })?
            .iter()
            .map(|s| s.to_string())
            .collect()
    } else {
        // Auto-generate V1, V2, ... column names from first record
        let ncols = rdr
            .records()
            .next()
            .and_then(|r| r.ok())
            .map(|r| r.len())
            .unwrap_or(0);
        (1..=ncols).map(|i| format!("V{}", i)).collect()
    };

    let ncols = col_names.len();
    let mut columns: Vec<Vec<Option<String>>> = vec![vec![]; ncols];
    let mut nrows = 0usize;

    for result in rdr.records() {
        let record = result.map_err(|source| IoError::CsvRead {
            context: "record".to_string(),
            source,
        })?;
        for (i, field) in record.iter().enumerate() {
            if i < ncols {
                if field == "NA" || field.is_empty() {
                    columns[i].push(None);
                } else {
                    columns[i].push(Some(field.to_string()));
                }
            }
        }
        nrows += 1;
    }

    // Try to coerce columns to numeric where possible
    let mut list_cols: Vec<(Option<String>, RValue)> = Vec::new();
    for (i, col_data) in columns.into_iter().enumerate() {
        let name = col_names.get(i).cloned();
        // Try parsing all as doubles
        let all_numeric = col_data.iter().all(|v| match v {
            None => true,
            Some(s) => s.parse::<f64>().is_ok(),
        });
        if all_numeric {
            // Try integer first
            let all_int = col_data.iter().all(|v| match v {
                None => true,
                Some(s) => s.parse::<i64>().is_ok(),
            });
            if all_int {
                let vals: Vec<Option<i64>> =
                    col_data.iter().map(|v| v.as_ref()?.parse().ok()).collect();
                list_cols.push((name, RValue::vec(Vector::Integer(vals.into()))));
            } else {
                let vals: Vec<Option<f64>> =
                    col_data.iter().map(|v| v.as_ref()?.parse().ok()).collect();
                list_cols.push((name, RValue::vec(Vector::Double(vals.into()))));
            }
        } else {
            list_cols.push((name, RValue::vec(Vector::Character(col_data.into()))));
        }
    }

    let mut list = RList::new(list_cols);
    list.set_attr(
        "class".to_string(),
        RValue::vec(Vector::Character(
            vec![Some("data.frame".to_string())].into(),
        )),
    );
    list.set_attr(
        "names".to_string(),
        RValue::vec(Vector::Character(
            col_names.into_iter().map(Some).collect::<Vec<_>>().into(),
        )),
    );
    let row_names: Vec<Option<i64>> = (1..=i64::try_from(nrows)?).map(Some).collect();
    list.set_attr(
        "row.names".to_string(),
        RValue::vec(Vector::Integer(row_names.into())),
    );
    Ok(RValue::List(list))
}

/// Write a data frame to a CSV file.
///
/// @param x data frame or list to write
/// @param file character scalar: output file path
/// @param row.names logical: include row names? (default TRUE)
/// @return NULL (invisibly)
#[builtin(name = "write.csv", min_args = 1)]
fn builtin_write_csv(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let data = args
        .first()
        .ok_or_else(|| RError::new(RErrorKind::Argument, "argument 'x' is missing".to_string()))?;
    let file = args
        .get(1)
        .or_else(|| named.iter().find(|(n, _)| n == "file").map(|(_, v)| v))
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "argument 'file' is missing".to_string(),
            )
        })?;

    let row_names = named
        .iter()
        .find(|(n, _)| n == "row.names")
        .and_then(|(_, v)| v.as_vector()?.as_logical_scalar())
        .unwrap_or(true);

    let RValue::List(list) = data else {
        return Err(RError::new(
            RErrorKind::Argument,
            "write.csv requires a data frame or list".to_string(),
        ));
    };

    let mut wtr = csv::Writer::from_path(&file).map_err(|source| IoError::CsvRead {
        context: format!("opening '{}'", file),
        source,
    })?;

    // Write header
    let col_names: Vec<String> = list
        .values
        .iter()
        .map(|(n, _)| n.clone().unwrap_or_default())
        .collect();

    if row_names {
        let mut header = vec!["".to_string()];
        header.extend(col_names.clone());
        wtr.write_record(&header)
            .map_err(|source| IoError::CsvWrite { source })?;
    } else {
        wtr.write_record(&col_names)
            .map_err(|source| IoError::CsvWrite { source })?;
    }

    // Determine number of rows
    let nrows = list.values.first().map(|(_, v)| v.length()).unwrap_or(0);

    // Write rows
    for row in 0..nrows {
        let mut record: Vec<String> = Vec::new();
        if row_names {
            record.push((row + 1).to_string());
        }
        for (_, col_val) in &list.values {
            if let RValue::Vector(rv) = col_val {
                let chars = rv.to_characters();
                record.push(
                    chars
                        .get(row)
                        .and_then(|v| v.clone())
                        .unwrap_or_else(|| "NA".to_string()),
                );
            } else {
                record.push("NA".to_string());
            }
        }
        wtr.write_record(&record)
            .map_err(|source| IoError::CsvWrite { source })?;
    }

    wtr.flush().map_err(|source| IoError::CsvWrite {
        source: csv::Error::from(source),
    })?;
    Ok(RValue::Null)
}

/// Read data from a file, splitting into tokens.
///
/// @param file character scalar: path to the file to read
/// @param what example value determining the return type (default: character)
/// @param sep character scalar: token separator (default: whitespace)
/// @return vector of tokens coerced to the type of `what`
#[builtin]
fn builtin_scan(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let file = args
        .first()
        .and_then(|v| match v {
            RValue::Vector(rv) => rv.inner.as_character_scalar(),
            _ => None,
        })
        .unwrap_or_default();

    if file.is_empty() {
        return Err(RError::new(
            RErrorKind::Argument,
            "scan() requires a file path — reading from stdin is not yet supported".to_string(),
        ));
    }

    let content = std::fs::read_to_string(&file).map_err(|source| IoError::CannotOpen {
        path: file.clone(),
        source,
    })?;

    // Determine separator
    let sep = named
        .iter()
        .find(|(n, _)| n == "sep")
        .and_then(|(_, v)| match v {
            RValue::Vector(rv) => rv.inner.as_character_scalar(),
            _ => None,
        });

    let tokens: Vec<&str> = match &sep {
        Some(s) if !s.is_empty() => content.split(s.as_str()).collect(),
        _ => content.split_whitespace().collect(),
    };

    // Determine what type to return (default: character)
    let what = args
        .get(1)
        .or_else(|| named.iter().find(|(n, _)| n == "what").map(|(_, v)| v));

    match what {
        Some(RValue::Vector(rv)) => match &rv.inner {
            Vector::Double(_) => {
                let vals: Vec<Option<f64>> = tokens.iter().map(|t| t.parse::<f64>().ok()).collect();
                Ok(RValue::vec(Vector::Double(vals.into())))
            }
            Vector::Integer(_) => {
                let vals: Vec<Option<i64>> = tokens.iter().map(|t| t.parse::<i64>().ok()).collect();
                Ok(RValue::vec(Vector::Integer(vals.into())))
            }
            Vector::Logical(_) => {
                let vals: Vec<Option<bool>> = tokens
                    .iter()
                    .map(|t| match *t {
                        "TRUE" | "T" => Some(true),
                        "FALSE" | "F" => Some(false),
                        _ => None,
                    })
                    .collect();
                Ok(RValue::vec(Vector::Logical(vals.into())))
            }
            _ => {
                let vals: Vec<Option<String>> =
                    tokens.iter().map(|t| Some(t.to_string())).collect();
                Ok(RValue::vec(Vector::Character(vals.into())))
            }
        },
        _ => {
            let vals: Vec<Option<String>> = tokens.iter().map(|t| Some(t.to_string())).collect();
            Ok(RValue::vec(Vector::Character(vals.into())))
        }
    }
}

/// Read a whitespace- or delimiter-separated table from a file.
///
/// @param file character scalar: path to the file
/// @param header logical: does the file have a header row? (default FALSE)
/// @param sep character scalar: field separator (default: whitespace)
/// @return data.frame (list of columns) with columns coerced to numeric where possible
#[builtin(name = "read.table", min_args = 1)]
fn builtin_read_table(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let file = match &args[0] {
        RValue::Vector(rv) => rv.inner.as_character_scalar().ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "read.table() requires a file path string".to_string(),
            )
        })?,
        _ => {
            return Err(RError::new(
                RErrorKind::Argument,
                "read.table() requires a file path string".to_string(),
            ))
        }
    };

    let header = named
        .iter()
        .find(|(n, _)| n == "header")
        .and_then(|(_, v)| match v {
            RValue::Vector(rv) => rv.inner.as_logical_scalar(),
            _ => None,
        })
        .unwrap_or(false);

    let sep = named
        .iter()
        .find(|(n, _)| n == "sep")
        .and_then(|(_, v)| match v {
            RValue::Vector(rv) => rv.inner.as_character_scalar(),
            _ => None,
        })
        .unwrap_or_else(|| "".to_string()); // empty = whitespace

    let content = std::fs::read_to_string(&file).map_err(|source| IoError::CannotOpen {
        path: file.clone(),
        source,
    })?;

    let mut lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        return Ok(RValue::List(RList::new(vec![])));
    }

    // Parse column names from header
    let col_names: Vec<String> = if header {
        let header_line = lines.remove(0);
        split_line(header_line, &sep)
            .iter()
            .map(|s| s.to_string())
            .collect()
    } else {
        vec![]
    };

    // Parse data
    let rows: Vec<Vec<String>> = lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| split_line(l, &sep).iter().map(|s| s.to_string()).collect())
        .collect();

    if rows.is_empty() {
        return Ok(RValue::List(RList::new(vec![])));
    }

    let ncols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    let mut columns: Vec<(Option<String>, RValue)> = Vec::with_capacity(ncols);

    for col_idx in 0..ncols {
        let col_data: Vec<Option<String>> = rows.iter().map(|r| r.get(col_idx).cloned()).collect();

        // Try to detect numeric columns
        let all_numeric = col_data.iter().all(|v| {
            v.as_ref()
                .is_none_or(|s| s.is_empty() || s == "NA" || s.parse::<f64>().is_ok())
        });

        let col_val = if all_numeric {
            let vals: Vec<Option<f64>> = col_data
                .iter()
                .map(|v| {
                    v.as_ref().and_then(|s| {
                        if s == "NA" || s.is_empty() {
                            None
                        } else {
                            s.parse().ok()
                        }
                    })
                })
                .collect();
            RValue::vec(Vector::Double(vals.into()))
        } else {
            RValue::vec(Vector::Character(col_data.into()))
        };

        let name = col_names
            .get(col_idx)
            .cloned()
            .or_else(|| Some(format!("V{}", col_idx + 1)));
        columns.push((name, col_val));
    }

    Ok(RValue::List(RList::new(columns)))
}

/// Write a data frame, list, or matrix to a text file.
///
/// @param x data frame, list, or matrix to write
/// @param file character scalar: output file path
/// @param sep character scalar: field separator (default " ")
/// @param col.names logical: include column names? (default TRUE)
/// @param quote logical: quote character fields? (default TRUE)
/// @return NULL (invisibly)
#[builtin(name = "write.table", min_args = 2)]
fn builtin_write_table(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let file = match &args[1] {
        RValue::Vector(rv) => rv.inner.as_character_scalar().ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "write.table() requires a file path".to_string(),
            )
        })?,
        _ => {
            return Err(RError::new(
                RErrorKind::Argument,
                "write.table() requires a file path as second argument".to_string(),
            ))
        }
    };

    let sep = named
        .iter()
        .find(|(n, _)| n == "sep")
        .and_then(|(_, v)| match v {
            RValue::Vector(rv) => rv.inner.as_character_scalar(),
            _ => None,
        })
        .unwrap_or_else(|| " ".to_string());

    let col_names = named
        .iter()
        .find(|(n, _)| n == "col.names")
        .and_then(|(_, v)| match v {
            RValue::Vector(rv) => rv.inner.as_logical_scalar(),
            _ => None,
        })
        .unwrap_or(true);

    let quote = named
        .iter()
        .find(|(n, _)| n == "quote")
        .and_then(|(_, v)| match v {
            RValue::Vector(rv) => rv.inner.as_logical_scalar(),
            _ => None,
        })
        .unwrap_or(true);

    let mut output = String::new();

    match &args[0] {
        RValue::List(list) => {
            let ncols = list.values.len();
            let nrows = list.values.first().map(|(_, v)| v.length()).unwrap_or(0);

            // Header
            if col_names {
                let names: Vec<String> = list
                    .values
                    .iter()
                    .enumerate()
                    .map(|(i, (name, _))| {
                        let n = name.clone().unwrap_or_else(|| format!("V{}", i + 1));
                        if quote {
                            format!("\"{}\"", n)
                        } else {
                            n
                        }
                    })
                    .collect();
                output.push_str(&names.join(&sep));
                output.push('\n');
            }

            // Rows
            for row_idx in 0..nrows {
                let cells: Vec<String> = (0..ncols)
                    .map(|col_idx| {
                        let (_, val) = &list.values[col_idx];
                        format_cell(val, row_idx, quote)
                    })
                    .collect();
                output.push_str(&cells.join(&sep));
                output.push('\n');
            }
        }
        RValue::Vector(rv) => {
            // Matrix — write rows
            let dim = rv.get_attr("dim");
            match dim {
                Some(RValue::Vector(dim_rv)) => {
                    if let Vector::Integer(d) = &dim_rv.inner {
                        if d.len() >= 2 {
                            let nrow = usize::try_from(d[0].unwrap_or(0))?;
                            let ncol = usize::try_from(d[1].unwrap_or(0))?;
                            for r in 0..nrow {
                                let cells: Vec<String> = (0..ncol)
                                    .map(|c| {
                                        let idx = c * nrow + r;
                                        format_cell(&args[0], idx, quote)
                                    })
                                    .collect();
                                output.push_str(&cells.join(&sep));
                                output.push('\n');
                            }
                        }
                    }
                }
                _ => {
                    // Plain vector — one element per line
                    for i in 0..rv.inner.len() {
                        output.push_str(&format_cell(&args[0], i, quote));
                        output.push('\n');
                    }
                }
            }
        }
        _ => {
            return Err(RError::new(
                RErrorKind::Type,
                "write.table() requires a list or matrix".to_string(),
            ))
        }
    }

    std::fs::write(&file, output).map_err(|source| IoError::WriteFailed {
        path: file.clone(),
        source,
    })?;

    Ok(RValue::Null)
}

/// Split a line by separator (whitespace if empty).
fn split_line<'a>(line: &'a str, sep: &str) -> Vec<&'a str> {
    if sep.is_empty() {
        line.split_whitespace().collect()
    } else {
        line.split(sep).collect()
    }
}

/// Format a single cell from a vector for write.table output.
fn format_cell(val: &RValue, idx: usize, quote: bool) -> String {
    match val {
        RValue::Vector(rv) => match &rv.inner {
            Vector::Raw(v) => v
                .get(idx)
                .map_or("00".to_string(), |b| format!("{:02x}", b)),
            Vector::Double(v) => v
                .get(idx)
                .and_then(|x| *x)
                .map_or("NA".to_string(), |f| format!("{}", f)),
            Vector::Integer(v) => v
                .get(idx)
                .and_then(|x| *x)
                .map_or("NA".to_string(), |i| format!("{}", i)),
            Vector::Logical(v) => v.get(idx).and_then(|x| *x).map_or("NA".to_string(), |b| {
                if b { "TRUE" } else { "FALSE" }.to_string()
            }),
            Vector::Complex(v) => v
                .get(idx)
                .and_then(|x| *x)
                .map_or("NA".to_string(), format_r_complex),
            Vector::Character(v) => {
                v.get(idx)
                    .and_then(|x| x.as_ref())
                    .map_or("NA".to_string(), |s| {
                        if quote {
                            format!("\"{}\"", s)
                        } else {
                            s.clone()
                        }
                    })
            }
        },
        _ => "NA".to_string(),
    }
}
