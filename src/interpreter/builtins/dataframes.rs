//! Data frame manipulation builtins — merge, subset, transform, with.
//!
//! These builtins operate on data frames (lists with class "data.frame").
//! `subset`, `transform`, and `with` are pre-eval builtins because they need
//! to evaluate expressions in the context of a data frame's columns.
//! `split()` is in `interp.rs` since it already existed there.

use crate::interpreter::environment::Environment;
use crate::interpreter::value::*;
use crate::interpreter::BuiltinContext;
use crate::parser::ast::{Arg, Expr};
use minir_macros::{builtin, pre_eval_builtin};

use super::has_class;
use super::pre_eval::recycle_value;

// region: helpers

/// Extract a data frame's column names from its "names" attribute.
pub(super) fn df_col_names(list: &RList) -> Vec<Option<String>> {
    match list.get_attr("names") {
        Some(RValue::Vector(rv)) => rv.inner.to_characters(),
        _ => list.values.iter().map(|(name, _)| name.clone()).collect(),
    }
}

/// Get the number of rows in a data frame.
pub(super) fn df_nrow(list: &RList) -> usize {
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

/// Find a column by name in a data frame, returning its index.
pub(super) fn df_col_index(list: &RList, name: &str) -> Option<usize> {
    let names = df_col_names(list);
    names.iter().position(|n| n.as_deref() == Some(name))
}

/// Subset a vector by a boolean mask (true = keep, false/NA = drop).
fn subset_vector_by_mask(vec: &Vector, mask: &[Option<bool>]) -> Vector {
    let indices: Vec<usize> = mask
        .iter()
        .enumerate()
        .filter(|(_, keep)| **keep == Some(true))
        .map(|(i, _)| i)
        .collect();
    vec.select_indices(&indices)
}

/// Subset an RValue column by a boolean mask.
fn subset_rvalue_by_mask(val: &RValue, mask: &[Option<bool>]) -> RValue {
    match val {
        RValue::Vector(rv) => {
            let new_inner = subset_vector_by_mask(&rv.inner, mask);
            let mut new_rv = RVector::from(new_inner);
            // Copy non-structural attrs (like class, levels for factors)
            if let Some(attrs) = &rv.attrs {
                for (k, v) in attrs.iter() {
                    if k != "names" && k != "dim" && k != "dimnames" {
                        new_rv.set_attr(k.clone(), v.clone());
                    }
                }
            }
            // Subset names if present
            if let Some(names_attr) = rv.get_attr("names") {
                if let Some(names_vec) = names_attr.as_vector() {
                    let new_names = subset_vector_by_mask(names_vec, mask);
                    new_rv.set_attr("names".to_string(), RValue::vec(new_names));
                }
            }
            RValue::Vector(new_rv)
        }
        other => other.clone(),
    }
}

/// Build a data frame from columns with automatic row names.
pub(super) fn build_data_frame(
    columns: Vec<(Option<String>, RValue)>,
    nrow: usize,
) -> Result<RValue, RError> {
    let names: Vec<Option<String>> = columns.iter().map(|(name, _)| name.clone()).collect();
    let mut list = RList::new(columns);
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
    list.set_attr(
        "row.names".to_string(),
        RValue::vec(Vector::Integer(
            (1..=i64::try_from(nrow)?)
                .map(Some)
                .collect::<Vec<_>>()
                .into(),
        )),
    );
    Ok(RValue::List(list))
}

/// Create a child environment with data frame columns as bindings.
fn df_to_env(list: &RList, parent: &Environment) -> Environment {
    let child = Environment::new_child(parent);
    let names = df_col_names(list);
    for (i, (_, val)) in list.values.iter().enumerate() {
        if let Some(Some(name)) = names.get(i) {
            child.set(name.clone(), val.clone());
        }
    }
    child
}

/// Coerce an RValue to a character vector of column names.
fn coerce_to_string_vec(val: &RValue) -> Option<Vec<String>> {
    match val {
        RValue::Vector(rv) => {
            let chars = rv.inner.to_characters();
            let result: Vec<String> = chars.into_iter().flatten().collect();
            if result.is_empty() {
                None
            } else {
                Some(result)
            }
        }
        _ => None,
    }
}

// endregion

// region: merge

/// Merge two data frames by common columns (SQL-style join).
///
/// Performs an inner join by default. Use `all`, `all.x`, or `all.y` for
/// outer joins. The `by` argument specifies join columns; if omitted, uses
/// columns with names common to both data frames.
///
/// @param x first data frame
/// @param y second data frame
/// @param by character vector of shared column names to join on
/// @param by.x character vector of column names in x to join on
/// @param by.y character vector of column names in y to join on
/// @param all logical; if TRUE, do a full outer join
/// @param all.x logical; if TRUE, keep all rows from x (left join)
/// @param all.y logical; if TRUE, keep all rows from y (right join)
/// @return merged data frame
#[builtin(min_args = 2)]
fn builtin_merge(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let (x_list, y_list) = match (&args[0], &args[1]) {
        (x @ RValue::List(xl), y @ RValue::List(yl))
            if has_class(x, "data.frame") && has_class(y, "data.frame") =>
        {
            (xl, yl)
        }
        _ => {
            return Err(RError::new(
                RErrorKind::Type,
                "merge() requires two data frames".to_string(),
            ))
        }
    };

    // Parse named arguments
    let by = named
        .iter()
        .find(|(k, _)| k == "by")
        .map(|(_, v)| v)
        .or(args.get(2))
        .and_then(coerce_to_string_vec);
    let by_x = named
        .iter()
        .find(|(k, _)| k == "by.x")
        .map(|(_, v)| v)
        .and_then(coerce_to_string_vec);
    let by_y = named
        .iter()
        .find(|(k, _)| k == "by.y")
        .map(|(_, v)| v)
        .and_then(coerce_to_string_vec);
    let all = named
        .iter()
        .find(|(k, _)| k == "all")
        .and_then(|(_, v)| v.as_vector()?.as_logical_scalar())
        .unwrap_or(false);
    let all_x = named
        .iter()
        .find(|(k, _)| k == "all.x")
        .and_then(|(_, v)| v.as_vector()?.as_logical_scalar())
        .unwrap_or(all);
    let all_y = named
        .iter()
        .find(|(k, _)| k == "all.y")
        .and_then(|(_, v)| v.as_vector()?.as_logical_scalar())
        .unwrap_or(all);

    let x_names = df_col_names(x_list);
    let y_names = df_col_names(y_list);

    // Determine join columns
    let (join_x, join_y) = match (by, by_x, by_y) {
        (_, Some(bx), Some(by)) => (bx, by),
        (Some(b), _, _) => (b.clone(), b),
        (None, _, _) => {
            // Auto-detect common columns
            let x_set: Vec<&str> = x_names.iter().filter_map(|n| n.as_deref()).collect();
            let common: Vec<String> = y_names
                .iter()
                .filter_map(|n| n.as_deref())
                .filter(|n| x_set.contains(n))
                .map(|n| n.to_string())
                .collect();
            if common.is_empty() {
                return Err(RError::new(
                    RErrorKind::Argument,
                    "no common columns to merge on — specify 'by', 'by.x', or 'by.y'".to_string(),
                ));
            }
            (common.clone(), common)
        }
    };

    if join_x.len() != join_y.len() {
        return Err(RError::new(
            RErrorKind::Argument,
            "by.x and by.y must have the same length".to_string(),
        ));
    }

    // Validate join columns exist
    let x_join_indices: Vec<usize> = join_x
        .iter()
        .map(|name| {
            df_col_index(x_list, name).ok_or_else(|| {
                RError::new(
                    RErrorKind::Argument,
                    format!("column '{}' not found in x", name),
                )
            })
        })
        .collect::<Result<_, _>>()?;
    let y_join_indices: Vec<usize> = join_y
        .iter()
        .map(|name| {
            df_col_index(y_list, name).ok_or_else(|| {
                RError::new(
                    RErrorKind::Argument,
                    format!("column '{}' not found in y", name),
                )
            })
        })
        .collect::<Result<_, _>>()?;

    let x_nrow = df_nrow(x_list);
    let y_nrow = df_nrow(y_list);

    // Extract join key values as character vectors for comparison
    fn extract_keys(list: &RList, indices: &[usize], nrow: usize) -> Vec<Vec<Option<String>>> {
        (0..nrow)
            .map(|row| {
                indices
                    .iter()
                    .map(|&col_idx| {
                        let (_, val) = &list.values[col_idx];
                        match val {
                            RValue::Vector(rv) => {
                                let chars = rv.inner.to_characters();
                                chars.get(row).cloned().flatten()
                            }
                            _ => None,
                        }
                    })
                    .collect()
            })
            .collect()
    }

    let x_keys = extract_keys(x_list, &x_join_indices, x_nrow);
    let y_keys = extract_keys(y_list, &y_join_indices, y_nrow);

    // Build match pairs: (x_row, y_row)
    let mut match_pairs: Vec<(Option<usize>, Option<usize>)> = Vec::new();
    let mut y_matched = vec![false; y_nrow];

    for (xi, x_key) in x_keys.iter().enumerate() {
        let mut found = false;
        for (yi, y_key) in y_keys.iter().enumerate() {
            if x_key == y_key {
                match_pairs.push((Some(xi), Some(yi)));
                y_matched[yi] = true;
                found = true;
            }
        }
        if !found && all_x {
            match_pairs.push((Some(xi), None));
        }
    }
    if all_y {
        for (yi, matched) in y_matched.iter().enumerate() {
            if !matched {
                match_pairs.push((None, Some(yi)));
            }
        }
    }

    let result_nrow = match_pairs.len();

    // Determine which columns go into the result
    let y_join_set: std::collections::HashSet<usize> = y_join_indices.iter().copied().collect();

    // Build output columns: join keys, then x non-key cols, then y non-key cols
    let mut output_columns: Vec<(Option<String>, RValue)> = Vec::new();

    // Join key columns (from x, with fallback to y for right-join rows)
    for (ki, &x_col_idx) in x_join_indices.iter().enumerate() {
        let y_col_idx = y_join_indices[ki];
        let name = x_names[x_col_idx].clone();
        let x_col = &x_list.values[x_col_idx].1;
        let y_col = &y_list.values[y_col_idx].1;
        let values: Vec<Option<String>> = match_pairs
            .iter()
            .map(|(mx, my)| {
                if let Some(xi) = mx {
                    match x_col {
                        RValue::Vector(rv) => rv.inner.to_characters().get(*xi).cloned().flatten(),
                        _ => None,
                    }
                } else if let Some(yi) = my {
                    match y_col {
                        RValue::Vector(rv) => rv.inner.to_characters().get(*yi).cloned().flatten(),
                        _ => None,
                    }
                } else {
                    None
                }
            })
            .collect();
        output_columns.push((name, RValue::vec(Vector::Character(values.into()))));
    }

    // x non-key columns
    let x_join_set: std::collections::HashSet<usize> = x_join_indices.iter().copied().collect();
    for (col_idx, (name, val)) in x_list.values.iter().enumerate() {
        if x_join_set.contains(&col_idx) {
            continue;
        }
        let col_name = name.clone().or_else(|| Some(format!("x.{}", col_idx + 1)));
        let new_val = select_rows_from_column(val, &match_pairs, true);
        output_columns.push((col_name, new_val));
    }

    // y non-key columns
    for (col_idx, (name, val)) in y_list.values.iter().enumerate() {
        if y_join_set.contains(&col_idx) {
            continue;
        }
        // Suffix duplicate names with ".y"
        let base_name = name.clone().unwrap_or_else(|| format!("y.{}", col_idx + 1));
        let col_name = if x_names.iter().any(|n| n.as_deref() == Some(&base_name))
            && !x_join_set
                .iter()
                .any(|&i| x_names[i].as_deref() == Some(&base_name))
        {
            Some(format!("{}.y", base_name))
        } else {
            Some(base_name)
        };
        let new_val = select_rows_from_column(val, &match_pairs, false);
        output_columns.push((col_name, new_val));
    }

    build_data_frame(output_columns, result_nrow)
}

/// Select rows from a column vector based on match pairs.
fn select_rows_from_column(
    val: &RValue,
    pairs: &[(Option<usize>, Option<usize>)],
    use_first: bool,
) -> RValue {
    match val {
        RValue::Vector(rv) => {
            let len = rv.inner.len();
            let indices: Vec<Option<usize>> = pairs
                .iter()
                .map(|(mx, my)| if use_first { *mx } else { *my })
                .collect();
            match &rv.inner {
                Vector::Character(vals) => {
                    let result: Vec<Option<String>> = indices
                        .iter()
                        .map(|idx| idx.and_then(|i| if i < len { vals[i].clone() } else { None }))
                        .collect();
                    RValue::vec(Vector::Character(result.into()))
                }
                Vector::Double(vals) => {
                    let result: Vec<Option<f64>> = indices
                        .iter()
                        .map(|idx| idx.and_then(|i| if i < len { vals.get_opt(i) } else { None }))
                        .collect();
                    RValue::vec(Vector::Double(result.into()))
                }
                Vector::Integer(vals) => {
                    let result: Vec<Option<i64>> = indices
                        .iter()
                        .map(|idx| idx.and_then(|i| if i < len { vals.get_opt(i) } else { None }))
                        .collect();
                    RValue::vec(Vector::Integer(result.into()))
                }
                Vector::Logical(vals) => {
                    let result: Vec<Option<bool>> = indices
                        .iter()
                        .map(|idx| idx.and_then(|i| if i < len { vals[i] } else { None }))
                        .collect();
                    RValue::vec(Vector::Logical(result.into()))
                }
                Vector::Complex(vals) => {
                    let result: Vec<Option<num_complex::Complex64>> = indices
                        .iter()
                        .map(|idx| idx.and_then(|i| if i < len { vals[i] } else { None }))
                        .collect();
                    RValue::vec(Vector::Complex(result.into()))
                }
                Vector::Raw(vals) => {
                    let result: Vec<u8> = indices
                        .iter()
                        .map(|idx| idx.map_or(0, |i| if i < len { vals[i] } else { 0 }))
                        .collect();
                    RValue::vec(Vector::Raw(result))
                }
            }
        }
        _ => RValue::Null,
    }
}

// endregion

// region: subset

/// Subset a data frame by condition and/or column selection.
///
/// Evaluates the `subset` expression in the context of the data frame's columns,
/// keeping rows where it evaluates to TRUE. The `select` argument specifies which
/// columns to keep.
///
/// @param x a data frame
/// @param subset logical expression evaluated in the data frame context
/// @param select expression indicating columns to select (e.g., c(a, b) or -c)
/// @return subsetted data frame
#[pre_eval_builtin(min_args = 1)]
fn pre_eval_subset(
    args: &[Arg],
    env: &Environment,
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    // First argument is the data frame (evaluate it)
    let x_arg = args.first().and_then(|a| a.value.as_ref()).ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "subset() requires a data frame argument".to_string(),
        )
    })?;

    let x_val = context
        .with_interpreter(|interp| interp.eval_in(x_arg, env))
        .map_err(RError::from)?;

    // Handle vector subset (non-data-frame case)
    if let RValue::Vector(ref rv) = x_val {
        if !has_class(&x_val, "data.frame") {
            return subset_vector_impl(rv, args, env, context);
        }
    }

    let RValue::List(ref x_list) = x_val else {
        return Err(RError::new(
            RErrorKind::Type,
            "subset() requires a data frame or vector".to_string(),
        ));
    };

    if !has_class(&x_val, "data.frame") {
        return Err(RError::new(
            RErrorKind::Type,
            "subset() requires a data frame".to_string(),
        ));
    }

    let nrow = df_nrow(x_list);
    let col_names = df_col_names(x_list);

    // Find subset and select arguments (by name or position)
    let subset_expr = args
        .iter()
        .find(|a| a.name.as_deref() == Some("subset"))
        .or_else(|| args.get(1).filter(|a| a.name.is_none()))
        .and_then(|a| a.value.as_ref());

    let select_expr = args
        .iter()
        .find(|a| a.name.as_deref() == Some("select"))
        .or_else(|| args.get(2).filter(|a| a.name.is_none()))
        .and_then(|a| a.value.as_ref());

    // Evaluate subset condition in data frame context
    let row_mask = if let Some(expr) = subset_expr {
        let df_env = df_to_env(x_list, env);
        let mask_val = context
            .with_interpreter(|interp| interp.eval_in(expr, &df_env))
            .map_err(RError::from)?;
        match mask_val {
            RValue::Vector(rv) => {
                let logicals = rv.inner.to_logicals();
                if logicals.len() != nrow {
                    return Err(RError::new(
                        RErrorKind::Argument,
                        format!(
                            "subset condition has length {} but data frame has {} rows",
                            logicals.len(),
                            nrow
                        ),
                    ));
                }
                logicals
            }
            _ => {
                return Err(RError::new(
                    RErrorKind::Type,
                    "subset condition must evaluate to a logical vector".to_string(),
                ))
            }
        }
    } else {
        vec![Some(true); nrow]
    };

    // Determine which columns to keep
    let selected_cols = if let Some(expr) = select_expr {
        resolve_column_selection(expr, &col_names)?
    } else {
        (0..col_names.len()).collect()
    };

    // Build result data frame
    let result_nrow = row_mask.iter().filter(|m| **m == Some(true)).count();
    let mut output_columns = Vec::new();
    for &col_idx in &selected_cols {
        let (name, val) = &x_list.values[col_idx];
        let new_val = subset_rvalue_by_mask(val, &row_mask);
        output_columns.push((name.clone(), new_val));
    }

    build_data_frame(output_columns, result_nrow)
}

