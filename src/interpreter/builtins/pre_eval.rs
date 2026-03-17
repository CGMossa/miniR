//! Pre-eval builtins — functions that intercept before argument evaluation.
//! Each is auto-registered via `#[pre_eval_builtin]`.
//! The interpreter is accessed via the `BuiltinContext` passed at dispatch time.

use std::collections::BTreeSet;

use crate::interpreter::environment::Environment;
use crate::interpreter::value::*;
use crate::interpreter::BuiltinContext;
use crate::parser::ast::{Arg, Expr};
use itertools::Itertools;
use minir_macros::pre_eval_builtin;

#[derive(Clone)]
struct DataFrameColumn {
    name: String,
    value: RValue,
    row_count: usize,
    row_names: Option<RowNames>,
}

type RowNames = Vec<Option<String>>;

fn is_data_frame_control_arg(name: &str) -> bool {
    matches!(
        name,
        "row.names" | "stringsAsFactors" | "check.rows" | "check.names" | "fix.empty.names"
    )
}

fn sanitize_data_frame_name(source: &str) -> String {
    let mut out = String::new();
    for ch in source.chars() {
        if ch.is_ascii_alphanumeric() || ch == '.' || ch == '_' {
            out.push(ch);
        } else {
            out.push('.');
        }
    }

    if out.is_empty() || out == "." {
        out = "X".to_string();
    }

    if out
        .chars()
        .next()
        .is_some_and(|ch| !(ch.is_ascii_alphabetic() || ch == '.'))
    {
        out.insert(0, 'X');
    }

    out
}

fn default_data_frame_name(expr: Option<&Expr>, index: usize) -> String {
    expr.map(|expr| sanitize_data_frame_name(&deparse_expr(expr)))
        .unwrap_or_else(|| format!("V{}", index))
}

fn row_names_to_strings(value: &RValue) -> Option<RowNames> {
    match value {
        RValue::Vector(rv) => match &rv.inner {
            Vector::Character(values) => Some(values.to_vec()),
            Vector::Integer(values) => {
                Some(values.iter().map(|v| v.map(|v| v.to_string())).collect())
            }
            Vector::Double(values) => Some(values.iter().map(|v| v.map(format_r_double)).collect()),
            _ => None,
        },
        _ => None,
    }
}

fn vector_names(rv: &RVector) -> Option<RowNames> {
    rv.get_attr("names").and_then(row_names_to_strings)
}

fn expr_vector_names(expr: &Expr) -> Option<RowNames> {
    let Expr::Call { func, args } = expr else {
        return None;
    };
    let Expr::Symbol(name) = func.as_ref() else {
        return None;
    };
    if name != "c" || !args.iter().any(|arg| arg.name.is_some()) {
        return None;
    }
    Some(args.iter().map(|arg| arg.name.clone()).collect())
}

fn matrix_dimnames(rv: &RVector) -> (Option<RowNames>, Option<RowNames>) {
    let Some(RValue::List(dimnames)) = rv.get_attr("dimnames") else {
        return (None, None);
    };

    let row_names = dimnames
        .values
        .first()
        .and_then(|(_, value)| row_names_to_strings(value));
    let col_names = dimnames
        .values
        .get(1)
        .and_then(|(_, value)| row_names_to_strings(value));

    (row_names, col_names)
}

fn factorize_character_vector(values: Vec<Option<String>>) -> Result<RValue, RError> {
    let levels: Vec<String> = values.iter().flatten().unique().sorted().cloned().collect();

    let codes: Vec<Option<i64>> = values
        .iter()
        .map(|value| match value {
            Some(value) => levels
                .iter()
                .position(|level| level == value)
                .map(|idx| i64::try_from(idx + 1))
                .transpose(),
            None => Ok(None),
        })
        .collect::<Result<_, _>>()?;

    let mut rv = RVector::from(Vector::Integer(codes.into()));
    rv.set_attr(
        "levels".to_string(),
        RValue::vec(Vector::Character(
            levels.into_iter().map(Some).collect::<Vec<_>>().into(),
        )),
    );
    rv.set_attr(
        "class".to_string(),
        RValue::vec(Vector::Character(vec![Some("factor".to_string())].into())),
    );
    Ok(RValue::Vector(rv))
}

fn maybe_factorize_strings(value: RValue, strings_as_factors: bool) -> Result<RValue, RError> {
    if !strings_as_factors {
        return Ok(value);
    }

    match value {
        RValue::Vector(rv)
            if matches!(rv.inner, Vector::Character(_)) && rv.get_attr("class").is_none() =>
        {
            let Vector::Character(values) = rv.inner else {
                unreachable!();
            };
            factorize_character_vector(values.to_vec())
        }
        other => Ok(other),
    }
}

fn strip_names_attr(value: &mut RValue) {
    match value {
        RValue::Vector(rv) => {
            rv.attrs.as_mut().map(|attrs| attrs.shift_remove("names"));
        }
        RValue::List(list) => {
            list.attrs.as_mut().map(|attrs| attrs.shift_remove("names"));
        }
        _ => {}
    }
}

