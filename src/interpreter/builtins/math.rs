use ndarray::{Array2, ShapeBuilder};

use crate::interpreter::value::*;
use newr_macros::{builtin, noop_builtin};

noop_builtin!("runif", 1);
noop_builtin!("rnorm", 1);
noop_builtin!("rbinom", 2);

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
            Ok(RValue::vec(Vector::Double(result.into())))
        }
        _ => Err(RError::Argument("non-numeric argument".to_string())),
    }
}

#[builtin(min_args = 1)]
fn builtin_signif(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let digits = named
        .iter()
        .find(|(n, _)| n == "digits")
        .and_then(|(_, v)| v.as_vector()?.as_integer_scalar())
        .or_else(|| args.get(1)?.as_vector()?.as_integer_scalar())
        .unwrap_or(6) as i32;
    match args.first() {
        Some(RValue::Vector(v)) => {
            let result: Vec<Option<f64>> = v
                .to_doubles()
                .iter()
                .map(|x| {
                    x.map(|f| {
                        if f == 0.0 || !f.is_finite() {
                            return f;
                        }
                        let d = f.abs().log10().ceil() as i32;
                        let factor = 10f64.powi(digits - d);
                        (f * factor).round() / factor
                    })
                })
                .collect();
            Ok(RValue::vec(Vector::Double(result.into())))
        }
        _ => Err(RError::Argument(
            "non-numeric argument to signif".to_string(),
        )),
    }
}

// === Parallel min/max ===

#[builtin(min_args = 1)]
fn builtin_pmin(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let na_rm = named.iter().any(|(n, v)| {
        n == "na.rm" && v.as_vector().and_then(|v| v.as_logical_scalar()) == Some(true)
    });
    parallel_minmax(args, na_rm, false)
}

#[builtin(min_args = 1)]
fn builtin_pmax(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let na_rm = named.iter().any(|(n, v)| {
        n == "na.rm" && v.as_vector().and_then(|v| v.as_logical_scalar()) == Some(true)
    });
    parallel_minmax(args, na_rm, true)
}

fn parallel_minmax(args: &[RValue], na_rm: bool, is_max: bool) -> Result<RValue, RError> {
    // Collect all argument vectors as doubles
    let vecs: Vec<Vec<Option<f64>>> = args
        .iter()
        .filter_map(|a| {
            if let RValue::Vector(v) = a {
                Some(v.to_doubles())
            } else {
                None
            }
        })
        .collect();
    if vecs.is_empty() {
        return Err(RError::Argument("no arguments to pmin/pmax".to_string()));
    }
    // Find max length for recycling
    let max_len = vecs.iter().map(|v| v.len()).max().unwrap_or(0);
    let mut result = Vec::with_capacity(max_len);
    for i in 0..max_len {
        let mut current: Option<f64> = None;
        let mut has_na = false;
        for vec in &vecs {
            let val = vec[i % vec.len()];
            match val {
                Some(f) => {
                    current = Some(match current {
                        Some(c) => {
                            if is_max {
                                c.max(f)
                            } else {
                                c.min(f)
                            }
                        }
                        None => f,
                    });
                }
                None => {
                    has_na = true;
                }
            }
        }
        if has_na && !na_rm {
            result.push(None);
        } else {
            result.push(current);
        }
    }
    Ok(RValue::vec(Vector::Double(result.into())))
}

// === Cumulative logical ===

#[builtin(min_args = 1)]
fn builtin_cumall(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(v)) => {
            let logicals = v.to_logicals();
            let mut acc = true;
            let result: Vec<Option<bool>> = logicals
                .iter()
                .map(|x| match x {
                    Some(b) => {
                        acc = acc && *b;
                        Some(acc)
                    }
                    None => None,
                })
                .collect();
            Ok(RValue::vec(Vector::Logical(result.into())))
        }
        _ => Err(RError::Argument("invalid argument to cumall".to_string())),
    }
}