/// Subset a plain vector by a logical condition.
fn subset_vector_impl(
    rv: &RVector,
    args: &[Arg],
    env: &Environment,
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let subset_expr = args
        .iter()
        .find(|a| a.name.as_deref() == Some("subset"))
        .or_else(|| args.get(1).filter(|a| a.name.is_none()))
        .and_then(|a| a.value.as_ref());

    if let Some(expr) = subset_expr {
        let mask_val = context
            .with_interpreter(|interp| interp.eval_in(expr, env))
            .map_err(RError::from)?;
        match mask_val {
            RValue::Vector(mask_rv) => {
                let logicals = mask_rv.inner.to_logicals();
                let result = subset_vector_by_mask(&rv.inner, &logicals);
                Ok(RValue::vec(result))
            }
            _ => Err(RError::new(
                RErrorKind::Type,
                "subset condition must evaluate to a logical vector".to_string(),
            )),
        }
    } else {
        Ok(RValue::Vector(rv.clone()))
    }
}

/// Resolve a column selection expression to column indices.
///
/// Supports: `c(a, b)`, `-c(a, b)`, bare symbol, negative bare symbol.
fn resolve_column_selection(
    expr: &Expr,
    col_names: &[Option<String>],
) -> Result<Vec<usize>, RError> {
    match expr {
        // Negative selection: -c(col1, col2) or -col
        Expr::UnaryOp {
            op: crate::parser::ast::UnaryOp::Neg,
            operand,
        } => {
            let exclude = resolve_column_selection(operand, col_names)?;
            let exclude_set: std::collections::HashSet<usize> = exclude.into_iter().collect();
            Ok((0..col_names.len())
                .filter(|i| !exclude_set.contains(i))
                .collect())
        }
        // c(col1, col2)
        Expr::Call { func, args, .. } => {
            if let Expr::Symbol(name) = func.as_ref() {
                if name == "c" {
                    let mut indices = Vec::new();
                    for arg in args {
                        if let Some(Expr::Symbol(col_name)) = arg.value.as_ref() {
                            if let Some(idx) = col_names
                                .iter()
                                .position(|n| n.as_deref() == Some(col_name.as_str()))
                            {
                                indices.push(idx);
                            } else {
                                return Err(RError::new(
                                    RErrorKind::Argument,
                                    format!("column '{}' not found in data frame", col_name),
                                ));
                            }
                        }
                    }
                    return Ok(indices);
                }
            }
            Err(RError::new(
                RErrorKind::Argument,
                "select argument must be a column name or c(...) of column names".to_string(),
            ))
        }
        // Bare symbol
        Expr::Symbol(name) => {
            if let Some(idx) = col_names
                .iter()
                .position(|n| n.as_deref() == Some(name.as_str()))
            {
                Ok(vec![idx])
            } else {
                Err(RError::new(
                    RErrorKind::Argument,
                    format!("column '{}' not found in data frame", name),
                ))
            }
        }
        _ => Err(RError::new(
            RErrorKind::Argument,
            "select argument must be a column name, c(...), or negation of these".to_string(),
        )),
    }
}