fn recycle_value(value: &RValue, target_len: usize) -> Result<RValue, RError> {
    match value {
        RValue::Vector(rv) => {
            let mut recycled = rv.clone();
            recycled.inner = match &rv.inner {
                Vector::Raw(values) => Vector::Raw(
                    (0..target_len)
                        .map(|idx| values[idx % values.len()])
                        .collect::<Vec<_>>(),
                ),
                Vector::Logical(values) => Vector::Logical(
                    (0..target_len)
                        .map(|idx| values[idx % values.len()])
                        .collect::<Vec<_>>()
                        .into(),
                ),
                Vector::Integer(values) => Vector::Integer(
                    (0..target_len)
                        .map(|idx| values[idx % values.len()])
                        .collect::<Vec<_>>()
                        .into(),
                ),
                Vector::Double(values) => Vector::Double(
                    (0..target_len)
                        .map(|idx| values[idx % values.len()])
                        .collect::<Vec<_>>()
                        .into(),
                ),
                Vector::Complex(values) => Vector::Complex(
                    (0..target_len)
                        .map(|idx| values[idx % values.len()])
                        .collect::<Vec<_>>()
                        .into(),
                ),
                Vector::Character(values) => Vector::Character(
                    (0..target_len)
                        .map(|idx| values[idx % values.len()].clone())
                        .collect::<Vec<_>>()
                        .into(),
                ),
            };
            Ok(RValue::Vector(recycled))
        }
        RValue::List(list) => {
            let mut recycled = list.clone();
            recycled.values = (0..target_len)
                .map(|idx| {
                    let (name, value) = &list.values[idx % list.values.len()];
                    (name.clone(), value.clone())
                })
                .collect();
            Ok(RValue::List(recycled))
        }
        other if target_len == 1 => Ok(other.clone()),
        other if other.length() == 1 => Ok(other.clone()),
        other => Err(RError::other(format!(
            "cannot recycle {} to {} rows",
            other.type_name(),
            target_len
        ))),
    }
}

fn matrix_columns(
    rv: &RVector,
    explicit_name: Option<&str>,
) -> Result<Vec<DataFrameColumn>, RError> {
    let Some(dims) = super::get_dim_ints(rv.get_attr("dim")) else {
        return Ok(Vec::new());
    };
    if dims.len() < 2 {
        return Ok(Vec::new());
    }

    let nrow = usize::try_from(dims[0].unwrap_or(0))?;
    let ncol = usize::try_from(dims[1].unwrap_or(0))?;
    let (row_names, col_names) = matrix_dimnames(rv);

    let columns = match &rv.inner {
        Vector::Raw(values) => (0..ncol)
            .map(|col_idx| {
                let start = col_idx * nrow;
                DataFrameColumn {
                    name: match (
                        explicit_name,
                        col_names
                            .as_ref()
                            .and_then(|n| n.get(col_idx))
                            .cloned()
                            .flatten(),
                    ) {
                        (Some(prefix), Some(name)) => format!("{}.{}", prefix, name),
                        (Some(prefix), None) => format!("{}.{}", prefix, col_idx + 1),
                        (None, Some(name)) => name,
                        (None, None) => format!("X{}", col_idx + 1),
                    },
                    value: RValue::vec(Vector::Raw(values[start..start + nrow].to_vec())),
                    row_count: nrow,
                    row_names: row_names.clone(),
                }
            })
            .collect(),
        Vector::Logical(values) => (0..ncol)
            .map(|col_idx| {
                let start = col_idx * nrow;
                DataFrameColumn {
                    name: match (
                        explicit_name,
                        col_names
                            .as_ref()
                            .and_then(|n| n.get(col_idx))
                            .cloned()
                            .flatten(),
                    ) {
                        (Some(prefix), Some(name)) => format!("{}.{}", prefix, name),
                        (Some(prefix), None) => format!("{}.{}", prefix, col_idx + 1),
                        (None, Some(name)) => name,
                        (None, None) => format!("X{}", col_idx + 1),
                    },
                    value: RValue::vec(Vector::Logical(
                        values[start..start + nrow].to_vec().into(),
                    )),
                    row_count: nrow,
                    row_names: row_names.clone(),
                }
            })
            .collect(),
        Vector::Integer(values) => (0..ncol)
            .map(|col_idx| {
                let start = col_idx * nrow;
                DataFrameColumn {
                    name: match (
                        explicit_name,
                        col_names
                            .as_ref()
                            .and_then(|n| n.get(col_idx))
                            .cloned()
                            .flatten(),
                    ) {
                        (Some(prefix), Some(name)) => format!("{}.{}", prefix, name),
                        (Some(prefix), None) => format!("{}.{}", prefix, col_idx + 1),
                        (None, Some(name)) => name,
                        (None, None) => format!("X{}", col_idx + 1),
                    },
                    value: RValue::vec(Vector::Integer(
                        values[start..start + nrow].to_vec().into(),
                    )),
                    row_count: nrow,
                    row_names: row_names.clone(),
                }
            })
            .collect(),
        Vector::Double(values) => (0..ncol)
            .map(|col_idx| {
                let start = col_idx * nrow;
                DataFrameColumn {
                    name: match (
                        explicit_name,
                        col_names
                            .as_ref()
                            .and_then(|n| n.get(col_idx))
                            .cloned()
                            .flatten(),
                    ) {
                        (Some(prefix), Some(name)) => format!("{}.{}", prefix, name),
                        (Some(prefix), None) => format!("{}.{}", prefix, col_idx + 1),
                        (None, Some(name)) => name,
                        (None, None) => format!("X{}", col_idx + 1),
                    },
                    value: RValue::vec(Vector::Double(values[start..start + nrow].to_vec().into())),
                    row_count: nrow,
                    row_names: row_names.clone(),
                }
            })
            .collect(),
        Vector::Complex(values) => (0..ncol)
            .map(|col_idx| {
                let start = col_idx * nrow;
                DataFrameColumn {
                    name: match (
                        explicit_name,
                        col_names
                            .as_ref()
                            .and_then(|n| n.get(col_idx))
                            .cloned()
                            .flatten(),
                    ) {
                        (Some(prefix), Some(name)) => format!("{}.{}", prefix, name),
                        (Some(prefix), None) => format!("{}.{}", prefix, col_idx + 1),
                        (None, Some(name)) => name,
                        (None, None) => format!("X{}", col_idx + 1),
                    },
                    value: RValue::vec(Vector::Complex(
                        values[start..start + nrow].to_vec().into(),
                    )),
                    row_count: nrow,
                    row_names: row_names.clone(),
                }
            })
            .collect(),
        Vector::Character(values) => (0..ncol)
            .map(|col_idx| {
                let start = col_idx * nrow;
                DataFrameColumn {
                    name: match (
                        explicit_name,
                        col_names
                            .as_ref()
                            .and_then(|n| n.get(col_idx))
                            .cloned()
                            .flatten(),
                    ) {
                        (Some(prefix), Some(name)) => format!("{}.{}", prefix, name),
                        (Some(prefix), None) => format!("{}.{}", prefix, col_idx + 1),
                        (None, Some(name)) => name,
                        (None, None) => format!("X{}", col_idx + 1),
                    },
                    value: RValue::vec(Vector::Character(
                        values[start..start + nrow].to_vec().into(),
                    )),
                    row_count: nrow,
                    row_names: row_names.clone(),
                }
            })
            .collect(),
    };

    Ok(columns)
}

