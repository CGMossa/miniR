use crate::interpreter::value::*;
use linkme::distributed_slice;
use newr_macros::{builtin, noop_builtin};

noop_builtin!("pmax");
noop_builtin!("pmin");
noop_builtin!("cumall", 1);
noop_builtin!("cumany", 1);
noop_builtin!("runif", 1);
noop_builtin!("rnorm", 1);
noop_builtin!("rbinom", 2);

#[distributed_slice(crate::interpreter::builtins::BUILTIN_REGISTRY)]
static ALIAS_SEQ_INT: (&str, crate::interpreter::builtins::BuiltinFn, usize) =
    ("seq.int", builtin_seq, 0);

// === Math functions ===

#[builtin(min_args = 1)]
fn builtin_abs(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    math_unary(args, f64::abs)
}
#[builtin(min_args = 1)]
fn builtin_sqrt(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    math_unary(args, f64::sqrt)
}
#[builtin(min_args = 1)]
fn builtin_exp(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    math_unary(args, f64::exp)
}
#[builtin(min_args = 1)]
fn builtin_log(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    math_unary(args, f64::ln)
}
#[builtin(min_args = 1)]
fn builtin_log2(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    math_unary(args, f64::log2)
}
#[builtin(min_args = 1)]
fn builtin_log10(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    math_unary(args, f64::log10)
}
#[builtin(min_args = 1)]
fn builtin_ceiling(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    math_unary(args, f64::ceil)
}
#[builtin(min_args = 1)]
fn builtin_floor(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    math_unary(args, f64::floor)
}
#[builtin(min_args = 1)]
fn builtin_trunc(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    math_unary(args, f64::trunc)
}
#[builtin(min_args = 1)]
fn builtin_sin(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    math_unary(args, f64::sin)
}
#[builtin(min_args = 1)]
fn builtin_cos(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    math_unary(args, f64::cos)
}
#[builtin(min_args = 1)]
fn builtin_tan(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    math_unary(args, f64::tan)
}
#[builtin(min_args = 1)]
fn builtin_sign(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    math_unary(args, f64::signum)
}

use super::math_unary_op as math_unary;

#[builtin(min_args = 1)]
fn builtin_round(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let digits = named
        .iter()
        .find(|(n, _)| n == "digits")
        .and_then(|(_, v)| v.as_vector()?.as_integer_scalar())
        .or_else(|| args.get(1)?.as_vector()?.as_integer_scalar())
        .unwrap_or(0);
    let factor = 10f64.powi(digits as i32);
    match args.first() {
        Some(RValue::Vector(v)) => {
            let result: Vec<Option<f64>> = v
                .to_doubles()
                .iter()
                .map(|x| x.map(|f| (f * factor).round() / factor))
                .collect();
            Ok(RValue::Vector(Vector::Double(result.into())))
        }
        _ => Err(RError::Argument("non-numeric argument".to_string())),
    }
}

#[builtin(min_args = 0)]
fn builtin_sum(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let na_rm = named.iter().any(|(n, v)| {
        n == "na.rm" && v.as_vector().and_then(|v| v.as_logical_scalar()) == Some(true)
    });
    let mut total = 0.0;
    for arg in args {
        if let RValue::Vector(v) = arg {
            for x in v.to_doubles() {
                match x {
                    Some(f) => total += f,
                    None if !na_rm => return Ok(RValue::Vector(Vector::Double(vec![None].into()))),
                    None => {}
                }
            }
        }
    }
    Ok(RValue::Vector(Vector::Double(vec![Some(total)].into())))
}

#[builtin(min_args = 0)]
fn builtin_prod(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let na_rm = named.iter().any(|(n, v)| {
        n == "na.rm" && v.as_vector().and_then(|v| v.as_logical_scalar()) == Some(true)
    });
    let mut total = 1.0;
    for arg in args {
        if let RValue::Vector(v) = arg {
            for x in v.to_doubles() {
                match x {
                    Some(f) => total *= f,
                    None if !na_rm => return Ok(RValue::Vector(Vector::Double(vec![None].into()))),
                    None => {}
                }
            }
        }
    }
    Ok(RValue::Vector(Vector::Double(vec![Some(total)].into())))
}

