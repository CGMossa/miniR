//! Read-side indexing helpers for vectors, lists, matrices, and data frames.
//! Write-side (replacement) is in `assignment.rs`.

use derive_more::{Display, Error};

use crate::interpreter::environment::Environment;
use crate::interpreter::value::*;
use crate::interpreter::Interpreter;
use crate::parser::ast::{Arg, Expr};

// region: IndexingError

/// Structured error type for indexing operations.
#[derive(Debug, Display, Error)]
pub enum IndexingError {
    #[display("can't mix positive and negative subscripts")]
    MixedSubscripts,

    #[display("invalid index type")]
    InvalidIndexType,

    #[display("object is not subsettable")]
    NotSubsettable,

    #[display("incorrect number of dimensions")]
    IncorrectDimensions,

    #[display("subscript out of bounds (no dimnames to match against)")]
    NoDimnames,

    #[display("subscript out of bounds: '{}'", name)]
    SubscriptOutOfBounds { name: String },

    #[display("invalid subscript type")]
    InvalidSubscriptType,

    #[display("undefined row selected: '{}'", name)]
    UndefinedRow { name: String },
}

impl From<IndexingError> for RError {
    fn from(e: IndexingError) -> Self {
        RError::from_source(RErrorKind::Index, e)
    }
}

impl From<IndexingError> for RFlow {
    fn from(e: IndexingError) -> Self {
        RFlow::Error(RError::from(e))
    }
}

// endregion

impl Interpreter {
    pub(super) fn eval_index(
        &self,
        object: &Expr,
        indices: &[Arg],
        env: &Environment,
    ) -> Result<RValue, RFlow> {
        eval_index(self, object, indices, env)
    }

    #[cfg(feature = "random")]
    pub(crate) fn index_by_integer(
        &self,
        v: &Vector,
        indices: &[Option<i64>],
    ) -> Result<RValue, RFlow> {
        index_by_integer(self, v, indices)
    }

    pub(super) fn eval_index_double(
        &self,
        object: &Expr,
        indices: &[Arg],
        env: &Environment,
    ) -> Result<RValue, RFlow> {
        eval_index_double(self, object, indices, env)
    }

    pub(super) fn eval_dollar(
        &self,
        object: &Expr,
        member: &str,
        env: &Environment,
    ) -> Result<RValue, RFlow> {
        eval_dollar(self, object, member, env)
    }
}