fn expand_data_frame_value(
    value: &RValue,
    explicit_name: Option<&str>,
    default_name: &str,
    fallback_row_names: Option<RowNames>,
    strings_as_factors: bool,
) -> Result<Vec<DataFrameColumn>, RError> {
    match value {
        RValue::Null => Ok(Vec::new()),
        RValue::List(list) => {
            let source_row_names = if get_class(value)
                .iter()
                .any(|class_name| class_name == "data.frame")
            {
                list.get_attr("row.names").and_then(row_names_to_strings)
            } else {
                None
            };

            let mut columns = Vec::new();
            for (idx, (name, column_value)) in list.values.iter().enumerate() {
                let column_name = match (explicit_name, name.as_deref()) {
                    (Some(prefix), Some(name)) => format!("{}.{}", prefix, name),
                    (Some(prefix), None) => format!("{}.{}", prefix, idx + 1),
                    (None, Some(name)) => name.to_string(),
                    (None, None) => format!("{}.{}", default_name, idx + 1),
                };
                let row_names = source_row_names.clone().or_else(|| match column_value {
                    RValue::Vector(rv) => vector_names(rv),
                    _ => None,
                });
                let value = maybe_factorize_strings(column_value.clone(), strings_as_factors)?;
                columns.push(DataFrameColumn {
                    name: column_name,
                    row_count: column_value.length(),
                    value,
                    row_names,
                });
            }
            Ok(columns)
        }
        RValue::Vector(rv) if super::get_dim_ints(rv.get_attr("dim")).is_some() => {
            let mut columns = matrix_columns(rv, explicit_name)?;
            for column in &mut columns {
                column.value = maybe_factorize_strings(column.value.clone(), strings_as_factors)?;
            }
            Ok(columns)
        }
        _ => Ok(vec![DataFrameColumn {
            name: explicit_name.unwrap_or(default_name).to_string(),
            row_count: value.length(),
            row_names: match value {
                RValue::Vector(rv) => vector_names(rv).or(fallback_row_names),
                _ => None,
            },
            value: maybe_factorize_strings(value.clone(), strings_as_factors)?,
        }]),
    }
}

fn automatic_row_names(count: usize) -> RValue {
    RValue::vec(Vector::Integer(
        (1..=i64::try_from(count).unwrap_or(0))
            .map(Some)
            .collect::<Vec<_>>()
            .into(),
    ))
}

