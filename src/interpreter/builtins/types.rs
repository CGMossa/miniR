//! Type-checking builtins: `is.null`, `is.na`, `is.numeric`, etc.
//!
//! Each function tests whether an R value belongs to a particular type
//! or satisfies a type predicate, returning a logical scalar or vector.

use crate::interpreter::value::*;
use minir_macros::builtin;

use super::{get_dim_ints, has_class};

/// Test if an object is NULL.
///
/// Also registered as stubs for is.ordered, is.call, is.symbol,
/// is.name, is.expression, and is.pairlist (all return FALSE for now).
///
/// @param x object to test
/// @return logical scalar
#[builtin(min_args = 1, names = ["is.ordered", "is.call", "is.symbol", "is.name", "is.expression", "is.pairlist"])]
fn builtin_is_null(args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    let r = matches!(args.first(), Some(RValue::Null));
    Ok(RValue::vec(Vector::Logical(vec![Some(r)].into())))
}

/// Test if an object is an environment.
///
/// @param x object to test
/// @return logical scalar
#[builtin(name = "is.environment", min_args = 1)]
fn builtin_is_environment(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let r = matches!(args.first(), Some(RValue::Environment(_)));
    Ok(RValue::vec(Vector::Logical(vec![Some(r)].into())))
}

/// Test if an object is a language object (unevaluated expression).
///
/// @param x object to test
/// @return logical scalar
#[builtin(name = "is.language", min_args = 1)]
fn builtin_is_language(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let r = matches!(args.first(), Some(RValue::Language(_)));
    Ok(RValue::vec(Vector::Logical(vec![Some(r)].into())))
}

/// Test which elements are NA (missing values).
///
/// For doubles, NaN is also considered NA.
///
/// @param x vector to test
/// @return logical vector of the same length
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

/// Test if an object is numeric (integer or double).
///
/// @param x object to test
/// @return logical scalar
#[builtin(min_args = 1)]
fn builtin_is_numeric(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let r = matches!(
        args.first(),
        Some(RValue::Vector(rv)) if matches!(rv.inner, Vector::Double(_) | Vector::Integer(_))
    );
    Ok(RValue::vec(Vector::Logical(vec![Some(r)].into())))
}

/// Test if an object is a character vector.
///
/// @param x object to test
/// @return logical scalar
#[builtin(min_args = 1)]
fn builtin_is_character(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let r = matches!(args.first(), Some(RValue::Vector(rv)) if matches!(rv.inner, Vector::Character(_)));
    Ok(RValue::vec(Vector::Logical(vec![Some(r)].into())))
}

/// Test if an object is a logical vector.
///
/// @param x object to test
/// @return logical scalar
#[builtin(min_args = 1)]
fn builtin_is_logical(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let r =
        matches!(args.first(), Some(RValue::Vector(rv)) if matches!(rv.inner, Vector::Logical(_)));
    Ok(RValue::vec(Vector::Logical(vec![Some(r)].into())))
}

/// Test if an object is an integer vector.
///
/// @param x object to test
/// @return logical scalar
#[builtin(min_args = 1)]
fn builtin_is_integer(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let r =
        matches!(args.first(), Some(RValue::Vector(rv)) if matches!(rv.inner, Vector::Integer(_)));
    Ok(RValue::vec(Vector::Logical(vec![Some(r)].into())))
}

/// Test if an object is a double (real-valued) vector.
///
/// @param x object to test
/// @return logical scalar
#[builtin(min_args = 1)]
fn builtin_is_double(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let r =
        matches!(args.first(), Some(RValue::Vector(rv)) if matches!(rv.inner, Vector::Double(_)));
    Ok(RValue::vec(Vector::Logical(vec![Some(r)].into())))
}

/// Test if an object is a function.
///
/// Also aliased as `is.primitive`.
///
/// @param x object to test
/// @return logical scalar
#[builtin(min_args = 1, names = ["is.primitive"])]
fn builtin_is_function(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let r = matches!(args.first(), Some(RValue::Function(_)));
    Ok(RValue::vec(Vector::Logical(vec![Some(r)].into())))
}