#[builtin(min_args = 0)]
fn builtin_max(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let na_rm = named.iter().any(|(n, v)| {
        n == "na.rm" && v.as_vector().and_then(|v| v.as_logical_scalar()) == Some(true)
    });
    let mut result: Option<f64> = None;
    for arg in args {
        if let RValue::Vector(v) = arg {
            for x in v.to_doubles() {
                match x {
                    Some(f) => {
                        result = Some(match result {
                            Some(r) => r.max(f),
                            None => f,
                        })
                    }
                    None if !na_rm => return Ok(RValue::Vector(Vector::Double(vec![None].into()))),
                    None => {}
                }
            }
        }
    }
    Ok(RValue::Vector(Vector::Double(
        vec![result.or(Some(f64::NEG_INFINITY))].into(),
    )))
}

#[builtin(min_args = 0)]
fn builtin_min(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let na_rm = named.iter().any(|(n, v)| {
        n == "na.rm" && v.as_vector().and_then(|v| v.as_logical_scalar()) == Some(true)
    });
    let mut result: Option<f64> = None;
    for arg in args {
        if let RValue::Vector(v) = arg {
            for x in v.to_doubles() {
                match x {
                    Some(f) => {
                        result = Some(match result {
                            Some(r) => r.min(f),
                            None => f,
                        })
                    }
                    None if !na_rm => return Ok(RValue::Vector(Vector::Double(vec![None].into()))),
                    None => {}
                }
            }
        }
    }
    Ok(RValue::Vector(Vector::Double(
        vec![result.or(Some(f64::INFINITY))].into(),
    )))
}

#[builtin(min_args = 1)]
fn builtin_mean(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let na_rm = named.iter().any(|(n, v)| {
        n == "na.rm" && v.as_vector().and_then(|v| v.as_logical_scalar()) == Some(true)
    });
    match args.first() {
        Some(RValue::Vector(v)) => {
            let vals = v.to_doubles();
            let mut sum = 0.0;
            let mut count = 0;
            for x in &vals {
                match x {
                    Some(f) => {
                        sum += f;
                        count += 1;
                    }
                    None if !na_rm => return Ok(RValue::Vector(Vector::Double(vec![None].into()))),
                    None => {}
                }
            }
            if count == 0 {
                return Ok(RValue::Vector(Vector::Double(vec![Some(f64::NAN)].into())));
            }
            Ok(RValue::Vector(Vector::Double(
                vec![Some(sum / count as f64)].into(),
            )))
        }
        _ => Ok(RValue::Vector(Vector::Double(vec![Some(f64::NAN)].into()))),
    }
}

#[builtin(min_args = 1)]
fn builtin_median(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(v)) => {
            let mut vals: Vec<f64> = v.to_doubles().into_iter().flatten().collect();
            if vals.is_empty() {
                return Ok(RValue::Vector(Vector::Double(vec![Some(f64::NAN)].into())));
            }
            vals.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let mid = vals.len() / 2;
            let median = if vals.len().is_multiple_of(2) {
                (vals[mid - 1] + vals[mid]) / 2.0
            } else {
                vals[mid]
            };
            Ok(RValue::Vector(Vector::Double(vec![Some(median)].into())))
        }
        _ => Ok(RValue::Vector(Vector::Double(vec![Some(f64::NAN)].into()))),
    }
}

#[builtin(min_args = 1)]
fn builtin_var(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(v)) => {
            let vals: Vec<f64> = v.to_doubles().into_iter().flatten().collect();
            let n = vals.len() as f64;
            if n < 2.0 {
                return Ok(RValue::Vector(Vector::Double(vec![Some(f64::NAN)].into())));
            }
            let mean = vals.iter().sum::<f64>() / n;
            let var = vals.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0);
            Ok(RValue::Vector(Vector::Double(vec![Some(var)].into())))
        }
        _ => Ok(RValue::Vector(Vector::Double(vec![Some(f64::NAN)].into()))),
    }
}