pub(super) fn eval_index(
    interp: &Interpreter,
    object: &Expr,
    indices: &[Arg],
    env: &Environment,
) -> Result<RValue, RFlow> {
    let obj = interp.eval_in(object, env)?;

    if indices.is_empty() {
        return Ok(obj);
    }

    if indices.len() >= 2 {
        return eval_matrix_index(interp, &obj, indices, env);
    }

    let idx_val = if let Some(val_expr) = &indices[0].value {
        interp.eval_in(val_expr, env)?
    } else {
        return Ok(obj);
    };

    match &obj {
        RValue::Vector(v) => match &idx_val {
            RValue::Vector(idx_vec) => {
                if let Vector::Logical(mask) = &idx_vec.inner {
                    return index_by_logical(interp, v, mask);
                }

                // Character indexing: look up names attribute to resolve positions
                if let Vector::Character(idx_names) = &idx_vec.inner {
                    let names_attr = v.get_attr("names").and_then(|a| a.as_vector());
                    let name_strs: Vec<Option<String>> =
                        names_attr.map(|nv| nv.to_characters()).unwrap_or_default();
                    let positions: Vec<Option<i64>> = idx_names
                        .iter()
                        .map(|idx_name| {
                            idx_name.as_ref().and_then(|name| {
                                name_strs
                                    .iter()
                                    .position(|n| n.as_deref() == Some(name.as_str()))
                                    .and_then(|p| i64::try_from(p + 1).ok())
                            })
                        })
                        .collect();
                    return index_by_integer(interp, v, &positions);
                }

                let indices = idx_vec.to_integers();
                // Drop zeros (R ignores them), then check sign consistency
                let nonzero: Vec<Option<i64>> = indices
                    .iter()
                    .filter(|x| !matches!(x, Some(0)))
                    .copied()
                    .collect();
                let has_pos = nonzero.iter().any(|x| x.map(|i| i > 0).unwrap_or(false));
                let has_neg = nonzero.iter().any(|x| x.map(|i| i < 0).unwrap_or(false));
                if has_pos && has_neg {
                    return Err(IndexingError::MixedSubscripts.into());
                }
                if has_neg {
                    return index_by_negative(interp, v, &nonzero);
                }
                index_by_integer(interp, v, &nonzero)
            }
            RValue::Null => Ok(obj.clone()),
            _ => Err(IndexingError::InvalidIndexType.into()),
        },
        RValue::List(list) => match &idx_val {
            RValue::Vector(idx_vec) => {
                if let Vector::Character(names) = &idx_vec.inner {
                    let mut result = Vec::new();
                    for name in names.iter().flatten() {
                        let found = list
                            .values
                            .iter()
                            .find(|(n, _)| n.as_ref() == Some(name))
                            .map(|(n, v)| (n.clone(), v.clone()));
                        if let Some(item) = found {
                            result.push(item);
                        }
                    }
                    return Ok(RValue::List(RList::new(result)));
                }

                let indices = idx_vec.to_integers();
                // Drop zeros, then check sign consistency
                let nonzero: Vec<Option<i64>> = indices
                    .iter()
                    .filter(|x| !matches!(x, Some(0)))
                    .copied()
                    .collect();
                let has_pos = nonzero.iter().any(|x| x.map(|i| i > 0).unwrap_or(false));
                let has_neg = nonzero.iter().any(|x| x.map(|i| i < 0).unwrap_or(false));
                if has_pos && has_neg {
                    return Err(IndexingError::MixedSubscripts.into());
                }
                if has_neg {
                    // Negative indexing: exclude those positions
                    let exclude: Vec<usize> = nonzero
                        .iter()
                        .filter_map(|x| x.and_then(|i| usize::try_from(-i).ok()))
                        .collect();
                    let result: Vec<_> = list
                        .values
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| !exclude.contains(&(i + 1)))
                        .map(|(_, v)| v.clone())
                        .collect();
                    return Ok(RValue::List(RList::new(result)));
                }
                let mut result = Vec::new();
                for i in nonzero.iter().flatten() {
                    let i = usize::try_from(*i).unwrap_or(0);
                    if i > 0 && i <= list.values.len() {
                        result.push(list.values[i - 1].clone());
                    }
                }
                Ok(RValue::List(RList::new(result)))
            }
            _ => Err(IndexingError::InvalidIndexType.into()),
        },
        RValue::Environment(target_env) => {
            // env["key"] — look up variable(s) in the environment, return as a named list
            match &idx_val {
                RValue::Vector(idx_vec) => {
                    if let Vector::Character(names) = &idx_vec.inner {
                        let mut result = Vec::new();
                        for name in names.iter().flatten() {
                            let val = target_env.get(name).unwrap_or(RValue::Null);
                            result.push((Some(name.clone()), val));
                        }
                        return Ok(RValue::List(RList::new(result)));
                    }
                    Err(IndexingError::InvalidIndexType.into())
                }
                RValue::Null => Ok(RValue::List(RList::new(Vec::new()))),
                _ => Err(IndexingError::InvalidIndexType.into()),
            }
        }
        _ => Err(IndexingError::NotSubsettable.into()),
    }
}