/// Construct a data frame from named or unnamed column vectors.
///
/// @param ... vectors, factors, matrices, or data frames to combine as columns
/// @param row.names character or integer vector of row names (optional)
/// @param stringsAsFactors if TRUE, convert character columns to factors (default FALSE)
/// @return a data frame (list with class "data.frame")
#[pre_eval_builtin(name = "data.frame")]
fn pre_eval_data_frame(
    args: &[Arg],
    env: &Environment,
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let mut explicit_row_names = None;
    let mut strings_as_factors = false;
    let mut columns = Vec::new();
    let mut unnamed_index = 0usize;

    context.with_interpreter(|interp| {
        for arg in args {
            let Some(name) = arg.name.as_deref() else {
                continue;
            };
            if !is_data_frame_control_arg(name) {
                continue;
            }
            let Some(expr) = arg.value.as_ref() else {
                continue;
            };
            let value = interp.eval_in(expr, env).map_err(RError::from)?;
            match name {
                "row.names" => explicit_row_names = Some(value),
                "stringsAsFactors" => {
                    strings_as_factors = value
                        .as_vector()
                        .and_then(Vector::as_logical_scalar)
                        .unwrap_or(false);
                }
                _ => {}
            }
        }

        for arg in args {
            let Some(expr) = arg.value.as_ref() else {
                continue;
            };
            if arg.name.as_deref().is_some_and(is_data_frame_control_arg) {
                continue;
            }

            unnamed_index += 1;
            let value = interp.eval_in(expr, env).map_err(RError::from)?;
            let default_name = default_data_frame_name(
                if arg.name.is_none() { Some(expr) } else { None },
                unnamed_index,
            );
            columns.extend(expand_data_frame_value(
                &value,
                arg.name.as_deref(),
                &default_name,
                expr_vector_names(expr),
                strings_as_factors,
            )?);
        }
        Ok::<(), RError>(())
    })?;

    let mut lengths = BTreeSet::new();
    for column in &columns {
        lengths.insert(column.row_count);
    }

    let target_rows = match explicit_row_names.as_ref() {
        Some(RValue::Null) => lengths.iter().copied().max().unwrap_or(0),
        Some(value) => value.length(),
        None => lengths.iter().copied().max().unwrap_or(0),
    };

    if let Some(row_names) = explicit_row_names.as_ref() {
        if !matches!(row_names, RValue::Null) && !columns.is_empty() {
            let valid = columns.iter().all(|column| column.row_count == target_rows);
            if !valid {
                return Err(RError::other(
                    "row names supplied are of the wrong length".to_string(),
                ));
            }
        }
    }

    let invalid_lengths: Vec<usize> = columns
        .iter()
        .filter_map(|column| {
            if column.row_count == target_rows {
                None
            } else if column.row_count == 0 || target_rows % column.row_count != 0 {
                Some(column.row_count)
            } else {
                None
            }
        })
        .collect();

    if !invalid_lengths.is_empty() {
        let mut all_lengths = lengths;
        all_lengths.insert(target_rows);
        return Err(RError::other(format!(
            "arguments imply differing number of rows: {}",
            all_lengths
                .iter()
                .map(|length| length.to_string())
                .join(", ")
        )));
    }

    let row_names_attr = match explicit_row_names {
        Some(RValue::Null) => automatic_row_names(target_rows),
        Some(value) => value,
        None => columns
            .iter()
            .find(|column| column.row_count == target_rows)
            .and_then(|column| column.row_names.clone())
            .map(|names| RValue::vec(Vector::Character(names.into())))
            .unwrap_or_else(|| automatic_row_names(target_rows)),
    };

    let mut output_columns = Vec::new();
    for mut column in columns {
        if column.row_count != target_rows {
            column.value = recycle_value(&column.value, target_rows)?;
        }
        strip_names_attr(&mut column.value);
        output_columns.push((Some(column.name), column.value));
    }

    let mut list = RList::new(output_columns);
    let names: Vec<Option<String>> = list.values.iter().map(|(name, _)| name.clone()).collect();
    list.set_attr(
        "class".to_string(),
        RValue::vec(Vector::Character(
            vec![Some("data.frame".to_string())].into(),
        )),
    );
    list.set_attr(
        "names".to_string(),
        RValue::vec(Vector::Character(names.into())),
    );
    list.set_attr("row.names".to_string(), row_names_attr);
    Ok(RValue::List(list))
}

/// Evaluate an expression with error/warning/message handlers.
///
/// @param expr the expression to evaluate
/// @param error handler function for error conditions (optional)
/// @param warning handler function for warning conditions (optional)
/// @param message handler function for message conditions (optional)
/// @param finally expression to evaluate after expr, regardless of outcome (optional)
/// @return the result of expr, or the return value of the matching handler
#[pre_eval_builtin(name = "tryCatch", min_args = 1)]
fn pre_eval_try_catch(
    args: &[Arg],
    env: &Environment,
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    use crate::interpreter::ConditionHandler;

    // First unnamed arg is the expression to evaluate
    let expr = args
        .iter()
        .find(|a| a.name.is_none())
        .and_then(|a| a.value.as_ref());

    // Collect named handlers and finally expression
    let mut handlers: Vec<(String, RValue)> = Vec::new();
    let mut finally_expr = None;
    context.with_interpreter(|interp| {
        for arg in args {
            match arg.name.as_deref() {
                Some("finally") => {
                    finally_expr = arg.value.clone();
                }
                Some(class) => {
                    if let Some(ref val_expr) = arg.value {
                        let handler = interp.eval_in(val_expr, env)?;
                        handlers.push((class.to_string(), handler));
                    }
                }
                None => {} // the expression itself
            }
        }
        Ok::<(), RError>(())
    })?;

    // For non-error classes (warning, message, etc.), install withCallingHandlers-style
    // handlers that convert them to unwinding RError::Condition so tryCatch can catch them.
    let non_error_classes: Vec<String> = handlers
        .iter()
        .filter(|(c, _)| c != "error")
        .map(|(c, _)| c.clone())
        .collect();

    let unwind_handlers: Vec<ConditionHandler> = non_error_classes
        .iter()
        .map(|class| ConditionHandler {
            class: class.clone(),
            handler: RValue::Function(RFunction::Builtin {
                name: "tryCatch_unwinder".to_string(),
                implementation: BuiltinImplementation::Eager(|args, _named| {
                    // Re-raise the condition to unwind past tryCatch
                    let condition = args.first().cloned().unwrap_or(RValue::Null);
                    let cond_classes = get_class(&condition);
                    let kind = if cond_classes.iter().any(|c| c == "warning") {
                        ConditionKind::Warning
                    } else if cond_classes.iter().any(|c| c == "message") {
                        ConditionKind::Message
                    } else {
                        ConditionKind::Error
                    };
                    Err(RError::Condition { condition, kind })
                }),
                min_args: 0,
                max_args: None,
            }),
            env: env.clone(),
        })
        .collect();

    // Install non-error handlers if any, then evaluate
    let result = context.with_interpreter(|interp| {
        if !unwind_handlers.is_empty() {
            interp.condition_handlers.borrow_mut().push(unwind_handlers);
        }
        let eval_result = match expr {
            Some(e) => interp.eval_in(e, env).map_err(RError::from),
            None => Ok(RValue::Null),
        };
        if !non_error_classes.is_empty() {
            interp.condition_handlers.borrow_mut().pop();
        }

        match eval_result {
            Ok(val) => Ok(val),
            Err(RError::Condition { condition, kind }) => {
                // Match against handler classes
                let cond_classes = get_class(&condition);
                for (handler_class, handler) in &handlers {
                    if cond_classes.iter().any(|c| c == handler_class) {
                        return interp
                            .call_function(handler, std::slice::from_ref(&condition), &[], env)
                            .map_err(RError::from);
                    }
                }
                // No matching handler — re-raise
                Err(RError::Condition { condition, kind })
            }
            Err(other) => {
                // Non-condition errors: check for "error" handler
                if let Some((_, handler)) = handlers.iter().find(|(c, _)| c == "error") {
                    let err_msg = format!("{}", other);
                    let condition =
                        make_condition(&err_msg, &["simpleError", "error", "condition"]);
                    interp
                        .call_function(handler, &[condition], &[], env)
                        .map_err(RError::from)
                } else {
                    Err(other)
                }
            }
        }
    });

    // Run finally block if present
    if let Some(ref fin) = finally_expr {
        context.with_interpreter(|interp| interp.eval_in(fin, env).map_err(RError::from))?;
    }

    result
}