#[builtin(min_args = 1)]
fn builtin_cumany(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(v)) => {
            let logicals = v.to_logicals();
            let mut acc = false;
            let result: Vec<Option<bool>> = logicals
                .iter()
                .map(|x| match x {
                    Some(b) => {
                        acc = acc || *b;
                        Some(acc)
                    }
                    None => None,
                })
                .collect();
            Ok(RValue::vec(Vector::Logical(result.into())))
        }
        _ => Err(RError::Argument("invalid argument to cumany".to_string())),
    }
}

// === Bitwise operations ===

fn bitwise_binary_op(args: &[RValue], op: fn(i64, i64) -> i64) -> Result<RValue, RError> {
    let a_ints = args
        .first()
        .and_then(|v| v.as_vector())
        .map(|v| v.to_integers())
        .ok_or_else(|| RError::Argument("non-integer argument to bitwise function".to_string()))?;
    let b_ints = args
        .get(1)
        .and_then(|v| v.as_vector())
        .map(|v| v.to_integers())
        .ok_or_else(|| RError::Argument("non-integer argument to bitwise function".to_string()))?;
    let max_len = a_ints.len().max(b_ints.len());
    let result: Vec<Option<i64>> = (0..max_len)
        .map(|i| {
            let a = a_ints[i % a_ints.len()];
            let b = b_ints[i % b_ints.len()];
            match (a, b) {
                (Some(x), Some(y)) => Some(op(x, y)),
                _ => None,
            }
        })
        .collect();
    Ok(RValue::vec(Vector::Integer(result.into())))
}

#[builtin(name = "bitwAnd", min_args = 2)]
fn builtin_bitw_and(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    bitwise_binary_op(args, |a, b| a & b)
}

#[builtin(name = "bitwOr", min_args = 2)]
fn builtin_bitw_or(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    bitwise_binary_op(args, |a, b| a | b)
}

#[builtin(name = "bitwXor", min_args = 2)]
fn builtin_bitw_xor(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    bitwise_binary_op(args, |a, b| a ^ b)
}

#[builtin(name = "bitwNot", min_args = 1)]
fn builtin_bitw_not(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let ints = args
        .first()
        .and_then(|v| v.as_vector())
        .map(|v| v.to_integers())
        .ok_or_else(|| RError::Argument("non-integer argument to bitwNot".to_string()))?;
    let result: Vec<Option<i64>> = ints.iter().map(|x| x.map(|i| !i)).collect();
    Ok(RValue::vec(Vector::Integer(result.into())))
}

#[builtin(name = "bitwShiftL", min_args = 2)]
fn builtin_bitw_shift_l(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    bitwise_binary_op(args, |a, n| a << (n as u32))
}

#[builtin(name = "bitwShiftR", min_args = 2)]
fn builtin_bitw_shift_r(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    bitwise_binary_op(args, |a, n| a >> (n as u32))
}

// === Triangular matrix extraction ===

#[builtin(name = "lower.tri", min_args = 1)]
fn builtin_lower_tri(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let diag_incl = named
        .iter()
        .find(|(n, _)| n == "diag")
        .and_then(|(_, v)| v.as_vector()?.as_logical_scalar())
        .or_else(|| args.get(1)?.as_vector()?.as_logical_scalar())
        .unwrap_or(false);
    tri_matrix(args, diag_incl, true)
}

#[builtin(name = "upper.tri", min_args = 1)]
fn builtin_upper_tri(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let diag_incl = named
        .iter()
        .find(|(n, _)| n == "diag")
        .and_then(|(_, v)| v.as_vector()?.as_logical_scalar())
        .or_else(|| args.get(1)?.as_vector()?.as_logical_scalar())
        .unwrap_or(false);
    tri_matrix(args, diag_incl, false)
}

