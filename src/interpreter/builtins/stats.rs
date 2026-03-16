//! Statistics builtins: cor, cov, weighted.mean, scale, complete.cases, na.omit,
//! and distribution functions (dnorm, pnorm, qnorm, dunif, punif, qunif).

use crate::interpreter::coerce::usize_to_f64;
use crate::interpreter::value::*;
use minir_macros::builtin;
use std::collections::HashMap;
use std::f64::consts::{FRAC_1_SQRT_2, PI};

// region: Helpers

/// Extract na.rm flag from named arguments.
fn extract_na_rm(named: &[(String, RValue)]) -> bool {
    named.iter().any(|(n, v)| {
        n == "na.rm" && v.as_vector().and_then(|v| v.as_logical_scalar()) == Some(true)
    })
}

/// Extract a named f64 parameter from named args, falling back to positional.
fn extract_param(
    args: &[RValue],
    named: &[(String, RValue)],
    name: &str,
    positional_index: usize,
    default: f64,
) -> f64 {
    for (k, v) in named {
        if k == name {
            if let Some(rv) = v.as_vector() {
                if let Some(d) = rv.as_double_scalar() {
                    return d;
                }
            }
        }
    }
    args.get(positional_index)
        .and_then(|v| v.as_vector())
        .and_then(|v| v.as_double_scalar())
        .unwrap_or(default)
}

/// Extract a named bool parameter from named args, falling back to positional.
fn extract_bool_param(
    args: &[RValue],
    named: &[(String, RValue)],
    name: &str,
    positional_index: usize,
    default: bool,
) -> bool {
    for (k, v) in named {
        if k == name {
            if let Some(rv) = v.as_vector() {
                if let Some(b) = rv.as_logical_scalar() {
                    return b;
                }
            }
        }
    }
    args.get(positional_index)
        .and_then(|v| v.as_vector())
        .and_then(|v| v.as_logical_scalar())
        .unwrap_or(default)
}

// endregion

// region: cov

/// Sample covariance of two numeric vectors.
///
/// Computes sum((x - mean(x)) * (y - mean(y))) / (n - 1).
///
/// @param x numeric vector
/// @param y numeric vector (same length as x)
/// @return scalar double
#[builtin(min_args = 2, namespace = "stats")]
fn builtin_cov(args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    let x_vals = args[0]
        .as_vector()
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "cov() requires numeric vectors".to_string(),
            )
        })?
        .to_doubles();
    let y_vals = args[1]
        .as_vector()
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "cov() requires numeric vectors".to_string(),
            )
        })?
        .to_doubles();
    if x_vals.len() != y_vals.len() {
        return Err(RError::new(
            RErrorKind::Argument,
            format!(
                "cov() requires vectors of equal length, got {} and {}",
                x_vals.len(),
                y_vals.len()
            ),
        ));
    }

    // Collect paired non-NA values
    let pairs: Vec<(f64, f64)> = x_vals
        .iter()
        .zip(y_vals.iter())
        .filter_map(|(x, y)| match (x, y) {
            (Some(a), Some(b)) => Some((*a, *b)),
            _ => None,
        })
        .collect();

    let n = usize_to_f64(pairs.len());
    if n < 2.0 {
        return Ok(RValue::vec(Vector::Double(vec![Some(f64::NAN)].into())));
    }
    let mean_x = pairs.iter().map(|(x, _)| x).sum::<f64>() / n;
    let mean_y = pairs.iter().map(|(_, y)| y).sum::<f64>() / n;
    let cov = pairs
        .iter()
        .map(|(x, y)| (x - mean_x) * (y - mean_y))
        .sum::<f64>()
        / (n - 1.0);
    Ok(RValue::vec(Vector::Double(vec![Some(cov)].into())))
}

// endregion

// region: cor