/// Evaluate an expression, catching errors and returning them as a string.
///
/// @param expr the expression to evaluate
/// @return the result of expr on success, or the error message as a character string
#[pre_eval_builtin(name = "try", min_args = 1)]
fn pre_eval_try(
    args: &[Arg],
    env: &Environment,
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let expr = args
        .iter()
        .find(|a| a.name.is_none())
        .and_then(|a| a.value.as_ref());
    context.with_interpreter(|interp| match expr {
        Some(e) => match interp.eval_in(e, env).map_err(RError::from) {
            Ok(val) => Ok(val),
            Err(err) => {
                let msg = format!("{}", err);
                eprintln!("Error in try : {}", msg);
                Ok(RValue::vec(Vector::Character(vec![Some(msg)].into())))
            }
        },
        None => Ok(RValue::Null),
    })
}

/// Evaluate an expression with calling handlers for conditions.
///
/// Unlike tryCatch, handlers run without unwinding the call stack, allowing
/// the signaling code to resume execution after the handler returns.
///
/// @param expr the expression to evaluate
/// @param ... named handlers: class = handler_function
/// @return the result of expr
#[pre_eval_builtin(name = "withCallingHandlers", min_args = 1)]
fn pre_eval_with_calling_handlers(
    args: &[Arg],
    env: &Environment,
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    use crate::interpreter::ConditionHandler;

    let expr = args
        .iter()
        .find(|a| a.name.is_none())
        .and_then(|a| a.value.as_ref());

    // Collect named handlers (class = handler_function)
    let mut handler_set: Vec<ConditionHandler> = Vec::new();
    context.with_interpreter(|interp| {
        for arg in args {
            if let Some(class) = &arg.name {
                if let Some(ref val_expr) = arg.value {
                    let handler = interp.eval_in(val_expr, env).map_err(RError::from)?;
                    handler_set.push(ConditionHandler {
                        class: class.clone(),
                        handler,
                        env: env.clone(),
                    });
                }
            }
        }
        Ok::<(), RError>(())
    })?;

    // Push handler set onto the stack, evaluate, then pop
    context.with_interpreter(|interp| {
        interp.condition_handlers.borrow_mut().push(handler_set);
        let result = match expr {
            Some(e) => interp.eval_in(e, env).map_err(RError::from),
            None => Ok(RValue::Null),
        };
        interp.condition_handlers.borrow_mut().pop();
        result
    })
}

/// Evaluate an expression, suppressing all warning conditions.
///
/// @param expr the expression to evaluate
/// @return the result of expr with warnings silenced
#[pre_eval_builtin(name = "suppressWarnings", min_args = 1)]
fn pre_eval_suppress_warnings(
    args: &[Arg],
    env: &Environment,
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    use crate::interpreter::ConditionHandler;

    let expr = args
        .first()
        .and_then(|a| a.value.as_ref())
        .ok_or_else(|| RError::new(RErrorKind::Argument, "argument is missing".to_string()))?;

    // Create a handler that muffles warnings by signaling muffleWarning
    let muffle_handler = RValue::Function(RFunction::Builtin {
        name: "suppressWarnings_handler".to_string(),
        implementation: BuiltinImplementation::Eager(|_args, _named| {
            Err(RError::other("muffleWarning".to_string()))
        }),
        min_args: 0,
        max_args: None,
    });

    let handler_set = vec![ConditionHandler {
        class: "warning".to_string(),
        handler: muffle_handler,
        env: env.clone(),
    }];

    context.with_interpreter(|interp| {
        interp.condition_handlers.borrow_mut().push(handler_set);
        let result = interp.eval_in(expr, env).map_err(RError::from);
        interp.condition_handlers.borrow_mut().pop();
        result
    })
}

/// Evaluate an expression, suppressing all message conditions.
///
/// @param expr the expression to evaluate
/// @return the result of expr with messages silenced
#[pre_eval_builtin(name = "suppressMessages", min_args = 1)]
fn pre_eval_suppress_messages(
    args: &[Arg],
    env: &Environment,
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    use crate::interpreter::ConditionHandler;

    let expr = args
        .first()
        .and_then(|a| a.value.as_ref())
        .ok_or_else(|| RError::new(RErrorKind::Argument, "argument is missing".to_string()))?;

    let muffle_handler = RValue::Function(RFunction::Builtin {
        name: "suppressMessages_handler".to_string(),
        implementation: BuiltinImplementation::Eager(|_args, _named| {
            Err(RError::other("muffleMessage".to_string()))
        }),
        min_args: 0,
        max_args: None,
    });

    let handler_set = vec![ConditionHandler {
        class: "message".to_string(),
        handler: muffle_handler,
        env: env.clone(),
    }];

    context.with_interpreter(|interp| {
        interp.condition_handlers.borrow_mut().push(handler_set);
        let result = interp.eval_in(expr, env).map_err(RError::from);
        interp.condition_handlers.borrow_mut().pop();
        result
    })
}