#[builtin(min_args = 1)]
fn builtin_sd(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    match builtin_var(args, named)? {
        RValue::Vector(Vector::Double(v)) => Ok(RValue::Vector(Vector::Double(
            v.iter()
                .map(|x| x.map(f64::sqrt))
                .collect::<Vec<_>>()
                .into(),
        ))),
        other => Ok(other),
    }
}

#[builtin(min_args = 1)]
fn builtin_cumsum(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(v)) => {
            let mut acc = 0.0;
            let result: Vec<Option<f64>> = v
                .to_doubles()
                .iter()
                .map(|x| {
                    x.map(|f| {
                        acc += f;
                        acc
                    })
                })
                .collect();
            Ok(RValue::Vector(Vector::Double(result.into())))
        }
        _ => Err(RError::Argument("invalid argument".to_string())),
    }
}

#[builtin(min_args = 1)]
fn builtin_cumprod(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(v)) => {
            let mut acc = 1.0;
            let result: Vec<Option<f64>> = v
                .to_doubles()
                .iter()
                .map(|x| {
                    x.map(|f| {
                        acc *= f;
                        acc
                    })
                })
                .collect();
            Ok(RValue::Vector(Vector::Double(result.into())))
        }
        _ => Err(RError::Argument("invalid argument".to_string())),
    }
}

#[builtin(min_args = 1)]
fn builtin_cummax(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(v)) => {
            let mut acc = f64::NEG_INFINITY;
            let result: Vec<Option<f64>> = v
                .to_doubles()
                .iter()
                .map(|x| {
                    x.map(|f| {
                        acc = acc.max(f);
                        acc
                    })
                })
                .collect();
            Ok(RValue::Vector(Vector::Double(result.into())))
        }
        _ => Err(RError::Argument("invalid argument".to_string())),
    }
}

#[builtin(min_args = 1)]
fn builtin_cummin(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(v)) => {
            let mut acc = f64::INFINITY;
            let result: Vec<Option<f64>> = v
                .to_doubles()
                .iter()
                .map(|x| {
                    x.map(|f| {
                        acc = acc.min(f);
                        acc
                    })
                })
                .collect();
            Ok(RValue::Vector(Vector::Double(result.into())))
        }
        _ => Err(RError::Argument("invalid argument".to_string())),
    }
}

#[builtin(min_args = 0)]
fn builtin_seq(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let from = named
        .iter()
        .find(|(n, _)| n == "from")
        .map(|(_, v)| v)
        .or(args.first())
        .and_then(|v| v.as_vector()?.as_double_scalar())
        .unwrap_or(1.0);
    let to = named
        .iter()
        .find(|(n, _)| n == "to")
        .map(|(_, v)| v)
        .or(args.get(1))
        .and_then(|v| v.as_vector()?.as_double_scalar())
        .unwrap_or(1.0);
    let by = named
        .iter()
        .find(|(n, _)| n == "by")
        .map(|(_, v)| v)
        .or(args.get(2))
        .and_then(|v| v.as_vector()?.as_double_scalar());
    let length_out = named
        .iter()
        .find(|(n, _)| n == "length.out")
        .map(|(_, v)| v)
        .and_then(|v| v.as_vector()?.as_integer_scalar());

    if let Some(len) = length_out {
        let len = len as usize;
        if len == 0 {
            return Ok(RValue::Vector(Vector::Double(vec![].into())));
        }
        if len == 1 {
            return Ok(RValue::Vector(Vector::Double(vec![Some(from)].into())));
        }
        let step = (to - from) / (len - 1) as f64;
        let result: Vec<Option<f64>> = (0..len).map(|i| Some(from + step * i as f64)).collect();
        return Ok(RValue::Vector(Vector::Double(result.into())));
    }

    let by = by.unwrap_or(if to >= from { 1.0 } else { -1.0 });
    if by == 0.0 {
        return Err(RError::Argument(
            "'by' argument must not be zero".to_string(),
        ));
    }

    let mut result = Vec::new();
    let mut val = from;
    if by > 0.0 {
        while val <= to + f64::EPSILON * 100.0 {
            result.push(Some(val));
            val += by;
        }
    } else {
        while val >= to - f64::EPSILON * 100.0 {
            result.push(Some(val));
            val += by;
        }
    }
    Ok(RValue::Vector(Vector::Double(result.into())))
}