/// Pearson correlation coefficient of two numeric vectors.
///
/// Computes cov(x, y) / (sd(x) * sd(y)). Only method = "pearson" is supported.
///
/// @param x numeric vector
/// @param y numeric vector (same length as x)
/// @param method character; only "pearson" is currently supported
/// @return scalar double
#[builtin(min_args = 2, namespace = "stats")]
fn builtin_cor(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    // Check method (only pearson supported)
    for (k, v) in named {
        if k == "method" {
            if let Some(vec) = v.as_vector() {
                if let Some(s) = vec.as_character_scalar() {
                    if s != "pearson" {
                        return Err(RError::new(
                            RErrorKind::Argument,
                            format!("cor() only supports method = \"pearson\", got \"{}\"", s),
                        ));
                    }
                }
            }
        }
    }

    let x_vals = args[0]
        .as_vector()
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "cor() requires numeric vectors".to_string(),
            )
        })?
        .to_doubles();
    let y_vals = args[1]
        .as_vector()
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "cor() requires numeric vectors".to_string(),
            )
        })?
        .to_doubles();
    if x_vals.len() != y_vals.len() {
        return Err(RError::new(
            RErrorKind::Argument,
            format!(
                "cor() requires vectors of equal length, got {} and {}",
                x_vals.len(),
                y_vals.len()
            ),
        ));
    }

    // Collect paired non-NA values
    let pairs: Vec<(f64, f64)> = x_vals
        .iter()
        .zip(y_vals.iter())
        .filter_map(|(x, y)| match (x, y) {
            (Some(a), Some(b)) => Some((*a, *b)),
            _ => None,
        })
        .collect();

    let n = usize_to_f64(pairs.len());
    if n < 2.0 {
        return Ok(RValue::vec(Vector::Double(vec![Some(f64::NAN)].into())));
    }
    let mean_x = pairs.iter().map(|(x, _)| x).sum::<f64>() / n;
    let mean_y = pairs.iter().map(|(_, y)| y).sum::<f64>() / n;
    let cov_xy = pairs
        .iter()
        .map(|(x, y)| (x - mean_x) * (y - mean_y))
        .sum::<f64>();
    let var_x = pairs.iter().map(|(x, _)| (x - mean_x).powi(2)).sum::<f64>();
    let var_y = pairs.iter().map(|(_, y)| (y - mean_y).powi(2)).sum::<f64>();
    let denom = (var_x * var_y).sqrt();
    if denom == 0.0 {
        return Ok(RValue::vec(Vector::Double(vec![Some(f64::NAN)].into())));
    }
    let r = cov_xy / denom;
    Ok(RValue::vec(Vector::Double(vec![Some(r)].into())))
}

// endregion

// region: weighted.mean

/// Weighted arithmetic mean.
///
/// @param x numeric vector
/// @param w numeric vector of weights (same length as x)
/// @param na.rm logical; if TRUE, remove NAs before computing
/// @return scalar double
#[builtin(name = "weighted.mean", min_args = 2, namespace = "stats")]
fn builtin_weighted_mean(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let na_rm = extract_na_rm(named);
    let x_vals = args[0]
        .as_vector()
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "weighted.mean() requires a numeric vector for 'x'".to_string(),
            )
        })?
        .to_doubles();
    let w_vals = args[1]
        .as_vector()
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "weighted.mean() requires a numeric vector for 'w'".to_string(),
            )
        })?
        .to_doubles();
    if x_vals.len() != w_vals.len() {
        return Err(RError::new(
            RErrorKind::Argument,
            format!(
                "weighted.mean() requires 'x' and 'w' of equal length, got {} and {}",
                x_vals.len(),
                w_vals.len()
            ),
        ));
    }

    let mut sum_wx = 0.0;
    let mut sum_w = 0.0;
    for (x, w) in x_vals.iter().zip(w_vals.iter()) {
        match (x, w) {
            (Some(xv), Some(wv)) => {
                sum_wx += xv * wv;
                sum_w += wv;
            }
            _ if !na_rm => {
                return Ok(RValue::vec(Vector::Double(vec![None].into())));
            }
            _ => {}
        }
    }
    if sum_w == 0.0 {
        return Ok(RValue::vec(Vector::Double(vec![Some(f64::NAN)].into())));
    }
    Ok(RValue::vec(Vector::Double(
        vec![Some(sum_wx / sum_w)].into(),
    )))
}

// endregion

// region: scale