// endregion

// region: transform

/// Add or modify columns in a data frame.
///
/// Evaluates each named `...` argument as an expression in the data frame's
/// column context, adding or replacing columns with the result.
///
/// @param _data a data frame
/// @param ... named expressions to evaluate (column_name = expression)
/// @return data frame with modified/added columns
#[pre_eval_builtin(name = "transform", min_args = 1)]
fn pre_eval_transform(
    args: &[Arg],
    env: &Environment,
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let data_arg = args.first().and_then(|a| a.value.as_ref()).ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "transform() requires a data frame argument".to_string(),
        )
    })?;

    let data_val = context
        .with_interpreter(|interp| interp.eval_in(data_arg, env))
        .map_err(RError::from)?;

    let RValue::List(mut data_list) = data_val.clone() else {
        return Err(RError::new(
            RErrorKind::Type,
            "transform() requires a data frame".to_string(),
        ));
    };

    if !has_class(&data_val, "data.frame") {
        return Err(RError::new(
            RErrorKind::Type,
            "transform() requires a data frame".to_string(),
        ));
    }

    let nrow = df_nrow(&data_list);
    let col_names = df_col_names(&data_list);

    // Process each named argument after the first
    for arg in args.iter().skip(1) {
        let Some(ref col_name) = arg.name else {
            continue; // Skip unnamed args
        };
        let Some(ref expr) = arg.value else {
            continue;
        };

        // Evaluate expression in data frame context
        let df_env = df_to_env(&data_list, env);
        let new_val = context
            .with_interpreter(|interp| interp.eval_in(expr, &df_env))
            .map_err(RError::from)?;

        // Validate length
        let val_len = new_val.length();
        if val_len != nrow && val_len != 1 && nrow > 0 {
            if !nrow.is_multiple_of(val_len) {
                return Err(RError::new(
                    RErrorKind::Argument,
                    format!(
                        "replacement has {} rows, data has {} — must be a divisor",
                        val_len, nrow
                    ),
                ));
            }
            // Recycle
            let recycled = recycle_value(&new_val, nrow)?;
            if let Some(idx) = col_names
                .iter()
                .position(|n| n.as_deref() == Some(col_name.as_str()))
            {
                data_list.values[idx] = (Some(col_name.clone()), recycled);
            } else {
                data_list.values.push((Some(col_name.clone()), recycled));
            }
            continue;
        }

        // Add or replace column
        if let Some(idx) = col_names
            .iter()
            .position(|n| n.as_deref() == Some(col_name.as_str()))
        {
            data_list.values[idx] = (Some(col_name.clone()), new_val);
        } else {
            data_list.values.push((Some(col_name.clone()), new_val));
        }
    }

    // Rebuild data frame with updated names
    let names: Vec<Option<String>> = data_list
        .values
        .iter()
        .map(|(name, _)| name.clone())
        .collect();
    data_list.set_attr(
        "names".to_string(),
        RValue::vec(Vector::Character(names.into())),
    );

    Ok(RValue::List(data_list))
}