fn eval_matrix_index(
    interp: &Interpreter,
    obj: &RValue,
    indices: &[Arg],
    env: &Environment,
) -> Result<RValue, RFlow> {
    let (data, dim_attr) = match obj {
        RValue::Vector(rv) => (&rv.inner, rv.get_attr("dim")),
        RValue::List(list) => {
            return eval_list_2d_index(interp, list, indices, env);
        }
        _ => {
            return Err(IndexingError::IncorrectDimensions.into());
        }
    };

    let dims: Vec<Option<i64>> = match dim_attr {
        Some(RValue::Vector(rv)) => match &rv.inner {
            Vector::Integer(d) => d.iter_opt().collect(),
            _ => {
                return Err(IndexingError::IncorrectDimensions.into());
            }
        },
        _ => {
            return Err(IndexingError::IncorrectDimensions.into());
        }
    };

    if dims.len() < 2 {
        return Err(IndexingError::IncorrectDimensions.into());
    }
    let nrow = usize::try_from(dims[0].unwrap_or(0)).unwrap_or(0);
    let ncol = usize::try_from(dims[1].unwrap_or(0)).unwrap_or(0);

    let row_idx = if let Some(val_expr) = &indices[0].value {
        Some(interp.eval_in(val_expr, env)?)
    } else {
        None
    };

    let col_idx = if let Some(val_expr) = &indices[1].value {
        Some(interp.eval_in(val_expr, env)?)
    } else {
        None
    };

    // Get dimnames for character index resolution
    let dimnames = match obj {
        RValue::Vector(rv) => rv.get_attr("dimnames"),
        _ => None,
    };
    let row_names = extract_dim_names(dimnames, 0);
    let col_names = extract_dim_names(dimnames, 1);

    let rows: Vec<usize> = resolve_dim_index(&row_idx, nrow, &row_names)?;
    let cols: Vec<usize> = resolve_dim_index(&col_idx, ncol, &col_names)?;

    // Collect flat indices in column-major order
    let flat_indices: Vec<usize> = cols
        .iter()
        .flat_map(|&j| rows.iter().map(move |&i| j * nrow + i))
        .collect();

    let result = data.select_indices(&flat_indices);

    if rows.len() == 1 && cols.len() == 1 {
        return Ok(RValue::vec(result));
    }

    let mut rv = RVector::from(result);
    if rows.len() > 1 || cols.len() > 1 {
        rv.set_attr(
            "dim".to_string(),
            RValue::vec(Vector::Integer(
                vec![
                    Some(i64::try_from(rows.len())?),
                    Some(i64::try_from(cols.len())?),
                ]
                .into(),
            )),
        );
    }
    Ok(RValue::Vector(rv))
}