/// Center and/or scale a numeric vector.
///
/// When center = TRUE, subtracts the mean. When scale = TRUE, divides by the
/// standard deviation. Returns a double vector with "scaled:center" and
/// "scaled:scale" attributes.
///
/// @param x numeric vector
/// @param center logical (default TRUE)
/// @param scale logical (default TRUE)
/// @return numeric vector
#[builtin(min_args = 1, namespace = "stats")]
fn builtin_scale(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let do_center = extract_bool_param(args, named, "center", 1, true);
    let do_scale = extract_bool_param(args, named, "scale", 2, true);

    let vals = args[0]
        .as_vector()
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "scale() requires a numeric vector".to_string(),
            )
        })?
        .to_doubles();

    let non_na: Vec<f64> = vals.iter().copied().flatten().collect();
    let n = usize_to_f64(non_na.len());

    let center_val = if do_center && n > 0.0 {
        non_na.iter().sum::<f64>() / n
    } else {
        0.0
    };

    let scale_val = if do_scale && n > 1.0 {
        let mean = if do_center {
            center_val
        } else {
            non_na.iter().sum::<f64>() / n
        };
        let var = non_na.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0);
        var.sqrt()
    } else if do_scale {
        // n <= 1, can't compute sd
        f64::NAN
    } else {
        1.0
    };

    let result: Vec<Option<f64>> = vals
        .iter()
        .map(|x| {
            x.map(|v| {
                let centered = if do_center { v - center_val } else { v };
                if do_scale && scale_val != 0.0 && !scale_val.is_nan() {
                    centered / scale_val
                } else {
                    centered
                }
            })
        })
        .collect();

    let mut attrs: HashMap<String, RValue> = HashMap::new();
    if do_center {
        attrs.insert(
            "scaled:center".to_string(),
            RValue::vec(Vector::Double(vec![Some(center_val)].into())),
        );
    }
    if do_scale {
        attrs.insert(
            "scaled:scale".to_string(),
            RValue::vec(Vector::Double(vec![Some(scale_val)].into())),
        );
    }

    Ok(RValue::Vector(RVector {
        inner: Vector::Double(result.into()),
        attrs: if attrs.is_empty() {
            None
        } else {
            Some(Box::new(attrs))
        },
    }))
}

// endregion

// region: complete.cases

/// Return a logical vector indicating which "rows" have no NA values.
///
/// For vectors, returns TRUE for each non-NA element.
/// For multiple arguments (conceptually columns of a data frame), returns TRUE
/// for positions where all arguments are non-NA.
///
/// @param ... one or more vectors
/// @return logical vector
#[builtin(name = "complete.cases", min_args = 1, namespace = "stats")]
fn builtin_complete_cases(args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    // Determine length from first argument
    let first_vec = args[0].as_vector().ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "complete.cases() requires vector arguments".to_string(),
        )
    })?;
    let n = first_vec.len();

    // Collect all vectors
    let mut cols: Vec<Vec<bool>> = Vec::with_capacity(args.len());
    for arg in args {
        let vec = arg.as_vector().ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "complete.cases() requires vector arguments".to_string(),
            )
        })?;
        if vec.len() != n {
            return Err(RError::new(
                RErrorKind::Argument,
                "complete.cases() requires all arguments to have the same length".to_string(),
            ));
        }
        let is_na = is_na_vec(vec);
        cols.push(is_na);
    }

    // A row is complete if no column has NA at that position
    let result: Vec<Option<bool>> = (0..n)
        .map(|i| Some(!cols.iter().any(|col| col[i])))
        .collect();
    Ok(RValue::vec(Vector::Logical(result.into())))
}

/// Returns a Vec<bool> where true means the element is NA.
fn is_na_vec(v: &Vector) -> Vec<bool> {
    match v {
        Vector::Logical(vals) => vals.iter().map(|x| x.is_none()).collect(),
        Vector::Integer(vals) => vals.iter().map(|x| x.is_none()).collect(),
        Vector::Double(vals) => vals
            .iter()
            .map(|x| x.is_none() || x.map(|f| f.is_nan()).unwrap_or(false))
            .collect(),
        Vector::Complex(vals) => vals.iter().map(|x| x.is_none()).collect(),
        Vector::Character(vals) => vals.iter().map(|x| x.is_none()).collect(),
        Vector::Raw(vals) => vals.iter().map(|_| false).collect(),
    }
}

// endregion

// region: na.omit

