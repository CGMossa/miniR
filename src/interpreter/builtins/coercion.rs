//! Coercion builtins: `as.integer`, `as.double`, `as.character`, etc.
//!
//! Each function coerces an R value to a specific type, following R's
//! standard coercion rules.

use crate::interpreter::value::*;
use indexmap::IndexMap;
use minir_macros::builtin;

/// Coerce an object to double (numeric).
///
/// Also aliased as `as.double`.
///
/// @param x object to coerce
/// @return double vector
#[builtin(min_args = 1, names = ["as.double"])]
fn builtin_as_numeric(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(v)) => Ok(RValue::vec(Vector::Double(v.to_doubles().into()))),
        Some(RValue::Null) => Ok(RValue::vec(Vector::Double(vec![].into()))),
        _ => Ok(RValue::vec(Vector::Double(vec![None].into()))),
    }
}

/// Coerce an object to integer.
///
/// Doubles are truncated toward zero.
///
/// @param x object to coerce
/// @return integer vector
#[builtin(min_args = 1)]
fn builtin_as_integer(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(v)) => Ok(RValue::vec(Vector::Integer(v.to_integers().into()))),
        Some(RValue::Null) => Ok(RValue::vec(Vector::Integer(vec![].into()))),
        _ => Ok(RValue::vec(Vector::Integer(vec![None].into()))),
    }
}

/// Coerce an object to character (string).
///
/// For factors, maps integer codes back to level labels rather than
/// stringifying the codes.
///
/// @param x object to coerce
/// @return character vector
#[builtin(min_args = 1)]
fn builtin_as_character(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(v)) => {
            // Check if the vector is a factor — if so, map codes to level labels
            if is_factor(v) {
                return factor_to_character(v);
            }
            Ok(RValue::vec(Vector::Character(v.to_characters().into())))
        }
        Some(RValue::Null) => Ok(RValue::vec(Vector::Character(vec![].into()))),
        _ => Ok(RValue::vec(Vector::Character(vec![None].into()))),
    }
}

/// Check whether an `RVector` has class "factor".
fn is_factor(v: &RVector) -> bool {
    if let Some(RValue::Vector(cls_vec)) = v.get_attr("class") {
        if let Vector::Character(cls) = &cls_vec.inner {
            return cls.iter().any(|c| c.as_deref() == Some("factor"));
        }
    }
    false
}

/// Convert a factor (integer codes + levels attr) to a character vector of labels.
fn factor_to_character(v: &RVector) -> Result<RValue, RError> {
    let levels: Vec<Option<String>> = match v.get_attr("levels") {
        Some(RValue::Vector(lv)) => match &lv.inner {
            Vector::Character(c) => c.to_vec(),
            _ => vec![],
        },
        _ => vec![],
    };

    let codes = v.inner.to_integers();
    let labels: Vec<Option<String>> = codes
        .iter()
        .map(|code| {
            code.and_then(|i| {
                let idx = usize::try_from(i).ok()?.checked_sub(1)?;
                levels.get(idx).cloned().flatten()
            })
        })
        .collect();

    Ok(RValue::vec(Vector::Character(labels.into())))
}

/// Coerce an object to logical.
///
/// @param x object to coerce
/// @return logical vector
#[builtin(min_args = 1)]
fn builtin_as_logical(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(v)) => Ok(RValue::vec(Vector::Logical(v.to_logicals().into()))),
        Some(RValue::Null) => Ok(RValue::vec(Vector::Logical(vec![].into()))),
        _ => Ok(RValue::vec(Vector::Logical(vec![None].into()))),
    }
}

/// Coerce an object to a list.
///
/// Atomic vectors are split into single-element list entries.
///
/// @param x object to coerce
/// @return list
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

/// Coerce an object to a matrix.
///
/// For vectors, creates a single-column matrix. For data frames, converts
/// all columns to a common type and combines into a matrix.
///
/// @param x object to coerce
/// @return matrix (vector with dim attribute)
/// @namespace base
#[builtin(name = "as.matrix", min_args = 1)]
fn builtin_as_matrix(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(v)) => {
            let len = v.len();
            let mut rv = v.clone();
            rv.set_attr(
                "dim".to_string(),
                RValue::vec(Vector::Integer(vec![Some(len as i64), Some(1)].into())),
            );
            Ok(RValue::Vector(rv))
        }
        Some(RValue::List(list)) => {
            // Data frame → matrix: convert all columns to double, combine column-major
            let ncol = list.values.len();
            if ncol == 0 {
                return Ok(RValue::vec(Vector::Double(vec![].into())));
            }
            let nrow = list
                .values
                .first()
                .map(|(_, v)| match v {
                    RValue::Vector(rv) => rv.len(),
                    _ => 1,
                })
                .unwrap_or(0);

            let mut data: Vec<Option<f64>> = Vec::with_capacity(nrow * ncol);
            for (_, val) in &list.values {
                if let Some(v) = val.as_vector() {
                    let doubles = v.to_doubles();
                    data.extend(&doubles);
                } else {
                    for _ in 0..nrow {
                        data.push(None);
                    }
                }
            }
            let mut rv = RVector::from(Vector::Double(data.into()));
            rv.set_attr(
                "dim".to_string(),
                RValue::vec(Vector::Integer(
                    vec![Some(nrow as i64), Some(ncol as i64)].into(),
                )),
            );
            // Copy column names as dimnames
            let col_names: Vec<Option<String>> =
                list.values.iter().map(|(n, _)| n.clone()).collect();
            if col_names.iter().any(|n| n.is_some()) {
                let dimnames = RValue::List(RList::new(vec![
                    (None, RValue::Null),
                    (None, RValue::vec(Vector::Character(col_names.into()))),
                ]));
                rv.set_attr("dimnames".to_string(), dimnames);
            }
            Ok(RValue::Vector(rv))
        }
        Some(RValue::Null) => Ok(RValue::vec(Vector::Double(vec![].into()))),
        _ => Ok(RValue::vec(Vector::Double(vec![None].into()))),
    }
}

