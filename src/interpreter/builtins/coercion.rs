//! Coercion builtins: `as.integer`, `as.double`, `as.character`, etc.
//!
//! Each function coerces an R value to a specific type, following R's
//! standard coercion rules.

use crate::interpreter::value::*;
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
