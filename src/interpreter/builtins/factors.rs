//! Factor builtins — factor construction, levels, and nlevels.
//!
//! A factor is an integer vector with a "levels" attribute (character) and
//! class "factor" (or c("ordered", "factor") if ordered).

use crate::interpreter::value::*;
use minir_macros::builtin;

/// Coerce an RValue to a character vector for level matching.
pub(super) fn rvalue_to_char_vec(x: &RValue) -> Result<Vec<Option<String>>, RError> {
    match x {
        RValue::Vector(rv) => match &rv.inner {
            Vector::Character(c) => Ok(c.to_vec()),
            Vector::Integer(v) => Ok(v.iter().map(|i| i.map(|n| n.to_string())).collect()),
            Vector::Double(v) => Ok(v.iter().map(|d| d.map(|n| n.to_string())).collect()),
            Vector::Complex(v) => Ok(v.iter().map(|c| c.map(format_r_complex)).collect()),
            Vector::Raw(v) => Ok(v.iter().map(|b| Some(format!("{:02x}", b))).collect()),
            Vector::Logical(v) => Ok(v
                .iter()
                .map(|b| {
                    b.map(|b| {
                        if b {
                            "TRUE".to_string()
                        } else {
                            "FALSE".to_string()
                        }
                    })
                })
                .collect()),
        },
        RValue::Null => Ok(vec![]),
        _ => Err(RError::new(
            RErrorKind::Type,
            "expected an atomic vector".to_string(),
        )),
    }
}

#[builtin]
fn builtin_factor(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let x = args.first().cloned().unwrap_or(RValue::Null);
    let char_vals = rvalue_to_char_vec(&x)?;

    // Get levels: explicit or unique values in order of appearance
    let explicit_levels = named.iter().find(|(n, _)| n == "levels").map(|(_, v)| v);
    let levels: Vec<String> = if let Some(lv) = explicit_levels {
        match lv {
            RValue::Vector(rv) => match &rv.inner {
                Vector::Character(c) => c.iter().filter_map(|s| s.clone()).collect(),
                _ => {
                    return Err(RError::new(
                        RErrorKind::Argument,
                        "levels must be a character vector".to_string(),
                    ))
                }
            },
            RValue::Null => {
                let mut seen = Vec::new();
                for s in char_vals.iter().flatten() {
                    if !seen.contains(s) {
                        seen.push(s.clone());
                    }
                }
                seen
            }
            _ => {
                return Err(RError::new(
                    RErrorKind::Argument,
                    "levels must be a character vector or NULL".to_string(),
                ))
            }
        }
    } else {
        let mut seen = Vec::new();
        for s in char_vals.iter().flatten() {
            if !seen.contains(s) {
                seen.push(s.clone());
            }
        }
        seen.sort();
        seen
    };

    // Get labels (default = levels themselves)
    let labels: Vec<String> = if let Some((_, lbl_val)) = named.iter().find(|(n, _)| n == "labels")
    {
        match lbl_val {
            RValue::Vector(rv) => match &rv.inner {
                Vector::Character(c) => c.iter().filter_map(|s| s.clone()).collect(),
                _ => levels.clone(),
            },
            _ => levels.clone(),
        }
    } else {
        levels.clone()
    };

    let ordered = named
        .iter()
        .find(|(n, _)| n == "ordered")
        .and_then(|(_, v)| match v {
            RValue::Vector(rv) => rv.inner.as_logical_scalar(),
            _ => None,
        })
        .unwrap_or(false);

    // Map each value to its 1-based level index (NA if not in levels)
    let codes: Vec<Option<i64>> = char_vals
        .iter()
        .map(|v| match v {
            Some(s) => levels
                .iter()
                .position(|l| l == s)
                .map(|i| i64::try_from(i + 1))
                .transpose(),
            None => Ok(None),
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut rv = RVector::from(Vector::Integer(codes.into()));
    rv.set_attr(
        "levels".to_string(),
        RValue::vec(Vector::Character(
            labels
                .iter()
                .map(|s| Some(s.clone()))
                .collect::<Vec<_>>()
                .into(),
        )),
    );
    let class = if ordered {
        RValue::vec(Vector::Character(
            vec![Some("ordered".to_string()), Some("factor".to_string())].into(),
        ))
    } else {
        RValue::vec(Vector::Character(vec![Some("factor".to_string())].into()))
    };
    rv.set_attr("class".to_string(), class);

    Ok(RValue::Vector(rv))
}

/// `levels(x)` — get the levels of a factor.
#[builtin(min_args = 1)]
fn builtin_levels(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(rv)) => Ok(rv.get_attr("levels").cloned().unwrap_or(RValue::Null)),
        _ => Ok(RValue::Null),
    }
}

/// `nlevels(x)` — number of levels of a factor.
#[builtin(min_args = 1)]
fn builtin_nlevels(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(rv)) => match rv.get_attr("levels") {
            Some(RValue::Vector(lv)) => Ok(RValue::vec(Vector::Integer(
                vec![Some(i64::try_from(lv.inner.len())?)].into(),
            ))),
            _ => Ok(RValue::vec(Vector::Integer(vec![Some(0i64)].into()))),
        },
        _ => Ok(RValue::vec(Vector::Integer(vec![Some(0i64)].into()))),
    }
}