/// Remove NA values from a vector.
///
/// Returns a new vector with all NA elements removed. Sets the "na.action"
/// attribute to the indices of removed elements.
///
/// @param object vector
/// @return vector with NAs removed
#[builtin(name = "na.omit", min_args = 1, namespace = "stats")]
fn builtin_na_omit(args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    let vec = args[0].as_vector().ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "na.omit() requires a vector argument".to_string(),
        )
    })?;

    let na_flags = is_na_vec(vec);
    let na_indices: Vec<Option<i64>> = na_flags
        .iter()
        .enumerate()
        .filter(|(_, &is_na)| is_na)
        .map(|(i, _)| {
            // R uses 1-based indexing
            i64::try_from(i + 1).ok()
        })
        .collect();

    let result = filter_non_na(vec, &na_flags);

    let mut attrs: HashMap<String, RValue> = HashMap::new();
    if !na_indices.is_empty() {
        attrs.insert(
            "na.action".to_string(),
            RValue::vec(Vector::Integer(na_indices.into())),
        );
    }

    Ok(RValue::Vector(RVector {
        inner: result,
        attrs: if attrs.is_empty() {
            None
        } else {
            Some(Box::new(attrs))
        },
    }))
}

/// Filter a vector, keeping only non-NA elements.
fn filter_non_na(v: &Vector, na_flags: &[bool]) -> Vector {
    match v {
        Vector::Logical(vals) => Vector::Logical(
            vals.iter()
                .zip(na_flags)
                .filter(|(_, &is_na)| !is_na)
                .map(|(x, _)| *x)
                .collect::<Vec<_>>()
                .into(),
        ),
        Vector::Integer(vals) => Vector::Integer(
            vals.iter()
                .zip(na_flags)
                .filter(|(_, &is_na)| !is_na)
                .map(|(x, _)| *x)
                .collect::<Vec<_>>()
                .into(),
        ),
        Vector::Double(vals) => Vector::Double(
            vals.iter()
                .zip(na_flags)
                .filter(|(_, &is_na)| !is_na)
                .map(|(x, _)| *x)
                .collect::<Vec<_>>()
                .into(),
        ),
        Vector::Complex(vals) => Vector::Complex(
            vals.iter()
                .zip(na_flags)
                .filter(|(_, &is_na)| !is_na)
                .map(|(x, _)| *x)
                .collect::<Vec<_>>()
                .into(),
        ),
        Vector::Character(vals) => Vector::Character(
            vals.iter()
                .zip(na_flags)
                .filter(|(_, &is_na)| !is_na)
                .map(|(x, _)| x.clone())
                .collect::<Vec<_>>()
                .into(),
        ),
        Vector::Raw(vals) => Vector::Raw(
            vals.iter()
                .zip(na_flags)
                .filter(|(_, &is_na)| !is_na)
                .map(|(x, _)| *x)
                .collect(),
        ),
    }
}

// endregion

// region: Normal distribution (dnorm, pnorm, qnorm)

/// Standard normal PDF: exp(-x^2/2) / sqrt(2*pi)
fn std_normal_pdf(x: f64) -> f64 {
    (-0.5 * x * x).exp() / (2.0 * PI).sqrt()
}

/// Standard normal CDF using the error function.
/// pnorm(x) = 0.5 * erfc(-x / sqrt(2))
fn std_normal_cdf(x: f64) -> f64 {
    0.5 * erfc(-x * FRAC_1_SQRT_2)
}

/// Complementary error function approximation (Abramowitz and Stegun 7.1.26).
/// Maximum error: |epsilon(x)| < 1.5e-7
fn erfc(x: f64) -> f64 {
    // erfc(x) = 1 - erf(x)
    // For negative x: erfc(-x) = 2 - erfc(x)
    if x < 0.0 {
        return 2.0 - erfc(-x);
    }
    let t = 1.0 / (1.0 + 0.327_591_1 * x);
    let poly = t
        * (0.254_829_592
            + t * (-0.284_496_736
                + t * (1.421_413_741 + t * (-1.453_152_027 + t * 1.061_405_429))));
    poly * (-x * x).exp()
}