// endregion

// region: with

/// Evaluate an expression in the context of a data frame.
///
/// Creates an environment where the data frame's columns are accessible
/// as variables, then evaluates `expr` in that environment.
///
/// @param data a data frame (or list)
/// @param expr expression to evaluate
/// @return the result of evaluating expr
#[pre_eval_builtin(name = "with", min_args = 2)]
fn pre_eval_with(
    args: &[Arg],
    env: &Environment,
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let data_arg = args.first().and_then(|a| a.value.as_ref()).ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "with() requires a data argument".to_string(),
        )
    })?;

    let data_val = context
        .with_interpreter(|interp| interp.eval_in(data_arg, env))
        .map_err(RError::from)?;

    let data_list = match &data_val {
        RValue::List(l) => l,
        _ => {
            return Err(RError::new(
                RErrorKind::Type,
                "with() requires a list or data frame as first argument".to_string(),
            ))
        }
    };

    let expr = args.get(1).and_then(|a| a.value.as_ref()).ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "with() requires an expression argument".to_string(),
        )
    })?;

    let df_env = df_to_env(data_list, env);
    context
        .with_interpreter(|interp| interp.eval_in(expr, &df_env))
        .map_err(RError::from)
}

// endregion

// Note: split() is implemented in interp.rs as interp_split