#[builtin(name = "seq_len", min_args = 1)]
fn builtin_seq_len(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let n = args
        .first()
        .and_then(|v| v.as_vector()?.as_integer_scalar())
        .unwrap_or(0);
    let result: Vec<Option<i64>> = (1..=n).map(Some).collect();
    Ok(RValue::Vector(Vector::Integer(result.into())))
}

#[builtin(name = "seq_along", min_args = 1)]
fn builtin_seq_along(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let n = args.first().map(|v| v.length()).unwrap_or(0);
    let result: Vec<Option<i64>> = (1..=n as i64).map(Some).collect();
    Ok(RValue::Vector(Vector::Integer(result.into())))
}

#[builtin(min_args = 1)]
fn builtin_rep(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let times = named
        .iter()
        .find(|(n, _)| n == "times")
        .and_then(|(_, v)| v.as_vector()?.as_integer_scalar())
        .or_else(|| args.get(1)?.as_vector()?.as_integer_scalar())
        .unwrap_or(1) as usize;

    match args.first() {
        Some(RValue::Vector(v)) => match v {
            Vector::Double(vals) => Ok(RValue::Vector(Vector::Double(
                vals.iter()
                    .cycle()
                    .take(vals.len() * times)
                    .cloned()
                    .collect::<Vec<_>>()
                    .into(),
            ))),
            Vector::Integer(vals) => Ok(RValue::Vector(Vector::Integer(
                vals.iter()
                    .cycle()
                    .take(vals.len() * times)
                    .cloned()
                    .collect::<Vec<_>>()
                    .into(),
            ))),
            Vector::Logical(vals) => Ok(RValue::Vector(Vector::Logical(
                vals.iter()
                    .cycle()
                    .take(vals.len() * times)
                    .cloned()
                    .collect::<Vec<_>>()
                    .into(),
            ))),
            Vector::Character(vals) => Ok(RValue::Vector(Vector::Character(
                vals.iter()
                    .cycle()
                    .take(vals.len() * times)
                    .cloned()
                    .collect::<Vec<_>>()
                    .into(),
            ))),
        },
        _ => Err(RError::Argument("invalid argument".to_string())),
    }
}

#[builtin(min_args = 1)]
fn builtin_rev(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(v)) => {
            let result = match v {
                Vector::Double(vals) => {
                    let mut v = vals.clone();
                    v.reverse();
                    Vector::Double(v)
                }
                Vector::Integer(vals) => {
                    let mut v = vals.clone();
                    v.reverse();
                    Vector::Integer(v)
                }
                Vector::Logical(vals) => {
                    let mut v = vals.clone();
                    v.reverse();
                    Vector::Logical(v)
                }
                Vector::Character(vals) => {
                    let mut v = vals.clone();
                    v.reverse();
                    Vector::Character(v)
                }
            };
            Ok(RValue::Vector(result))
        }
        _ => Ok(RValue::Null),
    }
}