fn tri_matrix(args: &[RValue], diag_incl: bool, lower: bool) -> Result<RValue, RError> {
    let (nrow, ncol) = match args.first() {
        Some(RValue::Vector(rv)) => match rv.get_attr("dim") {
            Some(RValue::Vector(dim_rv)) => match &dim_rv.inner {
                Vector::Integer(d) if d.len() >= 2 => {
                    (d[0].unwrap_or(0) as usize, d[1].unwrap_or(0) as usize)
                }
                _ => return Err(RError::Argument("argument is not a matrix".to_string())),
            },
            _ => return Err(RError::Argument("argument is not a matrix".to_string())),
        },
        _ => return Err(RError::Argument("argument is not a matrix".to_string())),
    };

    // R stores matrices column-major
    let mut result = Vec::with_capacity(nrow * ncol);
    for j in 0..ncol {
        for i in 0..nrow {
            let val = if lower {
                if diag_incl {
                    i >= j
                } else {
                    i > j
                }
            } else if diag_incl {
                i <= j
            } else {
                i < j
            };
            result.push(Some(val));
        }
    }
    let mut rv = RVector::from(Vector::Logical(result.into()));
    rv.set_attr(
        "dim".to_string(),
        RValue::vec(Vector::Integer(
            vec![Some(nrow as i64), Some(ncol as i64)].into(),
        )),
    );
    Ok(RValue::Vector(rv))
}

// === Diagonal matrix ===

#[builtin(min_args = 1)]
fn builtin_diag(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(rv)) => {
            // If x is a matrix, extract the diagonal
            if let Some(RValue::Vector(dim_rv)) = rv.get_attr("dim") {
                if let Vector::Integer(d) = &dim_rv.inner {
                    if d.len() >= 2 {
                        let nrow = d[0].unwrap_or(0) as usize;
                        let ncol = d[1].unwrap_or(0) as usize;
                        let data = rv.to_doubles();
                        let n = nrow.min(ncol);
                        let result: Vec<Option<f64>> = (0..n)
                            .map(|i| {
                                let idx = i * nrow + i; // column-major: col * nrow + row
                                if idx < data.len() {
                                    data[idx]
                                } else {
                                    None
                                }
                            })
                            .collect();
                        return Ok(RValue::vec(Vector::Double(result.into())));
                    }
                }
            }

            let len = rv.len();
            if len == 1 {
                // Scalar integer n: create n x n identity matrix
                let n = rv.as_integer_scalar().unwrap_or(1) as usize;
                let mut result = vec![Some(0.0); n * n];
                for i in 0..n {
                    result[i * n + i] = Some(1.0);
                }
                let mut out = RVector::from(Vector::Double(result.into()));
                out.set_attr(
                    "dim".to_string(),
                    RValue::vec(Vector::Integer(vec![Some(n as i64), Some(n as i64)].into())),
                );
                Ok(RValue::Vector(out))
            } else {
                // Vector: create diagonal matrix from vector
                let vals = rv.to_doubles();
                let n = vals.len();
                let mut result = vec![Some(0.0); n * n];
                for (i, v) in vals.iter().enumerate() {
                    result[i * n + i] = *v;
                }
                let mut out = RVector::from(Vector::Double(result.into()));
                out.set_attr(
                    "dim".to_string(),
                    RValue::vec(Vector::Integer(vec![Some(n as i64), Some(n as i64)].into())),
                );
                Ok(RValue::Vector(out))
            }
        }
        _ => Err(RError::Argument("'x' must be numeric".to_string())),
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
                    None if !na_rm => return Ok(RValue::vec(Vector::Double(vec![None].into()))),
                    None => {}
                }
            }
        }
    }
    Ok(RValue::vec(Vector::Double(vec![Some(total)].into())))
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
                    None if !na_rm => return Ok(RValue::vec(Vector::Double(vec![None].into()))),
                    None => {}
                }
            }
        }
    }
    Ok(RValue::vec(Vector::Double(vec![Some(total)].into())))
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
                    None if !na_rm => return Ok(RValue::vec(Vector::Double(vec![None].into()))),
                    None => {}
                }
            }
        }
    }
    Ok(RValue::vec(Vector::Double(
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
                    None if !na_rm => return Ok(RValue::vec(Vector::Double(vec![None].into()))),
                    None => {}
                }
            }
        }
    }
    Ok(RValue::vec(Vector::Double(
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
                    None if !na_rm => return Ok(RValue::vec(Vector::Double(vec![None].into()))),
                    None => {}
                }
            }
            if count == 0 {
                return Ok(RValue::vec(Vector::Double(vec![Some(f64::NAN)].into())));
            }
            Ok(RValue::vec(Vector::Double(
                vec![Some(sum / count as f64)].into(),
            )))
        }
        _ => Ok(RValue::vec(Vector::Double(vec![Some(f64::NAN)].into()))),
    }
}

