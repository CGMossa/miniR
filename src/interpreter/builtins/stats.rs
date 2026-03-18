//! Statistics builtins: cor, cov, weighted.mean, scale, complete.cases, na.omit,
//! and distribution functions (d/p/q for norm, unif, exp, gamma, beta, cauchy,
//! weibull, lnorm, chisq, t, f, binom, pois, geom, hyper).

use crate::interpreter::coerce::usize_to_f64;
use crate::interpreter::value::*;
use indexmap::IndexMap;
use minir_macros::builtin;
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

    let mut attrs: IndexMap<String, RValue> = IndexMap::new();
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

    let mut attrs: IndexMap<String, RValue> = IndexMap::new();
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

// region: Distribution parameter helpers

/// Extract the `log` flag from named arguments (for d* density functions).
fn extract_log_flag(named: &[(String, RValue)]) -> bool {
    named
        .iter()
        .find(|(n, _)| n == "log")
        .and_then(|(_, v)| v.as_vector()?.as_logical_scalar())
        .unwrap_or(false)
}

/// Extract the `lower.tail` flag from named arguments (for p*/q* functions).
fn extract_lower_tail(named: &[(String, RValue)]) -> bool {
    named
        .iter()
        .find(|(n, _)| n == "lower.tail")
        .and_then(|(_, v)| v.as_vector()?.as_logical_scalar())
        .unwrap_or(true)
}

/// Extract the `log.p` flag from named arguments (for p*/q* functions).
fn extract_log_p(named: &[(String, RValue)]) -> bool {
    named
        .iter()
        .find(|(n, _)| n == "log.p")
        .and_then(|(_, v)| v.as_vector()?.as_logical_scalar())
        .unwrap_or(false)
}

/// Post-process a density value: apply log if requested.
fn apply_d_log(val: f64, log_flag: bool) -> f64 {
    if log_flag {
        val.ln()
    } else {
        val
    }
}

/// Post-process a probability value: apply lower.tail and log.p.
fn apply_p_flags(val: f64, lower_tail: bool, log_p: bool) -> f64 {
    let p = if lower_tail { val } else { 1.0 - val };
    if log_p {
        p.ln()
    } else {
        p
    }
}