/// Coerce an object to a data frame.
///
/// @param x object to coerce
/// @param row.names optional row names
/// @return data.frame
/// @namespace base
#[builtin(name = "as.data.frame", min_args = 1)]
fn builtin_as_data_frame(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::List(list)) => {
            // Already a list — add data.frame class if not present
            let mut list = list.clone();
            let mut attrs = *list.attrs.take().unwrap_or_default();
            attrs.insert(
                "class".to_string(),
                RValue::vec(Vector::Character(
                    vec![Some("data.frame".to_string())].into(),
                )),
            );
            // Add row.names if missing
            if !attrs.contains_key("row.names") {
                let nrow = list
                    .values
                    .first()
                    .map(|(_, v)| match v {
                        RValue::Vector(rv) => rv.len(),
                        _ => 1,
                    })
                    .unwrap_or(0);
                attrs.insert(
                    "row.names".to_string(),
                    RValue::vec(Vector::Integer(
                        (1..=nrow as i64).map(Some).collect::<Vec<_>>().into(),
                    )),
                );
            }
            list.attrs = Some(Box::new(attrs));
            Ok(RValue::List(list))
        }
        Some(RValue::Vector(v)) => {
            // Single vector → single-column data frame
            let col_name = named
                .iter()
                .find(|(n, _)| n == "col.names")
                .and_then(|(_, v)| v.as_vector()?.as_character_scalar())
                .unwrap_or_else(|| "V1".to_string());
            let nrow = v.len();
            let mut list = RList::new(vec![(Some(col_name.clone()), RValue::Vector(v.clone()))]);
            let mut attrs: IndexMap<String, RValue> = IndexMap::new();
            attrs.insert(
                "class".to_string(),
                RValue::vec(Vector::Character(
                    vec![Some("data.frame".to_string())].into(),
                )),
            );
            attrs.insert(
                "names".to_string(),
                RValue::vec(Vector::Character(vec![Some(col_name)].into())),
            );
            attrs.insert(
                "row.names".to_string(),
                RValue::vec(Vector::Integer(
                    (1..=nrow as i64).map(Some).collect::<Vec<_>>().into(),
                )),
            );
            list.attrs = Some(Box::new(attrs));
            Ok(RValue::List(list))
        }
        Some(RValue::Null) => {
            let list = RList::new(vec![]);
            let mut attrs: IndexMap<String, RValue> = IndexMap::new();
            attrs.insert(
                "class".to_string(),
                RValue::vec(Vector::Character(
                    vec![Some("data.frame".to_string())].into(),
                )),
            );
            attrs.insert(
                "row.names".to_string(),
                RValue::vec(Vector::Integer(vec![].into())),
            );
            let mut list = list;
            list.attrs = Some(Box::new(attrs));
            Ok(RValue::List(list))
        }
        _ => Err(RError::new(
            RErrorKind::Argument,
            "cannot coerce to data.frame".to_string(),
        )),
    }
}

/// Coerce an object to a factor.
///
/// @param x object to coerce
/// @return factor
/// @namespace base
#[builtin(name = "as.factor", min_args = 1)]
fn builtin_as_factor(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    // Convert to character first, then create a factor
    let chars = match args.first() {
        Some(RValue::Vector(v)) => {
            if is_factor(v) {
                return Ok(RValue::Vector(v.clone()));
            }
            v.to_characters()
        }
        _ => vec![],
    };

    // Get unique levels
    let mut levels: Vec<String> = Vec::new();
    for s in chars.iter().flatten() {
        if !levels.contains(s) {
            levels.push(s.clone());
        }
    }
    levels.sort();

    // Map values to integer codes
    let codes: Vec<Option<i64>> = chars
        .iter()
        .map(|c| {
            c.as_ref()
                .and_then(|s| levels.iter().position(|l| l == s).map(|i| (i + 1) as i64))
        })
        .collect();

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

/// Coerce a string to a symbol/name.
///
/// @param x character scalar
/// @return name (Language wrapping Expr::Symbol)
/// @namespace base
#[builtin(name = "as.name", min_args = 1, names = ["as.symbol"])]
fn builtin_as_name(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let name = match args.first() {
        Some(RValue::Vector(v)) => v.as_character_scalar().unwrap_or_default(),
        _ => String::new(),
    };
    Ok(RValue::Language(Language::new(
        crate::parser::ast::Expr::Symbol(name),
    )))
}

/// Look up a function by name or return it if already a function.
///
/// @param FUN function or character string naming a function
/// @return the function
/// @namespace base
#[builtin(name = "match.fun", min_args = 1)]
fn builtin_match_fun(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Function(_)) => Ok(args[0].clone()),
        Some(RValue::Vector(v)) => {
            let name = v.as_character_scalar().ok_or_else(|| {
                RError::new(
                    RErrorKind::Argument,
                    "'FUN' must be a function or a character string".to_string(),
                )
            })?;
            // Can't do environment lookup from a plain builtin — return an error
            // suggesting the user pass the function directly
            Err(RError::new(
                RErrorKind::Other,
                format!(
                    "match.fun cannot resolve '{}' — pass the function directly",
                    name
                ),
            ))
        }
        _ => Err(RError::new(
            RErrorKind::Argument,
            "'FUN' must be a function or a character string".to_string(),
        )),
    }
}

/// Coerce an object to a vector, stripping all attributes.
///
/// @param x object to coerce
/// @return the object with all attributes removed
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