#[builtin(min_args = 1)]
fn builtin_median(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(v)) => {
            let mut vals: Vec<f64> = v.to_doubles().into_iter().flatten().collect();
            if vals.is_empty() {
                return Ok(RValue::vec(Vector::Double(vec![Some(f64::NAN)].into())));
            }
            vals.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let mid = vals.len() / 2;
            let median = if vals.len().is_multiple_of(2) {
                (vals[mid - 1] + vals[mid]) / 2.0
            } else {
                vals[mid]
            };
            Ok(RValue::vec(Vector::Double(vec![Some(median)].into())))
        }
        _ => Ok(RValue::vec(Vector::Double(vec![Some(f64::NAN)].into()))),
    }
}

#[builtin(min_args = 1)]
fn builtin_var(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(v)) => {
            let vals: Vec<f64> = v.to_doubles().into_iter().flatten().collect();
            let n = vals.len() as f64;
            if n < 2.0 {
                return Ok(RValue::vec(Vector::Double(vec![Some(f64::NAN)].into())));
            }
            let mean = vals.iter().sum::<f64>() / n;
            let var = vals.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0);
            Ok(RValue::vec(Vector::Double(vec![Some(var)].into())))
        }
        _ => Ok(RValue::vec(Vector::Double(vec![Some(f64::NAN)].into()))),
    }
}

#[builtin(min_args = 1)]
fn builtin_sd(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    match builtin_var(args, named)? {
        RValue::Vector(rv) => match rv.inner {
            Vector::Double(v) => Ok(RValue::vec(Vector::Double(
                v.iter()
                    .map(|x| x.map(f64::sqrt))
                    .collect::<Vec<_>>()
                    .into(),
            ))),
            other => Ok(RValue::vec(other)),
        },
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
            Ok(RValue::vec(Vector::Double(result.into())))
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
            Ok(RValue::vec(Vector::Double(result.into())))
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
            Ok(RValue::vec(Vector::Double(result.into())))
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
            Ok(RValue::vec(Vector::Double(result.into())))
        }
        _ => Err(RError::Argument("invalid argument".to_string())),
    }
}

#[builtin(min_args = 0, names = ["seq.int"])]
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
            return Ok(RValue::vec(Vector::Double(vec![].into())));
        }
        if len == 1 {
            return Ok(RValue::vec(Vector::Double(vec![Some(from)].into())));
        }
        let step = (to - from) / (len - 1) as f64;
        let result: Vec<Option<f64>> = (0..len).map(|i| Some(from + step * i as f64)).collect();
        return Ok(RValue::vec(Vector::Double(result.into())));
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
    Ok(RValue::vec(Vector::Double(result.into())))
}

#[builtin(name = "seq_len", min_args = 1)]
fn builtin_seq_len(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let n = args
        .first()
        .and_then(|v| v.as_vector()?.as_integer_scalar())
        .unwrap_or(0);
    let result: Vec<Option<i64>> = (1..=n).map(Some).collect();
    Ok(RValue::vec(Vector::Integer(result.into())))
}