fn eval_list_2d_index(
    interp: &Interpreter,
    list: &RList,
    indices: &[Arg],
    env: &Environment,
) -> Result<RValue, RFlow> {
    let is_df = if let Some(RValue::Vector(rv)) = list.get_attr("class") {
        if let Vector::Character(cls) = &rv.inner {
            cls.iter().any(|c| c.as_deref() == Some("data.frame"))
        } else {
            false
        }
    } else {
        false
    };
    if !is_df {
        if let Some(val_expr) = &indices[0].value {
            let idx_val = interp.eval_in(val_expr, env)?;
            return match &idx_val {
                RValue::Vector(iv) => {
                    let i = usize::try_from(iv.as_integer_scalar().unwrap_or(0)).unwrap_or(0);
                    if i > 0 && i <= list.values.len() {
                        Ok(list.values[i - 1].1.clone())
                    } else {
                        Ok(RValue::Null)
                    }
                }
                _ => Ok(RValue::Null),
            };
        }
        return Ok(RValue::Null);
    }

    // Extract `drop` named argument (defaults to TRUE in R)
    let drop = extract_drop_arg(interp, indices, env)?;

    let col_idx = if let Some(val_expr) = &indices[1].value {
        Some(interp.eval_in(val_expr, env)?)
    } else {
        None
    };

    let selected_cols: Vec<(Option<String>, RValue)> = match &col_idx {
        None => list.values.clone(),
        Some(RValue::Vector(rv)) if matches!(rv.inner, Vector::Character(_)) => {
            let Vector::Character(names) = &rv.inner else {
                unreachable!()
            };
            names
                .iter()
                .filter_map(|n| {
                    n.as_ref().and_then(|name| {
                        list.values
                            .iter()
                            .find(|(k, _)| k.as_ref() == Some(name))
                            .cloned()
                    })
                })
                .collect()
        }
        Some(RValue::Vector(rv)) => {
            let idxs = rv.to_integers();
            idxs.iter()
                .filter_map(|i| {
                    i.and_then(|i| {
                        let i = usize::try_from(i - 1).ok()?;
                        list.values.get(i).cloned()
                    })
                })
                .collect()
        }
        _ => list.values.clone(),
    };

    let row_idx = if let Some(val_expr) = &indices[0].value {
        Some(interp.eval_in(val_expr, env)?)
    } else {
        None
    };

    // Number of rows in the data frame (from first column)
    let nrows = selected_cols.first().map(|(_, v)| v.length()).unwrap_or(0);

    if row_idx.is_none() {
        // No row subsetting — drop single column to vector if drop=TRUE
        if drop && col_idx.is_some() && selected_cols.len() == 1 {
            return Ok(selected_cols
                .into_iter()
                .next()
                .expect("selected_cols.len() == 1 guarantees an element")
                .1);
        }

        let col_names: Vec<Option<String>> = selected_cols.iter().map(|(n, _)| n.clone()).collect();
        let mut result = RList::new(selected_cols);
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
        let row_names_attr = subset_row_names(
            list,
            &(1..=i64::try_from(nrows).unwrap_or(0))
                .map(Some)
                .collect::<Vec<_>>(),
        );
        result.set_attr("row.names".to_string(), row_names_attr);
        return Ok(RValue::List(result));
    }

    let int_rows: Vec<Option<i64>> = resolve_df_row_index(&row_idx, nrows, list)?;

    // Drop single column to vector when drop=TRUE (R default)
    if drop && selected_cols.len() == 1 {
        if let RValue::Vector(rv) = &selected_cols[0].1 {
            return index_by_integer(interp, &rv.inner, &int_rows);
        }
        return Ok(selected_cols[0].1.clone());
    }

    let mut result_cols = Vec::new();
    for (name, col_val) in &selected_cols {
        if let RValue::Vector(rv) = col_val {
            let indexed = index_by_integer(interp, &rv.inner, &int_rows)?;
            result_cols.push((name.clone(), indexed));
        } else {
            result_cols.push((name.clone(), col_val.clone()));
        }
    }
    let col_names: Vec<Option<String>> = result_cols.iter().map(|(n, _)| n.clone()).collect();
    let mut result = RList::new(result_cols);
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
    // Preserve selected row names from the original data frame
    let row_names_attr = subset_row_names(list, &int_rows);
    result.set_attr("row.names".to_string(), row_names_attr);
    Ok(RValue::List(result))
}

/// Extract the `drop` named argument from index args (3rd+ position).
/// Returns `true` if `drop` is not specified (R default).
fn extract_drop_arg(
    interp: &Interpreter,
    indices: &[Arg],
    env: &Environment,
) -> Result<bool, RFlow> {
    for arg in indices.iter().skip(2) {
        if arg.name.as_deref() == Some("drop") {
            if let Some(val_expr) = &arg.value {
                let val = interp.eval_in(val_expr, env)?;
                if let Some(rv) = val.as_vector() {
                    if let Some(b) = rv.as_logical_scalar() {
                        return Ok(b);
                    }
                }
            }
            return Ok(true);
        }
    }
    Ok(true) // default: drop = TRUE
}

