//! Table and tabulate builtins — contingency tables and integer bin counting.

use crate::interpreter::coerce::f64_to_i64;
use crate::interpreter::value::*;
use itertools::Itertools;
use minir_macros::builtin;

use super::factors::rvalue_to_char_vec;

/// `tabulate(bin, nbins)` — count occurrences of each integer value 1..nbins.
#[builtin(min_args = 1)]
fn builtin_tabulate(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let bins: Vec<Option<i64>> = match &args[0] {
        RValue::Vector(rv) => match &rv.inner {
            Vector::Integer(v) => v.to_option_vec(),
            Vector::Double(v) => v
                .iter_opt()
                .map(|d| d.map(f64_to_i64).transpose())
                .collect::<Result<Vec<_>, _>>()?,
            _ => {
                return Err(RError::new(
                    RErrorKind::Type,
                    "tabulate() requires an integer or double vector".to_string(),
                ))
            }
        },
        _ => {
            return Err(RError::new(
                RErrorKind::Type,
                "tabulate() requires a numeric vector".to_string(),
            ))
        }
    };

    let max_bin = bins.iter().filter_map(|b| *b).max().unwrap_or(0);
    let nbins = match args.get(1) {
        Some(RValue::Vector(rv)) => {
            usize::try_from(rv.inner.as_integer_scalar().unwrap_or(max_bin).max(0))?
        }
        _ => usize::try_from(max_bin.max(0))?,
    };

    let mut counts = vec![0i64; nbins];
    for v in bins.iter().flatten() {
        let idx = *v - 1;
        if idx >= 0 {
            if let Ok(uidx) = usize::try_from(idx) {
                if uidx < nbins {
                    counts[uidx] += 1;
                }
            }
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
    let counts = vals.iter().flatten().counts();
    let order: Vec<&String> = counts.keys().sorted().copied().collect();

    let names: Vec<Option<String>> = order.iter().map(|s| Some((*s).clone())).collect();
    let values: Vec<Option<i64>> = order
        .iter()
        .map(|s| Some(i64::try_from(counts[*s]).unwrap_or(0)))
        .collect();

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
