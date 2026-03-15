//! Read-side indexing helpers for vectors, lists, matrices, and data frames.
//! Write-side (replacement) is in `assignment.rs`.

use crate::interpreter::environment::Environment;
use crate::interpreter::value::*;
use crate::interpreter::Interpreter;
use crate::parser::ast::{Arg, Expr};

impl Interpreter {
    pub(super) fn eval_index(
        &self,
        object: &Expr,
        indices: &[Arg],
        env: &Environment,
    ) -> Result<RValue, RFlow> {
        eval_index(self, object, indices, env)
    }

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
                    return Err(RError::new(
                        RErrorKind::Index,
                        "can't mix positive and negative subscripts".to_string(),
                    )
                    .into());
                }
                if has_neg {
                    return index_by_negative(interp, v, &nonzero);
                }
                index_by_integer(interp, v, &nonzero)
            }
            RValue::Null => Ok(obj.clone()),
            _ => Err(RError::new(RErrorKind::Index, "invalid index type".to_string()).into()),
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
                let mut result = Vec::new();
                for i in indices.iter().flatten() {
                    let i = usize::try_from(*i).unwrap_or(0);
                    if i > 0 && i <= list.values.len() {
                        result.push(list.values[i - 1].clone());
                    }
                }
                Ok(RValue::List(RList::new(result)))
            }
            _ => Err(RError::new(RErrorKind::Index, "invalid index type".to_string()).into()),
        },
        _ => Err(RError::new(RErrorKind::Index, "object is not subsettable".to_string()).into()),
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
            return Err(RError::new(
                RErrorKind::Index,
                "incorrect number of dimensions".to_string(),
            )
            .into());
        }
    };

    let dims = match dim_attr {
        Some(RValue::Vector(rv)) => match &rv.inner {
            Vector::Integer(d) => d.0.clone(),
            _ => {
                return Err(RError::new(
                    RErrorKind::Index,
                    "incorrect number of dimensions".to_string(),
                )
                .into());
            }
        },
        _ => {
            return Err(RError::new(
                RErrorKind::Index,
                "incorrect number of dimensions".to_string(),
            )
            .into());
        }
    };

    if dims.len() < 2 {
        return Err(RError::new(
            RErrorKind::Index,
            "incorrect number of dimensions".to_string(),
        )
        .into());
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

    let rows: Vec<usize> = match &row_idx {
        None => (0..nrow).collect(),
        Some(RValue::Vector(rv)) => rv
            .to_integers()
            .iter()
            .filter_map(|x| x.and_then(|i| usize::try_from(i - 1).ok()))
            .collect(),
        _ => return Err(RError::new(RErrorKind::Index, "invalid row index".to_string()).into()),
    };

    let cols: Vec<usize> = match &col_idx {
        None => (0..ncol).collect(),
        Some(RValue::Vector(rv)) => rv
            .to_integers()
            .iter()
            .filter_map(|x| x.and_then(|i| usize::try_from(i - 1).ok()))
            .collect(),
        _ => {
            return Err(RError::new(RErrorKind::Index, "invalid column index".to_string()).into());
        }
    };

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

    if row_idx.is_none() {
        if col_idx.is_some() && selected_cols.len() == 1 {
            return Ok(selected_cols.into_iter().next().unwrap().1);
        }

        let col_names: Vec<Option<String>> = selected_cols.iter().map(|(n, _)| n.clone()).collect();
        let nrows = selected_cols.first().map(|(_, v)| v.length()).unwrap_or(0);
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
        let row_names: Vec<Option<i64>> =
            (1..=i64::try_from(nrows).unwrap_or(0)).map(Some).collect();
        result.set_attr(
            "row.names".to_string(),
            RValue::vec(Vector::Integer(row_names.into())),
        );
        return Ok(RValue::List(result));
    }

    let int_rows: Vec<Option<i64>> = match &row_idx {
        Some(RValue::Vector(rv)) if matches!(rv.inner, Vector::Logical(_)) => {
            let Vector::Logical(lv) = &rv.inner else {
                unreachable!()
            };
            lv.iter()
                .enumerate()
                .filter(|(_, v)| v.unwrap_or(false))
                .filter_map(|(i, _)| i64::try_from(i).ok().map(|i| Some(i + 1)))
                .collect()
        }
        Some(RValue::Vector(rv)) => rv.to_integers(),
        _ => vec![],
    };

    if selected_cols.len() == 1 {
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
    let nrows = result_cols.first().map(|(_, v)| v.length()).unwrap_or(0);
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
    let row_names: Vec<Option<i64>> = (1..=i64::try_from(nrows).unwrap_or(0)).map(Some).collect();
    result.set_attr(
        "row.names".to_string(),
        RValue::vec(Vector::Integer(row_names.into())),
    );
    Ok(RValue::List(result))
}

pub(crate) fn index_by_integer(
    _interp: &Interpreter,
    v: &Vector,
    indices: &[Option<i64>],
) -> Result<RValue, RFlow> {
    macro_rules! index_vec {
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
        Vector::Double(vals) => index_vec!(vals, Double),
        Vector::Integer(vals) => index_vec!(vals, Integer),
        Vector::Logical(vals) => index_vec!(vals, Logical),
        Vector::Complex(vals) => index_vec!(vals, Complex),
        Vector::Character(vals) => index_vec!(vals, Character),
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

    macro_rules! filter_vec {
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
        Vector::Double(vals) => filter_vec!(vals, Double),
        Vector::Integer(vals) => filter_vec!(vals, Integer),
        Vector::Logical(vals) => filter_vec!(vals, Logical),
        Vector::Complex(vals) => filter_vec!(vals, Complex),
        Vector::Character(vals) => filter_vec!(vals, Character),
    }
}

fn index_by_logical(
    _interp: &Interpreter,
    v: &Vector,
    mask: &[Option<bool>],
) -> Result<RValue, RFlow> {
    let vlen = v.len();
    // Recycle mask to target length (R's behavior)
    let test = |i: usize| -> bool {
        if mask.is_empty() {
            return false;
        }
        mask[i % mask.len()].unwrap_or(false)
    };

    macro_rules! mask_vec {
        ($vals:expr, $variant:ident) => {{
            let result: Vec<_> = $vals
                .iter()
                .enumerate()
                .filter(|(i, _)| test(*i))
                .map(|(_, v)| v.clone())
                .collect();
            Ok(RValue::vec(Vector::$variant(result.into())))
        }};
    }

    // If mask is longer than vector, R extends with NA — we just iterate up to vlen
    let _ = vlen;
    match v {
        Vector::Raw(vals) => {
            let result: Vec<u8> = vals
                .iter()
                .enumerate()
                .filter(|(i, _)| test(*i))
                .map(|(_, &v)| v)
                .collect();
            Ok(RValue::vec(Vector::Raw(result)))
        }
        Vector::Double(vals) => mask_vec!(vals, Double),
        Vector::Integer(vals) => mask_vec!(vals, Integer),
        Vector::Logical(vals) => mask_vec!(vals, Logical),
        Vector::Complex(vals) => mask_vec!(vals, Complex),
        Vector::Character(vals) => mask_vec!(vals, Character),
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
            let i = match &idx_val {
                RValue::Vector(iv) => {
                    usize::try_from(iv.as_integer_scalar().unwrap_or(0)).unwrap_or(0)
                }
                _ => 0,
            };
            if i > 0 && i <= v.len() {
                let idx = i - 1;
                match &v.inner {
                    Vector::Raw(vals) => Ok(RValue::vec(Vector::Raw(vec![vals[idx]]))),
                    Vector::Double(vals) => Ok(RValue::vec(Vector::Double(vec![vals[idx]].into()))),
                    Vector::Integer(vals) => {
                        Ok(RValue::vec(Vector::Integer(vec![vals[idx]].into())))
                    }
                    Vector::Logical(vals) => {
                        Ok(RValue::vec(Vector::Logical(vec![vals[idx]].into())))
                    }
                    Vector::Complex(vals) => {
                        Ok(RValue::vec(Vector::Complex(vec![vals[idx]].into())))
                    }
                    Vector::Character(vals) => Ok(RValue::vec(Vector::Character(
                        vec![vals[idx].clone()].into(),
                    ))),
                }
            } else {
                Ok(RValue::Null)
            }
        }
        _ => Err(RError::new(RErrorKind::Index, "object is not subsettable".to_string()).into()),
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