/// Resolve data frame row indices, handling positive integers, negative integers,
/// logical masks, and character row names. Returns 1-based positive indices.
fn resolve_df_row_index(
    row_idx: &Option<RValue>,
    nrows: usize,
    list: &RList,
) -> Result<Vec<Option<i64>>, RFlow> {
    match row_idx {
        None => Ok((1..=i64::try_from(nrows).unwrap_or(0)).map(Some).collect()),
        Some(RValue::Vector(rv)) => {
            match &rv.inner {
                // Logical mask
                Vector::Logical(lv) => Ok(lv
                    .iter()
                    .enumerate()
                    .filter(|(_, v)| v.unwrap_or(false))
                    .filter_map(|(i, _)| i64::try_from(i).ok().map(|i| Some(i + 1)))
                    .collect()),
                // Character: look up in row.names
                Vector::Character(names) => {
                    let row_names = get_row_names_vec(list);
                    let mut result = Vec::new();
                    for name in names.iter() {
                        match name {
                            Some(n) => {
                                let pos = row_names
                                    .iter()
                                    .position(|rn| rn.as_deref() == Some(n.as_str()));
                                match pos {
                                    Some(p) => result.push(Some(i64::try_from(p + 1)?)),
                                    None => {
                                        return Err(IndexingError::UndefinedRow {
                                            name: n.clone(),
                                        }
                                        .into());
                                    }
                                }
                            }
                            None => result.push(None),
                        }
                    }
                    Ok(result)
                }
                // Numeric: handle positive and negative
                _ => {
                    let ints = rv.to_integers();
                    // Filter zeros
                    let nonzero: Vec<Option<i64>> = ints
                        .iter()
                        .filter(|x| !matches!(x, Some(0)))
                        .copied()
                        .collect();
                    let has_pos = nonzero.iter().any(|x| x.map(|i| i > 0).unwrap_or(false));
                    let has_neg = nonzero.iter().any(|x| x.map(|i| i < 0).unwrap_or(false));
                    if has_pos && has_neg {
                        return Err(IndexingError::MixedSubscripts.into());
                    }
                    if has_neg {
                        // Negative indexing: exclude those rows
                        let exclude: Vec<usize> = nonzero
                            .iter()
                            .filter_map(|x| x.and_then(|i| usize::try_from(-i).ok()))
                            .collect();
                        Ok((1..=nrows)
                            .filter(|i| !exclude.contains(i))
                            .filter_map(|i| i64::try_from(i).ok().map(Some))
                            .collect())
                    } else {
                        Ok(nonzero)
                    }
                }
            }
        }
        _ => Ok(vec![]),
    }
}

/// Get row.names from a data frame list as character vector.
fn get_row_names_vec(list: &RList) -> Vec<Option<String>> {
    if let Some(rn_attr) = list.get_attr("row.names") {
        if let Some(rn_vec) = rn_attr.as_vector() {
            return rn_vec.to_characters();
        }
    }
    vec![]
}

pub(crate) fn index_by_integer(
    _interp: &Interpreter,
    v: &Vector,
    indices: &[Option<i64>],
) -> Result<RValue, RFlow> {
    macro_rules! index_vec_option {
        ($vals:expr, $variant:ident) => {{
            let result: Vec<_> = indices
                .iter()
                .map(|idx| {
                    idx.and_then(|i| {
                        let i = usize::try_from(i).unwrap_or(0);
                        if i > 0 && i <= $vals.len() {
                            $vals[i - 1].clone().into()
                        } else {
                            None
                        }
                    })
                })
                .collect();
            Ok(RValue::vec(Vector::$variant(result.into())))
        }};
    }

    macro_rules! index_vec_buffer {
        ($vals:expr, $variant:ident) => {{
            let result: Vec<_> = indices
                .iter()
                .map(|idx| {
                    idx.and_then(|i| {
                        let i = usize::try_from(i).unwrap_or(0);
                        if i > 0 && i <= $vals.len() {
                            $vals.get_opt(i - 1)
                        } else {
                            None
                        }
                    })
                })
                .collect();
            Ok(RValue::vec(Vector::$variant(result.into())))
        }};
    }

    match v {
        Vector::Raw(vals) => {
            let result: Vec<u8> = indices
                .iter()
                .map(|idx| {
                    idx.and_then(|i| {
                        let i = usize::try_from(i).unwrap_or(0);
                        if i > 0 && i <= vals.len() {
                            Some(vals[i - 1])
                        } else {
                            Some(0u8)
                        }
                    })
                    .unwrap_or(0u8)
                })
                .collect();
            Ok(RValue::vec(Vector::Raw(result)))
        }
        Vector::Double(vals) => index_vec_buffer!(vals, Double),
        Vector::Integer(vals) => index_vec_buffer!(vals, Integer),
        Vector::Logical(vals) => index_vec_option!(vals, Logical),
        Vector::Complex(vals) => index_vec_option!(vals, Complex),
        Vector::Character(vals) => index_vec_option!(vals, Character),
    }
}