/// Register an expression to be evaluated when the current function exits.
///
/// @param expr expression to evaluate on exit (or NULL to clear)
/// @param add if TRUE, append to existing on.exit expressions; if FALSE, replace them
/// @param after if TRUE (default), append after existing; if FALSE, prepend before existing
/// @return NULL, invisibly
#[pre_eval_builtin(name = "on.exit")]
fn pre_eval_on_exit(
    args: &[Arg],
    env: &Environment,
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let expr = args.first().and_then(|a| a.value.as_ref()).cloned();

    // Evaluate add= and after= arguments
    let (add, after) = context.with_interpreter(|interp| -> Result<(bool, bool), RError> {
        let mut add = false;
        let mut after = true;

        for arg in args.iter().skip(1) {
            match arg.name.as_deref() {
                Some("add") => {
                    if let Some(ref val_expr) = arg.value {
                        let val = interp.eval_in(val_expr, env)?;
                        add = val
                            .as_vector()
                            .and_then(|v| v.as_logical_scalar())
                            .unwrap_or(false);
                    }
                }
                Some("after") => {
                    if let Some(ref val_expr) = arg.value {
                        let val = interp.eval_in(val_expr, env)?;
                        after = val
                            .as_vector()
                            .and_then(|v| v.as_logical_scalar())
                            .unwrap_or(true);
                    }
                }
                _ => {}
            }
        }

        // Check positional args if named args were not found
        let has_named_add = args
            .iter()
            .skip(1)
            .any(|a| a.name.as_deref() == Some("add"));
        let has_named_after = args
            .iter()
            .skip(1)
            .any(|a| a.name.as_deref() == Some("after"));

        if !has_named_add {
            if let Some(arg) = args.get(1) {
                if arg.name.is_none() {
                    if let Some(ref val_expr) = arg.value {
                        let val = interp.eval_in(val_expr, env)?;
                        add = val
                            .as_vector()
                            .and_then(|v| v.as_logical_scalar())
                            .unwrap_or(false);
                    }
                }
            }
        }

        if !has_named_after {
            if let Some(arg) = args.get(2) {
                if arg.name.is_none() {
                    if let Some(ref val_expr) = arg.value {
                        let val = interp.eval_in(val_expr, env)?;
                        after = val
                            .as_vector()
                            .and_then(|v| v.as_logical_scalar())
                            .unwrap_or(true);
                    }
                }
            }
        }

        Ok((add, after))
    })?;

    match expr {
        Some(e) => env.push_on_exit(e, add, after),
        None => {
            // on.exit() with no args clears on.exit handlers
            env.take_on_exit();
        }
    }

    Ok(RValue::Null)
}

/// Test whether a formal argument was supplied in the current function call.
///
/// @param x unquoted name of a formal argument
/// @return TRUE if the argument was not supplied, FALSE otherwise
#[pre_eval_builtin(name = "missing", min_args = 1)]
fn pre_eval_missing(
    args: &[Arg],
    _env: &Environment,
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let expr = args
        .first()
        .and_then(|a| a.value.as_ref())
        .ok_or_else(|| RError::other("'missing(x)' did not find an argument".to_string()))?;

    let is_missing = context.with_interpreter(|interp| {
        let frame = interp
            .current_call_frame()
            .ok_or_else(|| RError::other("'missing(x)' did not find an argument".to_string()))?;

        match expr {
            Expr::Symbol(name) => {
                if !frame.formal_args.contains(name) {
                    return Err(RError::other(format!(
                        "'missing({})' did not find an argument",
                        name
                    )));
                }
                Ok(!frame.supplied_args.contains(name))
            }
            Expr::Dots => {
                if !frame.formal_args.contains("...") {
                    return Err(RError::other("'missing(...)' did not find an argument"));
                }
                let dots_len = match frame.env.get("...") {
                    Some(RValue::List(list)) => list.values.len(),
                    _ => 0,
                };
                Ok(dots_len == 0)
            }
            Expr::DotDot(n) => {
                if !frame.formal_args.contains("...") {
                    return Err(RError::other("'missing(...)' did not find an argument"));
                }
                let dots_len = match frame.env.get("...") {
                    Some(RValue::List(list)) => list.values.len(),
                    _ => 0,
                };
                Ok(dots_len < usize::try_from(*n).unwrap_or(0))
            }
            _ => Err(RError::other("invalid use of 'missing'".to_string())),
        }
    })?;

    Ok(RValue::vec(Vector::Logical(vec![Some(is_missing)].into())))
}

/// Return an unevaluated expression (language object).
///
/// @param expr any R expression (not evaluated)
/// @return the expression as a language object
#[pre_eval_builtin(name = "quote", min_args = 1)]
fn pre_eval_quote(
    args: &[Arg],
    _env: &Environment,
    _context: &BuiltinContext,
) -> Result<RValue, RError> {
    match args.first().and_then(|a| a.value.as_ref()) {
        Some(expr) => Ok(RValue::Language(Language::new(expr.clone()))),
        None => Ok(RValue::Null),
    }
}

/// Return an unevaluated expression with variables substituted from the environment.
///
/// @param expr any R expression (not evaluated)
/// @return the expression with symbols replaced by their values from the calling environment
#[pre_eval_builtin(name = "substitute", min_args = 1)]
fn pre_eval_substitute(
    args: &[Arg],
    env: &Environment,
    _context: &BuiltinContext,
) -> Result<RValue, RError> {
    let expr = match args.first().and_then(|a| a.value.as_ref()) {
        Some(e) => e.clone(),
        None => return Ok(RValue::Null),
    };
    // Walk the AST and replace symbols with their values from the environment
    let substituted = substitute_expr(&expr, env);
    Ok(RValue::Language(Language::new(substituted)))
}