/// Inverse standard normal CDF (quantile function).
///
/// Uses the rational approximation from Peter Acklam (2003).
/// Accurate to roughly 1.15e-9 in the central region.
fn std_normal_quantile(p: f64) -> f64 {
    if p <= 0.0 {
        return f64::NEG_INFINITY;
    }
    if p >= 1.0 {
        return f64::INFINITY;
    }
    if (p - 0.5).abs() < f64::EPSILON {
        return 0.0;
    }

    // Coefficients for the rational approximation
    const A: [f64; 6] = [
        -3.969_683_028_665_376e1,
        2.209_460_984_245_205e2,
        -2.759_285_104_469_687e2,
        1.383_577_518_672_69e2,
        -3.066_479_806_614_716e1,
        2.506_628_277_459_239,
    ];
    const B: [f64; 5] = [
        -5.447_609_879_822_406e1,
        1.615_858_368_580_409e2,
        -1.556_989_798_598_866e2,
        6.680_131_188_771_972e1,
        -1.328_068_155_288_572e1,
    ];
    const C: [f64; 6] = [
        -7.784_894_002_430_293e-3,
        -3.223_964_580_411_365e-1,
        -2.400_758_277_161_838,
        -2.549_732_539_343_734,
        4.374_664_141_464_968,
        2.938_163_982_698_783,
    ];
    const D: [f64; 4] = [
        7.784_695_709_041_462e-3,
        3.224_671_290_700_398e-1,
        2.445_134_137_142_996,
        3.754_408_661_907_416,
    ];

    const P_LOW: f64 = 0.02425;
    const P_HIGH: f64 = 1.0 - P_LOW;

    if p < P_LOW {
        // Lower tail
        let q = (-2.0 * p.ln()).sqrt();
        (((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    } else if p <= P_HIGH {
        // Central region
        let q = p - 0.5;
        let r = q * q;
        (((((A[0] * r + A[1]) * r + A[2]) * r + A[3]) * r + A[4]) * r + A[5]) * q
            / (((((B[0] * r + B[1]) * r + B[2]) * r + B[3]) * r + B[4]) * r + 1.0)
    } else {
        // Upper tail
        let q = (-2.0 * (1.0 - p).ln()).sqrt();
        -(((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    }
}

/// Normal density function.
///
/// @param x quantile vector
/// @param mean mean of the distribution (default 0)
/// @param sd standard deviation (default 1)
/// @return numeric vector of densities
#[builtin(min_args = 1, namespace = "stats")]
fn builtin_dnorm(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let mean = extract_param(args, named, "mean", 1, 0.0);
    let sd = extract_param(args, named, "sd", 2, 1.0);
    if sd < 0.0 {
        return Err(RError::new(
            RErrorKind::Argument,
            "dnorm(): 'sd' must be non-negative".to_string(),
        ));
    }
    let vals = args[0]
        .as_vector()
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "dnorm() requires a numeric vector".to_string(),
            )
        })?
        .to_doubles();
    let result: Vec<Option<f64>> = vals
        .iter()
        .map(|x| {
            x.map(|v| {
                if sd == 0.0 {
                    if v == mean {
                        f64::INFINITY
                    } else {
                        0.0
                    }
                } else {
                    let z = (v - mean) / sd;
                    std_normal_pdf(z) / sd
                }
            })
        })
        .collect();
    Ok(RValue::vec(Vector::Double(result.into())))
}

/// Normal cumulative distribution function.
///
/// @param q quantile vector
/// @param mean mean of the distribution (default 0)
/// @param sd standard deviation (default 1)
/// @return numeric vector of probabilities
#[builtin(min_args = 1, namespace = "stats")]
fn builtin_pnorm(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let mean = extract_param(args, named, "mean", 1, 0.0);
    let sd = extract_param(args, named, "sd", 2, 1.0);
    if sd < 0.0 {
        return Err(RError::new(
            RErrorKind::Argument,
            "pnorm(): 'sd' must be non-negative".to_string(),
        ));
    }
    let vals = args[0]
        .as_vector()
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "pnorm() requires a numeric vector".to_string(),
            )
        })?
        .to_doubles();
    let result: Vec<Option<f64>> = vals
        .iter()
        .map(|x| {
            x.map(|v| {
                if sd == 0.0 {
                    if v < mean {
                        0.0
                    } else {
                        1.0
                    }
                } else {
                    let z = (v - mean) / sd;
                    std_normal_cdf(z)
                }
            })
        })
        .collect();
    Ok(RValue::vec(Vector::Double(result.into())))
}