fn index_by_negative(
    _interp: &Interpreter,
    v: &Vector,
    indices: &[Option<i64>],
) -> Result<RValue, RFlow> {
    let exclude: Vec<usize> = indices
        .iter()
        .filter_map(|x| x.and_then(|i| usize::try_from(-i).ok()))
        .collect();

    macro_rules! filter_vec_option {
        ($vals:expr, $variant:ident) => {{
            let result: Vec<_> = $vals
                .iter()
                .enumerate()
                .filter(|(i, _)| !exclude.contains(&(i + 1)))
                .map(|(_, v)| v.clone())
                .collect();
            Ok(RValue::vec(Vector::$variant(result.into())))
        }};
    }

    macro_rules! filter_vec_buffer {
        ($vals:expr, $variant:ident) => {{
            let result: Vec<_> = $vals
                .iter()
                .enumerate()
                .filter(|(i, _)| !exclude.contains(&(i + 1)))
                .map(|(_, v)| v.copied())
                .collect();
            Ok(RValue::vec(Vector::$variant(result.into())))
        }};
    }

    match v {
        Vector::Raw(vals) => {
            let result: Vec<u8> = vals
                .iter()
                .enumerate()
                .filter(|(i, _)| !exclude.contains(&(i + 1)))
                .map(|(_, &v)| v)
                .collect();
            Ok(RValue::vec(Vector::Raw(result)))
        }
        Vector::Double(vals) => filter_vec_buffer!(vals, Double),
        Vector::Integer(vals) => filter_vec_buffer!(vals, Integer),
        Vector::Logical(vals) => filter_vec_option!(vals, Logical),
        Vector::Complex(vals) => filter_vec_option!(vals, Complex),
        Vector::Character(vals) => filter_vec_option!(vals, Character),
    }
}

fn index_by_logical(
    _interp: &Interpreter,
    v: &Vector,
    mask: &[Option<bool>],
) -> Result<RValue, RFlow> {
    let vlen = v.len();
    if mask.is_empty() {
        // Empty mask selects nothing
        return Ok(RValue::vec(v.select_indices(&[])));
    }

    // Recycle mask to vector length.
    // For each position: Some(true) -> include element, Some(false) -> skip,
    // None (NA) -> include NA.
    let recycled_mask = |i: usize| -> Option<bool> { mask[i % mask.len()] };

    macro_rules! mask_vec_option {
        ($vals:expr, $variant:ident) => {{
            let result: Vec<_> = (0..vlen)
                .filter_map(|i| match recycled_mask(i) {
                    Some(true) => Some($vals.get(i).cloned().unwrap_or(None)),
                    Some(false) => None,
                    None => Some(None), // NA in mask -> NA in result
                })
                .collect();
            Ok(RValue::vec(Vector::$variant(result.into())))
        }};
    }

    macro_rules! mask_vec_buffer {
        ($vals:expr, $variant:ident) => {{
            let result: Vec<Option<_>> = (0..vlen)
                .filter_map(|i| match recycled_mask(i) {
                    Some(true) => Some($vals.get_opt(i)),
                    Some(false) => None,
                    None => Some(None), // NA in mask -> NA in result
                })
                .collect();
            Ok(RValue::vec(Vector::$variant(result.into())))
        }};
    }

    match v {
        Vector::Raw(vals) => {
            let result: Vec<u8> = (0..vlen)
                .filter_map(|i| match recycled_mask(i) {
                    Some(true) => Some(vals.get(i).copied().unwrap_or(0)),
                    Some(false) => None,
                    None => Some(0u8), // Raw has no NA representation
                })
                .collect();
            Ok(RValue::vec(Vector::Raw(result)))
        }
        Vector::Double(vals) => mask_vec_buffer!(vals, Double),
        Vector::Integer(vals) => mask_vec_buffer!(vals, Integer),
        Vector::Logical(vals) => mask_vec_option!(vals, Logical),
        Vector::Complex(vals) => mask_vec_option!(vals, Complex),
        Vector::Character(vals) => mask_vec_option!(vals, Character),
    }
}