#[builtin(name = "seq_along", min_args = 1)]
fn builtin_seq_along(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let n = args.first().map(|v| v.length()).unwrap_or(0);
    let result: Vec<Option<i64>> = (1..=n as i64).map(Some).collect();
    Ok(RValue::vec(Vector::Integer(result.into())))
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
        Some(RValue::Vector(v)) => match &v.inner {
            Vector::Double(vals) => Ok(RValue::vec(Vector::Double(
                vals.iter()
                    .cycle()
                    .take(vals.len() * times)
                    .cloned()
                    .collect::<Vec<_>>()
                    .into(),
            ))),
            Vector::Integer(vals) => Ok(RValue::vec(Vector::Integer(
                vals.iter()
                    .cycle()
                    .take(vals.len() * times)
                    .cloned()
                    .collect::<Vec<_>>()
                    .into(),
            ))),
            Vector::Logical(vals) => Ok(RValue::vec(Vector::Logical(
                vals.iter()
                    .cycle()
                    .take(vals.len() * times)
                    .cloned()
                    .collect::<Vec<_>>()
                    .into(),
            ))),
            Vector::Character(vals) => Ok(RValue::vec(Vector::Character(
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
            let result = match &v.inner {
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
            Ok(RValue::vec(result))
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
            let result = match &v.inner {
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
            Ok(RValue::vec(result))
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
            Ok(RValue::vec(Vector::Integer(result.into())))
        }
        _ => Ok(RValue::Null),
    }
}

#[builtin(min_args = 1)]
fn builtin_unique(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(v)) => {
            let result = match &v.inner {
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
            Ok(RValue::vec(result))
        }
        _ => Ok(RValue::Null),
    }
}

#[builtin(min_args = 1)]
fn builtin_which(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(rv)) if matches!(rv.inner, Vector::Logical(_)) => {
            let Vector::Logical(vals) = &rv.inner else {
                unreachable!()
            };
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
            Ok(RValue::vec(Vector::Integer(result.into())))
        }
        _ => Ok(RValue::vec(Vector::Integer(vec![].into()))),
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
            Ok(RValue::vec(Vector::Integer(
                vec![min_idx.map(|i| i as i64 + 1)].into(),
            )))
        }
        _ => Ok(RValue::vec(Vector::Integer(vec![].into()))),
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
            Ok(RValue::vec(Vector::Integer(
                vec![max_idx.map(|i| i as i64 + 1)].into(),
            )))
        }
        _ => Ok(RValue::vec(Vector::Integer(vec![].into()))),
    }
}