/// Walk an AST, replacing symbols with their values from the environment.
/// If a symbol is bound to an RValue::Language, splice in the inner Expr.
/// If bound to a literal value, convert to the appropriate Expr literal.
fn substitute_expr(expr: &Expr, env: &Environment) -> Expr {
    match expr {
        Expr::Symbol(name) => {
            if let Some(val) = env.get(name) {
                rvalue_to_expr(&val)
            } else {
                expr.clone()
            }
        }
        Expr::Call { func, args } => Expr::Call {
            func: Box::new(substitute_expr(func, env)),
            args: args
                .iter()
                .map(|a| Arg {
                    name: a.name.clone(),
                    value: a.value.as_ref().map(|v| substitute_expr(v, env)),
                })
                .collect(),
        },
        Expr::BinaryOp { op, lhs, rhs } => Expr::BinaryOp {
            op: *op,
            lhs: Box::new(substitute_expr(lhs, env)),
            rhs: Box::new(substitute_expr(rhs, env)),
        },
        Expr::UnaryOp { op, operand } => Expr::UnaryOp {
            op: *op,
            operand: Box::new(substitute_expr(operand, env)),
        },
        Expr::If {
            condition,
            then_body,
            else_body,
        } => Expr::If {
            condition: Box::new(substitute_expr(condition, env)),
            then_body: Box::new(substitute_expr(then_body, env)),
            else_body: else_body
                .as_ref()
                .map(|e| Box::new(substitute_expr(e, env))),
        },
        Expr::Block(exprs) => Expr::Block(exprs.iter().map(|e| substitute_expr(e, env)).collect()),
        // For other AST nodes, return as-is (can expand later)
        _ => expr.clone(),
    }
}

/// Evaluate a quoted expression in a specified environment.
///
/// Equivalent to eval(quote(expr), envir). The expression is not evaluated
/// in the calling environment before being passed to eval.
///
/// @param expr expression to evaluate (quoted, not evaluated first)
/// @param envir environment in which to evaluate (default: calling environment)
/// @return the result of evaluating expr in envir
#[pre_eval_builtin(name = "evalq", min_args = 1)]
fn pre_eval_evalq(
    args: &[Arg],
    env: &Environment,
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    // evalq(expr, envir) is equivalent to eval(quote(expr), envir)
    // First arg is the expression to quote-then-eval
    let expr = match args.first().and_then(|a| a.value.as_ref()) {
        Some(e) => e,
        None => return Ok(RValue::Null),
    };

    // Determine evaluation environment from second arg or named envir=
    let eval_env = context.with_interpreter(|interp| -> Result<Option<Environment>, RError> {
        // Check named envir= first
        for arg in args.iter().skip(1) {
            if arg.name.as_deref() == Some("envir") {
                if let Some(ref val_expr) = arg.value {
                    let val = interp.eval_in(val_expr, env)?;
                    if let RValue::Environment(e) = val {
                        return Ok(Some(e));
                    }
                }
            }
        }
        // Check second positional arg
        if let Some(arg) = args.get(1) {
            if arg.name.is_none() {
                if let Some(ref val_expr) = arg.value {
                    let val = interp.eval_in(val_expr, env)?;
                    if let RValue::Environment(e) = val {
                        return Ok(Some(e));
                    }
                }
            }
        }
        Ok(None)
    })?;

    let target_env = eval_env.unwrap_or_else(|| env.clone());
    context
        .with_interpreter(|interp| interp.eval_in(expr, &target_env))
        .map_err(RError::from)
}

/// Partial substitution: quote an expression, evaluating only .() splices.
///
/// @param expr expression to quote (not evaluated, except for .() sub-expressions)
/// @return a language object with .() splices replaced by their evaluated values
#[pre_eval_builtin(name = "bquote", min_args = 1)]
fn pre_eval_bquote(
    args: &[Arg],
    env: &Environment,
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    // bquote(expr) is like quote() but evaluates anything wrapped in .()
    let expr = match args.first().and_then(|a| a.value.as_ref()) {
        Some(e) => e.clone(),
        None => return Ok(RValue::Null),
    };
    let interp = context.interpreter();
    let result = bquote_expr(&expr, env, interp)?;
    Ok(RValue::Language(Language::new(result)))
}

/// Walk an AST for bquote: evaluate .() splice expressions, leave everything else quoted.
fn bquote_expr(
    expr: &Expr,
    env: &Environment,
    interp: &crate::interpreter::Interpreter,
) -> Result<Expr, RError> {
    match expr {
        // Check for .(expr) — a call to `.` with one argument
        Expr::Call { func, args } => {
            if let Expr::Symbol(name) = func.as_ref() {
                if name == "." && args.len() == 1 {
                    // Evaluate the inner expression
                    if let Some(ref inner) = args[0].value {
                        let val = interp.eval_in(inner, env).map_err(RError::from)?;
                        return Ok(rvalue_to_expr(&val));
                    }
                }
            }
            // Not a .() call — recurse into func and args
            let new_func = Box::new(bquote_expr(func, env, interp)?);
            let new_args: Result<Vec<Arg>, RError> = args
                .iter()
                .map(|a| {
                    Ok(Arg {
                        name: a.name.clone(),
                        value: match &a.value {
                            Some(v) => Some(bquote_expr(v, env, interp)?),
                            None => None,
                        },
                    })
                })
                .collect();
            Ok(Expr::Call {
                func: new_func,
                args: new_args?,
            })
        }
        Expr::BinaryOp { op, lhs, rhs } => Ok(Expr::BinaryOp {
            op: *op,
            lhs: Box::new(bquote_expr(lhs, env, interp)?),
            rhs: Box::new(bquote_expr(rhs, env, interp)?),
        }),
        Expr::UnaryOp { op, operand } => Ok(Expr::UnaryOp {
            op: *op,
            operand: Box::new(bquote_expr(operand, env, interp)?),
        }),
        Expr::If {
            condition,
            then_body,
            else_body,
        } => Ok(Expr::If {
            condition: Box::new(bquote_expr(condition, env, interp)?),
            then_body: Box::new(bquote_expr(then_body, env, interp)?),
            else_body: match else_body {
                Some(e) => Some(Box::new(bquote_expr(e, env, interp)?)),
                None => None,
            },
        }),
        Expr::Block(exprs) => {
            let new_exprs: Result<Vec<Expr>, RError> =
                exprs.iter().map(|e| bquote_expr(e, env, interp)).collect();
            Ok(Expr::Block(new_exprs?))
        }
        // Everything else stays as-is
        _ => Ok(expr.clone()),
    }
}