#[builtin(min_args = 1)]
fn builtin_sort(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let decreasing = named
        .iter()
        .find(|(n, _)| n == "decreasing")
        .and_then(|(_, v)| v.as_vector()?.as_logical_scalar())
        .unwrap_or(false);
    match args.first() {
        Some(RValue::Vector(v)) => {
            let result = match v {
                Vector::Double(vals) => {
                    let mut v: Vec<Option<f64>> = vals.0.clone();
                    v.sort_by(|a, b| {
                        let a = a.unwrap_or(f64::NAN);
                        let b = b.unwrap_or(f64::NAN);
                        if decreasing {
                            b.partial_cmp(&a).unwrap_or(std::cmp::Ordering::Equal)
                        } else {
                            a.partial_cmp(&b).unwrap_or(std::cmp::Ordering::Equal)
                        }
                    });
                    Vector::Double(v.into())
                }
                Vector::Integer(vals) => {
                    let mut v = vals.clone();
                    v.sort_by(|a, b| if decreasing { b.cmp(a) } else { a.cmp(b) });
                    Vector::Integer(v)
                }
                Vector::Character(vals) => {
                    let mut v = vals.clone();
                    v.sort_by(|a, b| if decreasing { b.cmp(a) } else { a.cmp(b) });
                    Vector::Character(v)
                }
                other => other.clone(),
            };
            Ok(RValue::Vector(result))
        }
        _ => Ok(RValue::Null),
    }
}

#[builtin(min_args = 1)]
fn builtin_order(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(v)) => {
            let doubles = v.to_doubles();
            let mut indices: Vec<usize> = (0..doubles.len()).collect();
            indices.sort_by(|&a, &b| {
                let va = doubles[a].unwrap_or(f64::NAN);
                let vb = doubles[b].unwrap_or(f64::NAN);
                va.partial_cmp(&vb).unwrap_or(std::cmp::Ordering::Equal)
            });
            let result: Vec<Option<i64>> = indices.iter().map(|&i| Some(i as i64 + 1)).collect();
            Ok(RValue::Vector(Vector::Integer(result.into())))
        }
        _ => Ok(RValue::Null),
    }
}

#[builtin(min_args = 1)]
fn builtin_unique(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(v)) => {
            let result = match v {
                Vector::Double(vals) => {
                    let mut seen = Vec::new();
                    let mut result = Vec::new();
                    for x in vals.iter() {
                        let key = format!("{:?}", x);
                        if !seen.contains(&key) {
                            seen.push(key);
                            result.push(*x);
                        }
                    }
                    Vector::Double(result.into())
                }
                Vector::Integer(vals) => {
                    let mut seen: Vec<Option<i64>> = Vec::new();
                    let mut result = Vec::new();
                    for x in vals.iter() {
                        if !seen.contains(x) {
                            seen.push(*x);
                            result.push(*x);
                        }
                    }
                    Vector::Integer(result.into())
                }
                Vector::Character(vals) => {
                    let mut seen: Vec<Option<String>> = Vec::new();
                    let mut result = Vec::new();
                    for x in vals.iter() {
                        if !seen.contains(x) {
                            seen.push(x.clone());
                            result.push(x.clone());
                        }
                    }
                    Vector::Character(result.into())
                }
                Vector::Logical(vals) => {
                    let mut seen = Vec::new();
                    let mut result = Vec::new();
                    for x in vals.iter() {
                        if !seen.contains(x) {
                            seen.push(*x);
                            result.push(*x);
                        }
                    }
                    Vector::Logical(result.into())
                }
            };
            Ok(RValue::Vector(result))
        }
        _ => Ok(RValue::Null),
    }
}

#[builtin(min_args = 1)]
fn builtin_which(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(Vector::Logical(vals))) => {
            let result: Vec<Option<i64>> = vals
                .iter()
                .enumerate()
                .filter_map(|(i, v)| {
                    if *v == Some(true) {
                        Some(Some(i as i64 + 1))
                    } else {
                        None
                    }
                })
                .collect();
            Ok(RValue::Vector(Vector::Integer(result.into())))
        }
        _ => Ok(RValue::Vector(Vector::Integer(vec![].into()))),
    }
}