/// Resolve a row or column index against dimension size and optional dimnames.
/// Returns 0-based indices.
fn resolve_dim_index(
    idx: &Option<RValue>,
    dim_size: usize,
    dim_names: &Option<Vec<String>>,
) -> Result<Vec<usize>, RFlow> {
    match idx {
        None => Ok((0..dim_size).collect()),
        Some(RValue::Vector(rv)) => {
            // Logical indices: filter by mask (recycled to dim_size)
            if let Vector::Logical(mask) = &rv.inner {
                let result: Vec<usize> = (0..dim_size)
                    .filter(|&i| {
                        if mask.is_empty() {
                            false
                        } else {
                            mask[i % mask.len()].unwrap_or(false)
                        }
                    })
                    .collect();
                return Ok(result);
            }
            // Character indices: look up in dimnames
            if matches!(rv.inner, Vector::Character(_)) {
                let names = dim_names.as_ref().ok_or(IndexingError::NoDimnames)?;
                let chars = rv.inner.to_characters();
                let mut result = Vec::new();
                for ch in chars.into_iter().flatten() {
                    let pos = names
                        .iter()
                        .position(|n| n == &ch)
                        .ok_or_else(|| IndexingError::SubscriptOutOfBounds { name: ch.clone() })?;
                    result.push(pos);
                }
                return Ok(result);
            }
            // Numeric indices (positive or negative)
            let ints = rv.to_integers();
            let nonzero: Vec<Option<i64>> = ints
                .iter()
                .filter(|x| !matches!(x, Some(0)))
                .copied()
                .collect();
            let has_neg = nonzero.iter().any(|x| x.map(|i| i < 0).unwrap_or(false));
            if has_neg {
                let exclude: Vec<usize> = nonzero
                    .iter()
                    .filter_map(|x| x.and_then(|i| usize::try_from(-i).ok()))
                    .collect();
                return Ok((0..dim_size)
                    .filter(|i| !exclude.contains(&(i + 1)))
                    .collect());
            }
            Ok(nonzero
                .iter()
                .filter_map(|x| x.and_then(|i| usize::try_from(i - 1).ok()))
                .collect())
        }
        _ => Err(IndexingError::InvalidSubscriptType.into()),
    }
}

/// Subset row.names from a data frame list using 1-based integer indices.
/// If the original has row names, select the corresponding ones.
/// Otherwise generate fresh 1-based row names.
fn subset_row_names(list: &RList, int_rows: &[Option<i64>]) -> RValue {
    if let Some(rn_attr) = list.get_attr("row.names") {
        if let Some(rn_vec) = rn_attr.as_vector() {
            let orig = rn_vec.to_characters();
            let selected: Vec<Option<String>> = int_rows
                .iter()
                .map(|idx| {
                    idx.and_then(|i| {
                        let i = usize::try_from(i - 1).ok()?;
                        orig.get(i).cloned().flatten()
                    })
                })
                .collect();
            return RValue::vec(Vector::Character(selected.into()));
        }
    }
    // Fallback: generate 1-based row names
    let row_names: Vec<Option<i64>> = (1..=i64::try_from(int_rows.len()).unwrap_or(0))
        .map(Some)
        .collect();
    RValue::vec(Vector::Integer(row_names.into()))
}

/// Extract a dimnames component (row names at index 0, col names at index 1).
fn extract_dim_names(dimnames: Option<&RValue>, dim: usize) -> Option<Vec<String>> {
    let RValue::List(list) = dimnames? else {
        return None;
    };
    let (_, val) = list.values.get(dim)?;
    let vec = val.as_vector()?;
    let chars = vec.to_characters();
    let names: Vec<String> = chars.into_iter().flatten().collect();
    if names.is_empty() {
        None
    } else {
        Some(names)
    }
}