/// Pre-process a probability input for quantile functions: undo log.p and lower.tail.
fn apply_q_flags(p: f64, lower_tail: bool, log_p: bool) -> f64 {
    let p = if log_p { p.exp() } else { p };
    if lower_tail {
        p
    } else {
        1.0 - p
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
/// @param log logical; if TRUE, return log-density (default FALSE)
/// @return numeric vector of densities
#[builtin(min_args = 1, namespace = "stats")]
fn builtin_dnorm(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let mean = extract_param(args, named, "mean", 1, 0.0);
    let sd = extract_param(args, named, "sd", 2, 1.0);
    let log_flag = extract_log_flag(named);
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
                let d = if sd == 0.0 {
                    if v == mean {
                        f64::INFINITY
                    } else {
                        0.0
                    }
                } else {
                    let z = (v - mean) / sd;
                    std_normal_pdf(z) / sd
                };
                apply_d_log(d, log_flag)
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
/// @param lower.tail logical; if TRUE (default), return P(X <= q), else P(X > q)
/// @param log.p logical; if TRUE, return log-probability (default FALSE)
/// @return numeric vector of probabilities
#[builtin(min_args = 1, namespace = "stats")]
fn builtin_pnorm(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let mean = extract_param(args, named, "mean", 1, 0.0);
    let sd = extract_param(args, named, "sd", 2, 1.0);
    let lower_tail = extract_lower_tail(named);
    let log_p = extract_log_p(named);
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
                let p = if sd == 0.0 {
                    if v < mean {
                        0.0
                    } else {
                        1.0
                    }
                } else {
                    let z = (v - mean) / sd;
                    std_normal_cdf(z)
                };
                apply_p_flags(p, lower_tail, log_p)
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
/// @param lower.tail logical; if TRUE (default), p is P(X <= q), else P(X > q)
/// @param log.p logical; if TRUE, p is given as log(p) (default FALSE)
/// @return numeric vector of quantiles
#[builtin(min_args = 1, namespace = "stats")]
fn builtin_qnorm(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let mean = extract_param(args, named, "mean", 1, 0.0);
    let sd = extract_param(args, named, "sd", 2, 1.0);
    let lower_tail = extract_lower_tail(named);
    let log_p = extract_log_p(named);
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
            x.map(|raw_p| {
                let p = apply_q_flags(raw_p, lower_tail, log_p);
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
    let log_flag = extract_log_flag(named);
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
                let d = if min == max {
                    if v == min {
                        f64::INFINITY
                    } else {
                        0.0
                    }
                } else if v >= min && v <= max {
                    1.0 / (max - min)
                } else {
                    0.0
                };
                apply_d_log(d, log_flag)
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
    let lower_tail = extract_lower_tail(named);
    let log_p = extract_log_p(named);
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
                let p = if min == max {
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
                };
                apply_p_flags(p, lower_tail, log_p)
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
    let lower_tail = extract_lower_tail(named);
    let log_p = extract_log_p(named);
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
            x.map(|raw_p| {
                let p = apply_q_flags(raw_p, lower_tail, log_p);
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

// region: Mathematical helpers for distribution functions

/// Log of the gamma function (via libm).
fn ln_gamma(x: f64) -> f64 {
    libm::lgamma(x)
}

/// Log of the binomial coefficient: lchoose(n, k) = lgamma(n+1) - lgamma(k+1) - lgamma(n-k+1).
fn lchoose(n: f64, k: f64) -> f64 {
    ln_gamma(n + 1.0) - ln_gamma(k + 1.0) - ln_gamma(n - k + 1.0)
}

/// Regularized lower incomplete gamma function P(a, x) = gamma(a, x) / Gamma(a).
/// Uses series expansion for x < a+1, continued fraction otherwise.
fn regularized_gamma_p(a: f64, x: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    if a <= 0.0 {
        return 1.0;
    }
    if x < a + 1.0 {
        gamma_series(a, x)
    } else {
        1.0 - gamma_cf_lentz(a, x, ln_gamma(a))
    }
}

/// Series expansion for the regularized incomplete gamma function.
fn gamma_series(a: f64, x: f64) -> f64 {
    let ln_gamma_a = ln_gamma(a);
    let mut sum = 1.0 / a;
    let mut term = 1.0 / a;
    for n in 1..200 {
        let nf = f64::from(n);
        term *= x / (a + nf);
        sum += term;
        if term.abs() < sum.abs() * 1e-15 {
            break;
        }
    }
    sum * (-x + a * x.ln() - ln_gamma_a).exp()
}

/// Lentz's algorithm for the continued fraction representation of Q(a, x).
fn gamma_cf_lentz(a: f64, x: f64, ln_gamma_a: f64) -> f64 {
    // CF for Q(a, x): b_0 = x+1-a, a_n = n*(a-n), b_n = x+2n+1-a
    let b0 = x + 1.0 - a;
    let mut f = if b0.abs() < 1e-30 { 1e-30 } else { b0 };
    let mut c = f;
    let mut d = 0.0;

    for n in 1..200 {
        let nf = f64::from(n);
        let an = nf * (a - nf);
        let bn = x + 2.0 * nf + 1.0 - a;

        d = bn + an * d;
        if d.abs() < 1e-30 {
            d = 1e-30;
        }
        d = 1.0 / d;

        c = bn + an / c;
        if c.abs() < 1e-30 {
            c = 1e-30;
        }

        let delta = c * d;
        f *= delta;

        if (delta - 1.0).abs() < 1e-15 {
            break;
        }
    }

    (-x + a * x.ln() - ln_gamma_a).exp() / f
}

/// Regularized incomplete beta function I_x(a, b) using the NR continued fraction.
fn regularized_beta(x: f64, a: f64, b: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    // Use the symmetry relation when x > (a+1)/(a+b+2) for better convergence
    if x > (a + 1.0) / (a + b + 2.0) {
        return 1.0 - regularized_beta(1.0 - x, b, a);
    }
    let log_prefix = a * x.ln() + b * (1.0 - x).ln() - ln_gamma(a) - ln_gamma(b) + ln_gamma(a + b);
    let prefix = log_prefix.exp();

    prefix * betacf(a, b, x) / a
}

/// Continued fraction for the incomplete beta function (Numerical Recipes, 3rd ed).
///
/// Evaluates the CF representation of I_x(a,b) using the modified Lentz method.
fn betacf(a: f64, b: f64, x: f64) -> f64 {
    const TINY: f64 = 1e-30;
    const EPS: f64 = 1e-14;

    let qab = a + b;
    let qap = a + 1.0;
    let qam = a - 1.0;

    let mut c = 1.0_f64;
    let mut d = 1.0 - qab * x / qap;
    if d.abs() < TINY {
        d = TINY;
    }
    d = 1.0 / d;
    let mut h = d;

    for m in 1..300i32 {
        let mf = f64::from(m);
        let m2f = f64::from(2 * m);

        // Even step coefficient: m(b-m)x / ((a+2m-1)(a+2m))
        let aa = mf * (b - mf) * x / ((qam + m2f) * (a + m2f));
        d = 1.0 + aa * d;
        if d.abs() < TINY {
            d = TINY;
        }
        d = 1.0 / d;
        c = 1.0 + aa / c;
        if c.abs() < TINY {
            c = TINY;
        }
        h *= d * c;

        // Odd step coefficient: -(a+m)(a+b+m)x / ((a+2m)(a+2m+1))
        let aa = -(a + mf) * (qab + mf) * x / ((a + m2f) * (qap + m2f));
        d = 1.0 + aa * d;
        if d.abs() < TINY {
            d = TINY;
        }
        d = 1.0 / d;
        c = 1.0 + aa / c;
        if c.abs() < TINY {
            c = TINY;
        }
        let del = d * c;
        h *= del;

        if (del - 1.0).abs() < EPS {
            break;
        }
    }

    h
}

/// Bisection-based quantile finder given a CDF.
/// Finds x such that cdf(x) = p.
fn quantile_bisect(p: f64, mut lo: f64, mut hi: f64, cdf: impl Fn(f64) -> f64) -> f64 {
    if p <= 0.0 {
        return lo;
    }
    if p >= 1.0 {
        return hi;
    }
    // Widen bounds if needed
    while cdf(lo) > p {
        lo = if lo <= 0.0 { lo - 1.0 } else { lo * 0.5 };
        if lo < -1e15 {
            return f64::NEG_INFINITY;
        }
    }
    while cdf(hi) < p {
        hi *= 2.0;
        hi += 1.0;
        if hi > 1e15 {
            return f64::INFINITY;
        }
    }
    for _ in 0..100 {
        let mid = 0.5 * (lo + hi);
        if (hi - lo).abs() < 1e-12 * (1.0 + mid.abs()) {
            return mid;
        }
        if cdf(mid) < p {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// Bisection-based quantile finder for discrete distributions.
/// Finds smallest integer x such that cdf(x) >= p, searching in [lo, hi].
fn quantile_bisect_discrete(p: f64, lo: i64, hi: i64, cdf: impl Fn(i64) -> f64) -> f64 {
    if p <= 0.0 {
        return lo as f64;
    }
    if p >= 1.0 {
        return hi as f64;
    }
    let (mut left, mut right) = (lo, hi);
    while left < right {
        let mid = left + (right - left) / 2;
        if cdf(mid) < p {
            left = mid + 1;
        } else {
            right = mid;
        }
    }
    left as f64
}

/// Helper to vectorize a distribution function over the first argument.
fn dist_vectorize(args: &[RValue], fname: &str, f: impl Fn(f64) -> f64) -> Result<RValue, RError> {
    let vals = args[0]
        .as_vector()
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                format!("{fname}() requires a numeric vector"),
            )
        })?
        .to_doubles();
    let result: Vec<Option<f64>> = vals.iter().map(|x| x.map(&f)).collect();
    Ok(RValue::vec(Vector::Double(result.into())))
}

/// Like `dist_vectorize` but applies `log` post-processing for density functions.
fn dist_vectorize_d(
    args: &[RValue],
    fname: &str,
    log_flag: bool,
    f: impl Fn(f64) -> f64,
) -> Result<RValue, RError> {
    dist_vectorize(args, fname, |x| apply_d_log(f(x), log_flag))
}

/// Like `dist_vectorize` but applies `lower.tail`/`log.p` post-processing for CDF functions.
fn dist_vectorize_p(
    args: &[RValue],
    fname: &str,
    lower_tail: bool,
    log_p: bool,
    f: impl Fn(f64) -> f64,
) -> Result<RValue, RError> {
    dist_vectorize(args, fname, |x| apply_p_flags(f(x), lower_tail, log_p))
}

/// Like `dist_vectorize` but applies `lower.tail`/`log.p` pre-processing for quantile functions.
fn dist_vectorize_q(
    args: &[RValue],
    fname: &str,
    lower_tail: bool,
    log_p: bool,
    f: impl Fn(f64) -> f64,
) -> Result<RValue, RError> {
    dist_vectorize(args, fname, |raw_p| {
        f(apply_q_flags(raw_p, lower_tail, log_p))
    })
}

// endregion

// region: Exponential distribution (dexp, pexp, qexp)

/// Exponential density function.
///
/// @param x quantile vector
/// @param rate rate parameter (default 1)
/// @return numeric vector of densities
#[builtin(min_args = 1, namespace = "stats")]
fn builtin_dexp(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let rate = extract_param(args, named, "rate", 1, 1.0);
    let log_flag = extract_log_flag(named);
    if rate <= 0.0 {
        return Err(RError::new(
            RErrorKind::Argument,
            "dexp(): 'rate' must be positive".to_string(),
        ));
    }
    dist_vectorize_d(args, "dexp", log_flag, |x| {
        if x < 0.0 {
            0.0
        } else {
            rate * (-rate * x).exp()
        }
    })
}

/// Exponential cumulative distribution function.
///
/// @param q quantile vector
/// @param rate rate parameter (default 1)
/// @return numeric vector of probabilities
#[builtin(min_args = 1, namespace = "stats")]
fn builtin_pexp(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let rate = extract_param(args, named, "rate", 1, 1.0);
    let lower_tail = extract_lower_tail(named);
    let log_p = extract_log_p(named);
    if rate <= 0.0 {
        return Err(RError::new(
            RErrorKind::Argument,
            "pexp(): 'rate' must be positive".to_string(),
        ));
    }
    dist_vectorize_p(args, "pexp", lower_tail, log_p, |q| {
        if q < 0.0 {
            0.0
        } else {
            1.0 - (-rate * q).exp()
        }
    })
}

/// Exponential quantile function (inverse CDF).
///
/// @param p probability vector
/// @param rate rate parameter (default 1)
/// @return numeric vector of quantiles
#[builtin(min_args = 1, namespace = "stats")]
fn builtin_qexp(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let rate = extract_param(args, named, "rate", 1, 1.0);
    let lower_tail = extract_lower_tail(named);
    let log_p = extract_log_p(named);
    if rate <= 0.0 {
        return Err(RError::new(
            RErrorKind::Argument,
            "qexp(): 'rate' must be positive".to_string(),
        ));
    }
    dist_vectorize_q(args, "qexp", lower_tail, log_p, |p| {
        if !(0.0..=1.0).contains(&p) {
            f64::NAN
        } else if p == 1.0 {
            f64::INFINITY
        } else {
            -(1.0 - p).ln() / rate
        }
    })
}

// endregion

// region: Gamma distribution (dgamma, pgamma, qgamma)

/// Gamma density function.
///
/// @param x quantile vector
/// @param shape shape parameter
/// @param rate rate parameter (default 1); scale = 1/rate
/// @return numeric vector of densities
#[builtin(min_args = 2, namespace = "stats")]
fn builtin_dgamma(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let shape = extract_param(args, named, "shape", 1, f64::NAN);
    let rate = extract_param(args, named, "rate", 2, 1.0);
    let log_flag = extract_log_flag(named);
    if shape <= 0.0 || rate <= 0.0 || shape.is_nan() {
        return Err(RError::new(
            RErrorKind::Argument,
            "dgamma(): 'shape' and 'rate' must be positive".to_string(),
        ));
    }
    let scale = 1.0 / rate;
    let log_norm = shape * scale.ln() + ln_gamma(shape);
    dist_vectorize_d(args, "dgamma", log_flag, |x| {
        if x < 0.0 {
            0.0
        } else if x == 0.0 {
            if shape < 1.0 {
                f64::INFINITY
            } else if shape == 1.0 {
                rate
            } else {
                0.0
            }
        } else {
            ((shape - 1.0) * x.ln() - x / scale - log_norm).exp()
        }
    })
}

/// Gamma cumulative distribution function.
///
/// Uses the regularized incomplete gamma function.
///
/// @param q quantile vector
/// @param shape shape parameter
/// @param rate rate parameter (default 1)
/// @return numeric vector of probabilities
#[builtin(min_args = 2, namespace = "stats")]
fn builtin_pgamma(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let shape = extract_param(args, named, "shape", 1, f64::NAN);
    let rate = extract_param(args, named, "rate", 2, 1.0);
    let lower_tail = extract_lower_tail(named);
    let log_p = extract_log_p(named);
    if shape <= 0.0 || rate <= 0.0 || shape.is_nan() {
        return Err(RError::new(
            RErrorKind::Argument,
            "pgamma(): 'shape' and 'rate' must be positive".to_string(),
        ));
    }
    let scale = 1.0 / rate;
    dist_vectorize_p(args, "pgamma", lower_tail, log_p, |q| {
        if q <= 0.0 {
            0.0
        } else {
            regularized_gamma_p(shape, q / scale)
        }
    })
}

/// Gamma quantile function (inverse CDF).
///
/// Uses bisection on the CDF.
///
/// @param p probability vector
/// @param shape shape parameter
/// @param rate rate parameter (default 1)
/// @return numeric vector of quantiles
#[builtin(min_args = 2, namespace = "stats")]
fn builtin_qgamma(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let shape = extract_param(args, named, "shape", 1, f64::NAN);
    let rate = extract_param(args, named, "rate", 2, 1.0);
    let lower_tail = extract_lower_tail(named);
    let log_p = extract_log_p(named);
    if shape <= 0.0 || rate <= 0.0 || shape.is_nan() {
        return Err(RError::new(
            RErrorKind::Argument,
            "qgamma(): 'shape' and 'rate' must be positive".to_string(),
        ));
    }
    let scale = 1.0 / rate;
    dist_vectorize_q(args, "qgamma", lower_tail, log_p, |p| {
        if !(0.0..=1.0).contains(&p) {
            f64::NAN
        } else if p == 0.0 {
            0.0
        } else if p == 1.0 {
            f64::INFINITY
        } else {
            let mean = shape * scale;
            quantile_bisect(p, 0.0, mean * 5.0 + 10.0, |x| {
                regularized_gamma_p(shape, x / scale)
            })
        }
    })
}

// endregion

// region: Beta distribution (dbeta, pbeta, qbeta)

/// Beta density function.
///
/// @param x quantile vector (values in [0, 1])
/// @param shape1 first shape parameter
/// @param shape2 second shape parameter
/// @return numeric vector of densities
#[builtin(min_args = 3, namespace = "stats")]
fn builtin_dbeta(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let shape1 = extract_param(args, named, "shape1", 1, f64::NAN);
    let shape2 = extract_param(args, named, "shape2", 2, f64::NAN);
    let log_flag = extract_log_flag(named);
    if shape1 <= 0.0 || shape2 <= 0.0 || shape1.is_nan() || shape2.is_nan() {
        return Err(RError::new(
            RErrorKind::Argument,
            "dbeta(): 'shape1' and 'shape2' must be positive".to_string(),
        ));
    }
    let log_beta = ln_gamma(shape1) + ln_gamma(shape2) - ln_gamma(shape1 + shape2);
    dist_vectorize_d(args, "dbeta", log_flag, |x| {
        if !(0.0..=1.0).contains(&x) {
            0.0
        } else if x == 0.0 {
            if shape1 < 1.0 {
                f64::INFINITY
            } else if shape1 == 1.0 {
                shape2
            } else {
                0.0
            }
        } else if x == 1.0 {
            if shape2 < 1.0 {
                f64::INFINITY
            } else if shape2 == 1.0 {
                shape1
            } else {
                0.0
            }
        } else {
            ((shape1 - 1.0) * x.ln() + (shape2 - 1.0) * (1.0 - x).ln() - log_beta).exp()
        }
    })
}

/// Beta cumulative distribution function.
///
/// Uses the regularized incomplete beta function.
///
/// @param q quantile vector
/// @param shape1 first shape parameter
/// @param shape2 second shape parameter
/// @return numeric vector of probabilities
#[builtin(min_args = 3, namespace = "stats")]
fn builtin_pbeta(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let shape1 = extract_param(args, named, "shape1", 1, f64::NAN);
    let shape2 = extract_param(args, named, "shape2", 2, f64::NAN);
    let lower_tail = extract_lower_tail(named);
    let log_p = extract_log_p(named);
    if shape1 <= 0.0 || shape2 <= 0.0 || shape1.is_nan() || shape2.is_nan() {
        return Err(RError::new(
            RErrorKind::Argument,
            "pbeta(): 'shape1' and 'shape2' must be positive".to_string(),
        ));
    }
    dist_vectorize_p(args, "pbeta", lower_tail, log_p, |q| {
        if q <= 0.0 {
            0.0
        } else if q >= 1.0 {
            1.0
        } else {
            regularized_beta(q, shape1, shape2)
        }
    })
}

/// Beta quantile function (inverse CDF).
///
/// Uses bisection on the CDF.
///
/// @param p probability vector
/// @param shape1 first shape parameter
/// @param shape2 second shape parameter
/// @return numeric vector of quantiles
#[builtin(min_args = 3, namespace = "stats")]
fn builtin_qbeta(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let shape1 = extract_param(args, named, "shape1", 1, f64::NAN);
    let shape2 = extract_param(args, named, "shape2", 2, f64::NAN);
    let lower_tail = extract_lower_tail(named);
    let log_p = extract_log_p(named);
    if shape1 <= 0.0 || shape2 <= 0.0 || shape1.is_nan() || shape2.is_nan() {
        return Err(RError::new(
            RErrorKind::Argument,
            "qbeta(): 'shape1' and 'shape2' must be positive".to_string(),
        ));
    }
    dist_vectorize_q(args, "qbeta", lower_tail, log_p, |p| {
        if !(0.0..=1.0).contains(&p) {
            f64::NAN
        } else if p == 0.0 {
            0.0
        } else if p == 1.0 {
            1.0
        } else {
            quantile_bisect(p, 0.0, 1.0, |x| regularized_beta(x, shape1, shape2))
        }
    })
}

// endregion

// region: Cauchy distribution (dcauchy, pcauchy, qcauchy)

/// Cauchy density function.
///
/// @param x quantile vector
/// @param location location parameter (default 0)
/// @param scale scale parameter (default 1)
/// @return numeric vector of densities
#[builtin(min_args = 1, namespace = "stats")]
fn builtin_dcauchy(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let location = extract_param(args, named, "location", 1, 0.0);
    let scale = extract_param(args, named, "scale", 2, 1.0);
    let log_flag = extract_log_flag(named);
    if scale <= 0.0 {
        return Err(RError::new(
            RErrorKind::Argument,
            "dcauchy(): 'scale' must be positive".to_string(),
        ));
    }
    dist_vectorize_d(args, "dcauchy", log_flag, |x| {
        let z = (x - location) / scale;
        1.0 / (PI * scale * (1.0 + z * z))
    })
}

/// Cauchy cumulative distribution function.
///
/// @param q quantile vector
/// @param location location parameter (default 0)
/// @param scale scale parameter (default 1)
/// @return numeric vector of probabilities
#[builtin(min_args = 1, namespace = "stats")]
fn builtin_pcauchy(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let location = extract_param(args, named, "location", 1, 0.0);
    let scale = extract_param(args, named, "scale", 2, 1.0);
    let lower_tail = extract_lower_tail(named);
    let log_p = extract_log_p(named);
    if scale <= 0.0 {
        return Err(RError::new(
            RErrorKind::Argument,
            "pcauchy(): 'scale' must be positive".to_string(),
        ));
    }
    dist_vectorize_p(args, "pcauchy", lower_tail, log_p, |q| {
        0.5 + ((q - location) / scale).atan() / PI
    })
}

/// Cauchy quantile function (inverse CDF).
///
/// @param p probability vector
/// @param location location parameter (default 0)
/// @param scale scale parameter (default 1)
/// @return numeric vector of quantiles
#[builtin(min_args = 1, namespace = "stats")]
fn builtin_qcauchy(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let location = extract_param(args, named, "location", 1, 0.0);
    let scale = extract_param(args, named, "scale", 2, 1.0);
    let lower_tail = extract_lower_tail(named);
    let log_p = extract_log_p(named);
    if scale <= 0.0 {
        return Err(RError::new(
            RErrorKind::Argument,
            "qcauchy(): 'scale' must be positive".to_string(),
        ));
    }
    dist_vectorize_q(args, "qcauchy", lower_tail, log_p, |p| {
        if !(0.0..=1.0).contains(&p) {
            f64::NAN
        } else if p == 0.0 {
            f64::NEG_INFINITY
        } else if p == 1.0 {
            f64::INFINITY
        } else {
            location + scale * (PI * (p - 0.5)).tan()
        }
    })
}

// endregion

// region: Weibull distribution (dweibull, pweibull, qweibull)

/// Weibull density function.
///
/// @param x quantile vector
/// @param shape shape parameter
/// @param scale scale parameter (default 1)
/// @return numeric vector of densities
#[builtin(min_args = 2, namespace = "stats")]
fn builtin_dweibull(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let shape = extract_param(args, named, "shape", 1, f64::NAN);
    let scale = extract_param(args, named, "scale", 2, 1.0);
    let log_flag = extract_log_flag(named);
    if shape <= 0.0 || scale <= 0.0 || shape.is_nan() {
        return Err(RError::new(
            RErrorKind::Argument,
            "dweibull(): 'shape' and 'scale' must be positive".to_string(),
        ));
    }
    dist_vectorize_d(args, "dweibull", log_flag, |x| {
        if x < 0.0 {
            0.0
        } else if x == 0.0 {
            if shape < 1.0 {
                f64::INFINITY
            } else if shape == 1.0 {
                1.0 / scale
            } else {
                0.0
            }
        } else {
            let z = x / scale;
            (shape / scale) * z.powf(shape - 1.0) * (-z.powf(shape)).exp()
        }
    })
}

/// Weibull cumulative distribution function.
///
/// @param q quantile vector
/// @param shape shape parameter
/// @param scale scale parameter (default 1)
/// @return numeric vector of probabilities
#[builtin(min_args = 2, namespace = "stats")]
fn builtin_pweibull(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let shape = extract_param(args, named, "shape", 1, f64::NAN);
    let scale = extract_param(args, named, "scale", 2, 1.0);
    let lower_tail = extract_lower_tail(named);
    let log_p = extract_log_p(named);
    if shape <= 0.0 || scale <= 0.0 || shape.is_nan() {
        return Err(RError::new(
            RErrorKind::Argument,
            "pweibull(): 'shape' and 'scale' must be positive".to_string(),
        ));
    }
    dist_vectorize_p(args, "pweibull", lower_tail, log_p, |q| {
        if q <= 0.0 {
            0.0
        } else {
            1.0 - (-(q / scale).powf(shape)).exp()
        }
    })
}

/// Weibull quantile function (inverse CDF).
///
/// @param p probability vector
/// @param shape shape parameter
/// @param scale scale parameter (default 1)
/// @return numeric vector of quantiles
#[builtin(min_args = 2, namespace = "stats")]
fn builtin_qweibull(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let shape = extract_param(args, named, "shape", 1, f64::NAN);
    let scale = extract_param(args, named, "scale", 2, 1.0);
    let lower_tail = extract_lower_tail(named);
    let log_p = extract_log_p(named);
    if shape <= 0.0 || scale <= 0.0 || shape.is_nan() {
        return Err(RError::new(
            RErrorKind::Argument,
            "qweibull(): 'shape' and 'scale' must be positive".to_string(),
        ));
    }
    dist_vectorize_q(args, "qweibull", lower_tail, log_p, |p| {
        if !(0.0..=1.0).contains(&p) {
            f64::NAN
        } else if p == 0.0 {
            0.0
        } else if p == 1.0 {
            f64::INFINITY
        } else {
            scale * (-(1.0 - p).ln()).powf(1.0 / shape)
        }
    })
}

// endregion

// region: Log-normal distribution (dlnorm, plnorm, qlnorm)

/// Log-normal density function.
///
/// @param x quantile vector
/// @param meanlog mean of the distribution on the log scale (default 0)
/// @param sdlog standard deviation on the log scale (default 1)
/// @return numeric vector of densities
#[builtin(min_args = 1, namespace = "stats")]
fn builtin_dlnorm(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let meanlog = extract_param(args, named, "meanlog", 1, 0.0);
    let sdlog = extract_param(args, named, "sdlog", 2, 1.0);
    let log_flag = extract_log_flag(named);
    if sdlog < 0.0 {
        return Err(RError::new(
            RErrorKind::Argument,
            "dlnorm(): 'sdlog' must be non-negative".to_string(),
        ));
    }
    dist_vectorize_d(args, "dlnorm", log_flag, |x| {
        if x <= 0.0 {
            0.0
        } else if sdlog == 0.0 {
            if (x.ln() - meanlog).abs() < f64::EPSILON {
                f64::INFINITY
            } else {
                0.0
            }
        } else {
            let z = (x.ln() - meanlog) / sdlog;
            std_normal_pdf(z) / (x * sdlog)
        }
    })
}

/// Log-normal cumulative distribution function.
///
/// @param q quantile vector
/// @param meanlog mean of the distribution on the log scale (default 0)
/// @param sdlog standard deviation on the log scale (default 1)
/// @return numeric vector of probabilities
#[builtin(min_args = 1, namespace = "stats")]
fn builtin_plnorm(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let meanlog = extract_param(args, named, "meanlog", 1, 0.0);
    let sdlog = extract_param(args, named, "sdlog", 2, 1.0);
    let lower_tail = extract_lower_tail(named);
    let log_p = extract_log_p(named);
    if sdlog < 0.0 {
        return Err(RError::new(
            RErrorKind::Argument,
            "plnorm(): 'sdlog' must be non-negative".to_string(),
        ));
    }
    dist_vectorize_p(args, "plnorm", lower_tail, log_p, |q| {
        if q <= 0.0 {
            0.0
        } else if sdlog == 0.0 {
            if q.ln() < meanlog {
                0.0
            } else {
                1.0
            }
        } else {
            let z = (q.ln() - meanlog) / sdlog;
            std_normal_cdf(z)
        }
    })
}

/// Log-normal quantile function (inverse CDF).
///
/// @param p probability vector
/// @param meanlog mean of the distribution on the log scale (default 0)
/// @param sdlog standard deviation on the log scale (default 1)
/// @return numeric vector of quantiles
#[builtin(min_args = 1, namespace = "stats")]
fn builtin_qlnorm(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let meanlog = extract_param(args, named, "meanlog", 1, 0.0);
    let sdlog = extract_param(args, named, "sdlog", 2, 1.0);
    let lower_tail = extract_lower_tail(named);
    let log_p = extract_log_p(named);
    if sdlog < 0.0 {
        return Err(RError::new(
            RErrorKind::Argument,
            "qlnorm(): 'sdlog' must be non-negative".to_string(),
        ));
    }
    dist_vectorize_q(args, "qlnorm", lower_tail, log_p, |p| {
        if !(0.0..=1.0).contains(&p) {
            f64::NAN
        } else if p == 0.0 {
            0.0
        } else if p == 1.0 {
            f64::INFINITY
        } else {
            (std_normal_quantile(p) * sdlog + meanlog).exp()
        }
    })
}

// endregion

// region: Chi-squared distribution (dchisq, pchisq, qchisq)

/// Chi-squared density function.
///
/// The chi-squared distribution with df degrees of freedom is
/// Gamma(df/2, 1/2), so dchisq(x, df) = dgamma(x, df/2, rate = 0.5).
///
/// @param x quantile vector
/// @param df degrees of freedom
/// @return numeric vector of densities
#[builtin(min_args = 2, namespace = "stats")]
fn builtin_dchisq(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let df = extract_param(args, named, "df", 1, f64::NAN);
    let log_flag = extract_log_flag(named);
    if df <= 0.0 || df.is_nan() {
        return Err(RError::new(
            RErrorKind::Argument,
            "dchisq(): 'df' must be positive".to_string(),
        ));
    }
    let shape = df / 2.0;
    let scale: f64 = 2.0;
    let log_norm = shape * scale.ln() + ln_gamma(shape);
    dist_vectorize_d(args, "dchisq", log_flag, |x| {
        if x < 0.0 {
            0.0
        } else if x == 0.0 {
            if shape < 1.0 {
                f64::INFINITY
            } else if shape == 1.0 {
                0.5
            } else {
                0.0
            }
        } else {
            ((shape - 1.0) * x.ln() - x / scale - log_norm).exp()
        }
    })
}

/// Chi-squared cumulative distribution function.
///
/// @param q quantile vector
/// @param df degrees of freedom
/// @return numeric vector of probabilities
#[builtin(min_args = 2, namespace = "stats")]
fn builtin_pchisq(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let df = extract_param(args, named, "df", 1, f64::NAN);
    let lower_tail = extract_lower_tail(named);
    let log_p = extract_log_p(named);
    if df <= 0.0 || df.is_nan() {
        return Err(RError::new(
            RErrorKind::Argument,
            "pchisq(): 'df' must be positive".to_string(),
        ));
    }
    let shape = df / 2.0;
    dist_vectorize_p(args, "pchisq", lower_tail, log_p, |q| {
        if q <= 0.0 {
            0.0
        } else {
            regularized_gamma_p(shape, q / 2.0)
        }
    })
}

/// Chi-squared quantile function (inverse CDF).
///
/// @param p probability vector
/// @param df degrees of freedom
/// @return numeric vector of quantiles
#[builtin(min_args = 2, namespace = "stats")]
fn builtin_qchisq(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let df = extract_param(args, named, "df", 1, f64::NAN);
    let lower_tail = extract_lower_tail(named);
    let log_p = extract_log_p(named);
    if df <= 0.0 || df.is_nan() {
        return Err(RError::new(
            RErrorKind::Argument,
            "qchisq(): 'df' must be positive".to_string(),
        ));
    }
    let shape = df / 2.0;
    dist_vectorize_q(args, "qchisq", lower_tail, log_p, |p| {
        if !(0.0..=1.0).contains(&p) {
            f64::NAN
        } else if p == 0.0 {
            0.0
        } else if p == 1.0 {
            f64::INFINITY
        } else {
            quantile_bisect(p, 0.0, df * 5.0 + 10.0, |x| {
                regularized_gamma_p(shape, x / 2.0)
            })
        }
    })
}

// endregion

// region: Student's t distribution (dt, pt, qt)

/// Student's t density function.
///
/// dt(x, df) = gamma((df+1)/2) / (sqrt(df*pi) * gamma(df/2)) * (1 + x^2/df)^(-(df+1)/2)
///
/// @param x quantile vector
/// @param df degrees of freedom
/// @return numeric vector of densities
#[builtin(min_args = 2, namespace = "stats")]
fn builtin_dt(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let df = extract_param(args, named, "df", 1, f64::NAN);
    let log_flag = extract_log_flag(named);
    if df <= 0.0 || df.is_nan() {
        return Err(RError::new(
            RErrorKind::Argument,
            "dt(): 'df' must be positive".to_string(),
        ));
    }
    let log_const = ln_gamma((df + 1.0) / 2.0) - ln_gamma(df / 2.0) - 0.5 * (df * PI).ln();
    dist_vectorize_d(args, "dt", log_flag, |x| {
        (log_const + (-(df + 1.0) / 2.0) * (1.0 + x * x / df).ln()).exp()
    })
}

/// Student's t cumulative distribution function.
///
/// Uses the regularized incomplete beta function:
/// pt(x, df) = 1 - 0.5 * I_{df/(df+x^2)}(df/2, 1/2) for x >= 0
///
/// @param q quantile vector
/// @param df degrees of freedom
/// @return numeric vector of probabilities
#[builtin(min_args = 2, namespace = "stats")]
fn builtin_pt(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let df = extract_param(args, named, "df", 1, f64::NAN);
    let lower_tail = extract_lower_tail(named);
    let log_p = extract_log_p(named);
    if df <= 0.0 || df.is_nan() {
        return Err(RError::new(
            RErrorKind::Argument,
            "pt(): 'df' must be positive".to_string(),
        ));
    }
    dist_vectorize_p(args, "pt", lower_tail, log_p, |q| {
        let x2 = q * q;
        let t = df / (df + x2);
        let half_ib = 0.5 * regularized_beta(t, df / 2.0, 0.5);
        if q >= 0.0 {
            1.0 - half_ib
        } else {
            half_ib
        }
    })
}

/// Student's t quantile function (inverse CDF).
///
/// Uses bisection on the CDF.
///
/// @param p probability vector
/// @param df degrees of freedom
/// @return numeric vector of quantiles
#[builtin(min_args = 2, namespace = "stats")]
fn builtin_qt(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let df = extract_param(args, named, "df", 1, f64::NAN);
    let lower_tail = extract_lower_tail(named);
    let log_p = extract_log_p(named);
    if df <= 0.0 || df.is_nan() {
        return Err(RError::new(
            RErrorKind::Argument,
            "qt(): 'df' must be positive".to_string(),
        ));
    }
    dist_vectorize_q(args, "qt", lower_tail, log_p, |p| {
        if !(0.0..=1.0).contains(&p) {
            f64::NAN
        } else if p == 0.0 {
            f64::NEG_INFINITY
        } else if p == 1.0 {
            f64::INFINITY
        } else {
            let pt_cdf = |q: f64| -> f64 {
                let x2 = q * q;
                let t = df / (df + x2);
                let half_ib = 0.5 * regularized_beta(t, df / 2.0, 0.5);
                if q >= 0.0 {
                    1.0 - half_ib
                } else {
                    half_ib
                }
            };
            let z = std_normal_quantile(p);
            let lo = z * 5.0 - 10.0;
            let hi = z * 5.0 + 10.0;
            quantile_bisect(p, lo, hi, pt_cdf)
        }
    })
}

// endregion

// region: F distribution (df, pf, qf)

/// F density function.
///
/// @param x quantile vector
/// @param df1 numerator degrees of freedom
/// @param df2 denominator degrees of freedom
/// @return numeric vector of densities
#[builtin(name = "df", min_args = 3, namespace = "stats")]
fn builtin_df_dist(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let df1 = extract_param(args, named, "df1", 1, f64::NAN);
    let df2 = extract_param(args, named, "df2", 2, f64::NAN);
    let log_flag = extract_log_flag(named);
    if df1 <= 0.0 || df2 <= 0.0 || df1.is_nan() || df2.is_nan() {
        return Err(RError::new(
            RErrorKind::Argument,
            "df(): 'df1' and 'df2' must be positive".to_string(),
        ));
    }
    let log_const = 0.5 * (df1 * df1.ln() + df2 * df2.ln()) + ln_gamma((df1 + df2) / 2.0)
        - ln_gamma(df1 / 2.0)
        - ln_gamma(df2 / 2.0);
    dist_vectorize_d(args, "df", log_flag, |x| {
        if x < 0.0 {
            0.0
        } else if x == 0.0 {
            if df1 < 2.0 {
                f64::INFINITY
            } else if df1 == 2.0 {
                1.0
            } else {
                0.0
            }
        } else {
            (log_const + (df1 / 2.0 - 1.0) * x.ln() - ((df1 + df2) / 2.0) * (df1 * x + df2).ln())
                .exp()
        }
    })
}

/// F cumulative distribution function.
///
/// Uses the regularized incomplete beta function:
/// pf(x, d1, d2) = I_{d1*x/(d1*x+d2)}(d1/2, d2/2)
///
/// @param q quantile vector
/// @param df1 numerator degrees of freedom
/// @param df2 denominator degrees of freedom
/// @return numeric vector of probabilities
#[builtin(min_args = 3, namespace = "stats")]
fn builtin_pf(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let df1 = extract_param(args, named, "df1", 1, f64::NAN);
    let df2 = extract_param(args, named, "df2", 2, f64::NAN);
    let lower_tail = extract_lower_tail(named);
    let log_p = extract_log_p(named);
    if df1 <= 0.0 || df2 <= 0.0 || df1.is_nan() || df2.is_nan() {
        return Err(RError::new(
            RErrorKind::Argument,
            "pf(): 'df1' and 'df2' must be positive".to_string(),
        ));
    }
    dist_vectorize_p(args, "pf", lower_tail, log_p, |q| {
        if q <= 0.0 {
            0.0
        } else {
            let t = df1 * q / (df1 * q + df2);
            regularized_beta(t, df1 / 2.0, df2 / 2.0)
        }
    })
}

/// F quantile function (inverse CDF).
///
/// Uses bisection on the CDF.
///
/// @param p probability vector
/// @param df1 numerator degrees of freedom
/// @param df2 denominator degrees of freedom
/// @return numeric vector of quantiles
#[builtin(min_args = 3, namespace = "stats")]
fn builtin_qf(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let df1 = extract_param(args, named, "df1", 1, f64::NAN);
    let df2 = extract_param(args, named, "df2", 2, f64::NAN);
    let lower_tail = extract_lower_tail(named);
    let log_p = extract_log_p(named);
    if df1 <= 0.0 || df2 <= 0.0 || df1.is_nan() || df2.is_nan() {
        return Err(RError::new(
            RErrorKind::Argument,
            "qf(): 'df1' and 'df2' must be positive".to_string(),
        ));
    }
    dist_vectorize_q(args, "qf", lower_tail, log_p, |p| {
        if !(0.0..=1.0).contains(&p) {
            f64::NAN
        } else if p == 0.0 {
            0.0
        } else if p == 1.0 {
            f64::INFINITY
        } else {
            let pf_cdf = |q: f64| -> f64 {
                let t = df1 * q / (df1 * q + df2);
                regularized_beta(t, df1 / 2.0, df2 / 2.0)
            };
            let mean_est = if df2 > 2.0 { df2 / (df2 - 2.0) } else { 1.0 };
            quantile_bisect(p, 0.0, mean_est * 10.0 + 10.0, pf_cdf)
        }
    })
}

// endregion

// region: Binomial distribution (dbinom, pbinom, qbinom)

/// Binomial density (probability mass) function.
///
/// dbinom(x, size, prob) = choose(size, x) * prob^x * (1-prob)^(size-x)
///
/// @param x quantile vector (integer values)
/// @param size number of trials
/// @param prob probability of success on each trial
/// @return numeric vector of probabilities
#[builtin(min_args = 3, namespace = "stats")]
fn builtin_dbinom(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let size = extract_param(args, named, "size", 1, f64::NAN);
    let prob = extract_param(args, named, "prob", 2, f64::NAN);
    let log_flag = extract_log_flag(named);
    if size < 0.0 || size.is_nan() || size != size.floor() {
        return Err(RError::new(
            RErrorKind::Argument,
            "dbinom(): 'size' must be a non-negative integer".to_string(),
        ));
    }
    if !(0.0..=1.0).contains(&prob) || prob.is_nan() {
        return Err(RError::new(
            RErrorKind::Argument,
            "dbinom(): 'prob' must be in [0, 1]".to_string(),
        ));
    }
    let n = size;
    dist_vectorize_d(args, "dbinom", log_flag, |x| {
        let k = x.round();
        if (x - k).abs() > 1e-7 || k < 0.0 || k > n {
            0.0
        } else if prob == 0.0 {
            if k == 0.0 {
                1.0
            } else {
                0.0
            }
        } else if prob == 1.0 {
            if k == n {
                1.0
            } else {
                0.0
            }
        } else {
            (lchoose(n, k) + k * prob.ln() + (n - k) * (1.0 - prob).ln()).exp()
        }
    })
}

/// Binomial cumulative distribution function.
///
/// pbinom(q, size, prob) = sum_{k=0}^{floor(q)} dbinom(k, size, prob)
/// Uses regularized beta: pbinom(k, n, p) = 1 - I_p(k+1, n-k)
///
/// @param q quantile vector
/// @param size number of trials
/// @param prob probability of success on each trial
/// @return numeric vector of probabilities
#[builtin(min_args = 3, namespace = "stats")]
fn builtin_pbinom(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let size = extract_param(args, named, "size", 1, f64::NAN);
    let prob = extract_param(args, named, "prob", 2, f64::NAN);
    let lower_tail = extract_lower_tail(named);
    let log_p = extract_log_p(named);
    if size < 0.0 || size.is_nan() || size != size.floor() {
        return Err(RError::new(
            RErrorKind::Argument,
            "pbinom(): 'size' must be a non-negative integer".to_string(),
        ));
    }
    if !(0.0..=1.0).contains(&prob) || prob.is_nan() {
        return Err(RError::new(
            RErrorKind::Argument,
            "pbinom(): 'prob' must be in [0, 1]".to_string(),
        ));
    }
    let n = size as i64;
    dist_vectorize_p(args, "pbinom", lower_tail, log_p, |q| {
        if q < 0.0 {
            0.0
        } else if q >= size {
            1.0
        } else {
            let k = q.floor() as i64;
            let kf = k as f64;
            1.0 - regularized_beta(prob, kf + 1.0, (n - k) as f64)
        }
    })
}

/// Binomial quantile function (inverse CDF).
///
/// Uses bisection on the discrete CDF.
///
/// @param p probability vector
/// @param size number of trials
/// @param prob probability of success on each trial
/// @return numeric vector of quantiles
#[builtin(min_args = 3, namespace = "stats")]
fn builtin_qbinom(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let size = extract_param(args, named, "size", 1, f64::NAN);
    let prob = extract_param(args, named, "prob", 2, f64::NAN);
    let lower_tail = extract_lower_tail(named);
    let log_p = extract_log_p(named);
    if size < 0.0 || size.is_nan() || size != size.floor() {
        return Err(RError::new(
            RErrorKind::Argument,
            "qbinom(): 'size' must be a non-negative integer".to_string(),
        ));
    }
    if !(0.0..=1.0).contains(&prob) || prob.is_nan() {
        return Err(RError::new(
            RErrorKind::Argument,
            "qbinom(): 'prob' must be in [0, 1]".to_string(),
        ));
    }
    let n = size as i64;
    dist_vectorize_q(args, "qbinom", lower_tail, log_p, |p| {
        if !(0.0..=1.0).contains(&p) {
            f64::NAN
        } else if p == 0.0 {
            0.0
        } else if p == 1.0 {
            size
        } else {
            quantile_bisect_discrete(p, 0, n, |k| {
                let kf = k as f64;
                1.0 - regularized_beta(prob, kf + 1.0, (n - k) as f64)
            })
        }
    })
}

// endregion

// region: Poisson distribution (dpois, ppois, qpois)

/// Poisson density (probability mass) function.
///
/// dpois(x, lambda) = lambda^x * exp(-lambda) / x!
///
/// @param x quantile vector (non-negative integers)
/// @param lambda mean rate parameter
/// @return numeric vector of probabilities
#[builtin(min_args = 2, namespace = "stats")]
fn builtin_dpois(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let lambda = extract_param(args, named, "lambda", 1, f64::NAN);
    let log_flag = extract_log_flag(named);
    if lambda < 0.0 || lambda.is_nan() {
        return Err(RError::new(
            RErrorKind::Argument,
            "dpois(): 'lambda' must be non-negative".to_string(),
        ));
    }
    dist_vectorize_d(args, "dpois", log_flag, |x| {
        let k = x.round();
        if (x - k).abs() > 1e-7 || k < 0.0 {
            0.0
        } else if lambda == 0.0 {
            if k == 0.0 {
                1.0
            } else {
                0.0
            }
        } else {
            (k * lambda.ln() - lambda - ln_gamma(k + 1.0)).exp()
        }
    })
}

/// Poisson cumulative distribution function.
///
/// ppois(q, lambda) = sum_{k=0}^{floor(q)} dpois(k, lambda)
/// Uses the regularized gamma: ppois(k, lambda) = 1 - P(k+1, lambda)
///
/// @param q quantile vector
/// @param lambda mean rate parameter
/// @return numeric vector of probabilities
#[builtin(min_args = 2, namespace = "stats")]
fn builtin_ppois(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let lambda = extract_param(args, named, "lambda", 1, f64::NAN);
    let lower_tail = extract_lower_tail(named);
    let log_p = extract_log_p(named);
    if lambda < 0.0 || lambda.is_nan() {
        return Err(RError::new(
            RErrorKind::Argument,
            "ppois(): 'lambda' must be non-negative".to_string(),
        ));
    }
    dist_vectorize_p(args, "ppois", lower_tail, log_p, |q| {
        if q < 0.0 {
            0.0
        } else if lambda == 0.0 {
            1.0
        } else {
            let k = q.floor();
            1.0 - regularized_gamma_p(k + 1.0, lambda)
        }
    })
}

/// Poisson quantile function (inverse CDF).
///
/// Uses bisection on the discrete CDF.
///
/// @param p probability vector
/// @param lambda mean rate parameter
/// @return numeric vector of quantiles
#[builtin(min_args = 2, namespace = "stats")]
fn builtin_qpois(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let lambda = extract_param(args, named, "lambda", 1, f64::NAN);
    let lower_tail = extract_lower_tail(named);
    let log_p = extract_log_p(named);
    if lambda < 0.0 || lambda.is_nan() {
        return Err(RError::new(
            RErrorKind::Argument,
            "qpois(): 'lambda' must be non-negative".to_string(),
        ));
    }
    dist_vectorize_q(args, "qpois", lower_tail, log_p, |p| {
        if !(0.0..=1.0).contains(&p) {
            f64::NAN
        } else if p == 0.0 {
            0.0
        } else if p == 1.0 {
            f64::INFINITY
        } else if lambda == 0.0 {
            0.0
        } else {
            let hi = (lambda + 6.0 * lambda.sqrt() + 10.0).ceil() as i64;
            quantile_bisect_discrete(p, 0, hi, |k| {
                let kf = k as f64;
                1.0 - regularized_gamma_p(kf + 1.0, lambda)
            })
        }
    })
}

// endregion

// region: Geometric distribution (dgeom, pgeom, qgeom)

/// Geometric density (probability mass) function.
///
/// dgeom(x, prob) = prob * (1-prob)^x (number of failures before first success)
///
/// @param x quantile vector (non-negative integers)
/// @param prob probability of success
/// @return numeric vector of probabilities
#[builtin(min_args = 2, namespace = "stats")]
fn builtin_dgeom(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let prob = extract_param(args, named, "prob", 1, f64::NAN);
    let log_flag = extract_log_flag(named);
    if !(0.0..=1.0).contains(&prob) || prob.is_nan() {
        return Err(RError::new(
            RErrorKind::Argument,
            "dgeom(): 'prob' must be in (0, 1]".to_string(),
        ));
    }
    dist_vectorize_d(args, "dgeom", log_flag, |x| {
        let k = x.round();
        if (x - k).abs() > 1e-7 || k < 0.0 {
            0.0
        } else {
            prob * (1.0 - prob).powf(k)
        }
    })
}

/// Geometric cumulative distribution function.
///
/// pgeom(q, prob) = 1 - (1-prob)^(floor(q)+1)
///
/// @param q quantile vector
/// @param prob probability of success
/// @return numeric vector of probabilities
#[builtin(min_args = 2, namespace = "stats")]
fn builtin_pgeom(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let prob = extract_param(args, named, "prob", 1, f64::NAN);
    let lower_tail = extract_lower_tail(named);
    let log_p = extract_log_p(named);
    if !(0.0..=1.0).contains(&prob) || prob.is_nan() {
        return Err(RError::new(
            RErrorKind::Argument,
            "pgeom(): 'prob' must be in (0, 1]".to_string(),
        ));
    }
    dist_vectorize_p(args, "pgeom", lower_tail, log_p, |q| {
        if q < 0.0 {
            0.0
        } else {
            1.0 - (1.0 - prob).powf(q.floor() + 1.0)
        }
    })
}

/// Geometric quantile function (inverse CDF).
///
/// qgeom(p, prob) = ceil(log(1-p) / log(1-prob)) - 1
///
/// @param p probability vector
/// @param prob probability of success
/// @return numeric vector of quantiles
#[builtin(min_args = 2, namespace = "stats")]
fn builtin_qgeom(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let prob = extract_param(args, named, "prob", 1, f64::NAN);
    let lower_tail = extract_lower_tail(named);
    let log_p = extract_log_p(named);
    if !(0.0..=1.0).contains(&prob) || prob.is_nan() {
        return Err(RError::new(
            RErrorKind::Argument,
            "qgeom(): 'prob' must be in (0, 1]".to_string(),
        ));
    }
    dist_vectorize_q(args, "qgeom", lower_tail, log_p, |p| {
        if !(0.0..=1.0).contains(&p) {
            f64::NAN
        } else if p == 0.0 {
            0.0
        } else if p == 1.0 {
            f64::INFINITY
        } else if prob == 1.0 {
            0.0
        } else {
            ((1.0 - p).ln() / (1.0 - prob).ln() - 1.0).ceil().max(0.0)
        }
    })
}

// endregion

// region: Hypergeometric distribution (dhyper, phyper, qhyper)

/// Hypergeometric density (probability mass) function.
///
/// dhyper(x, m, n, k) = choose(m,x) * choose(n,k-x) / choose(m+n,k)
///
/// @param x quantile vector (integer values)
/// @param m number of white balls in the urn
/// @param n number of black balls in the urn
/// @param k number of balls drawn from the urn
/// @return numeric vector of probabilities
#[builtin(min_args = 4, namespace = "stats")]
fn builtin_dhyper(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let m = extract_param(args, named, "m", 1, f64::NAN);
    let n = extract_param(args, named, "n", 2, f64::NAN);
    let k = extract_param(args, named, "k", 3, f64::NAN);
    let log_flag = extract_log_flag(named);
    if m < 0.0 || n < 0.0 || k < 0.0 || m.is_nan() || n.is_nan() || k.is_nan() {
        return Err(RError::new(
            RErrorKind::Argument,
            "dhyper(): parameters must be non-negative".to_string(),
        ));
    }
    if k > m + n {
        return Err(RError::new(
            RErrorKind::Argument,
            "dhyper(): 'k' must not exceed 'm + n'".to_string(),
        ));
    }
    dist_vectorize_d(args, "dhyper", log_flag, |x| {
        let xi = x.round();
        if (x - xi).abs() > 1e-7 || xi < 0.0 || xi > m || xi > k || (k - xi) > n {
            0.0
        } else {
            (lchoose(m, xi) + lchoose(n, k - xi) - lchoose(m + n, k)).exp()
        }
    })
}

/// Hypergeometric cumulative distribution function.
///
/// @param q quantile vector
/// @param m number of white balls in the urn
/// @param n number of black balls in the urn
/// @param k number of balls drawn from the urn
/// @return numeric vector of probabilities
#[builtin(min_args = 4, namespace = "stats")]
fn builtin_phyper(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let m = extract_param(args, named, "m", 1, f64::NAN);
    let n = extract_param(args, named, "n", 2, f64::NAN);
    let k = extract_param(args, named, "k", 3, f64::NAN);
    let lower_tail = extract_lower_tail(named);
    let log_p = extract_log_p(named);
    if m < 0.0 || n < 0.0 || k < 0.0 || m.is_nan() || n.is_nan() || k.is_nan() {
        return Err(RError::new(
            RErrorKind::Argument,
            "phyper(): parameters must be non-negative".to_string(),
        ));
    }
    if k > m + n {
        return Err(RError::new(
            RErrorKind::Argument,
            "phyper(): 'k' must not exceed 'm + n'".to_string(),
        ));
    }
    let lo = (k - n).max(0.0) as i64;
    dist_vectorize_p(args, "phyper", lower_tail, log_p, |q| {
        if q < lo as f64 {
            0.0
        } else if q >= m.min(k) {
            1.0
        } else {
            let qi = q.floor() as i64;
            let mut sum = 0.0;
            for j in lo..=qi {
                let jf = j as f64;
                sum += (lchoose(m, jf) + lchoose(n, k - jf) - lchoose(m + n, k)).exp();
            }
            sum.min(1.0)
        }
    })
}

/// Hypergeometric quantile function (inverse CDF).
///
/// @param p probability vector
/// @param m number of white balls in the urn
/// @param n number of black balls in the urn
/// @param k number of balls drawn from the urn
/// @return numeric vector of quantiles
#[builtin(min_args = 4, namespace = "stats")]
fn builtin_qhyper(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let m = extract_param(args, named, "m", 1, f64::NAN);
    let n = extract_param(args, named, "n", 2, f64::NAN);
    let k = extract_param(args, named, "k", 3, f64::NAN);
    let lower_tail = extract_lower_tail(named);
    let log_p = extract_log_p(named);
    if m < 0.0 || n < 0.0 || k < 0.0 || m.is_nan() || n.is_nan() || k.is_nan() {
        return Err(RError::new(
            RErrorKind::Argument,
            "qhyper(): parameters must be non-negative".to_string(),
        ));
    }
    if k > m + n {
        return Err(RError::new(
            RErrorKind::Argument,
            "qhyper(): 'k' must not exceed 'm + n'".to_string(),
        ));
    }
    let lo = (k - n).max(0.0) as i64;
    let hi = m.min(k) as i64;
    dist_vectorize_q(args, "qhyper", lower_tail, log_p, |p| {
        if !(0.0..=1.0).contains(&p) {
            f64::NAN
        } else if p == 0.0 {
            lo as f64
        } else if p == 1.0 {
            hi as f64
        } else {
            let mut cum = 0.0;
            for j in lo..=hi {
                let jf = j as f64;
                cum += (lchoose(m, jf) + lchoose(n, k - jf) - lchoose(m + n, k)).exp();
                if cum >= p {
                    return jf;
                }
            }
            hi as f64
        }
    })
}

// endregion