#[builtin(min_args = 1)]
fn builtin_which_min(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(v)) => {
            let doubles = v.to_doubles();
            let mut min_idx = None;
            let mut min_val = f64::INFINITY;
            for (i, x) in doubles.iter().enumerate() {
                if let Some(f) = x {
                    if *f < min_val {
                        min_val = *f;
                        min_idx = Some(i);
                    }
                }
            }
            Ok(RValue::Vector(Vector::Integer(
                vec![min_idx.map(|i| i as i64 + 1)].into(),
            )))
        }
        _ => Ok(RValue::Vector(Vector::Integer(vec![].into()))),
    }
}

#[builtin(min_args = 1)]
fn builtin_which_max(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(v)) => {
            let doubles = v.to_doubles();
            let mut max_idx = None;
            let mut max_val = f64::NEG_INFINITY;
            for (i, x) in doubles.iter().enumerate() {
                if let Some(f) = x {
                    if *f > max_val {
                        max_val = *f;
                        max_idx = Some(i);
                    }
                }
            }
            Ok(RValue::Vector(Vector::Integer(
                vec![max_idx.map(|i| i as i64 + 1)].into(),
            )))
        }
        _ => Ok(RValue::Vector(Vector::Integer(vec![].into()))),
    }
}

#[builtin(min_args = 2)]
fn builtin_append(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match (args.first(), args.get(1)) {
        (Some(RValue::Vector(v1)), Some(RValue::Vector(v2))) => {
            let mut chars = v1.to_characters();
            chars.extend(v2.to_characters());
            Ok(RValue::Vector(Vector::Character(chars.into())))
        }
        _ => Ok(args.first().cloned().unwrap_or(RValue::Null)),
    }
}

#[builtin(min_args = 1)]
fn builtin_head(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let n = named
        .iter()
        .find(|(k, _)| k == "n")
        .map(|(_, v)| v)
        .or(args.get(1))
        .and_then(|v| v.as_vector()?.as_integer_scalar())
        .unwrap_or(6) as usize;
    match args.first() {
        Some(RValue::Vector(v)) => {
            let result = match v {
                Vector::Double(vals) => Vector::Double(vals[..n.min(vals.len())].to_vec().into()),
                Vector::Integer(vals) => Vector::Integer(vals[..n.min(vals.len())].to_vec().into()),
                Vector::Logical(vals) => Vector::Logical(vals[..n.min(vals.len())].to_vec().into()),
                Vector::Character(vals) => {
                    Vector::Character(vals[..n.min(vals.len())].to_vec().into())
                }
            };
            Ok(RValue::Vector(result))
        }
        _ => Ok(RValue::Null),
    }
}

#[builtin(min_args = 1)]
fn builtin_tail(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let n = named
        .iter()
        .find(|(k, _)| k == "n")
        .map(|(_, v)| v)
        .or(args.get(1))
        .and_then(|v| v.as_vector()?.as_integer_scalar())
        .unwrap_or(6) as usize;
    match args.first() {
        Some(RValue::Vector(v)) => {
            let len = v.len();
            let start = len.saturating_sub(n);
            let result = match v {
                Vector::Double(vals) => Vector::Double(vals[start..].to_vec().into()),
                Vector::Integer(vals) => Vector::Integer(vals[start..].to_vec().into()),
                Vector::Logical(vals) => Vector::Logical(vals[start..].to_vec().into()),
                Vector::Character(vals) => Vector::Character(vals[start..].to_vec().into()),
            };
            Ok(RValue::Vector(result))
        }
        _ => Ok(RValue::Null),
    }
}

#[builtin(min_args = 0)]
fn builtin_range(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    for arg in args {
        if let RValue::Vector(v) = arg {
            for x in v.to_doubles().into_iter().flatten() {
                if x < min {
                    min = x;
                }
                if x > max {
                    max = x;
                }
            }
        }
    }
    Ok(RValue::Vector(Vector::Double(
        vec![Some(min), Some(max)].into(),
    )))
}