/// Test if an object is a vector with no attributes other than names.
///
/// Returns TRUE for atomic vectors and lists that have no attributes
/// beyond "names".
///
/// @param x object to test
/// @return logical scalar
#[builtin(min_args = 1)]
fn builtin_is_vector(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    // R's is.vector returns TRUE only if the vector has no attributes other than "names"
    let r = match args.first() {
        Some(RValue::Vector(rv)) => match &rv.attrs {
            None => true,
            Some(attrs) => attrs.keys().all(|k| k == "names"),
        },
        Some(RValue::List(l)) => match &l.attrs {
            None => true,
            Some(attrs) => attrs.keys().all(|k| k == "names"),
        },
        _ => false,
    };
    Ok(RValue::vec(Vector::Logical(vec![Some(r)].into())))
}

/// Test if an object is a list.
///
/// @param x object to test
/// @return logical scalar
#[builtin(min_args = 1)]
fn builtin_is_list(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let r = matches!(args.first(), Some(RValue::List(_)));
    Ok(RValue::vec(Vector::Logical(vec![Some(r)].into())))
}

/// Test if an object is recursive (list or environment).
///
/// @param x object to test
/// @return logical scalar
#[builtin(min_args = 1)]
fn builtin_is_recursive(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    // R's is.recursive: TRUE for lists and environments
    let r = matches!(
        args.first(),
        Some(RValue::List(_)) | Some(RValue::Environment(_))
    );
    Ok(RValue::vec(Vector::Logical(vec![Some(r)].into())))
}

/// Test if an object is atomic (vector or NULL).
///
/// @param x object to test
/// @return logical scalar
#[builtin(min_args = 1)]
fn builtin_is_atomic(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    // R's is.atomic: TRUE for atomic vectors and NULL
    let r = matches!(args.first(), Some(RValue::Vector(_)) | Some(RValue::Null));
    Ok(RValue::vec(Vector::Logical(vec![Some(r)].into())))
}

/// Test which elements are finite (not Inf, -Inf, NaN, or NA).
///
/// @param x numeric vector to test
/// @return logical vector of the same length
#[builtin(min_args = 1)]
fn builtin_is_finite(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(v)) => {
            let result: Vec<Option<bool>> = match &v.inner {
                Vector::Double(vals) => vals
                    .iter()
                    .map(|x| Some(x.map(|f| f.is_finite()).unwrap_or(false)))
                    .collect(),
                // Non-NA integers and logicals are always finite
                Vector::Integer(vals) => vals.iter().map(|x| Some(x.is_some())).collect(),
                Vector::Logical(vals) => vals.iter().map(|x| Some(x.is_some())).collect(),
                _ => {
                    return Err(RError::new(
                        RErrorKind::Argument,
                        "default method not implemented for type".to_string(),
                    ))
                }
            };
            Ok(RValue::vec(Vector::Logical(result.into())))
        }
        _ => Ok(RValue::vec(Vector::Logical(vec![Some(false)].into()))),
    }
}

/// Test which elements are infinite (Inf or -Inf).
///
/// @param x numeric vector to test
/// @return logical vector of the same length
#[builtin(min_args = 1)]
fn builtin_is_infinite(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(v)) => {
            let result: Vec<Option<bool>> = match &v.inner {
                Vector::Double(vals) => vals
                    .iter()
                    .map(|x| Some(x.map(|f| f.is_infinite()).unwrap_or(false)))
                    .collect(),
                // Integers and logicals are never infinite
                Vector::Integer(_) | Vector::Logical(_) => {
                    vec![Some(false); v.inner.len()]
                }
                _ => {
                    return Err(RError::new(
                        RErrorKind::Argument,
                        "default method not implemented for type".to_string(),
                    ))
                }
            };
            Ok(RValue::vec(Vector::Logical(result.into())))
        }
        _ => Ok(RValue::vec(Vector::Logical(vec![Some(false)].into()))),
    }
}