#[builtin(min_args = 2)]
fn builtin_append(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match (args.first(), args.get(1)) {
        (Some(RValue::Vector(v1)), Some(RValue::Vector(v2))) => {
            let mut chars = v1.to_characters();
            chars.extend(v2.to_characters());
            Ok(RValue::vec(Vector::Character(chars.into())))
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
            let result = match &v.inner {
                Vector::Double(vals) => Vector::Double(vals[..n.min(vals.len())].to_vec().into()),
                Vector::Integer(vals) => Vector::Integer(vals[..n.min(vals.len())].to_vec().into()),
                Vector::Logical(vals) => Vector::Logical(vals[..n.min(vals.len())].to_vec().into()),
                Vector::Character(vals) => {
                    Vector::Character(vals[..n.min(vals.len())].to_vec().into())
                }
            };
            Ok(RValue::vec(result))
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
            let result = match &v.inner {
                Vector::Double(vals) => Vector::Double(vals[start..].to_vec().into()),
                Vector::Integer(vals) => Vector::Integer(vals[start..].to_vec().into()),
                Vector::Logical(vals) => Vector::Logical(vals[start..].to_vec().into()),
                Vector::Character(vals) => Vector::Character(vals[start..].to_vec().into()),
            };
            Ok(RValue::vec(result))
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
    Ok(RValue::vec(Vector::Double(
        vec![Some(min), Some(max)].into(),
    )))
}

#[builtin(min_args = 1)]
fn builtin_diff(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(v)) => {
            let vals = v.to_doubles();
            if vals.len() < 2 {
                return Ok(RValue::vec(Vector::Double(vec![].into())));
            }
            let result: Vec<Option<f64>> = vals
                .windows(2)
                .map(|w| match (w[0], w[1]) {
                    (Some(a), Some(b)) => Some(b - a),
                    _ => None,
                })
                .collect();
            Ok(RValue::vec(Vector::Double(result.into())))
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
            match &v.inner {
                Vector::Double(vals) => Ok(RValue::vec(Vector::Double(
                    vals.iter()
                        .cycle()
                        .take(length_out)
                        .cloned()
                        .collect::<Vec<_>>()
                        .into(),
                ))),
                Vector::Integer(vals) => Ok(RValue::vec(Vector::Integer(
                    vals.iter()
                        .cycle()
                        .take(length_out)
                        .cloned()
                        .collect::<Vec<_>>()
                        .into(),
                ))),
                Vector::Logical(vals) => Ok(RValue::vec(Vector::Logical(
                    vals.iter()
                        .cycle()
                        .take(length_out)
                        .cloned()
                        .collect::<Vec<_>>()
                        .into(),
                ))),
                Vector::Character(vals) => Ok(RValue::vec(Vector::Character(
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
        RValue::Vector(v) => match &v.inner {
            Vector::Double(vals) => Ok(RValue::vec(Vector::Double(
                vals.iter()
                    .cycle()
                    .take(vals.len() * times)
                    .cloned()
                    .collect::<Vec<_>>()
                    .into(),
            ))),
            Vector::Integer(vals) => Ok(RValue::vec(Vector::Integer(
                vals.iter()
                    .cycle()
                    .take(vals.len() * times)
                    .cloned()
                    .collect::<Vec<_>>()
                    .into(),
            ))),
            Vector::Logical(vals) => Ok(RValue::vec(Vector::Logical(
                vals.iter()
                    .cycle()
                    .take(vals.len() * times)
                    .cloned()
                    .collect::<Vec<_>>()
                    .into(),
            ))),
            Vector::Character(vals) => Ok(RValue::vec(Vector::Character(
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

/// Convert an RValue to an ndarray Array2 (column-major)
fn rvalue_to_array2(val: &RValue) -> Result<Array2<f64>, RError> {
    let (data, dim_attr) = match val {
        RValue::Vector(rv) => (rv.to_doubles(), rv.get_attr("dim")),
        _ => {
            return Err(RError::Type(
                "requires numeric matrix/vector arguments".to_string(),
            ))
        }
    };
    let (nrow, ncol) = match dim_attr {
        Some(RValue::Vector(rv)) => match &rv.inner {
            Vector::Integer(d) if d.len() >= 2 => {
                (d[0].unwrap_or(0) as usize, d[1].unwrap_or(0) as usize)
            }
            _ => (data.len(), 1),
        },
        _ => (data.len(), 1),
    };
    let flat: Vec<f64> = data.iter().map(|x| x.unwrap_or(f64::NAN)).collect();
    Array2::from_shape_vec((nrow, ncol).f(), flat)
        .map_err(|e| RError::Other(format!("matrix shape error: {}", e)))
}

/// Convert an ndarray Array2 back to an RValue matrix
fn array2_to_rvalue(arr: &Array2<f64>) -> RValue {
    let (nrow, ncol) = (arr.nrows(), arr.ncols());
    let mut result = Vec::with_capacity(nrow * ncol);
    for j in 0..ncol {
        for i in 0..nrow {
            result.push(Some(arr[[i, j]]));
        }
    }
    let mut rv = RVector::from(Vector::Double(result.into()));
    rv.set_attr(
        "dim".to_string(),
        RValue::vec(Vector::Integer(
            vec![Some(nrow as i64), Some(ncol as i64)].into(),
        )),
    );
    RValue::Vector(rv)
}

/// crossprod(x, y) = t(x) %*% y
#[builtin(min_args = 1)]
fn builtin_crossprod(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let x = rvalue_to_array2(args.first().unwrap_or(&RValue::Null))?;
    let y = if let Some(b) = args.get(1) {
        rvalue_to_array2(b)?
    } else {
        x.clone()
    };
    let xt = x.t();
    let result = xt.dot(&y);
    Ok(array2_to_rvalue(&result))
}

/// tcrossprod(x, y) = x %*% t(y)
#[builtin(min_args = 1)]
fn builtin_tcrossprod(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let x = rvalue_to_array2(args.first().unwrap_or(&RValue::Null))?;
    let y = if let Some(b) = args.get(1) {
        rvalue_to_array2(b)?
    } else {
        x.clone()
    };
    let yt = y.t();
    let result = x.dot(&yt);
    Ok(array2_to_rvalue(&result))
}

// region: norm, solve, outer

/// `norm(x, type = "O")` — matrix/vector norm.
///
/// Supported types: "O"/"1" (one-norm), "I" (infinity-norm), "F" (Frobenius), "M" (max modulus).
#[builtin(min_args = 1)]
fn builtin_norm(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let norm_type = named
        .iter()
        .find(|(n, _)| n == "type")
        .map(|(_, v)| v)
        .or(args.get(1))
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_else(|| "O".to_string());

    let mat = rvalue_to_array2(args.first().unwrap_or(&RValue::Null))?;
    let nrow = mat.nrows();
    let ncol = mat.ncols();

    let result = match norm_type.as_str() {
        "O" | "o" | "1" => {
            // One-norm: max column sum of absolute values
            (0..ncol)
                .map(|j| (0..nrow).map(|i| mat[[i, j]].abs()).sum::<f64>())
                .fold(f64::NEG_INFINITY, f64::max)
        }
        "I" | "i" => {
            // Infinity-norm: max row sum of absolute values
            (0..nrow)
                .map(|i| (0..ncol).map(|j| mat[[i, j]].abs()).sum::<f64>())
                .fold(f64::NEG_INFINITY, f64::max)
        }
        "F" | "f" => {
            // Frobenius norm: sqrt of sum of squares
            mat.iter().map(|x| x * x).sum::<f64>().sqrt()
        }
        "M" | "m" => {
            // Max modulus: max absolute value
            mat.iter()
                .map(|x| x.abs())
                .fold(f64::NEG_INFINITY, f64::max)
        }
        other => {
            return Err(RError::Argument(format!(
                "invalid norm type '{}'. Use \"O\" (one-norm), \"I\" (infinity-norm), \
                 \"F\" (Frobenius), or \"M\" (max modulus)",
                other
            )));
        }
    };

    Ok(RValue::vec(Vector::Double(vec![Some(result)].into())))
}

/// `solve(a, b)` — solve linear system or compute matrix inverse.
///
/// - `solve(a)`: returns the inverse of matrix a
/// - `solve(a, b)`: solves the linear system Ax = b
#[builtin(min_args = 1)]
fn builtin_solve(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let a = rvalue_to_array2(args.first().unwrap_or(&RValue::Null))?;
    let nrow = a.nrows();
    let ncol = a.ncols();

    if nrow != ncol {
        return Err(RError::Argument(format!(
            "solve() requires a square matrix, but got {}x{}. \
             Non-square systems need qr.solve() or a least-squares method",
            nrow, ncol
        )));
    }
    let n = nrow;

    if n == 0 {
        return Err(RError::Argument(
            "solve() requires a non-empty matrix".to_string(),
        ));
    }

    let b_arg = named
        .iter()
        .find(|(name, _)| name == "b")
        .map(|(_, v)| v)
        .or(args.get(1));

    let b = match b_arg {
        Some(val) => rvalue_to_array2(val)?,
        None => Array2::eye(n),
    };

    if b.nrows() != n {
        return Err(RError::Argument(format!(
            "solve(a, b): nrow(a) = {} but nrow(b) = {} — they must match",
            n,
            b.nrows()
        )));
    }

    let b_ncol = b.ncols();

    // Gaussian elimination with partial pivoting
    let mut aug = Array2::<f64>::zeros((n, n + b_ncol));
    for i in 0..n {
        for j in 0..n {
            aug[[i, j]] = a[[i, j]];
        }
        for j in 0..b_ncol {
            aug[[i, n + j]] = b[[i, j]];
        }
    }

    // Forward elimination with partial pivoting
    for col in 0..n {
        let mut max_val = aug[[col, col]].abs();
        let mut max_row = col;
        for row in (col + 1)..n {
            let val = aug[[row, col]].abs();
            if val > max_val {
                max_val = val;
                max_row = row;
            }
        }

        if max_val < 1e-15 {
            return Err(RError::Other(
                "solve(): matrix is singular (or very close to singular). \
                 Check that your matrix has full rank — its determinant is effectively zero"
                    .to_string(),
            ));
        }

        if max_row != col {
            for j in 0..(n + b_ncol) {
                let tmp = aug[[col, j]];
                aug[[col, j]] = aug[[max_row, j]];
                aug[[max_row, j]] = tmp;
            }
        }

        for row in (col + 1)..n {
            let factor = aug[[row, col]] / aug[[col, col]];
            for j in col..(n + b_ncol) {
                aug[[row, j]] -= factor * aug[[col, j]];
            }
        }
    }

    // Back substitution
    let mut result = Array2::<f64>::zeros((n, b_ncol));
    for bcol in 0..b_ncol {
        for row in (0..n).rev() {
            let mut sum = aug[[row, n + bcol]];
            for j in (row + 1)..n {
                sum -= aug[[row, j]] * result[[j, bcol]];
            }
            result[[row, bcol]] = sum / aug[[row, row]];
        }
    }

    Ok(array2_to_rvalue(&result))
}

/// `outer(X, Y, FUN = "*")` — outer product.
///
/// For each (x_i, y_j), computes FUN(x_i, y_j).
/// Returns a matrix with dim = c(length(X), length(Y)).
#[builtin(min_args = 2)]
fn builtin_outer(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let x_vec = match args.first() {
        Some(RValue::Vector(rv)) => rv.to_doubles(),
        _ => {
            return Err(RError::Argument(
                "outer() requires numeric vectors for X and Y".to_string(),
            ))
        }
    };
    let y_vec = match args.get(1) {
        Some(RValue::Vector(rv)) => rv.to_doubles(),
        _ => {
            return Err(RError::Argument(
                "outer() requires numeric vectors for X and Y".to_string(),
            ))
        }
    };

    let fun_str = named
        .iter()
        .find(|(n, _)| n == "FUN")
        .map(|(_, v)| v)
        .or(args.get(2))
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_else(|| "*".to_string());

    let op: fn(f64, f64) -> f64 = match fun_str.as_str() {
        "*" => |a, b| a * b,
        "+" => |a, b| a + b,
        "-" => |a, b| a - b,
        "/" => |a, b| a / b,
        "^" | "**" => |a: f64, b: f64| a.powf(b),
        "%%" => |a, b| a % b,
        "%/%" => |a: f64, b: f64| (a / b).floor(),
        other => {
            return Err(RError::Argument(format!(
                "outer() with FUN = \"{}\" is not supported. \
                 Supported operators: \"*\", \"+\", \"-\", \"/\", \"^\", \"%%\", \"%/%\"",
                other
            )));
        }
    };

    let nx = x_vec.len();
    let ny = y_vec.len();

    // R stores matrices column-major: iterate columns (Y) then rows (X)
    let mut result = Vec::with_capacity(nx * ny);
    for y_val in &y_vec {
        for x_val in &x_vec {
            let val = match (x_val, y_val) {
                (Some(x), Some(y)) => Some(op(*x, *y)),
                _ => None,
            };
            result.push(val);
        }
    }

    let mut rv = RVector::from(Vector::Double(result.into()));
    rv.set_attr(
        "dim".to_string(),
        RValue::vec(Vector::Integer(
            vec![Some(nx as i64), Some(ny as i64)].into(),
        )),
    );
    Ok(RValue::Vector(rv))
}
// endregion