pub(super) fn eval_index_double(
    interp: &Interpreter,
    object: &Expr,
    indices: &[Arg],
    env: &Environment,
) -> Result<RValue, RFlow> {
    let obj = interp.eval_in(object, env)?;
    if indices.is_empty() {
        return Ok(obj);
    }

    let idx_val = if let Some(val_expr) = &indices[0].value {
        interp.eval_in(val_expr, env)?
    } else {
        return Ok(obj);
    };

    match &obj {
        RValue::List(list) => match &idx_val {
            RValue::Vector(rv) if matches!(rv.inner, Vector::Character(_)) => {
                let Vector::Character(names) = &rv.inner else {
                    unreachable!()
                };
                if let Some(Some(name)) = names.first() {
                    for (n, v) in &list.values {
                        if n.as_ref() == Some(name) {
                            return Ok(v.clone());
                        }
                    }
                }
                Ok(RValue::Null)
            }
            RValue::Vector(v) => {
                let i = usize::try_from(v.as_integer_scalar().unwrap_or(0)).unwrap_or(0);
                if i > 0 && i <= list.values.len() {
                    Ok(list.values[i - 1].1.clone())
                } else {
                    Ok(RValue::Null)
                }
            }
            _ => Ok(RValue::Null),
        },
        RValue::Vector(v) => {
            // Character indexing: look up name in the "names" attribute
            if let RValue::Vector(iv) = &idx_val {
                if let Vector::Character(idx_names) = &iv.inner {
                    if let Some(Some(name)) = idx_names.first() {
                        if let Some(names_attr) = v.get_attr("names") {
                            if let Some(names_vec) = names_attr.as_vector() {
                                let name_strs = names_vec.to_characters();
                                for (j, n) in name_strs.iter().enumerate() {
                                    if n.as_deref() == Some(name.as_str()) && j < v.len() {
                                        return Ok(extract_vector_element(v, j));
                                    }
                                }
                            }
                        }
                        return Ok(RValue::Null);
                    }
                }
            }
            let i = match &idx_val {
                RValue::Vector(iv) => {
                    usize::try_from(iv.as_integer_scalar().unwrap_or(0)).unwrap_or(0)
                }
                _ => 0,
            };
            if i > 0 && i <= v.len() {
                Ok(extract_vector_element(v, i - 1))
            } else {
                Ok(RValue::Null)
            }
        }
        RValue::Language(lang) => {
            let i = match &idx_val {
                RValue::Vector(iv) => {
                    usize::try_from(iv.as_integer_scalar().unwrap_or(0)).unwrap_or(0)
                }
                _ => 0,
            };
            lang.language_element(i).ok_or_else(|| {
                RFlow::Error(RError::new(
                    RErrorKind::Index,
                    format!(
                        "subscript out of bounds: index {} into language object of length {}",
                        i,
                        lang.language_length()
                    ),
                ))
            })
        }
        RValue::Environment(target_env) => {
            // env[["key"]] — look up a variable in the environment
            if let Some(name) = idx_val.as_vector().and_then(|v| v.as_character_scalar()) {
                Ok(target_env.get(&name).unwrap_or(RValue::Null))
            } else {
                Ok(RValue::Null)
            }
        }
        _ => Err(IndexingError::NotSubsettable.into()),
    }
}

/// Extract a single element from an RVector at `idx` (0-based).
pub fn extract_vector_element(v: &RVector, idx: usize) -> RValue {
    match &v.inner {
        Vector::Raw(vals) => RValue::vec(Vector::Raw(vec![vals[idx]])),
        Vector::Double(vals) => RValue::vec(Vector::Double(vec![vals.get_opt(idx)].into())),
        Vector::Integer(vals) => RValue::vec(Vector::Integer(vec![vals.get_opt(idx)].into())),
        Vector::Logical(vals) => RValue::vec(Vector::Logical(vec![vals[idx]].into())),
        Vector::Complex(vals) => RValue::vec(Vector::Complex(vec![vals[idx]].into())),
        Vector::Character(vals) => RValue::vec(Vector::Character(vec![vals[idx].clone()].into())),
    }
}

pub(super) fn eval_dollar(
    interp: &Interpreter,
    object: &Expr,
    member: &str,
    env: &Environment,
) -> Result<RValue, RFlow> {
    let obj = interp.eval_in(object, env)?;
    match &obj {
        RValue::List(list) => {
            for (name, val) in &list.values {
                if name.as_deref() == Some(member) {
                    return Ok(val.clone());
                }
            }
            Ok(RValue::Null)
        }
        RValue::Environment(e) => e
            .get(member)
            .ok_or_else(|| RError::new(RErrorKind::Name, member.to_string()).into()),
        _ => Ok(RValue::Null),
    }
}