#[builtin(min_args = 1)]
fn builtin_diff(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(v)) => {
            let vals = v.to_doubles();
            if vals.len() < 2 {
                return Ok(RValue::Vector(Vector::Double(vec![].into())));
            }
            let result: Vec<Option<f64>> = vals
                .windows(2)
                .map(|w| match (w[0], w[1]) {
                    (Some(a), Some(b)) => Some(b - a),
                    _ => None,
                })
                .collect();
            Ok(RValue::Vector(Vector::Double(result.into())))
        }
        _ => Err(RError::Argument("invalid argument".to_string())),
    }
}

#[builtin(min_args = 1)]
fn builtin_sample(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(v)) => Ok(RValue::Vector(v.clone())),
        _ => Ok(RValue::Null),
    }
}

#[builtin(min_args = 1)]
fn builtin_set_seed(_args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    Ok(RValue::Null)
}

#[builtin(name = "rep_len", min_args = 2)]
fn builtin_rep_len(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    if args.len() < 2 {
        return Err(RError::Argument("need 2 arguments".to_string()));
    }
    let length_out = args[1]
        .as_vector()
        .and_then(|v| v.as_integer_scalar())
        .unwrap_or(0) as usize;
    match &args[0] {
        RValue::Vector(v) => {
            if v.is_empty() {
                return Ok(RValue::Vector(v.clone()));
            }
            match v {
                Vector::Double(vals) => Ok(RValue::Vector(Vector::Double(
                    vals.iter()
                        .cycle()
                        .take(length_out)
                        .cloned()
                        .collect::<Vec<_>>()
                        .into(),
                ))),
                Vector::Integer(vals) => Ok(RValue::Vector(Vector::Integer(
                    vals.iter()
                        .cycle()
                        .take(length_out)
                        .cloned()
                        .collect::<Vec<_>>()
                        .into(),
                ))),
                Vector::Logical(vals) => Ok(RValue::Vector(Vector::Logical(
                    vals.iter()
                        .cycle()
                        .take(length_out)
                        .cloned()
                        .collect::<Vec<_>>()
                        .into(),
                ))),
                Vector::Character(vals) => Ok(RValue::Vector(Vector::Character(
                    vals.iter()
                        .cycle()
                        .take(length_out)
                        .cloned()
                        .collect::<Vec<_>>()
                        .into(),
                ))),
            }
        }
        _ => Err(RError::Argument("invalid argument".to_string())),
    }
}

#[builtin(min_args = 2)]
fn builtin_rep_int(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    if args.len() < 2 {
        return Err(RError::Argument("need 2 arguments".to_string()));
    }
    let times = args[1]
        .as_vector()
        .and_then(|v| v.as_integer_scalar())
        .unwrap_or(1) as usize;
    match &args[0] {
        RValue::Vector(v) => match v {
            Vector::Double(vals) => Ok(RValue::Vector(Vector::Double(
                vals.iter()
                    .cycle()
                    .take(vals.len() * times)
                    .cloned()
                    .collect::<Vec<_>>()
                    .into(),
            ))),
            Vector::Integer(vals) => Ok(RValue::Vector(Vector::Integer(
                vals.iter()
                    .cycle()
                    .take(vals.len() * times)
                    .cloned()
                    .collect::<Vec<_>>()
                    .into(),
            ))),
            Vector::Logical(vals) => Ok(RValue::Vector(Vector::Logical(
                vals.iter()
                    .cycle()
                    .take(vals.len() * times)
                    .cloned()
                    .collect::<Vec<_>>()
                    .into(),
            ))),
            Vector::Character(vals) => Ok(RValue::Vector(Vector::Character(
                vals.iter()
                    .cycle()
                    .take(vals.len() * times)
                    .cloned()
                    .collect::<Vec<_>>()
                    .into(),
            ))),
        },
        _ => Err(RError::Argument("invalid argument".to_string())),
    }
}