/// Test which elements are NaN (not-a-number).
///
/// Unlike `is.na()`, this returns FALSE for NA values that are not NaN.
///
/// @param x numeric vector to test
/// @return logical vector of the same length
#[builtin(min_args = 1)]
fn builtin_is_nan(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(v)) => {
            let result: Vec<Option<bool>> = match &v.inner {
                Vector::Double(vals) => vals
                    .iter()
                    .map(|x| Some(x.map(|f| f.is_nan()).unwrap_or(false)))
                    .collect(),
                // Integers and logicals are never NaN
                Vector::Integer(_) | Vector::Logical(_) => {
                    vec![Some(false); v.inner.len()]
                }
                _ => {
                    return Err(RError::new(
                        RErrorKind::Argument,
                        "default method not implemented for type".to_string(),
                    ))
                }
            };
            Ok(RValue::vec(Vector::Logical(result.into())))
        }
        _ => Ok(RValue::vec(Vector::Logical(vec![Some(false)].into()))),
    }
}

/// Test if an object is a factor.
///
/// @param x object to test
/// @return logical scalar
#[builtin(min_args = 1)]
fn builtin_is_factor(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let r = args.first().is_some_and(|v| has_class(v, "factor"));
    Ok(RValue::vec(Vector::Logical(vec![Some(r)].into())))
}

/// Test if an object is a data frame.
///
/// @param x object to test
/// @return logical scalar
#[builtin(min_args = 1)]
fn builtin_is_data_frame(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let r = args.first().is_some_and(|v| has_class(v, "data.frame"));
    Ok(RValue::vec(Vector::Logical(vec![Some(r)].into())))
}

/// Test if an object is a matrix (has dim attribute of length 2).
///
/// @param x object to test
/// @return logical scalar
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

/// Test if an object is an array.
///
/// @param x object to test
/// @return logical scalar
#[builtin(min_args = 1)]
fn builtin_is_array(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let r = args.first().is_some_and(|v| has_class(v, "array"));
    Ok(RValue::vec(Vector::Logical(vec![Some(r)].into())))
}

/// Test set membership: is each element of el in table?
///
/// Uses numeric comparison for numeric vectors (matching `%in%` behavior),
/// and string comparison when either side is character.
///
/// @param el values to test
/// @param table values to match against
/// @return logical vector of the same length as el
#[builtin(min_args = 2)]
fn builtin_is_element(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    if args.len() < 2 {
        return Err(RError::new(
            RErrorKind::Argument,
            "need 2 arguments".to_string(),
        ));
    }
    let (lv, rv) = match (&args[0], &args[1]) {
        (RValue::Vector(l), RValue::Vector(r)) => (l, r),
        _ => return Ok(RValue::vec(Vector::Logical(vec![Some(false)].into()))),
    };

    // Character comparison when either side is character
    if matches!(lv.inner, Vector::Character(_)) || matches!(rv.inner, Vector::Character(_)) {
        let table = rv.to_characters();
        let vals = lv.to_characters();
        let result: Vec<Option<bool>> = vals
            .iter()
            .map(|xi| {
                Some(
                    xi.as_ref()
                        .is_some_and(|xi| table.iter().any(|t| t.as_ref() == Some(xi))),
                )
            })
            .collect();
        return Ok(RValue::vec(Vector::Logical(result.into())));
    }

    // Numeric comparison (handles int/double/logical via doubles)
    let table = rv.to_doubles();
    let vals = lv.to_doubles();
    let result: Vec<Option<bool>> = vals
        .iter()
        .map(|x| match x {
            Some(v) => Some(table.iter().any(|t| match t {
                Some(t) => (*t == *v) || (t.is_nan() && v.is_nan()),
                None => false,
            })),
            None => Some(table.iter().any(|t| t.is_none())),
        })
        .collect();
    Ok(RValue::vec(Vector::Logical(result.into())))
}

/// Test if x is a single TRUE value.
///
/// Returns TRUE only if x is a length-1 logical vector equal to TRUE (not NA).
///
/// @param x object to test
/// @return logical scalar
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

/// Test if x is a single FALSE value.
///
/// Returns TRUE only if x is a length-1 logical vector equal to FALSE (not NA).
///
/// @param x object to test
/// @return logical scalar
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