/// Evaluate an expression and return the result with a visibility flag.
///
/// @param expr the expression to evaluate
/// @return a list with components "value" (the result) and "visible" (logical)
#[pre_eval_builtin(name = "withVisible", min_args = 1)]
fn pre_eval_with_visible(
    args: &[Arg],
    env: &Environment,
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let expr = args
        .first()
        .and_then(|a| a.value.as_ref())
        .ok_or_else(|| RError::new(RErrorKind::Argument, "argument 'x' is missing".to_string()))?;

    let value = context.with_interpreter(|interp| interp.eval_in(expr, env))?;

    // We don't track visibility yet, so always TRUE
    Ok(RValue::List(RList::new(vec![
        (Some("value".to_string()), value),
        (
            Some("visible".to_string()),
            RValue::vec(Vector::Logical(vec![Some(true)].into())),
        ),
    ])))
}

/// `expression(...)` — construct an expression object from unevaluated arguments.
/// Returns a list of Language objects, each wrapping the unevaluated expression.
#[pre_eval_builtin(name = "expression")]
fn pre_eval_expression(
    args: &[Arg],
    _env: &Environment,
    _context: &BuiltinContext,
) -> Result<RValue, RError> {
    let entries: Vec<(Option<String>, RValue)> = args
        .iter()
        .filter_map(|a| {
            a.value
                .as_ref()
                .map(|expr| (None, RValue::Language(Language::new(expr.clone()))))
        })
        .collect();
    Ok(RValue::List(RList::new(entries)))
}

/// Measure the wall-clock time to evaluate an expression.
///
/// @param expr the expression to time
/// @return numeric vector c(user, system, elapsed) where user and system are 0 (wall-clock only)
#[pre_eval_builtin(name = "system.time", min_args = 1)]
fn pre_eval_system_time(
    args: &[Arg],
    env: &Environment,
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let expr = args
        .first()
        .and_then(|a| a.value.as_ref())
        .ok_or_else(|| RError::new(RErrorKind::Argument, "argument is missing".to_string()))?;
    let start = std::time::Instant::now();
    let _result = context.with_interpreter(|interp| interp.eval_in(expr, env));
    let elapsed = start.elapsed().as_secs_f64();
    // Returns c(user, system, elapsed) — we only measure wall clock
    Ok(RValue::vec(Vector::Double(
        vec![Some(elapsed), Some(0.0), Some(elapsed)].into(),
    )))
}

/// Evaluate an expression in a temporary local environment.
///
/// Creates a new child of `envir` (default: the calling environment) and evaluates
/// `expr` in it. The local environment is discarded after evaluation, so any
/// bindings created inside are not visible to the caller.
///
/// @param expr expression to evaluate (not evaluated before dispatch)
/// @param envir parent environment for the local scope (default: calling environment)
/// @return the result of evaluating expr
#[pre_eval_builtin(name = "local", min_args = 1)]
fn pre_eval_local(
    args: &[Arg],
    env: &Environment,
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let expr = match args.first().and_then(|a| a.value.as_ref()) {
        Some(e) => e,
        None => return Ok(RValue::Null),
    };

    // Determine parent environment from second positional or named envir= arg
    let parent_env = context.with_interpreter(|interp| -> Result<Option<Environment>, RError> {
        for arg in args.iter().skip(1) {
            if arg.name.as_deref() == Some("envir") {
                if let Some(ref val_expr) = arg.value {
                    let val = interp.eval_in(val_expr, env)?;
                    if let RValue::Environment(e) = val {
                        return Ok(Some(e));
                    }
                }
            }
        }
        if let Some(arg) = args.get(1) {
            if arg.name.is_none() {
                if let Some(ref val_expr) = arg.value {
                    let val = interp.eval_in(val_expr, env)?;
                    if let RValue::Environment(e) = val {
                        return Ok(Some(e));
                    }
                }
            }
        }
        Ok(None)
    })?;

    let parent = parent_env.unwrap_or_else(|| env.clone());
    let local_env = Environment::new_child(&parent);
    context
        .with_interpreter(|interp| interp.eval_in(expr, &local_env))
        .map_err(RError::from)
}

/// Convert an RValue back to an AST expression (for substitute).
fn rvalue_to_expr(val: &RValue) -> Expr {
    match val {
        RValue::Language(expr) => *expr.inner.clone(),
        RValue::Null => Expr::Null,
        RValue::Vector(rv) => match &rv.inner {
            Vector::Double(d) if d.len() == 1 => match d[0] {
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