/// Normal quantile function (inverse CDF).
///
/// @param p probability vector
/// @param mean mean of the distribution (default 0)
/// @param sd standard deviation (default 1)
/// @return numeric vector of quantiles
#[builtin(min_args = 1, namespace = "stats")]
fn builtin_qnorm(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let mean = extract_param(args, named, "mean", 1, 0.0);
    let sd = extract_param(args, named, "sd", 2, 1.0);
    if sd < 0.0 {
        return Err(RError::new(
            RErrorKind::Argument,
            "qnorm(): 'sd' must be non-negative".to_string(),
        ));
    }
    let vals = args[0]
        .as_vector()
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "qnorm() requires a numeric vector".to_string(),
            )
        })?
        .to_doubles();
    let result: Vec<Option<f64>> = vals
        .iter()
        .map(|x| {
            x.map(|p| {
                if !(0.0..=1.0).contains(&p) {
                    f64::NAN
                } else {
                    std_normal_quantile(p) * sd + mean
                }
            })
        })
        .collect();
    Ok(RValue::vec(Vector::Double(result.into())))
}

// endregion

// region: Uniform distribution (dunif, punif, qunif)

/// Uniform density function.
///
/// @param x quantile vector
/// @param min lower limit (default 0)
/// @param max upper limit (default 1)
/// @return numeric vector of densities
#[builtin(min_args = 1, namespace = "stats")]
fn builtin_dunif(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let min = extract_param(args, named, "min", 1, 0.0);
    let max = extract_param(args, named, "max", 2, 1.0);
    if min > max {
        return Err(RError::new(
            RErrorKind::Argument,
            "dunif(): 'min' must not be greater than 'max'".to_string(),
        ));
    }
    let vals = args[0]
        .as_vector()
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "dunif() requires a numeric vector".to_string(),
            )
        })?
        .to_doubles();
    let result: Vec<Option<f64>> = vals
        .iter()
        .map(|x| {
            x.map(|v| {
                if min == max {
                    if v == min {
                        f64::INFINITY
                    } else {
                        0.0
                    }
                } else if v >= min && v <= max {
                    1.0 / (max - min)
                } else {
                    0.0
                }
            })
        })
        .collect();
    Ok(RValue::vec(Vector::Double(result.into())))
}

/// Uniform cumulative distribution function.
///
/// @param q quantile vector
/// @param min lower limit (default 0)
/// @param max upper limit (default 1)
/// @return numeric vector of probabilities
#[builtin(min_args = 1, namespace = "stats")]
fn builtin_punif(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let min = extract_param(args, named, "min", 1, 0.0);
    let max = extract_param(args, named, "max", 2, 1.0);
    if min > max {
        return Err(RError::new(
            RErrorKind::Argument,
            "punif(): 'min' must not be greater than 'max'".to_string(),
        ));
    }
    let vals = args[0]
        .as_vector()
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "punif() requires a numeric vector".to_string(),
            )
        })?
        .to_doubles();
    let result: Vec<Option<f64>> = vals
        .iter()
        .map(|x| {
            x.map(|v| {
                if min == max {
                    if v < min {
                        0.0
                    } else {
                        1.0
                    }
                } else if v <= min {
                    0.0
                } else if v >= max {
                    1.0
                } else {
                    (v - min) / (max - min)
                }
            })
        })
        .collect();
    Ok(RValue::vec(Vector::Double(result.into())))
}

/// Uniform quantile function (inverse CDF).
///
/// @param p probability vector
/// @param min lower limit (default 0)
/// @param max upper limit (default 1)
/// @return numeric vector of quantiles
#[builtin(min_args = 1, namespace = "stats")]
fn builtin_qunif(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let min = extract_param(args, named, "min", 1, 0.0);
    let max = extract_param(args, named, "max", 2, 1.0);
    if min > max {
        return Err(RError::new(
            RErrorKind::Argument,
            "qunif(): 'min' must not be greater than 'max'".to_string(),
        ));
    }
    let vals = args[0]
        .as_vector()
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "qunif() requires a numeric vector".to_string(),
            )
        })?
        .to_doubles();
    let result: Vec<Option<f64>> = vals
        .iter()
        .map(|x| {
            x.map(|p| {
                if !(0.0..=1.0).contains(&p) {
                    f64::NAN
                } else {
                    min + p * (max - min)
                }
            })
        })
        .collect();
    Ok(RValue::vec(Vector::Double(result.into())))
}

// endregion
