//! Table and tabulate builtins — contingency tables and integer bin counting.

use crate::interpreter::value::*;
use newr_macros::builtin;

use super::factors::rvalue_to_char_vec;

/// `tabulate(bin, nbins)` — count occurrences of each integer value 1..nbins.
#[builtin(min_args = 1)]
fn builtin_tabulate(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let bins: Vec<Option<i64>> = match &args[0] {
        RValue::Vector(rv) => match &rv.inner {
            Vector::Integer(v) => v.to_vec(),
            Vector::Double(v) => v.iter().map(|d| d.map(|f| f as i64)).collect(),
            _ => {
                return Err(RError::Type(
                    "tabulate() requires an integer or double vector".to_string(),
                ))
            }
        },
        _ => {
            return Err(RError::Type(
                "tabulate() requires a numeric vector".to_string(),
            ))
        }
    };

    let max_bin = bins.iter().filter_map(|b| *b).max().unwrap_or(0);
    let nbins = match args.get(1) {
        Some(RValue::Vector(rv)) => rv.inner.as_integer_scalar().unwrap_or(max_bin).max(0) as usize,
        _ => max_bin.max(0) as usize,
    };

    let mut counts = vec![0i64; nbins];
    for v in bins.iter().flatten() {
        let idx = *v - 1;
        if idx >= 0 && (idx as usize) < nbins {
            counts[idx as usize] += 1;
        }
    }

    Ok(RValue::vec(Vector::Integer(
        counts.into_iter().map(Some).collect::<Vec<_>>().into(),
    )))
}

/// `table(...)` — contingency table (one-way for now).
///
/// For a single vector, counts occurrences of each unique value.
/// Returns a named integer vector with class "table".
#[builtin]
fn builtin_table(args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    if args.is_empty() {
        return Ok(RValue::Null);
    }

    let vals = rvalue_to_char_vec(&args[0])?;

    // Count unique values, sorted
    let mut order: Vec<String> = Vec::new();
    let mut counts: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    for s in vals.iter().flatten() {
        *counts.entry(s.clone()).or_insert(0) += 1;
        if !order.contains(s) {
            order.push(s.clone());
        }
    }
    order.sort();

    let names: Vec<Option<String>> = order.iter().map(|s| Some(s.clone())).collect();
    let values: Vec<Option<i64>> = order.iter().map(|s| Some(counts[s])).collect();

    let mut rv = RVector::from(Vector::Integer(values.into()));
    rv.set_attr(
        "names".to_string(),
        RValue::vec(Vector::Character(names.into())),
    );
    rv.set_attr(
        "class".to_string(),
        RValue::vec(Vector::Character(vec![Some("table".to_string())].into())),
    );
    Ok(RValue::Vector(rv))
}
