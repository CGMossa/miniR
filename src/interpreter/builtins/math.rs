use std::collections::BTreeSet;

#[cfg(feature = "linalg")]
use derive_more::{Display, Error};
use itertools::Itertools;
#[cfg(feature = "linalg")]
use nalgebra::DMatrix;
#[cfg(feature = "linalg")]
use ndarray::{Array1, Array2, ShapeBuilder};

use crate::interpreter::coerce::{f64_to_i32, usize_to_f64};
use crate::interpreter::value::*;
#[cfg(feature = "linalg")]
use crate::interpreter::BuiltinContext;
#[cfg(feature = "linalg")]
use crate::parser::ast::Expr;
use minir_macros::builtin;
#[cfg(feature = "linalg")]
use minir_macros::interpreter_builtin;

type DimNameVec = Vec<Option<String>>;
type MatrixDimNames = (Option<DimNameVec>, Option<DimNameVec>);

// region: MathError

/// Structured error type for math/linear algebra operations.
#[cfg(feature = "linalg")]
#[derive(Debug, Display, Error)]
pub enum MathError {
    #[display("matrix shape error: {}", source)]
    Shape {
        #[error(source)]
        source: ndarray::ShapeError,
    },
}

#[cfg(feature = "linalg")]
impl From<MathError> for RError {
    fn from(e: MathError) -> Self {
        RError::from_source(RErrorKind::Other, e)
    }
}

// endregion

// === Math functions ===

/// Absolute value.
///
/// @param x numeric vector
/// @return numeric vector of absolute values
#[builtin(min_args = 1)]
fn builtin_abs(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    math_unary(args, f64::abs)
}

/// Square root.
///
/// @param x numeric vector
/// @return numeric vector of square roots
#[builtin(min_args = 1)]
fn builtin_sqrt(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    math_unary(args, f64::sqrt)
}

/// Exponential function (e^x).
///
/// @param x numeric vector
/// @return numeric vector of e raised to each element
#[builtin(min_args = 1)]
fn builtin_exp(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    math_unary(args, f64::exp)
}

/// Logarithm. With one argument, computes the natural log. With a second
/// positional argument or named `base` argument, computes log in that base
/// via the change-of-base formula: log(x, base) = ln(x) / ln(base).
///
/// @param x numeric vector
/// @param base the base of the logarithm (default: e, i.e. natural log)
/// @return numeric vector of logarithms
#[builtin(min_args = 1)]
fn builtin_log(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let base: Option<f64> = named
        .iter()
        .find(|(n, _)| n == "base")
        .and_then(|(_, v)| v.as_vector()?.as_double_scalar())
        .or_else(|| args.get(1)?.as_vector()?.as_double_scalar());

    match base {
        Some(b) => {
            let ln_base = b.ln();
            match args.first() {
                Some(RValue::Vector(v)) => {
                    let result: Vec<Option<f64>> = v
                        .to_doubles()
                        .iter()
                        .map(|x| x.map(|f| f.ln() / ln_base))
                        .collect();
                    Ok(RValue::vec(Vector::Double(result.into())))
                }
                _ => Err(RError::new(
                    RErrorKind::Argument,
                    "non-numeric argument to mathematical function".to_string(),
                )),
            }
        }
        None => math_unary(args, f64::ln),
    }
}

/// Base-2 logarithm.
///
/// @param x numeric vector
/// @return numeric vector of base-2 logarithms
#[builtin(min_args = 1)]
fn builtin_log2(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    math_unary(args, f64::log2)
}

/// Base-10 logarithm.
///
/// @param x numeric vector
/// @return numeric vector of base-10 logarithms
#[builtin(min_args = 1)]
fn builtin_log10(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    math_unary(args, f64::log10)
}

/// Ceiling (smallest integer not less than x).
///
/// @param x numeric vector
/// @return numeric vector rounded up to integers
#[builtin(min_args = 1)]
fn builtin_ceiling(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    math_unary(args, f64::ceil)
}

/// Floor (largest integer not greater than x).
///
/// @param x numeric vector
/// @return numeric vector rounded down to integers
#[builtin(min_args = 1)]
fn builtin_floor(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    math_unary(args, f64::floor)
}

/// Truncation (round toward zero).
///
/// @param x numeric vector
/// @return numeric vector truncated toward zero
#[builtin(min_args = 1)]
fn builtin_trunc(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    math_unary(args, f64::trunc)
}

/// Sine (in radians).
///
/// @param x numeric vector
/// @return numeric vector of sines
#[builtin(min_args = 1)]
fn builtin_sin(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    math_unary(args, f64::sin)
}

/// Cosine (in radians).
///
/// @param x numeric vector
/// @return numeric vector of cosines
#[builtin(min_args = 1)]
fn builtin_cos(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    math_unary(args, f64::cos)
}

/// Tangent (in radians).
///
/// @param x numeric vector
/// @return numeric vector of tangents
#[builtin(min_args = 1)]
fn builtin_tan(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    math_unary(args, f64::tan)
}

/// Sign of each element (-1, 0, or 1).
///
/// R's sign() returns 0 for zero inputs (unlike Rust's f64::signum which returns 1.0).
/// For integer inputs, returns an integer vector; for double inputs, returns a double vector.
///
/// @param x numeric vector
/// @return numeric vector of signs
#[builtin(min_args = 1)]
fn builtin_sign(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(v)) => match &v.inner {
            Vector::Integer(vals) => {
                let result: Vec<Option<i64>> = vals
                    .iter()
                    .map(|x| {
                        x.map(|i| match i.cmp(&0) {
                            std::cmp::Ordering::Greater => 1i64,
                            std::cmp::Ordering::Equal => 0i64,
                            std::cmp::Ordering::Less => -1i64,
                        })
                    })
                    .collect();
                Ok(RValue::vec(Vector::Integer(result.into())))
            }
            _ => {
                let result: Vec<Option<f64>> = v
                    .to_doubles()
                    .iter()
                    .map(|x| {
                        x.map(|f| {
                            if f.is_nan() {
                                f64::NAN
                            } else if f > 0.0 {
                                1.0
                            } else if f < 0.0 {
                                -1.0
                            } else {
                                0.0
                            }
                        })
                    })
                    .collect();
                Ok(RValue::vec(Vector::Double(result.into())))
            }
        },
        _ => Err(RError::new(
            RErrorKind::Argument,
            "non-numeric argument to mathematical function".to_string(),
        )),
    }
}

use super::math_unary_op as math_unary;

// region: Inverse trigonometric

/// Inverse sine (arc sine).
///
/// @param x numeric vector with values in [-1, 1]
/// @return numeric vector of angles in radians
#[builtin(min_args = 1)]
fn builtin_asin(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    math_unary(args, f64::asin)
}

/// Inverse cosine (arc cosine).
///
/// @param x numeric vector with values in [-1, 1]
/// @return numeric vector of angles in radians
#[builtin(min_args = 1)]
fn builtin_acos(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    math_unary(args, f64::acos)
}

/// Inverse tangent (arc tangent).
///
/// @param x numeric vector
/// @return numeric vector of angles in radians
#[builtin(min_args = 1)]
fn builtin_atan(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    math_unary(args, f64::atan)
}

/// Two-argument inverse tangent.
///
/// Computes atan(y/x) but uses the signs of both arguments to determine
/// the correct quadrant. Returns angles in [-pi, pi].
///
/// @param y numeric vector (y-coordinates)
/// @param x numeric vector (x-coordinates)
/// @return numeric vector of angles in radians
#[builtin(min_args = 2)]
fn builtin_atan2(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    math_binary(args, f64::atan2)
}

// endregion

// region: Hyperbolic

/// Hyperbolic sine.
///
/// @param x numeric vector
/// @return numeric vector of hyperbolic sines
#[builtin(min_args = 1)]
fn builtin_sinh(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    math_unary(args, f64::sinh)
}

/// Hyperbolic cosine.
///
/// @param x numeric vector
/// @return numeric vector of hyperbolic cosines
#[builtin(min_args = 1)]
fn builtin_cosh(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    math_unary(args, f64::cosh)
}

/// Hyperbolic tangent.
///
/// @param x numeric vector
/// @return numeric vector of hyperbolic tangents
#[builtin(min_args = 1)]
fn builtin_tanh(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    math_unary(args, f64::tanh)
}

// endregion

// region: Inverse hyperbolic

/// Inverse hyperbolic sine.
///
/// @param x numeric vector
/// @return numeric vector of inverse hyperbolic sines
#[builtin(min_args = 1)]
fn builtin_asinh(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    math_unary(args, f64::asinh)
}

/// Inverse hyperbolic cosine.
///
/// @param x numeric vector with values >= 1
/// @return numeric vector of inverse hyperbolic cosines
#[builtin(min_args = 1)]
fn builtin_acosh(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    math_unary(args, f64::acosh)
}

/// Inverse hyperbolic tangent.
///
/// @param x numeric vector with values in (-1, 1)
/// @return numeric vector of inverse hyperbolic tangents
#[builtin(min_args = 1)]
fn builtin_atanh(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    math_unary(args, f64::atanh)
}

// endregion

// region: Numerically stable exp/log variants

/// Numerically stable exp(x) - 1.
///
/// More accurate than `exp(x) - 1` for x near zero, where catastrophic
/// cancellation would otherwise lose precision.
///
/// @param x numeric vector
/// @return numeric vector of exp(x) - 1
#[builtin(min_args = 1)]
fn builtin_expm1(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    math_unary(args, f64::exp_m1)
}

/// Numerically stable log(1 + x).
///
/// More accurate than `log(1 + x)` for x near zero, where catastrophic
/// cancellation would otherwise lose precision.
///
/// @param x numeric vector with values > -1
/// @return numeric vector of log(1 + x)
#[builtin(min_args = 1)]
fn builtin_log1p(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    math_unary(args, f64::ln_1p)
}

// endregion

// region: Gamma-related and combinatorial

/// Gamma function.
///
/// Computes the gamma function, which extends factorial to real numbers:
/// gamma(n) = (n-1)! for positive integers.
///
/// @param x numeric vector
/// @return numeric vector of gamma values
#[builtin(min_args = 1)]
fn builtin_gamma(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    math_unary(args, libm::tgamma)
}

/// Natural logarithm of the absolute value of the gamma function.
///
/// More numerically stable than log(abs(gamma(x))) for large x.
///
/// @param x numeric vector
/// @return numeric vector of log-gamma values
#[builtin(min_args = 1)]
fn builtin_lgamma(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    math_unary(args, libm::lgamma)
}

/// Beta function: gamma(a) * gamma(b) / gamma(a + b).
///
/// @param a numeric vector
/// @param b numeric vector
/// @return numeric vector of beta function values
#[builtin(min_args = 2)]
fn builtin_beta(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    math_binary(args, |a, b| {
        (libm::lgamma(a) + libm::lgamma(b) - libm::lgamma(a + b)).exp()
    })
}

/// Natural logarithm of the beta function.
///
/// More numerically stable than log(beta(a, b)) for large a or b.
///
/// @param a numeric vector
/// @param b numeric vector
/// @return numeric vector of log-beta values
#[builtin(min_args = 2)]
fn builtin_lbeta(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    math_binary(args, |a, b| {
        libm::lgamma(a) + libm::lgamma(b) - libm::lgamma(a + b)
    })
}

/// Factorial: n! = 1 * 2 * ... * n.
///
/// Uses gamma(n + 1) for non-integer and large values.
///
/// @param x numeric vector (non-negative)
/// @return numeric vector of factorials
#[builtin(min_args = 1)]
fn builtin_factorial(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    math_unary(args, |x| libm::tgamma(x + 1.0))
}

/// Binomial coefficient: choose(n, k) = n! / (k! * (n - k)!).
///
/// Uses the log-gamma formulation for numerical stability.
///
/// @param n numeric vector
/// @param k numeric vector
/// @return numeric vector of binomial coefficients
#[builtin(min_args = 2)]
fn builtin_choose(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    math_binary(args, |n, k| {
        if k < 0.0 || k > n {
            return 0.0;
        }
        // For integer-valued inputs, use the log-gamma identity:
        // choose(n, k) = exp(lgamma(n+1) - lgamma(k+1) - lgamma(n-k+1))
        (libm::lgamma(n + 1.0) - libm::lgamma(k + 1.0) - libm::lgamma(n - k + 1.0))
            .exp()
            .round()
    })
}

/// All combinations of n elements taken k at a time.
///
/// Returns a matrix with k rows and choose(n, k) columns, where each
/// column is one combination.
///
/// @param x if numeric scalar, treated as 1:x; if vector, elements to combine
/// @param m number of elements to choose
/// @return matrix of combinations (k rows, choose(n,k) columns)
#[builtin(min_args = 2)]
fn builtin_combn(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    // Resolve the pool of elements
    let pool: Vec<f64> = match args.first() {
        Some(RValue::Vector(v)) => {
            let doubles = v.to_doubles();
            if doubles.len() == 1 {
                // Scalar n => pool is 1:n
                let n = doubles[0].ok_or_else(|| {
                    RError::new(
                        RErrorKind::Argument,
                        "NA in combn first argument".to_string(),
                    )
                })?;
                let n_int = n as i64;
                (1..=n_int).map(|i| i as f64).collect()
            } else {
                doubles
                    .into_iter()
                    .map(|x| {
                        x.ok_or_else(|| {
                            RError::new(RErrorKind::Argument, "NA in combn input".to_string())
                        })
                    })
                    .collect::<Result<Vec<f64>, RError>>()?
            }
        }
        _ => {
            return Err(RError::new(
                RErrorKind::Argument,
                "first argument must be numeric".to_string(),
            ))
        }
    };

    let m = match args.get(1) {
        Some(RValue::Vector(v)) => v.as_integer_scalar().ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "second argument (m) must be a single integer".to_string(),
            )
        })?,
        _ => {
            return Err(RError::new(
                RErrorKind::Argument,
                "second argument (m) must be a single integer".to_string(),
            ))
        }
    };

    let n = pool.len();
    let m_usize = usize::try_from(m).map_err(|_| {
        RError::new(
            RErrorKind::Argument,
            format!("m must be non-negative, got {m}"),
        )
    })?;

    if m_usize > n {
        return Err(RError::new(
            RErrorKind::Argument,
            format!("m ({m_usize}) must be <= n ({n}) in combn"),
        ));
    }

    // Generate all combinations using itertools
    let combos: Vec<Vec<f64>> = (0..n)
        .combinations(m_usize)
        .map(|indices| indices.iter().map(|&i| pool[i]).collect())
        .collect();

    let ncol = combos.len();
    let nrow = m_usize;

    // Fill column-major (R convention): column by column
    let mut data: Vec<Option<f64>> = Vec::with_capacity(nrow * ncol);
    for combo in &combos {
        for &val in combo {
            data.push(Some(val));
        }
    }

    let mut rv = RVector::from(Vector::Double(data.into()));
    set_matrix_attrs(&mut rv, nrow, ncol, None, None)?;
    Ok(RValue::Vector(rv))
}

// endregion

// region: Digamma and trigamma

/// Digamma function (psi function): the logarithmic derivative of the gamma function.
///
/// Computes d/dx log(gamma(x)) = gamma'(x) / gamma(x).
/// Uses an asymptotic expansion for large x and the recurrence relation
/// psi(x) = psi(x+1) - 1/x for small x.
///
/// @param x numeric vector
/// @return numeric vector of digamma values
#[builtin(min_args = 1)]
fn builtin_digamma(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    math_unary(args, digamma_f64)
}

/// Trigamma function: the derivative of the digamma function.
///
/// Computes d^2/dx^2 log(gamma(x)).
/// Uses an asymptotic expansion for large x and the recurrence relation
/// trigamma(x) = trigamma(x+1) + 1/x^2 for small x.
///
/// @param x numeric vector
/// @return numeric vector of trigamma values
#[builtin(min_args = 1)]
fn builtin_trigamma(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    math_unary(args, trigamma_f64)
}

/// Compute digamma(x) using asymptotic expansion and recurrence.
///
/// For large x, use the asymptotic series:
///   psi(x) ~ ln(x) - 1/(2x) - B_2/(2*x^2) - B_4/(4*x^4) - ...
/// where B_{2k} are Bernoulli numbers.
/// For small x, use psi(x) = psi(x+1) - 1/x repeatedly.
fn digamma_f64(mut x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    if x == f64::NEG_INFINITY {
        return f64::NAN;
    }
    if x == f64::INFINITY {
        return f64::INFINITY;
    }

    // Reflection formula for x < 0: psi(x) = psi(1-x) - pi / tan(pi*x)
    let mut result = 0.0;
    if x < 0.0 {
        let pi_x = std::f64::consts::PI * x;
        let tan_val = pi_x.tan();
        if tan_val == 0.0 {
            return f64::NAN; // poles at non-positive integers
        }
        result -= std::f64::consts::PI / tan_val;
        x = 1.0 - x;
    }

    // Pole at x = 0
    if x == 0.0 {
        return f64::NAN;
    }

    // Recurrence: psi(x) = psi(x+1) - 1/x, until x is large enough
    while x < 12.0 {
        result -= 1.0 / x;
        x += 1.0;
    }

    // Asymptotic expansion for large x:
    // psi(x) ~ ln(x) - 1/(2x) - sum_{k=1..} B_{2k}/(2k * x^{2k})
    // Coefficients -B_{2k}/(2k):
    //   k=1: -1/12,  k=2: 1/120,  k=3: -1/252,
    //   k=4: 1/240,  k=5: -1/132, k=6: 691/32760
    let inv_x = 1.0 / x;
    let inv_x2 = inv_x * inv_x;

    // Horner evaluation: S = (1/x^2) * (c1 + (1/x^2) * (c2 + ...))
    // where c_k = -B_{2k}/(2k), starting from innermost (highest k)
    let mut series = 691.0 / 32760.0; // k=6
    series = series * inv_x2 + (-1.0 / 132.0); // k=5
    series = series * inv_x2 + (1.0 / 240.0); // k=4
    series = series * inv_x2 + (-1.0 / 252.0); // k=3
    series = series * inv_x2 + (1.0 / 120.0); // k=2
    series = series * inv_x2 + (-1.0 / 12.0); // k=1
    series *= inv_x2;

    result += x.ln() - 0.5 * inv_x + series;

    result
}

/// Compute trigamma(x) using asymptotic expansion and recurrence.
///
/// For large x, use the asymptotic series:
///   trigamma(x) ~ 1/x + 1/(2x^2) + sum_{k=1..} B_{2k}/x^{2k+1}
/// where B_{2k} are Bernoulli numbers.
/// For small x, use trigamma(x) = trigamma(x+1) + 1/x^2 repeatedly.
fn trigamma_f64(mut x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    if x == f64::INFINITY {
        return 0.0;
    }
    if x == f64::NEG_INFINITY {
        return f64::NAN;
    }

    // Reflection formula: trigamma(1-x) + trigamma(x) = pi^2 / sin^2(pi*x)
    let mut result = 0.0;
    if x < 0.0 {
        let pi_x = std::f64::consts::PI * x;
        let sin_val = pi_x.sin();
        if sin_val == 0.0 {
            return f64::NAN; // poles at non-positive integers
        }
        let pi2 = std::f64::consts::PI * std::f64::consts::PI;
        result += pi2 / (sin_val * sin_val);
        x = 1.0 - x;
        return result - trigamma_f64(x);
    }

    // Pole at x = 0
    if x == 0.0 {
        return f64::NAN;
    }

    // Recurrence: trigamma(x) = trigamma(x+1) + 1/x^2
    while x < 12.0 {
        result += 1.0 / (x * x);
        x += 1.0;
    }

    // Asymptotic expansion for large x:
    // trigamma(x) ~ 1/x + 1/(2x^2) + B_2/x^3 + B_4/x^5 + B_6/x^7 + ...
    // B_2=1/6, B_4=-1/30, B_6=1/42, B_8=-1/30, B_10=5/66, B_12=-691/2730
    let inv_x = 1.0 / x;
    let inv_x2 = inv_x * inv_x;

    // Horner-like evaluation: multiply by inv_x2 each step, then by inv_x at end
    // Start from highest-order term and work backward
    let mut series = -691.0 / 2730.0;
    series = series * inv_x2 + 5.0 / 66.0;
    series = series * inv_x2 + -1.0 / 30.0;
    series = series * inv_x2 + 1.0 / 42.0;
    series = series * inv_x2 + -1.0 / 30.0;
    series = series * inv_x2 + 1.0 / 6.0;
    series *= inv_x2 * inv_x; // multiply by 1/x^3

    result += inv_x + 0.5 * inv_x2 + series;

    result
}

// endregion

// region: Bessel functions

/// Bessel function of the first kind, J_nu(x).
///
/// Computes J_nu(x) for integer order nu using libm's jn function.
/// R's besselJ also supports non-integer nu, but this implementation
/// currently handles integer orders only (non-integer orders return NaN
/// with a warning).
///
/// @param x numeric vector
/// @param nu numeric scalar (order, rounded to nearest integer)
/// @return numeric vector of Bessel J values
#[builtin(name = "besselJ", min_args = 2)]
fn builtin_bessel_j(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let nu = args
        .get(1)
        .and_then(|v| v.as_vector())
        .and_then(|v| v.as_double_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "besselJ requires a numeric 'nu' (order) argument".to_string(),
            )
        })?;
    let n = f64_to_i32(nu.round())?;
    match args.first() {
        Some(RValue::Vector(v)) => {
            let result: Vec<Option<f64>> = v
                .to_doubles()
                .iter()
                .map(|x| x.map(|x| libm::jn(n, x)))
                .collect();
            Ok(RValue::vec(Vector::Double(result.into())))
        }
        _ => Err(RError::new(
            RErrorKind::Argument,
            "non-numeric argument to mathematical function".to_string(),
        )),
    }
}

/// Bessel function of the second kind, Y_nu(x).
///
/// Computes Y_nu(x) for integer order nu using libm's yn function.
/// R's besselY also supports non-integer nu, but this implementation
/// currently handles integer orders only.
///
/// @param x numeric vector (must be positive for finite results)
/// @param nu numeric scalar (order, rounded to nearest integer)
/// @return numeric vector of Bessel Y values
#[builtin(name = "besselY", min_args = 2)]
fn builtin_bessel_y(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let nu = args
        .get(1)
        .and_then(|v| v.as_vector())
        .and_then(|v| v.as_double_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "besselY requires a numeric 'nu' (order) argument".to_string(),
            )
        })?;
    let n = f64_to_i32(nu.round())?;
    match args.first() {
        Some(RValue::Vector(v)) => {
            let result: Vec<Option<f64>> = v
                .to_doubles()
                .iter()
                .map(|x| x.map(|x| libm::yn(n, x)))
                .collect();
            Ok(RValue::vec(Vector::Double(result.into())))
        }
        _ => Err(RError::new(
            RErrorKind::Argument,
            "non-numeric argument to mathematical function".to_string(),
        )),
    }
}

// endregion

// region: libm special functions

/// Cube root.
///
/// Computes the real cube root of x. Unlike x^(1/3), this correctly
/// handles negative values: cbrt(-8) = -2.
///
/// @param x numeric vector
/// @return numeric vector of cube roots
#[builtin(min_args = 1)]
fn builtin_cbrt(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    math_unary(args, libm::cbrt)
}

/// Euclidean distance: sqrt(x^2 + y^2) without overflow.
///
/// Computes the hypotenuse length avoiding intermediate overflow
/// that would occur with naive sqrt(x*x + y*y).
///
/// @param x numeric vector
/// @param y numeric vector
/// @return numeric vector of hypotenuse lengths
#[builtin(min_args = 2)]
fn builtin_hypot(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    math_binary(args, libm::hypot)
}

// endregion

// region: Binary math helper

/// Helper for binary math builtins: applies `(f64, f64) -> f64` element-wise
/// with recycling.
fn math_binary(args: &[RValue], f: fn(f64, f64) -> f64) -> Result<RValue, RError> {
    let (a_vec, b_vec) = match (args.first(), args.get(1)) {
        (Some(RValue::Vector(a)), Some(RValue::Vector(b))) => (a.to_doubles(), b.to_doubles()),
        _ => {
            return Err(RError::new(
                RErrorKind::Argument,
                "non-numeric argument to mathematical function".to_string(),
            ))
        }
    };
    let len = a_vec.len().max(b_vec.len());
    let result: Vec<Option<f64>> = (0..len)
        .map(|i| {
            let a = a_vec[i % a_vec.len()];
            let b = b_vec[i % b_vec.len()];
            match (a, b) {
                (Some(x), Some(y)) => Some(f(x, y)),
                _ => None,
            }
        })
        .collect();
    Ok(RValue::vec(Vector::Double(result.into())))
}

// endregion

/// Round to the specified number of decimal places using IEEE 754
/// round-half-to-even (banker's rounding): when the fractional part is
/// exactly 0.5, round to the nearest even number.
///
/// @param x numeric vector
/// @param digits number of decimal places (default 0)
/// @return numeric vector of rounded values
#[builtin(min_args = 1)]
fn builtin_round(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let digits = named
        .iter()
        .find(|(n, _)| n == "digits")
        .and_then(|(_, v)| v.as_vector()?.as_integer_scalar())
        .or_else(|| args.get(1)?.as_vector()?.as_integer_scalar())
        .unwrap_or(0);
    let factor = 10f64.powi(i32::try_from(digits)?);
    match args.first() {
        Some(RValue::Vector(v)) => {
            let result: Vec<Option<f64>> = v
                .to_doubles()
                .iter()
                .map(|x| x.map(|f| round_half_to_even(f * factor) / factor))
                .collect();
            Ok(RValue::vec(Vector::Double(result.into())))
        }
        _ => Err(RError::new(
            RErrorKind::Argument,
            "non-numeric argument".to_string(),
        )),
    }
}

/// IEEE 754 round-half-to-even: if the fractional part is exactly 0.5,
/// round to the nearest even integer. Otherwise round normally.
fn round_half_to_even(x: f64) -> f64 {
    let rounded = x.round();
    // Check if we're exactly at a .5 boundary
    let diff = x - x.floor();
    if (diff - 0.5).abs() < f64::EPSILON {
        // Exactly half: round to even
        let floor = x.floor();
        let ceil = x.ceil();
        if floor % 2.0 == 0.0 {
            floor
        } else {
            ceil
        }
    } else {
        rounded
    }
}

/// Round to the specified number of significant digits.
///
/// @param x numeric vector
/// @param digits number of significant digits (default 6)
/// @return numeric vector rounded to significant digits
#[builtin(min_args = 1)]
fn builtin_signif(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let digits = named
        .iter()
        .find(|(n, _)| n == "digits")
        .and_then(|(_, v)| v.as_vector()?.as_integer_scalar())
        .or_else(|| args.get(1)?.as_vector()?.as_integer_scalar())
        .unwrap_or(6);
    let digits = i32::try_from(digits)?;
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
                        let d = f64_to_i32(f.abs().log10().ceil()).unwrap_or(0);
                        let factor = 10f64.powi(digits - d);
                        (f * factor).round() / factor
                    })
                })
                .collect();
            Ok(RValue::vec(Vector::Double(result.into())))
        }
        _ => Err(RError::new(
            RErrorKind::Argument,
            "non-numeric argument to signif".to_string(),
        )),
    }
}

// === Parallel min/max ===

/// Parallel (element-wise) minimum across vectors.
///
/// @param ... numeric vectors (recycled to common length)
/// @param na.rm logical; if TRUE, remove NAs before comparison
/// @return numeric vector of element-wise minima
#[builtin(min_args = 1)]
fn builtin_pmin(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let na_rm = named.iter().any(|(n, v)| {
        n == "na.rm" && v.as_vector().and_then(|v| v.as_logical_scalar()) == Some(true)
    });
    parallel_minmax(args, na_rm, false)
}

/// Parallel (element-wise) maximum across vectors.
///
/// @param ... numeric vectors (recycled to common length)
/// @param na.rm logical; if TRUE, remove NAs before comparison
/// @return numeric vector of element-wise maxima
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
        return Err(RError::new(
            RErrorKind::Argument,
            "no arguments to pmin/pmax".to_string(),
        ));
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

/// Cumulative all: TRUE while all preceding elements are TRUE.
///
/// @param x logical vector
/// @return logical vector of cumulative conjunction
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
        _ => Err(RError::new(
            RErrorKind::Argument,
            "invalid argument to cumall".to_string(),
        )),
    }
}

/// Cumulative any: TRUE once any preceding element is TRUE.
///
/// @param x logical vector
/// @return logical vector of cumulative disjunction
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
        _ => Err(RError::new(
            RErrorKind::Argument,
            "invalid argument to cumany".to_string(),
        )),
    }
}

// === Bitwise operations ===

fn bitwise_binary_op_fallible(
    args: &[RValue],
    op: impl Fn(i64, i64) -> Result<i64, RError>,
) -> Result<RValue, RError> {
    let a_ints = args
        .first()
        .and_then(|v| v.as_vector())
        .map(|v| v.to_integers())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "non-integer argument to bitwise function".to_string(),
            )
        })?;
    let b_ints = args
        .get(1)
        .and_then(|v| v.as_vector())
        .map(|v| v.to_integers())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "non-integer argument to bitwise function".to_string(),
            )
        })?;
    let max_len = a_ints.len().max(b_ints.len());
    let result: Result<Vec<Option<i64>>, RError> = (0..max_len)
        .map(|i| {
            let a = a_ints[i % a_ints.len()];
            let b = b_ints[i % b_ints.len()];
            match (a, b) {
                (Some(x), Some(y)) => Ok(Some(op(x, y)?)),
                _ => Ok(None),
            }
        })
        .collect();
    Ok(RValue::vec(Vector::Integer(result?.into())))
}

fn bitwise_binary_op(args: &[RValue], op: fn(i64, i64) -> i64) -> Result<RValue, RError> {
    let a_ints = args
        .first()
        .and_then(|v| v.as_vector())
        .map(|v| v.to_integers())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "non-integer argument to bitwise function".to_string(),
            )
        })?;
    let b_ints = args
        .get(1)
        .and_then(|v| v.as_vector())
        .map(|v| v.to_integers())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "non-integer argument to bitwise function".to_string(),
            )
        })?;
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

/// Bitwise AND.
///
/// @param a integer vector
/// @param b integer vector
/// @return integer vector of bitwise AND results
#[builtin(name = "bitwAnd", min_args = 2)]
fn builtin_bitw_and(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    bitwise_binary_op(args, |a, b| a & b)
}

/// Bitwise OR.
///
/// @param a integer vector
/// @param b integer vector
/// @return integer vector of bitwise OR results
#[builtin(name = "bitwOr", min_args = 2)]
fn builtin_bitw_or(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    bitwise_binary_op(args, |a, b| a | b)
}

/// Bitwise XOR.
///
/// @param a integer vector
/// @param b integer vector
/// @return integer vector of bitwise XOR results
#[builtin(name = "bitwXor", min_args = 2)]
fn builtin_bitw_xor(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    bitwise_binary_op(args, |a, b| a ^ b)
}

/// Bitwise NOT (ones' complement).
///
/// @param a integer vector
/// @return integer vector of bitwise-negated values
#[builtin(name = "bitwNot", min_args = 1)]
fn builtin_bitw_not(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let ints = args
        .first()
        .and_then(|v| v.as_vector())
        .map(|v| v.to_integers())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "non-integer argument to bitwNot".to_string(),
            )
        })?;
    let result: Vec<Option<i64>> = ints.iter().map(|x| x.map(|i| !i)).collect();
    Ok(RValue::vec(Vector::Integer(result.into())))
}

/// Bitwise left shift.
///
/// @param a integer vector
/// @param n number of positions to shift
/// @return integer vector of left-shifted values
#[builtin(name = "bitwShiftL", min_args = 2)]
fn builtin_bitw_shift_l(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    bitwise_binary_op_fallible(args, |a, n| Ok(a << u32::try_from(n)?))
}

/// Bitwise right shift.
///
/// @param a integer vector
/// @param n number of positions to shift
/// @return integer vector of right-shifted values
#[builtin(name = "bitwShiftR", min_args = 2)]
fn builtin_bitw_shift_r(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    bitwise_binary_op_fallible(args, |a, n| Ok(a >> u32::try_from(n)?))
}

// === Triangular matrix extraction ===

/// Lower triangle of a matrix.
///
/// @param x a matrix
/// @param diag logical; include the diagonal? (default FALSE)
/// @return logical matrix with TRUE in the lower triangle
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

/// Upper triangle of a matrix.
///
/// @param x a matrix
/// @param diag logical; include the diagonal? (default FALSE)
/// @return logical matrix with TRUE in the upper triangle
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
                Vector::Integer(d) if d.len() >= 2 => (
                    usize::try_from(d[0].unwrap_or(0)).unwrap_or(0),
                    usize::try_from(d[1].unwrap_or(0)).unwrap_or(0),
                ),
                _ => {
                    return Err(RError::new(
                        RErrorKind::Argument,
                        "argument is not a matrix".to_string(),
                    ))
                }
            },
            _ => {
                return Err(RError::new(
                    RErrorKind::Argument,
                    "argument is not a matrix".to_string(),
                ))
            }
        },
        _ => {
            return Err(RError::new(
                RErrorKind::Argument,
                "argument is not a matrix".to_string(),
            ))
        }
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
    set_matrix_attrs(&mut rv, nrow, ncol, None, None)?;
    Ok(RValue::Vector(rv))
}

// === Diagonal matrix ===

/// Diagonal of a matrix, or construct a diagonal matrix.
///
/// If x is a matrix, extracts the diagonal. If x is a scalar n, creates an
/// n-by-n identity matrix. If x is a vector, creates a diagonal matrix with
/// x on the diagonal.
///
/// @param x a matrix, scalar, or vector
/// @return diagonal vector or diagonal matrix
#[builtin(min_args = 1)]
fn builtin_diag(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(rv)) => {
            // If x is a matrix, extract the diagonal
            if let Some(RValue::Vector(dim_rv)) = rv.get_attr("dim") {
                if let Vector::Integer(d) = &dim_rv.inner {
                    if d.len() >= 2 {
                        let nrow = usize::try_from(d[0].unwrap_or(0)).unwrap_or(0);
                        let ncol = usize::try_from(d[1].unwrap_or(0)).unwrap_or(0);
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
                let n = usize::try_from(rv.as_integer_scalar().unwrap_or(1)).unwrap_or(0);
                let mut result = vec![Some(0.0); n * n];
                for i in 0..n {
                    result[i * n + i] = Some(1.0);
                }
                let mut out = RVector::from(Vector::Double(result.into()));
                set_matrix_attrs(&mut out, n, n, None, None)?;
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
                set_matrix_attrs(&mut out, n, n, None, None)?;
                Ok(RValue::Vector(out))
            }
        }
        _ => Err(RError::new(
            RErrorKind::Argument,
            "'x' must be numeric".to_string(),
        )),
    }
}

/// Sum of all elements.
///
/// @param ... numeric vectors
/// @param na.rm logical; if TRUE, remove NAs before summing
/// @return scalar double
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

/// Product of all elements.
///
/// @param ... numeric vectors
/// @param na.rm logical; if TRUE, remove NAs before multiplying
/// @return scalar double
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

/// Maximum value across all arguments.
///
/// @param ... numeric vectors
/// @param na.rm logical; if TRUE, remove NAs
/// @return scalar double (or -Inf if no values)
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

/// Minimum value across all arguments.
///
/// @param ... numeric vectors
/// @param na.rm logical; if TRUE, remove NAs
/// @return scalar double (or Inf if no values)
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

/// Arithmetic mean.
///
/// @param x numeric vector
/// @param na.rm logical; if TRUE, remove NAs before averaging
/// @return scalar double
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
                vec![Some(sum / usize_to_f64(count))].into(),
            )))
        }
        _ => Ok(RValue::vec(Vector::Double(vec![Some(f64::NAN)].into()))),
    }
}

/// Median value.
///
/// @param x numeric vector
/// @param na.rm logical; if TRUE, remove NAs before computing
/// @return scalar double (NA if input contains NAs and na.rm is FALSE)
#[builtin(min_args = 1, namespace = "stats")]
fn builtin_median(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let na_rm = named.iter().any(|(n, v)| {
        n == "na.rm" && v.as_vector().and_then(|v| v.as_logical_scalar()) == Some(true)
    });
    match args.first() {
        Some(RValue::Vector(v)) => {
            let doubles = v.to_doubles();
            // Check for NAs before filtering
            if !na_rm && doubles.iter().any(|x| x.is_none()) {
                return Ok(RValue::vec(Vector::Double(vec![None].into())));
            }
            let mut vals: Vec<f64> = doubles.into_iter().flatten().collect();
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

/// Sample variance (Bessel-corrected, divides by n-1).
///
/// @param x numeric vector
/// @param na.rm logical; if TRUE, remove NAs before computing
/// @return scalar double (NA if input contains NAs and na.rm is FALSE)
#[builtin(min_args = 1, namespace = "stats")]
fn builtin_var(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let na_rm = named.iter().any(|(n, v)| {
        n == "na.rm" && v.as_vector().and_then(|v| v.as_logical_scalar()) == Some(true)
    });
    match args.first() {
        Some(RValue::Vector(v)) => {
            let doubles = v.to_doubles();
            // Check for NAs before filtering
            if !na_rm && doubles.iter().any(|x| x.is_none()) {
                return Ok(RValue::vec(Vector::Double(vec![None].into())));
            }
            let vals: Vec<f64> = doubles.into_iter().flatten().collect();
            let n = usize_to_f64(vals.len());
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

/// Sample standard deviation (square root of var).
///
/// @param x numeric vector
/// @return scalar double
#[builtin(min_args = 1, namespace = "stats")]
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

/// Cumulative sum.
///
/// Once an NA is encountered, all subsequent values become NA.
///
/// @param x numeric vector
/// @return numeric vector of running sums
#[builtin(min_args = 1)]
fn builtin_cumsum(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(v)) => {
            let mut acc = 0.0;
            let mut seen_na = false;
            let result: Vec<Option<f64>> = v
                .to_doubles()
                .iter()
                .map(|x| {
                    if seen_na {
                        return None;
                    }
                    match *x {
                        Some(f) => {
                            acc += f;
                            Some(acc)
                        }
                        None => {
                            seen_na = true;
                            None
                        }
                    }
                })
                .collect();
            Ok(RValue::vec(Vector::Double(result.into())))
        }
        _ => Err(RError::new(
            RErrorKind::Argument,
            "invalid argument".to_string(),
        )),
    }
}

/// Cumulative product.
///
/// Once an NA is encountered, all subsequent values become NA.
///
/// @param x numeric vector
/// @return numeric vector of running products
#[builtin(min_args = 1)]
fn builtin_cumprod(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(v)) => {
            let mut acc = 1.0;
            let mut seen_na = false;
            let result: Vec<Option<f64>> = v
                .to_doubles()
                .iter()
                .map(|x| {
                    if seen_na {
                        return None;
                    }
                    match *x {
                        Some(f) => {
                            acc *= f;
                            Some(acc)
                        }
                        None => {
                            seen_na = true;
                            None
                        }
                    }
                })
                .collect();
            Ok(RValue::vec(Vector::Double(result.into())))
        }
        _ => Err(RError::new(
            RErrorKind::Argument,
            "invalid argument".to_string(),
        )),
    }
}

/// Cumulative maximum.
///
/// Once an NA is encountered, all subsequent values become NA.
///
/// @param x numeric vector
/// @return numeric vector of running maxima
#[builtin(min_args = 1)]
fn builtin_cummax(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(v)) => {
            let mut acc = f64::NEG_INFINITY;
            let mut seen_na = false;
            let result: Vec<Option<f64>> = v
                .to_doubles()
                .iter()
                .map(|x| {
                    if seen_na {
                        return None;
                    }
                    match *x {
                        Some(f) => {
                            acc = acc.max(f);
                            Some(acc)
                        }
                        None => {
                            seen_na = true;
                            None
                        }
                    }
                })
                .collect();
            Ok(RValue::vec(Vector::Double(result.into())))
        }
        _ => Err(RError::new(
            RErrorKind::Argument,
            "invalid argument".to_string(),
        )),
    }
}

/// Cumulative minimum.
///
/// Once an NA is encountered, all subsequent values become NA.
///
/// @param x numeric vector
/// @return numeric vector of running minima
#[builtin(min_args = 1)]
fn builtin_cummin(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(v)) => {
            let mut acc = f64::INFINITY;
            let mut seen_na = false;
            let result: Vec<Option<f64>> = v
                .to_doubles()
                .iter()
                .map(|x| {
                    if seen_na {
                        return None;
                    }
                    match *x {
                        Some(f) => {
                            acc = acc.min(f);
                            Some(acc)
                        }
                        None => {
                            seen_na = true;
                            None
                        }
                    }
                })
                .collect();
            Ok(RValue::vec(Vector::Double(result.into())))
        }
        _ => Err(RError::new(
            RErrorKind::Argument,
            "invalid argument".to_string(),
        )),
    }
}

/// Generate a regular sequence.
///
/// @param from starting value (default 1)
/// @param to ending value (default 1)
/// @param by increment (default 1 or -1)
/// @param length.out desired length of the sequence
/// @return numeric vector
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
        let len = usize::try_from(len)?;
        if len == 0 {
            return Ok(RValue::vec(Vector::Double(vec![].into())));
        }
        if len == 1 {
            return Ok(RValue::vec(Vector::Double(vec![Some(from)].into())));
        }
        let step = (to - from) / usize_to_f64(len - 1);
        let result: Vec<Option<f64>> = (0..len)
            .map(|i| Some(from + step * usize_to_f64(i)))
            .collect();
        return Ok(RValue::vec(Vector::Double(result.into())));
    }

    let by = by.unwrap_or(if to >= from { 1.0 } else { -1.0 });
    if by == 0.0 {
        return Err(RError::new(
            RErrorKind::Argument,
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

/// Generate 1:n as an integer vector.
///
/// @param length.out the desired length
/// @return integer vector 1, 2, ..., n
#[builtin(name = "seq_len", min_args = 1)]
fn builtin_seq_len(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let n = args
        .first()
        .and_then(|v| v.as_vector()?.as_integer_scalar())
        .unwrap_or(0);
    let result: Vec<Option<i64>> = (1..=n).map(Some).collect();
    Ok(RValue::vec(Vector::Integer(result.into())))
}

/// Generate 1:length(along.with) as an integer vector.
///
/// @param along.with the object whose length determines the sequence
/// @return integer vector 1, 2, ..., length(along.with)
#[builtin(name = "seq_along", min_args = 1)]
fn builtin_seq_along(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let n = args.first().map(|v| v.length()).unwrap_or(0);
    let n_i64 = i64::try_from(n)?;
    let result: Vec<Option<i64>> = (1..=n_i64).map(Some).collect();
    Ok(RValue::vec(Vector::Integer(result.into())))
}

/// Replicate elements of a vector.
///
/// @param x a vector
/// @param times number of times to repeat the whole vector (default 1)
/// @param each number of times to repeat each element before moving to the next
/// @param length.out desired output length (truncates or extends the result)
/// @return vector with elements repeated
#[builtin(min_args = 1)]
fn builtin_rep(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let each = named
        .iter()
        .find(|(n, _)| n == "each")
        .and_then(|(_, v)| v.as_vector()?.as_integer_scalar());

    let length_out = named
        .iter()
        .find(|(n, _)| n == "length.out")
        .and_then(|(_, v)| v.as_vector()?.as_integer_scalar());

    let times = named
        .iter()
        .find(|(n, _)| n == "times")
        .and_then(|(_, v)| v.as_vector()?.as_integer_scalar())
        .or_else(|| {
            // Only use positional arg 1 for times if `each` is not set
            if each.is_none() {
                args.get(1)?.as_vector()?.as_integer_scalar()
            } else {
                None
            }
        })
        .unwrap_or(1);
    let times = usize::try_from(times)?;

    let each_n = each.map(usize::try_from).transpose()?.unwrap_or(1);

    match args.first() {
        Some(RValue::Vector(v)) => {
            let result = rep_vector(&v.inner, each_n, times, length_out)?;
            Ok(RValue::vec(result))
        }
        _ => Err(RError::new(
            RErrorKind::Argument,
            "invalid argument".to_string(),
        )),
    }
}

/// Helper: replicate a vector with `each`, `times`, and optional `length.out`.
///
/// Algorithm: first apply `each` (repeat each element), then apply `times`
/// (repeat the whole result), then truncate/extend to `length.out` if given.
fn rep_vector(
    v: &Vector,
    each: usize,
    times: usize,
    length_out: Option<i64>,
) -> Result<Vector, RError> {
    // Apply `each` first, then `times`, using the doubles representation as a
    // type-agnostic strategy won't work — we need to preserve the original type.
    // Instead, convert to doubles, do the replication, then figure out the type.
    // Actually, let's handle it per-type to preserve type fidelity.
    let result = match v {
        Vector::Double(vals) => {
            let expanded = rep_each_then_times(vals.as_slice(), each, times);
            Vector::Double(apply_length_out_cloneable(expanded, length_out)?.into())
        }
        Vector::Integer(vals) => {
            let expanded = rep_each_then_times(vals.as_slice(), each, times);
            Vector::Integer(apply_length_out_cloneable(expanded, length_out)?.into())
        }
        Vector::Logical(vals) => {
            let expanded = rep_each_then_times(vals.as_slice(), each, times);
            Vector::Logical(apply_length_out_cloneable(expanded, length_out)?.into())
        }
        Vector::Character(vals) => {
            let expanded = rep_each_then_times(vals.as_slice(), each, times);
            Vector::Character(apply_length_out_cloneable(expanded, length_out)?.into())
        }
        Vector::Complex(vals) => {
            let expanded = rep_each_then_times(vals.as_slice(), each, times);
            Vector::Complex(apply_length_out_cloneable(expanded, length_out)?.into())
        }
        Vector::Raw(vals) => {
            let expanded = rep_each_then_times_copy(vals, each, times);
            let final_vec = if let Some(lo) = length_out {
                let lo = usize::try_from(lo)?;
                if expanded.is_empty() {
                    vec![]
                } else {
                    expanded.iter().cycle().take(lo).copied().collect()
                }
            } else {
                expanded
            };
            Vector::Raw(final_vec)
        }
    };
    Ok(result)
}

/// Repeat each element `each` times, then repeat the whole result `times` times.
fn rep_each_then_times<T: Clone>(vals: &[T], each: usize, times: usize) -> Vec<T> {
    let with_each: Vec<T> = if each <= 1 {
        vals.to_vec()
    } else {
        vals.iter()
            .flat_map(|v| std::iter::repeat_n(v.clone(), each))
            .collect()
    };
    if times <= 1 {
        with_each
    } else {
        let len = with_each.len();
        with_each.into_iter().cycle().take(len * times).collect()
    }
}

/// Same as `rep_each_then_times` but for Copy types (Raw/u8).
fn rep_each_then_times_copy<T: Copy>(vals: &[T], each: usize, times: usize) -> Vec<T> {
    let with_each: Vec<T> = if each <= 1 {
        vals.to_vec()
    } else {
        vals.iter()
            .flat_map(|&v| std::iter::repeat_n(v, each))
            .collect()
    };
    if times <= 1 {
        with_each
    } else {
        let len = with_each.len();
        with_each.into_iter().cycle().take(len * times).collect()
    }
}

/// Truncate or cycle-extend a Vec<T: Clone> to the requested `length.out`.
fn apply_length_out_cloneable<T: Clone>(
    v: Vec<T>,
    length_out: Option<i64>,
) -> Result<Vec<T>, RError> {
    match length_out {
        None => Ok(v),
        Some(lo) => {
            let lo = usize::try_from(lo)?;
            if v.is_empty() {
                return Ok(vec![]);
            }
            Ok(v.iter().cycle().take(lo).cloned().collect())
        }
    }
}

/// Reverse a vector.
///
/// @param x a vector
/// @return vector with elements in reverse order
#[builtin(min_args = 1)]
fn builtin_rev(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(v)) => {
            let result = match &v.inner {
                Vector::Raw(vals) => {
                    let mut v = vals.clone();
                    v.reverse();
                    Vector::Raw(v)
                }
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
                Vector::Complex(vals) => {
                    let mut v = vals.clone();
                    v.reverse();
                    Vector::Complex(v)
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

/// Sort a vector.
///
/// @param x a vector
/// @param decreasing logical; sort in descending order? (default FALSE)
/// @param na.last logical; TRUE puts NAs at end, FALSE at beginning, NA removes them (default TRUE)
/// @return sorted vector
#[builtin(min_args = 1)]
fn builtin_sort(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let decreasing = named
        .iter()
        .find(|(n, _)| n == "decreasing")
        .and_then(|(_, v)| v.as_vector()?.as_logical_scalar())
        .unwrap_or(false);

    // na.last: TRUE (default) = NAs at end, FALSE = NAs at beginning, NA = remove NAs
    // R's sort() defaults to na.last = NA (remove NAs), unlike order() which defaults to TRUE
    let na_last_val = named.iter().find(|(n, _)| n == "na.last").map(|(_, v)| v);
    let na_last: Option<bool> = match na_last_val {
        Some(RValue::Vector(rv)) => rv.inner.as_logical_scalar(),
        Some(RValue::Null) => None,
        None => None, // R default for sort is na.last=NA (remove NAs)
        _ => None,
    };

    match args.first() {
        Some(RValue::Vector(v)) => {
            let result = match &v.inner {
                Vector::Double(vals) => {
                    let (mut non_na, na_count) = partition_na_doubles(&vals.0);
                    non_na.sort_by(|a, b| {
                        if decreasing {
                            b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal)
                        } else {
                            a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)
                        }
                    });
                    let result = reassemble_with_na(
                        non_na.into_iter().map(Some).collect(),
                        na_count,
                        na_last,
                    );
                    Vector::Double(result.into())
                }
                Vector::Integer(vals) => {
                    let (mut non_na, na_count) = partition_na_options(vals.as_slice());
                    non_na.sort_by(|a, b| if decreasing { b.cmp(a) } else { a.cmp(b) });
                    let result = reassemble_with_na(
                        non_na.into_iter().map(Some).collect(),
                        na_count,
                        na_last,
                    );
                    Vector::Integer(result.into())
                }
                Vector::Character(vals) => {
                    let (mut non_na, na_count) = partition_na_options(vals.as_slice());
                    non_na.sort_by(|a, b| if decreasing { b.cmp(a) } else { a.cmp(b) });
                    let result = reassemble_with_na(
                        non_na.into_iter().map(Some).collect(),
                        na_count,
                        na_last,
                    );
                    Vector::Character(result.into())
                }
                other => other.clone(),
            };
            Ok(RValue::vec(result))
        }
        _ => Ok(RValue::Null),
    }
}

/// Separate non-NA f64 values from NA count.
fn partition_na_doubles(vals: &[Option<f64>]) -> (Vec<f64>, usize) {
    let mut non_na = Vec::with_capacity(vals.len());
    let mut na_count = 0;
    for v in vals {
        match v {
            Some(f) if !f.is_nan() => non_na.push(*f),
            _ => na_count += 1,
        }
    }
    (non_na, na_count)
}

/// Separate non-NA Option values from NA count for Clone types.
fn partition_na_options<T: Clone>(vals: &[Option<T>]) -> (Vec<T>, usize) {
    let mut non_na = Vec::with_capacity(vals.len());
    let mut na_count = 0;
    for v in vals {
        match v {
            Some(x) => non_na.push(x.clone()),
            None => na_count += 1,
        }
    }
    (non_na, na_count)
}

/// Reassemble sorted non-NA values with NAs placed according to `na_last`.
///
/// - `Some(true)` — NAs go at end
/// - `Some(false)` — NAs go at beginning
/// - `None` — NAs are removed
fn reassemble_with_na<T: Clone>(
    sorted: Vec<Option<T>>,
    na_count: usize,
    na_last: Option<bool>,
) -> Vec<Option<T>> {
    match na_last {
        None => sorted, // remove NAs
        Some(true) => {
            let mut result = sorted;
            result.extend(std::iter::repeat_n(None, na_count));
            result
        }
        Some(false) => {
            let mut result: Vec<Option<T>> = std::iter::repeat_n(None, na_count).collect();
            result.extend(sorted);
            result
        }
    }
}

/// Permutation which rearranges a vector into ascending order.
///
/// @param x a vector
/// @return integer vector of 1-based indices
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
            let result: Vec<Option<i64>> = indices
                .iter()
                .map(|&i| Some(i64::try_from(i).unwrap_or(0) + 1))
                .collect();
            Ok(RValue::vec(Vector::Integer(result.into())))
        }
        _ => Ok(RValue::Null),
    }
}

/// Return the ranks of values in a numeric vector.
///
/// Computes the sample ranks of the values. Ties are handled according to
/// the `ties.method` argument: "average" (default) assigns the mean rank,
/// "first" preserves the original order, "min" uses the minimum rank, and
/// "max" uses the maximum rank.
///
/// @param x numeric vector to rank
/// @param ties.method character string: "average", "first", "min", or "max"
/// @return double vector of ranks (1-based)
#[builtin(min_args = 1)]
fn builtin_rank(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let ties_method = named
        .iter()
        .find(|(n, _)| n == "ties.method")
        .and_then(|(_, v)| v.as_vector()?.as_character_scalar())
        .unwrap_or_else(|| "average".to_string());

    let vals = match args.first() {
        Some(RValue::Vector(v)) => v.to_doubles(),
        _ => return Ok(RValue::Null),
    };

    let n = vals.len();
    if n == 0 {
        return Ok(RValue::vec(Vector::Double(Vec::new().into())));
    }

    // Build (value, original_index) pairs and sort by value.
    // NA values sort last (matching R's na.last = TRUE default).
    let mut indexed: Vec<(Option<f64>, usize)> = vals.iter().copied().zip(0..n).collect();
    indexed.sort_by(|a, b| match (a.0, b.0) {
        (None, None) => std::cmp::Ordering::Equal,
        (None, _) => std::cmp::Ordering::Greater,
        (_, None) => std::cmp::Ordering::Less,
        (Some(va), Some(vb)) => va.partial_cmp(&vb).unwrap_or(std::cmp::Ordering::Equal),
    });

    // Assign ranks based on ties method
    let mut ranks = vec![0.0f64; n];
    match ties_method.as_str() {
        "first" => {
            for (rank_minus_1, &(_, orig_idx)) in indexed.iter().enumerate() {
                ranks[orig_idx] = (rank_minus_1 + 1) as f64;
            }
        }
        "min" | "max" | "average" => {
            let mut i = 0;
            while i < n {
                // Find the run of tied values
                let mut j = i + 1;
                while j < n {
                    let same = match (indexed[i].0, indexed[j].0) {
                        (None, None) => true,
                        (Some(a), Some(b)) => a == b,
                        _ => false,
                    };
                    if !same {
                        break;
                    }
                    j += 1;
                }
                // Ranks for this tied group are i+1 .. j (1-based)
                let rank = match ties_method.as_str() {
                    "min" => (i + 1) as f64,
                    "max" => j as f64,
                    _ => {
                        // "average": mean of ranks i+1 .. j
                        let sum: f64 = ((i + 1)..=j).map(|r| r as f64).sum();
                        sum / (j - i) as f64
                    }
                };
                for item in indexed.iter().take(j).skip(i) {
                    ranks[item.1] = rank;
                }
                i = j;
            }
        }
        other => {
            return Err(RError::new(
                RErrorKind::Argument,
                format!(
                "ties.method must be one of \"average\", \"first\", \"min\", or \"max\", not {:?}",
                other
            ),
            ))
        }
    }

    let result: Vec<Option<f64>> = ranks.into_iter().map(Some).collect();
    Ok(RValue::vec(Vector::Double(result.into())))
}

/// Remove duplicate elements, preserving first occurrence order.
///
/// @param x a vector
/// @return vector of unique elements
#[builtin(min_args = 1)]
fn builtin_unique(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(v)) => {
            let result = match &v.inner {
                Vector::Raw(vals) => {
                    let mut seen = Vec::new();
                    let mut result = Vec::new();
                    for &x in vals.iter() {
                        if !seen.contains(&x) {
                            seen.push(x);
                            result.push(x);
                        }
                    }
                    Vector::Raw(result)
                }
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
                Vector::Complex(vals) => {
                    let mut seen = Vec::new();
                    let mut result = Vec::new();
                    for x in vals.iter() {
                        let key = format!("{:?}", x);
                        if !seen.contains(&key) {
                            seen.push(key);
                            result.push(*x);
                        }
                    }
                    Vector::Complex(result.into())
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

/// miniR extension: single-pass sorted unique using BTreeSet.
/// Faster than `sort(unique(x))` for large vectors.
#[builtin(name = "sort_unique", min_args = 1)]
fn builtin_sort_unique(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let decreasing = named
        .iter()
        .find(|(n, _)| n == "decreasing")
        .and_then(|(_, v)| v.as_vector()?.as_logical_scalar())
        .unwrap_or(false);

    match args.first() {
        Some(RValue::Vector(v)) => {
            let result = match &v.inner {
                Vector::Double(vals) => {
                    // Total ordering key: flip bits so BTreeSet gives numeric order.
                    // Positive floats: bits as-is. Negative floats: flip all bits.
                    // This maps f64 ordering to u64 ordering. NAs (None) sort last.
                    fn sort_key(v: &Option<f64>) -> (bool, u64) {
                        match v {
                            None => (true, 0), // NAs last
                            Some(f) => {
                                let bits = f.to_bits();
                                let key = if bits >> 63 == 1 {
                                    !bits
                                } else {
                                    bits ^ (1 << 63)
                                };
                                (false, key)
                            }
                        }
                    }
                    let set: BTreeSet<(bool, u64)> = vals.iter().map(sort_key).collect();
                    // Reverse-map keys back to f64 values
                    let mut result: Vec<Option<f64>> = set
                        .into_iter()
                        .map(|(is_na, key)| {
                            if is_na {
                                None
                            } else {
                                let bits = if key >> 63 == 0 {
                                    !key
                                } else {
                                    key ^ (1 << 63)
                                };
                                Some(f64::from_bits(bits))
                            }
                        })
                        .collect();
                    if decreasing {
                        result.reverse();
                    }
                    Vector::Double(result.into())
                }
                Vector::Integer(vals) => {
                    let has_na = vals.iter().any(|x| x.is_none());
                    let set: BTreeSet<i64> = vals.iter().filter_map(|x| *x).collect();
                    let mut result: Vec<Option<i64>> = set.into_iter().map(Some).collect();
                    if decreasing {
                        result.reverse();
                    }
                    if has_na {
                        result.push(None); // NAs sort last
                    }
                    Vector::Integer(result.into())
                }
                Vector::Character(vals) => {
                    let has_na = vals.iter().any(|x| x.is_none());
                    let set: BTreeSet<&str> = vals.iter().filter_map(|x| x.as_deref()).collect();
                    let mut result: Vec<Option<String>> =
                        set.into_iter().map(|s| Some(s.to_string())).collect();
                    if decreasing {
                        result.reverse();
                    }
                    if has_na {
                        result.push(None); // NAs sort last
                    }
                    Vector::Character(result.into())
                }
                other => other.clone(),
            };
            Ok(RValue::vec(result))
        }
        _ => Ok(RValue::Null),
    }
}

/// Indices of TRUE elements.
///
/// When `arr.ind = TRUE` and the input has a `dim` attribute (matrix),
/// returns a matrix of row/column subscripts instead of linear indices.
///
/// @param x logical vector or matrix
/// @param arr.ind if TRUE, return matrix subscripts for array input
/// @return integer vector of 1-based indices, or integer matrix of subscripts
#[builtin(min_args = 1)]
fn builtin_which(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let arr_ind = named
        .iter()
        .find(|(n, _)| n == "arr.ind")
        .and_then(|(_, v)| v.as_vector().and_then(|v| v.as_logical_scalar()))
        .unwrap_or(false);

    match args.first() {
        Some(RValue::Vector(rv)) => {
            let logicals = rv.to_logicals();
            let true_indices: Vec<i64> = logicals
                .iter()
                .enumerate()
                .filter_map(|(i, v)| {
                    if *v == Some(true) {
                        Some(i64::try_from(i).unwrap_or(0) + 1)
                    } else {
                        None
                    }
                })
                .collect();

            // Check for arr.ind with matrix dim attribute
            if arr_ind {
                if let Some(RValue::Vector(dim_rv)) = rv.get_attr("dim") {
                    let dims = dim_rv.to_doubles();
                    if dims.len() >= 2 {
                        let n = true_indices.len();
                        let ndim = dims.len();
                        let dim_sizes: Vec<i64> =
                            dims.iter().map(|d| d.unwrap_or(0.0) as i64).collect();
                        let mut result: Vec<Option<i64>> = vec![None; n * ndim];

                        for (idx_pos, &linear_1based) in true_indices.iter().enumerate() {
                            let mut linear = linear_1based - 1;
                            for d in 0..ndim {
                                let subscript = linear % dim_sizes[d];
                                result[d * n + idx_pos] = Some(subscript + 1);
                                linear /= dim_sizes[d];
                            }
                        }

                        let mut result_rv = RVector::from(Vector::Integer(result.into()));
                        result_rv.set_attr(
                            "dim".to_string(),
                            RValue::vec(Vector::Integer(
                                vec![
                                    Some(i64::try_from(n).unwrap_or(0)),
                                    Some(i64::try_from(ndim).unwrap_or(0)),
                                ]
                                .into(),
                            )),
                        );
                        return Ok(RValue::Vector(result_rv));
                    }
                }
            }

            let result: Vec<Option<i64>> = true_indices.into_iter().map(Some).collect();
            Ok(RValue::vec(Vector::Integer(result.into())))
        }
        _ => Ok(RValue::vec(Vector::Integer(vec![].into()))),
    }
}

/// Index of the minimum element.
///
/// @param x numeric vector
/// @return scalar integer (1-based index)
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
                vec![min_idx.map(|i| i64::try_from(i).unwrap_or(0) + 1)].into(),
            )))
        }
        _ => Ok(RValue::vec(Vector::Integer(vec![].into()))),
    }
}

/// Index of the maximum element.
///
/// @param x numeric vector
/// @return scalar integer (1-based index)
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
                vec![max_idx.map(|i| i64::try_from(i).unwrap_or(0) + 1)].into(),
            )))
        }
        _ => Ok(RValue::vec(Vector::Integer(vec![].into()))),
    }
}

/// Append elements to a vector.
///
/// Concatenates `values` into `x` at position `after`. Type coercion follows
/// R's hierarchy: raw < logical < integer < double < complex < character.
///
/// @param x a vector
/// @param values values to append
/// @param after index after which to insert (default: end of x)
/// @return concatenated vector preserving the highest type
#[builtin(min_args = 2)]
fn builtin_append(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    match (args.first(), args.get(1)) {
        (Some(RValue::Vector(v1)), Some(RValue::Vector(v2))) => {
            // Determine `after` position (1-based, default = length of x)
            let x_len = v1.inner.len();
            let after = named
                .iter()
                .find(|(k, _)| k == "after")
                .map(|(_, v)| v)
                .or(args.get(2))
                .and_then(|v| v.as_vector()?.as_integer_scalar())
                .map(|a| usize::try_from(a.max(0)).unwrap_or(0).min(x_len))
                .unwrap_or(x_len);

            // Determine the highest type between both vectors
            let has_char = matches!(v1.inner, Vector::Character(_))
                || matches!(v2.inner, Vector::Character(_));
            let has_complex =
                matches!(v1.inner, Vector::Complex(_)) || matches!(v2.inner, Vector::Complex(_));
            let has_double =
                matches!(v1.inner, Vector::Double(_)) || matches!(v2.inner, Vector::Double(_));
            let has_int =
                matches!(v1.inner, Vector::Integer(_)) || matches!(v2.inner, Vector::Integer(_));

            let result = if has_char {
                let x = v1.to_characters();
                let vals = v2.to_characters();
                let mut out = x[..after].to_vec();
                out.extend(vals);
                out.extend_from_slice(&x[after..]);
                Vector::Character(out.into())
            } else if has_complex {
                let x = v1.inner.to_complex();
                let vals = v2.inner.to_complex();
                let mut out = x[..after].to_vec();
                out.extend(vals);
                out.extend_from_slice(&x[after..]);
                Vector::Complex(out.into())
            } else if has_double {
                let x = v1.to_doubles();
                let vals = v2.to_doubles();
                let mut out = x[..after].to_vec();
                out.extend(vals);
                out.extend_from_slice(&x[after..]);
                Vector::Double(out.into())
            } else if has_int {
                let x = v1.to_integers();
                let vals = v2.to_integers();
                let mut out = x[..after].to_vec();
                out.extend(vals);
                out.extend_from_slice(&x[after..]);
                Vector::Integer(out.into())
            } else {
                // Both logical (or raw)
                let x = v1.to_logicals();
                let vals = v2.to_logicals();
                let mut out = x[..after].to_vec();
                out.extend(vals);
                out.extend_from_slice(&x[after..]);
                Vector::Logical(out.into())
            };
            Ok(RValue::vec(result))
        }
        (Some(RValue::List(l1)), Some(RValue::List(l2))) => {
            let x_len = l1.values.len();
            let after = named
                .iter()
                .find(|(k, _)| k == "after")
                .map(|(_, v)| v)
                .or(args.get(2))
                .and_then(|v| v.as_vector()?.as_integer_scalar())
                .map(|a| usize::try_from(a.max(0)).unwrap_or(0).min(x_len))
                .unwrap_or(x_len);
            let mut out = l1.values[..after].to_vec();
            out.extend(l2.values.clone());
            out.extend_from_slice(&l1.values[after..]);
            Ok(RValue::List(RList::new(out)))
        }
        _ => Ok(args.first().cloned().unwrap_or(RValue::Null)),
    }
}

/// Return the first n elements of a vector.
///
/// @param x a vector
/// @param n number of elements to return (default 6)
/// @return vector of the first n elements
#[builtin(min_args = 1, namespace = "utils")]
fn builtin_head(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let n = named
        .iter()
        .find(|(k, _)| k == "n")
        .map(|(_, v)| v)
        .or(args.get(1))
        .and_then(|v| v.as_vector()?.as_integer_scalar())
        .unwrap_or(6);
    let n = usize::try_from(n).unwrap_or(0);
    match args.first() {
        Some(RValue::Vector(v)) => {
            let result = match &v.inner {
                Vector::Raw(vals) => Vector::Raw(vals[..n.min(vals.len())].to_vec()),
                Vector::Double(vals) => Vector::Double(vals[..n.min(vals.len())].to_vec().into()),
                Vector::Integer(vals) => Vector::Integer(vals[..n.min(vals.len())].to_vec().into()),
                Vector::Logical(vals) => Vector::Logical(vals[..n.min(vals.len())].to_vec().into()),
                Vector::Complex(vals) => Vector::Complex(vals[..n.min(vals.len())].to_vec().into()),
                Vector::Character(vals) => {
                    Vector::Character(vals[..n.min(vals.len())].to_vec().into())
                }
            };
            Ok(RValue::vec(result))
        }
        _ => Ok(RValue::Null),
    }
}

/// Return the last n elements of a vector.
///
/// @param x a vector
/// @param n number of elements to return (default 6)
/// @return vector of the last n elements
#[builtin(min_args = 1, namespace = "utils")]
fn builtin_tail(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let n = named
        .iter()
        .find(|(k, _)| k == "n")
        .map(|(_, v)| v)
        .or(args.get(1))
        .and_then(|v| v.as_vector()?.as_integer_scalar())
        .unwrap_or(6);
    let n = usize::try_from(n).unwrap_or(0);
    match args.first() {
        Some(RValue::Vector(v)) => {
            let len = v.len();
            let start = len.saturating_sub(n);
            let result = match &v.inner {
                Vector::Raw(vals) => Vector::Raw(vals[start..].to_vec()),
                Vector::Double(vals) => Vector::Double(vals[start..].to_vec().into()),
                Vector::Integer(vals) => Vector::Integer(vals[start..].to_vec().into()),
                Vector::Logical(vals) => Vector::Logical(vals[start..].to_vec().into()),
                Vector::Complex(vals) => Vector::Complex(vals[start..].to_vec().into()),
                Vector::Character(vals) => Vector::Character(vals[start..].to_vec().into()),
            };
            Ok(RValue::vec(result))
        }
        _ => Ok(RValue::Null),
    }
}

/// Range (minimum and maximum) of all values.
///
/// @param ... numeric vectors
/// @param na.rm logical; if TRUE, remove NAs before computing
/// @return numeric vector of length 2: c(min, max) (c(NA, NA) if NAs present and na.rm is FALSE)
#[builtin(min_args = 0)]
fn builtin_range(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let na_rm = named.iter().any(|(n, v)| {
        n == "na.rm" && v.as_vector().and_then(|v| v.as_logical_scalar()) == Some(true)
    });
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    for arg in args {
        if let RValue::Vector(v) = arg {
            for x in v.to_doubles() {
                match x {
                    Some(f) => {
                        if f < min {
                            min = f;
                        }
                        if f > max {
                            max = f;
                        }
                    }
                    None if !na_rm => {
                        return Ok(RValue::vec(Vector::Double(vec![None, None].into())));
                    }
                    None => {}
                }
            }
        }
    }
    Ok(RValue::vec(Vector::Double(
        vec![Some(min), Some(max)].into(),
    )))
}

/// Lagged differences.
///
/// @param x numeric vector
/// @param lag the lag to use (default 1)
/// @param differences number of times to apply differencing (default 1)
/// @return numeric vector of length(x) - lag * differences
#[builtin(min_args = 1)]
fn builtin_diff(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let lag = named
        .iter()
        .find(|(n, _)| n == "lag")
        .map(|(_, v)| v)
        .or(args.get(1))
        .and_then(|v| v.as_vector()?.as_integer_scalar())
        .unwrap_or(1);
    let lag = usize::try_from(lag).map_err(|_| {
        RError::new(
            RErrorKind::Argument,
            format!("'lag' must be a positive integer, got {lag}"),
        )
    })?;
    if lag == 0 {
        return Err(RError::new(
            RErrorKind::Argument,
            "'lag' must be a positive integer, got 0".to_string(),
        ));
    }

    let differences = named
        .iter()
        .find(|(n, _)| n == "differences")
        .and_then(|(_, v)| v.as_vector()?.as_integer_scalar())
        .unwrap_or(1);
    let differences = usize::try_from(differences).map_err(|_| {
        RError::new(
            RErrorKind::Argument,
            format!("'differences' must be a positive integer, got {differences}"),
        )
    })?;
    if differences == 0 {
        return Err(RError::new(
            RErrorKind::Argument,
            "'differences' must be a positive integer, got 0".to_string(),
        ));
    }

    match args.first() {
        Some(RValue::Vector(v)) => {
            let mut vals = v.to_doubles();
            for _ in 0..differences {
                if vals.len() <= lag {
                    return Ok(RValue::vec(Vector::Double(vec![].into())));
                }
                vals = (lag..vals.len())
                    .map(|i| match (vals[i - lag], vals[i]) {
                        (Some(a), Some(b)) => Some(b - a),
                        _ => None,
                    })
                    .collect();
            }
            Ok(RValue::vec(Vector::Double(vals.into())))
        }
        _ => Err(RError::new(
            RErrorKind::Argument,
            "invalid argument".to_string(),
        )),
    }
}

/// Sample quantiles.
///
/// Computes quantiles using type 7 (R default): for probability p and sorted
/// data of length n, h = (n-1)*p, result = x[floor(h)] + (h - floor(h)) *
/// (x[ceil(h)] - x[floor(h)]).
///
/// @param x numeric vector
/// @param probs numeric vector of probabilities (default: c(0, 0.25, 0.5, 0.75, 1))
/// @param na.rm logical; remove NAs before computing? (default FALSE)
/// @param type integer; quantile algorithm type (only type 7 supported)
/// @return named numeric vector of quantiles
#[builtin(min_args = 1)]
fn builtin_quantile(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let na_rm = named
        .iter()
        .find(|(n, _)| n == "na.rm")
        .and_then(|(_, v)| v.as_vector()?.as_logical_scalar())
        .unwrap_or(false);

    let qtype = named
        .iter()
        .find(|(n, _)| n == "type")
        .and_then(|(_, v)| v.as_vector()?.as_integer_scalar())
        .unwrap_or(7);
    if qtype != 7 {
        return Err(RError::new(
            RErrorKind::Argument,
            format!(
                "only quantile type 7 is currently supported, got type {qtype}. \
                 Type 7 is R's default and the most commonly used algorithm."
            ),
        ));
    }

    // Parse probs: named arg, positional arg 1, or default c(0, 0.25, 0.5, 0.75, 1)
    let default_probs = vec![Some(0.0), Some(0.25), Some(0.5), Some(0.75), Some(1.0)];
    let probs: Vec<Option<f64>> = named
        .iter()
        .find(|(n, _)| n == "probs")
        .map(|(_, v)| v)
        .or(args.get(1))
        .map(|v| match v {
            RValue::Vector(rv) => rv.to_doubles(),
            RValue::Null => vec![],
            _ => default_probs.clone(),
        })
        .unwrap_or(default_probs);

    // Get and sort the data
    let vals = match args.first() {
        Some(RValue::Vector(v)) => v.to_doubles(),
        _ => {
            return Err(RError::new(
                RErrorKind::Argument,
                "non-numeric argument to quantile".to_string(),
            ))
        }
    };

    // Filter NAs
    let mut data: Vec<f64> = if na_rm {
        vals.iter()
            .filter_map(|v| *v)
            .filter(|f| !f.is_nan())
            .collect()
    } else {
        // If any NA/NaN present without na.rm, check and error
        let mut d = Vec::with_capacity(vals.len());
        for v in &vals {
            match v {
                Some(f) if !f.is_nan() => d.push(*f),
                _ => {
                    return Err(RError::new(
                        RErrorKind::Argument,
                        "missing values and NaN's not allowed if 'na.rm' is FALSE".to_string(),
                    ));
                }
            }
        }
        d
    };

    data.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = data.len();

    // Compute quantiles using type 7
    let mut result_vals: Vec<Option<f64>> = Vec::with_capacity(probs.len());
    let mut names: Vec<Option<String>> = Vec::with_capacity(probs.len());

    for prob in &probs {
        match prob {
            Some(p) => {
                if !(0.0..=1.0).contains(p) {
                    return Err(RError::new(
                        RErrorKind::Argument,
                        format!("'probs' outside [0, 1], got {p}"),
                    ));
                }
                if n == 0 {
                    result_vals.push(None);
                } else if n == 1 {
                    result_vals.push(Some(data[0]));
                } else {
                    let h = (n - 1) as f64 * p;
                    let lo = h.floor() as usize;
                    let hi = h.ceil() as usize;
                    let frac = h - h.floor();
                    let val = data[lo] + frac * (data[hi] - data[lo]);
                    result_vals.push(Some(val));
                }
                // Format name: e.g. "0%", "25%", "50%", "75%", "100%"
                let pct = p * 100.0;
                let name = if pct == pct.floor() {
                    format!("{}%", pct as i64)
                } else {
                    format!("{pct}%")
                };
                names.push(Some(name));
            }
            None => {
                result_vals.push(None);
                names.push(None);
            }
        }
    }

    let mut rv = RVector::from(Vector::Double(result_vals.into()));
    rv.set_attr(
        "names".to_string(),
        RValue::vec(Vector::Character(names.into())),
    );
    Ok(RValue::Vector(rv))
}

/// Replicate elements to a specified length.
///
/// @param x a vector
/// @param length.out desired output length
/// @return vector of the specified length, recycling x as needed
#[builtin(name = "rep_len", min_args = 2)]
fn builtin_rep_len(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    if args.len() < 2 {
        return Err(RError::new(
            RErrorKind::Argument,
            "need 2 arguments".to_string(),
        ));
    }
    let length_out = args[1]
        .as_vector()
        .and_then(|v| v.as_integer_scalar())
        .unwrap_or(0);
    let length_out = usize::try_from(length_out).unwrap_or(0);
    match &args[0] {
        RValue::Vector(v) => {
            if v.is_empty() {
                return Ok(RValue::Vector(v.clone()));
            }
            match &v.inner {
                Vector::Raw(vals) => Ok(RValue::vec(Vector::Raw(
                    vals.iter().cycle().take(length_out).copied().collect(),
                ))),
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
                Vector::Complex(vals) => Ok(RValue::vec(Vector::Complex(
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
        _ => Err(RError::new(
            RErrorKind::Argument,
            "invalid argument".to_string(),
        )),
    }
}

/// Replicate elements a specified number of times.
///
/// @param x a vector
/// @param times number of times to repeat
/// @return vector with elements repeated
#[builtin(min_args = 2)]
fn builtin_rep_int(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    if args.len() < 2 {
        return Err(RError::new(
            RErrorKind::Argument,
            "need 2 arguments".to_string(),
        ));
    }
    let times = args[1]
        .as_vector()
        .and_then(|v| v.as_integer_scalar())
        .unwrap_or(1);
    let times = usize::try_from(times)?;
    match &args[0] {
        RValue::Vector(v) => match &v.inner {
            Vector::Raw(vals) => Ok(RValue::vec(Vector::Raw(
                vals.iter()
                    .cycle()
                    .take(vals.len() * times)
                    .copied()
                    .collect(),
            ))),
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
            Vector::Complex(vals) => Ok(RValue::vec(Vector::Complex(
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
        _ => Err(RError::new(
            RErrorKind::Argument,
            "invalid argument".to_string(),
        )),
    }
}

/// Convert an RValue to an ndarray Array2 (column-major)
#[cfg(feature = "linalg")]
fn rvalue_to_array2(val: &RValue) -> Result<Array2<f64>, RError> {
    let (data, dim_attr) = match val {
        RValue::Vector(rv) => (rv.to_doubles(), rv.get_attr("dim")),
        _ => {
            return Err(RError::new(
                RErrorKind::Type,
                "requires numeric matrix/vector arguments".to_string(),
            ))
        }
    };
    let (nrow, ncol) = match dim_attr {
        Some(RValue::Vector(rv)) => match &rv.inner {
            Vector::Integer(d) if d.len() >= 2 => (
                usize::try_from(d[0].unwrap_or(0)).unwrap_or(0),
                usize::try_from(d[1].unwrap_or(0)).unwrap_or(0),
            ),
            _ => (data.len(), 1),
        },
        _ => (data.len(), 1),
    };
    let flat: Vec<f64> = data.iter().map(|x| x.unwrap_or(f64::NAN)).collect();
    Array2::from_shape_vec((nrow, ncol).f(), flat)
        .map_err(|source| -> RError { MathError::Shape { source }.into() })
}

/// Convert an ndarray Array2 back to an RValue matrix
#[cfg(feature = "linalg")]
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
            vec![
                Some(i64::try_from(nrow).unwrap_or(0)),
                Some(i64::try_from(ncol).unwrap_or(0)),
            ]
            .into(),
        )),
    );
    RValue::Vector(rv)
}

/// Convert an RValue matrix to a nalgebra DMatrix (column-major — zero-copy reorder).
#[cfg(feature = "linalg")]
fn rvalue_to_dmatrix(val: &RValue) -> Result<DMatrix<f64>, RError> {
    let (data, dim_attr) = match val {
        RValue::Vector(rv) => (rv.to_doubles(), rv.get_attr("dim")),
        _ => {
            return Err(RError::new(
                RErrorKind::Type,
                "requires numeric matrix/vector arguments".to_string(),
            ))
        }
    };
    let (nrow, ncol) = match dim_attr {
        Some(RValue::Vector(rv)) => match &rv.inner {
            Vector::Integer(d) if d.len() >= 2 => (
                usize::try_from(d[0].unwrap_or(0)).unwrap_or(0),
                usize::try_from(d[1].unwrap_or(0)).unwrap_or(0),
            ),
            _ => (data.len(), 1),
        },
        _ => (data.len(), 1),
    };
    let flat: Vec<f64> = data.iter().map(|x| x.unwrap_or(f64::NAN)).collect();
    // nalgebra DMatrix stores data in column-major order, same as R
    Ok(DMatrix::from_vec(nrow, ncol, flat))
}

/// Convert a nalgebra DMatrix back to an RValue matrix.
#[cfg(feature = "linalg")]
fn dmatrix_to_rvalue(mat: &DMatrix<f64>) -> RValue {
    let nrow = mat.nrows();
    let ncol = mat.ncols();
    // nalgebra stores column-major, so as_slice() gives us the right order
    let data: Vec<Option<f64>> = mat.as_slice().iter().copied().map(Some).collect();
    let mut rv = RVector::from(Vector::Double(data.into()));
    rv.set_attr(
        "dim".to_string(),
        RValue::vec(Vector::Integer(
            vec![
                Some(i64::try_from(nrow).unwrap_or(0)),
                Some(i64::try_from(ncol).unwrap_or(0)),
            ]
            .into(),
        )),
    );
    RValue::Vector(rv)
}

fn matrix_dimnames(value: &RValue) -> MatrixDimNames {
    let dimnames = match value {
        RValue::Vector(rv) => rv.get_attr("dimnames"),
        RValue::List(list) => list.get_attr("dimnames"),
        _ => None,
    };

    let row_names = super::dimnames_component(dimnames, 0);
    let col_names = super::dimnames_component(dimnames, 1);
    (row_names, col_names)
}

fn set_matrix_attrs(
    rv: &mut RVector,
    nrow: usize,
    ncol: usize,
    row_names: Option<DimNameVec>,
    col_names: Option<DimNameVec>,
) -> Result<(), RError> {
    rv.set_attr(
        "class".to_string(),
        RValue::vec(Vector::Character(
            vec![Some("matrix".to_string()), Some("array".to_string())].into(),
        )),
    );
    rv.set_attr(
        "dim".to_string(),
        RValue::vec(Vector::Integer(
            vec![Some(i64::try_from(nrow)?), Some(i64::try_from(ncol)?)].into(),
        )),
    );
    if let Some(dimnames) =
        super::bind_dimnames_value(row_names.unwrap_or_default(), col_names.unwrap_or_default())
    {
        rv.set_attr("dimnames".to_string(), dimnames);
    }
    Ok(())
}

/// crossprod(x, y) = t(x) %*% y
#[cfg(feature = "linalg")]
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
    let mut out = match array2_to_rvalue(&result) {
        RValue::Vector(rv) => rv,
        _ => unreachable!(),
    };
    let (_, x_col_names) = matrix_dimnames(args.first().unwrap_or(&RValue::Null));
    let (_, y_col_names) = args.get(1).map_or((None, None), matrix_dimnames);
    set_matrix_attrs(
        &mut out,
        result.nrows(),
        result.ncols(),
        x_col_names,
        if args.get(1).is_some() {
            y_col_names
        } else {
            matrix_dimnames(args.first().unwrap_or(&RValue::Null)).1
        },
    )?;
    Ok(RValue::Vector(out))
}

/// tcrossprod(x, y) = x %*% t(y)
#[cfg(feature = "linalg")]
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
    let mut out = match array2_to_rvalue(&result) {
        RValue::Vector(rv) => rv,
        _ => unreachable!(),
    };
    let (x_row_names, _) = matrix_dimnames(args.first().unwrap_or(&RValue::Null));
    let (y_row_names, _) = args.get(1).map_or((None, None), matrix_dimnames);
    set_matrix_attrs(
        &mut out,
        result.nrows(),
        result.ncols(),
        x_row_names,
        if args.get(1).is_some() {
            y_row_names
        } else {
            matrix_dimnames(args.first().unwrap_or(&RValue::Null)).0
        },
    )?;
    Ok(RValue::Vector(out))
}

// region: norm, solve, outer

/// `norm(x, type = "O")` — matrix/vector norm.
///
/// Supported types: "O"/"1" (one-norm), "I" (infinity-norm), "F" (Frobenius), "M" (max modulus).
#[cfg(feature = "linalg")]
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
            return Err(RError::new(
                RErrorKind::Argument,
                format!(
                    "invalid norm type '{}'. Use \"O\" (one-norm), \"I\" (infinity-norm), \
                 \"F\" (Frobenius), or \"M\" (max modulus)",
                    other
                ),
            ));
        }
    };

    Ok(RValue::vec(Vector::Double(vec![Some(result)].into())))
}

/// `solve(a, b)` — solve linear system or compute matrix inverse via LU decomposition.
///
/// - `solve(a)`: returns the inverse of matrix a
/// - `solve(a, b)`: solves the linear system Ax = b
///
/// Uses nalgebra's LU decomposition with partial pivoting for numerical stability.
#[cfg(feature = "linalg")]
#[builtin(min_args = 1)]
fn builtin_solve(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let a = rvalue_to_dmatrix(args.first().unwrap_or(&RValue::Null))?;
    let nrow = a.nrows();
    let ncol = a.ncols();

    if nrow != ncol {
        return Err(RError::new(
            RErrorKind::Argument,
            format!(
                "solve() requires a square matrix, but got {}x{}. \
             Non-square systems need qr.solve() or a least-squares method",
                nrow, ncol
            ),
        ));
    }
    let n = nrow;

    if n == 0 {
        return Err(RError::new(
            RErrorKind::Argument,
            "solve() requires a non-empty matrix".to_string(),
        ));
    }

    let b_arg = named
        .iter()
        .find(|(name, _)| name == "b")
        .map(|(_, v)| v)
        .or(args.get(1));

    let b = match b_arg {
        Some(val) => rvalue_to_dmatrix(val)?,
        None => DMatrix::identity(n, n),
    };

    if b.nrows() != n {
        return Err(RError::new(
            RErrorKind::Argument,
            format!(
                "solve(a, b): nrow(a) = {} but nrow(b) = {} — they must match",
                n,
                b.nrows()
            ),
        ));
    }

    let lu = a.lu();
    let result = lu.solve(&b).ok_or_else(|| {
        RError::other(
            "solve(): matrix is singular (or very close to singular). \
             Check that your matrix has full rank — its determinant is effectively zero",
        )
    })?;

    Ok(dmatrix_to_rvalue(&result))
}

/// `det(x)` — matrix determinant via LU decomposition.
#[cfg(feature = "linalg")]
#[builtin(min_args = 1)]
fn builtin_det(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let a = rvalue_to_dmatrix(args.first().unwrap_or(&RValue::Null))?;
    let n = a.nrows();
    if n != a.ncols() {
        return Err(RError::new(
            RErrorKind::Argument,
            format!("'x' must be a square matrix, got {}x{}", n, a.ncols()),
        ));
    }
    if n == 0 {
        return Ok(RValue::vec(Vector::Double(vec![Some(1.0)].into())));
    }

    let det = a.lu().determinant();
    Ok(RValue::vec(Vector::Double(vec![Some(det)].into())))
}

/// `chol(x)` — Cholesky decomposition (upper triangular R such that x = R'R).
///
/// Uses nalgebra's Cholesky decomposition and returns the upper triangular factor.
#[cfg(feature = "linalg")]
#[builtin(min_args = 1)]
fn builtin_chol(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let a = rvalue_to_dmatrix(args.first().unwrap_or(&RValue::Null))?;
    let n = a.nrows();
    if n != a.ncols() {
        return Err(RError::new(
            RErrorKind::Argument,
            format!("'x' must be a square matrix, got {}x{}", n, a.ncols()),
        ));
    }

    // nalgebra's cholesky() computes L such that A = L L^T (lower triangular).
    // R's chol() returns the upper triangular R such that A = R^T R, so R = L^T.
    let chol = a.cholesky().ok_or_else(|| {
        RError::other(
            "matrix is not positive definite — Cholesky decomposition failed. \
             Ensure the matrix is symmetric and all eigenvalues are positive",
        )
    })?;
    let l = chol.l();
    let r = l.transpose();

    Ok(dmatrix_to_rvalue(&r))
}

// region: QR, SVD, Eigen decompositions

/// `qr(x)` — QR decomposition via nalgebra's column-pivoted QR.
///
/// Returns a list with class "qr" containing:
/// - `$qr`: the compact QR matrix (R in upper triangle)
/// - `$rank`: integer rank estimate
/// - `$pivot`: integer permutation vector (1-based)
/// - `$Q`: the orthogonal Q matrix (for qr.Q() access)
#[cfg(feature = "linalg")]
#[builtin(min_args = 1)]
fn builtin_qr(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let a = rvalue_to_dmatrix(args.first().unwrap_or(&RValue::Null))?;
    let m = a.nrows();
    let n = a.ncols();

    let qr = a.col_piv_qr();
    let q = qr.q();
    let r = qr.r();

    // Build compact QR representation: upper triangle = R, zeros below
    let k = m.min(n);
    let mut compact = DMatrix::<f64>::zeros(m, n);
    for i in 0..m {
        for j in 0..n {
            if i <= j {
                compact[(i, j)] = r[(i, j)];
            }
        }
    }
    let qr_val = dmatrix_to_rvalue(&compact);

    // Estimate rank from diagonal of R
    let tol = f64::EPSILON * (m.max(n) as f64) * {
        let mut max_diag = 0.0f64;
        for i in 0..k {
            max_diag = max_diag.max(r[(i, i)].abs());
        }
        max_diag
    };
    let mut rank = 0i64;
    for i in 0..k {
        if r[(i, i)].abs() > tol {
            rank += 1;
        }
    }

    // Pivot vector (1-based): apply the column permutation to get column ordering.
    // We apply the permutation to a row matrix of column indices.
    let p = qr.p();
    let mut pivot_mat = DMatrix::<f64>::zeros(1, n);
    for j in 0..n {
        pivot_mat[(0, j)] = j as f64;
    }
    p.permute_columns(&mut pivot_mat);
    let pivot: Vec<Option<i64>> = (0..n).map(|j| Some(pivot_mat[(0, j)] as i64 + 1)).collect();

    let q_val = dmatrix_to_rvalue(&q);

    let mut list = RList::new(vec![
        (Some("qr".to_string()), qr_val),
        (
            Some("rank".to_string()),
            RValue::vec(Vector::Integer(vec![Some(rank)].into())),
        ),
        (
            Some("pivot".to_string()),
            RValue::vec(Vector::Integer(pivot.into())),
        ),
        (Some("Q".to_string()), q_val),
    ]);
    list.set_attr(
        "class".to_string(),
        RValue::vec(Vector::Character(vec![Some("qr".to_string())].into())),
    );
    Ok(RValue::List(list))
}

/// `svd(x)` — Singular Value Decomposition via nalgebra's bidiagonal SVD.
///
/// Returns a list with:
/// - `$d`: numeric vector of singular values (descending)
/// - `$u`: left singular vectors (m x min(m,n) matrix)
/// - `$v`: right singular vectors (n x min(m,n) matrix)
#[cfg(feature = "linalg")]
#[builtin(min_args = 1)]
fn builtin_svd(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let a = rvalue_to_dmatrix(args.first().unwrap_or(&RValue::Null))?;
    let m = a.nrows();
    let n = a.ncols();

    if m == 0 || n == 0 {
        return Err(RError::new(
            RErrorKind::Argument,
            "0 extent dimensions are not allowed".to_string(),
        ));
    }

    let svd = a.svd(true, true);
    let k = m.min(n);

    // Singular values (already in descending order from nalgebra)
    let d_vals: Vec<Option<f64>> = svd.singular_values.iter().copied().map(Some).collect();

    // Left singular vectors: m x k
    let u_full = svd
        .u
        .ok_or_else(|| RError::other("svd(): failed to compute left singular vectors"))?;
    let u = u_full.columns(0, k).clone_owned();

    // Right singular vectors: n x k
    let v_t_full = svd
        .v_t
        .ok_or_else(|| RError::other("svd(): failed to compute right singular vectors"))?;
    let v = v_t_full.rows(0, k).transpose();

    Ok(RValue::List(RList::new(vec![
        (
            Some("d".to_string()),
            RValue::vec(Vector::Double(d_vals.into())),
        ),
        (Some("u".to_string()), dmatrix_to_rvalue(&u)),
        (Some("v".to_string()), dmatrix_to_rvalue(&v)),
    ])))
}

/// `eigen(x)` — Eigenvalue decomposition via nalgebra.
///
/// Returns a list with:
/// - `$values`: numeric vector of eigenvalues (descending by absolute value)
/// - `$vectors`: matrix of eigenvectors (columns)
///
/// Supports both symmetric and non-symmetric real matrices.
/// For symmetric matrices, uses nalgebra's `symmetric_eigen()` (faster, all-real).
/// For non-symmetric matrices, uses Schur decomposition to extract real eigenvalues,
/// or reports complex eigenvalues as an error (R returns complex values, which we
/// don't yet support in this context).
#[cfg(feature = "linalg")]
#[builtin(min_args = 1)]
fn builtin_eigen(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let a = rvalue_to_dmatrix(args.first().unwrap_or(&RValue::Null))?;
    let n = a.nrows();
    if n != a.ncols() {
        return Err(RError::new(
            RErrorKind::Argument,
            format!(
                "non-square matrix in 'eigen': {}x{} — eigen() requires a square matrix",
                n,
                a.ncols()
            ),
        ));
    }

    if n == 0 {
        return Ok(RValue::List(RList::new(vec![
            (
                Some("values".to_string()),
                RValue::vec(Vector::Double(vec![].into())),
            ),
            (
                Some("vectors".to_string()),
                dmatrix_to_rvalue(&DMatrix::<f64>::zeros(0, 0)),
            ),
        ])));
    }

    // Check symmetry
    let sym_tol = 1e-10;
    let mut is_symmetric = true;
    'sym_check: for i in 0..n {
        for j in (i + 1)..n {
            if (a[(i, j)] - a[(j, i)]).abs() > sym_tol * (a[(i, j)].abs() + a[(j, i)].abs() + 1.0) {
                is_symmetric = false;
                break 'sym_check;
            }
        }
    }

    if is_symmetric {
        // Use optimized symmetric eigendecomposition
        let eig = a.symmetric_eigen();

        // nalgebra returns eigenvalues in arbitrary order — sort descending
        let mut eigen_pairs: Vec<(f64, usize)> = eig
            .eigenvalues
            .iter()
            .copied()
            .enumerate()
            .map(|(i, v)| (v, i))
            .collect();
        eigen_pairs.sort_by(|a_pair, b_pair| {
            b_pair
                .0
                .partial_cmp(&a_pair.0)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let values: Vec<Option<f64>> = eigen_pairs.iter().map(|&(val, _)| Some(val)).collect();

        let mut vectors = DMatrix::<f64>::zeros(n, n);
        for (new_j, &(_, old_j)) in eigen_pairs.iter().enumerate() {
            for i in 0..n {
                vectors[(i, new_j)] = eig.eigenvectors[(i, old_j)];
            }
        }

        Ok(RValue::List(RList::new(vec![
            (
                Some("values".to_string()),
                RValue::vec(Vector::Double(values.into())),
            ),
            (Some("vectors".to_string()), dmatrix_to_rvalue(&vectors)),
        ])))
    } else {
        // Non-symmetric: use Schur decomposition to extract eigenvalues.
        // The real Schur form has eigenvalues on the diagonal (for real eigenvalues)
        // or in 2x2 blocks (for complex conjugate pairs).
        let schur = a.schur();
        let (u_mat, t_mat) = schur.unpack();

        // Extract eigenvalues from diagonal/2x2 blocks of the quasi-triangular T
        let mut values = Vec::new();
        let mut has_complex = false;
        let mut i = 0;
        while i < n {
            if i + 1 < n && t_mat[(i + 1, i)].abs() > 1e-10 {
                // 2x2 block: complex conjugate pair
                has_complex = true;
                i += 2;
            } else {
                values.push(t_mat[(i, i)]);
                i += 1;
            }
        }

        if has_complex {
            return Err(RError::other(
                "non-symmetric matrix has complex eigenvalues — \
                 complex eigenvalue support is not yet implemented. \
                 If the matrix should be symmetric, consider using (x + t(x))/2 \
                 to symmetrize it.",
            ));
        }

        // Sort descending by value
        values.sort_by(|a_val, b_val| {
            b_val
                .partial_cmp(a_val)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let vals: Vec<Option<f64>> = values.iter().copied().map(Some).collect();

        // Use the Schur vectors as approximate eigenvectors
        let vectors = u_mat;

        Ok(RValue::List(RList::new(vec![
            (
                Some("values".to_string()),
                RValue::vec(Vector::Double(vals.into())),
            ),
            (Some("vectors".to_string()), dmatrix_to_rvalue(&vectors)),
        ])))
    }
}

// endregion

/// `t(x)` — matrix transpose.
#[builtin(min_args = 1)]
fn builtin_t(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let x = args.first().unwrap_or(&RValue::Null);
    let rv = match x {
        RValue::Vector(rv) => rv,
        _ => {
            return Err(RError::new(
                RErrorKind::Type,
                "argument is not a matrix".to_string(),
            ))
        }
    };

    let (nrow, ncol) = match rv.get_attr("dim") {
        Some(RValue::Vector(dim_rv)) => {
            let dims = dim_rv.to_integers();
            if dims.len() >= 2 {
                (
                    usize::try_from(dims[0].unwrap_or(0))?,
                    usize::try_from(dims[1].unwrap_or(0))?,
                )
            } else {
                // Treat as column vector
                (rv.inner.len(), 1)
            }
        }
        _ => (rv.inner.len(), 1), // Treat as column vector
    };

    // Build transposed flat indices: original[i,j] = flat[j*nrow + i] → transposed[j,i] = flat[i*ncol + j]
    let indices: Vec<usize> = (0..nrow)
        .flat_map(|i| (0..ncol).map(move |j| j * nrow + i))
        .collect();
    let transposed = rv.inner.select_indices(&indices);

    let mut out = RVector::from(transposed);

    // Swap dimnames
    let (row_names, col_names) = matrix_dimnames(x);
    set_matrix_attrs(&mut out, ncol, nrow, col_names, row_names)?;
    Ok(RValue::Vector(out))
}

// endregion

// region: Complex number builtins

/// Construct complex numbers from real and imaginary parts.
///
/// @param real numeric vector of real parts
/// @param imaginary numeric vector of imaginary parts
/// @param length.out desired output length
/// @return complex vector
#[builtin(name = "complex")]
fn builtin_complex(_args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let real = named
        .iter()
        .find(|(n, _)| n == "real")
        .and_then(|(_, v)| v.as_vector())
        .map(|v| v.to_doubles())
        .unwrap_or_default();

    let imaginary = named
        .iter()
        .find(|(n, _)| n == "imaginary")
        .and_then(|(_, v)| v.as_vector())
        .map(|v| v.to_doubles())
        .unwrap_or_default();

    let length_out = match named
        .iter()
        .find(|(n, _)| n == "length.out")
        .and_then(|(_, v)| v.as_vector()?.as_integer_scalar())
    {
        Some(n) => usize::try_from(n)?,
        None => real.len().max(imaginary.len()),
    };

    let result: Vec<Option<num_complex::Complex64>> = (0..length_out)
        .map(|i| {
            let re = real
                .get(i % real.len().max(1))
                .copied()
                .flatten()
                .unwrap_or(0.0);
            let im = imaginary
                .get(i % imaginary.len().max(1))
                .copied()
                .flatten()
                .unwrap_or(0.0);
            Some(num_complex::Complex64::new(re, im))
        })
        .collect();

    Ok(RValue::vec(Vector::Complex(result.into())))
}

/// Extract the real part of complex numbers.
///
/// @param z complex or numeric vector
/// @return numeric vector of real parts
#[builtin(name = "Re", min_args = 1)]
fn builtin_re(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let v = args
        .first()
        .and_then(|v| v.as_vector())
        .ok_or_else(|| RError::new(RErrorKind::Type, "non-numeric argument to Re".to_string()))?;
    match v {
        Vector::Complex(vals) => {
            let result: Vec<Option<f64>> = vals.iter().map(|x| x.map(|c| c.re)).collect();
            Ok(RValue::vec(Vector::Double(result.into())))
        }
        _ => {
            // For non-complex, Re is identity (the real part of a real number is itself)
            Ok(RValue::vec(Vector::Double(v.to_doubles().into())))
        }
    }
}

/// Extract the imaginary part of complex numbers.
///
/// @param z complex or numeric vector
/// @return numeric vector of imaginary parts (0 for reals)
#[builtin(name = "Im", min_args = 1)]
fn builtin_im(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let v = args
        .first()
        .and_then(|v| v.as_vector())
        .ok_or_else(|| RError::new(RErrorKind::Type, "non-numeric argument to Im".to_string()))?;
    match v {
        Vector::Complex(vals) => {
            let result: Vec<Option<f64>> = vals.iter().map(|x| x.map(|c| c.im)).collect();
            Ok(RValue::vec(Vector::Double(result.into())))
        }
        _ => {
            // For non-complex, Im is 0
            let result: Vec<Option<f64>> = vec![Some(0.0); v.len()];
            Ok(RValue::vec(Vector::Double(result.into())))
        }
    }
}

/// Modulus (absolute value) of complex numbers.
///
/// @param z complex or numeric vector
/// @return numeric vector of moduli
#[builtin(name = "Mod", min_args = 1)]
fn builtin_mod_complex(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let v = args
        .first()
        .and_then(|v| v.as_vector())
        .ok_or_else(|| RError::new(RErrorKind::Type, "non-numeric argument to Mod".to_string()))?;
    match v {
        Vector::Complex(vals) => {
            let result: Vec<Option<f64>> = vals.iter().map(|x| x.map(|c| c.norm())).collect();
            Ok(RValue::vec(Vector::Double(result.into())))
        }
        _ => {
            // For non-complex, Mod is abs
            let result: Vec<Option<f64>> = v.to_doubles().iter().map(|x| x.map(f64::abs)).collect();
            Ok(RValue::vec(Vector::Double(result.into())))
        }
    }
}

/// Argument (phase angle) of complex numbers, in radians.
///
/// @param z complex or numeric vector
/// @return numeric vector of arguments (0 for non-negative reals, pi for negative)
#[builtin(name = "Arg", min_args = 1)]
fn builtin_arg(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let v = args
        .first()
        .and_then(|v| v.as_vector())
        .ok_or_else(|| RError::new(RErrorKind::Type, "non-numeric argument to Arg".to_string()))?;
    match v {
        Vector::Complex(vals) => {
            let result: Vec<Option<f64>> = vals.iter().map(|x| x.map(|c| c.arg())).collect();
            Ok(RValue::vec(Vector::Double(result.into())))
        }
        _ => {
            // For non-negative reals Arg is 0, for negative reals it's pi
            let result: Vec<Option<f64>> = v
                .to_doubles()
                .iter()
                .map(|x| x.map(|f| if f >= 0.0 { 0.0 } else { std::f64::consts::PI }))
                .collect();
            Ok(RValue::vec(Vector::Double(result.into())))
        }
    }
}

/// Complex conjugate.
///
/// @param z complex or numeric vector
/// @return complex vector of conjugates (identity for reals)
#[builtin(name = "Conj", min_args = 1)]
fn builtin_conj(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let v = args
        .first()
        .and_then(|v| v.as_vector())
        .ok_or_else(|| RError::new(RErrorKind::Type, "non-numeric argument to Conj".to_string()))?;
    match v {
        Vector::Complex(vals) => {
            let result: Vec<Option<num_complex::Complex64>> =
                vals.iter().map(|x| x.map(|c| c.conj())).collect();
            Ok(RValue::vec(Vector::Complex(result.into())))
        }
        _ => {
            // For non-complex, Conj is identity
            Ok(args[0].clone())
        }
    }
}

/// Test if an object is of complex type.
///
/// @param x any R value
/// @return scalar logical
#[builtin(name = "is.complex", min_args = 1)]
fn builtin_is_complex(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let result = matches!(
        args.first(),
        Some(RValue::Vector(rv)) if matches!(rv.inner, Vector::Complex(_))
    );
    Ok(RValue::vec(Vector::Logical(vec![Some(result)].into())))
}

/// Coerce to complex type.
///
/// @param x numeric or complex vector
/// @return complex vector
#[builtin(name = "as.complex", min_args = 1)]
fn builtin_as_complex(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let v = args
        .first()
        .and_then(|v| v.as_vector())
        .ok_or_else(|| RError::new(RErrorKind::Type, "cannot coerce to complex".to_string()))?;
    match v {
        Vector::Complex(vals) => Ok(RValue::vec(Vector::Complex(vals.clone()))),
        _ => {
            let result: Vec<Option<num_complex::Complex64>> = v
                .to_doubles()
                .iter()
                .map(|x| x.map(|f| num_complex::Complex64::new(f, 0.0)))
                .collect();
            Ok(RValue::vec(Vector::Complex(result.into())))
        }
    }
}
// endregion

// region: Linear models (lm, summary.lm, coef)

#[cfg(feature = "linalg")]
/// Extract a named column from a data frame (RList) as a Vec<Option<f64>>.
fn df_column_doubles(list: &RList, name: &str) -> Result<Vec<Option<f64>>, RError> {
    for (col_name, col_val) in &list.values {
        if col_name.as_deref() == Some(name) {
            return match col_val {
                RValue::Vector(rv) => Ok(rv.to_doubles()),
                _ => Err(RError::new(
                    RErrorKind::Type,
                    format!(
                        "column '{}' is not a numeric vector — lm() requires numeric columns",
                        name
                    ),
                )),
            };
        }
    }
    Err(RError::new(
        RErrorKind::Name,
        format!(
            "column '{}' not found in data frame. Available columns: {}",
            name,
            list.values
                .iter()
                .filter_map(|(n, _)| n.as_deref())
                .join(", ")
        ),
    ))
}

#[cfg(feature = "linalg")]
/// Extract the response and predictor names from a formula expression.
///
/// For `y ~ x`, returns `("y", vec!["x"])`.
/// For `y ~ x1 + x2`, returns `("y", vec!["x1", "x2"])`.
fn parse_formula_terms(expr: &Expr) -> Result<(String, Vec<String>), RError> {
    match expr {
        Expr::Formula { lhs, rhs } => {
            let response = lhs
                .as_ref()
                .and_then(|e| match e.as_ref() {
                    Expr::Symbol(s) => Some(s.clone()),
                    _ => None,
                })
                .ok_or_else(|| {
                    RError::new(
                        RErrorKind::Argument,
                        "lm() formula must have a response variable on the left side of ~"
                            .to_string(),
                    )
                })?;
            let rhs_expr = rhs.as_ref().ok_or_else(|| {
                RError::new(
                    RErrorKind::Argument,
                    "lm() formula must have predictor(s) on the right side of ~".to_string(),
                )
            })?;
            let mut predictors = Vec::new();
            collect_additive_terms(rhs_expr, &mut predictors)?;
            if predictors.is_empty() {
                return Err(RError::new(
                    RErrorKind::Argument,
                    "lm() formula must have at least one predictor on the right side of ~"
                        .to_string(),
                ));
            }
            Ok((response, predictors))
        }
        _ => Err(RError::new(
            RErrorKind::Argument,
            "lm() requires a formula as its first argument (e.g. y ~ x)".to_string(),
        )),
    }
}

#[cfg(feature = "linalg")]
/// Recursively collect symbol names from additive terms (x1 + x2 + x3).
fn collect_additive_terms(expr: &Expr, out: &mut Vec<String>) -> Result<(), RError> {
    match expr {
        Expr::Symbol(s) => {
            out.push(s.clone());
            Ok(())
        }
        Expr::BinaryOp {
            op: crate::parser::ast::BinaryOp::Add,
            lhs,
            rhs,
        } => {
            collect_additive_terms(lhs, out)?;
            collect_additive_terms(rhs, out)?;
            Ok(())
        }
        _ => Err(RError::new(
            RErrorKind::Argument,
            format!(
                "lm() formula terms must be symbols or sums of symbols, got: {:?}",
                expr
            ),
        )),
    }
}

/// Fit a linear model using ordinary least squares.
///
/// Supports simple and multiple linear regression with formula syntax.
/// The formula `y ~ x` fits a simple regression; `y ~ x1 + x2` fits
/// multiple regression. An intercept is always included.
///
/// @param formula a formula specifying the model (e.g. y ~ x)
/// @param data a data frame containing the variables in the formula
/// @return a list of class "lm" with components: coefficients, residuals,
///         fitted.values, and call
#[cfg(feature = "linalg")]
#[interpreter_builtin(min_args = 1, namespace = "stats")]
fn interp_lm(
    args: &[RValue],
    named: &[(String, RValue)],
    _context: &BuiltinContext,
) -> Result<RValue, RError> {
    // Extract formula (first arg or named "formula")
    let formula_val = super::find_arg(args, named, "formula", 0).ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "lm() requires a formula argument".to_string(),
        )
    })?;

    let formula_expr = match formula_val {
        RValue::Language(lang) => &*lang.inner,
        _ => {
            return Err(RError::new(
                RErrorKind::Type,
                "lm() first argument must be a formula (e.g. y ~ x), got a non-formula value"
                    .to_string(),
            ))
        }
    };

    let (response_name, predictor_names) = parse_formula_terms(formula_expr)?;

    // Extract data frame (second arg or named "data")
    let data_val = super::find_arg(args, named, "data", 1).ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "lm() requires a 'data' argument — pass a data frame containing the model variables"
                .to_string(),
        )
    })?;

    let data_list = match data_val {
        RValue::List(list) => list,
        _ => {
            return Err(RError::new(
                RErrorKind::Type,
                "lm() 'data' argument must be a data frame".to_string(),
            ))
        }
    };

    // Extract response vector
    let y_raw = df_column_doubles(data_list, &response_name)?;
    let n = y_raw.len();
    if n == 0 {
        return Err(RError::new(
            RErrorKind::Argument,
            "lm() requires at least one observation".to_string(),
        ));
    }

    let p = predictor_names.len(); // number of predictors (not counting intercept)

    // Check for NA values in response
    let y: Vec<f64> = y_raw
        .into_iter()
        .map(|v| {
            v.ok_or_else(|| {
                RError::new(
                    RErrorKind::Argument,
                    format!(
                        "NA values in response variable '{}' — lm() does not yet support na.action",
                        response_name
                    ),
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    // Build design matrix X: n x (p+1), first column is intercept (all 1s)
    let ncol = p + 1;
    let mut x_data = vec![0.0; n * ncol];
    // Column 0: intercept
    for item in x_data.iter_mut().take(n) {
        *item = 1.0; // column-major: element (i, 0)
    }
    // Columns 1..=p: predictors
    for (j, pred_name) in predictor_names.iter().enumerate() {
        let col_raw = df_column_doubles(data_list, pred_name)?;
        if col_raw.len() != n {
            return Err(RError::new(
                RErrorKind::Argument,
                format!(
                    "predictor '{}' has {} observations but response '{}' has {} — they must match",
                    pred_name,
                    col_raw.len(),
                    response_name,
                    n
                ),
            ));
        }
        for (i, val) in col_raw.into_iter().enumerate() {
            let v = val.ok_or_else(|| {
                RError::new(
                    RErrorKind::Argument,
                    format!(
                        "NA values in predictor '{}' — lm() does not yet support na.action",
                        pred_name
                    ),
                )
            })?;
            x_data[i + (j + 1) * n] = v; // column-major: element (i, j+1)
        }
    }

    let x = Array2::from_shape_vec((n, ncol).f(), x_data)
        .map_err(|source| -> RError { MathError::Shape { source }.into() })?;
    let y_arr = Array1::from_vec(y);

    // Compute beta = (X'X)^{-1} X'y via normal equations
    let xt = x.t();
    let xtx = xt.dot(&x);
    let xty = xt.dot(&y_arr);

    // Solve X'X * beta = X'y using Gaussian elimination
    let beta = solve_linear_system(&xtx, &xty)?;

    // Compute fitted values and residuals
    let fitted: Vec<f64> = (0..n)
        .map(|i| {
            let mut val = 0.0;
            for j in 0..ncol {
                val += x[[i, j]] * beta[j];
            }
            val
        })
        .collect();

    let residuals: Vec<f64> = (0..n).map(|i| y_arr[i] - fitted[i]).collect();

    // Build coefficient names: (Intercept), pred1, pred2, ...
    let mut coef_names: Vec<Option<String>> = Vec::with_capacity(ncol);
    coef_names.push(Some("(Intercept)".to_string()));
    for name in &predictor_names {
        coef_names.push(Some(name.clone()));
    }

    // Build named coefficient vector
    let coef_doubles: Vec<Option<f64>> = beta.iter().copied().map(Some).collect();
    let mut coef_rv = RVector::from(Vector::Double(coef_doubles.into()));
    coef_rv.set_attr(
        "names".to_string(),
        RValue::vec(Vector::Character(coef_names.into())),
    );

    // Build fitted.values vector
    let fitted_doubles: Vec<Option<f64>> = fitted.into_iter().map(Some).collect();

    // Build residuals vector
    let residual_doubles: Vec<Option<f64>> = residuals.into_iter().map(Some).collect();

    // Build result list with class "lm"
    let mut result = RList::new(vec![
        (Some("coefficients".to_string()), RValue::Vector(coef_rv)),
        (
            Some("residuals".to_string()),
            RValue::vec(Vector::Double(residual_doubles.into())),
        ),
        (
            Some("fitted.values".to_string()),
            RValue::vec(Vector::Double(fitted_doubles.into())),
        ),
        (Some("call".to_string()), RValue::Null),
    ]);
    result.set_attr(
        "class".to_string(),
        RValue::vec(Vector::Character(vec![Some("lm".to_string())].into())),
    );

    Ok(RValue::List(result))
}

#[cfg(feature = "linalg")]
/// Solve a linear system A * x = b using Gaussian elimination with partial pivoting.
/// A must be square (ncol x ncol), b must have length ncol.
fn solve_linear_system(a: &Array2<f64>, b: &Array1<f64>) -> Result<Vec<f64>, RError> {
    let n = a.nrows();
    if n != a.ncols() || n != b.len() {
        return Err(RError::new(
            RErrorKind::Other,
            "internal error: solve_linear_system dimension mismatch".to_string(),
        ));
    }
    if n == 0 {
        return Ok(vec![]);
    }

    // Build augmented matrix [A | b]
    let mut aug = Array2::<f64>::zeros((n, n + 1));
    for i in 0..n {
        for j in 0..n {
            aug[[i, j]] = a[[i, j]];
        }
        aug[[i, n]] = b[i];
    }

    // Forward elimination with partial pivoting
    for col in 0..n {
        // Find pivot
        let mut max_row = col;
        let mut max_val = aug[[col, col]].abs();
        for row in (col + 1)..n {
            let val = aug[[row, col]].abs();
            if val > max_val {
                max_val = val;
                max_row = row;
            }
        }

        if max_val < 1e-12 {
            return Err(RError::new(
                RErrorKind::Other,
                "lm() design matrix is singular or nearly singular — \
                 check for collinear predictors or constant columns"
                    .to_string(),
            ));
        }

        // Swap rows
        if max_row != col {
            for k in 0..=n {
                let tmp = aug[[col, k]];
                aug[[col, k]] = aug[[max_row, k]];
                aug[[max_row, k]] = tmp;
            }
        }

        // Eliminate below
        for row in (col + 1)..n {
            let factor = aug[[row, col]] / aug[[col, col]];
            for k in col..=n {
                aug[[row, k]] -= factor * aug[[col, k]];
            }
        }
    }

    // Back substitution
    let mut x = vec![0.0; n];
    for i in (0..n).rev() {
        let mut sum = aug[[i, n]];
        for j in (i + 1)..n {
            sum -= aug[[i, j]] * x[j];
        }
        x[i] = sum / aug[[i, i]];
    }

    Ok(x)
}

/// Print a summary of a linear model.
///
/// Displays the coefficients table for an lm object.
///
/// @param object an lm object (result of lm())
/// @return the object, invisibly
#[cfg(feature = "linalg")]
#[interpreter_builtin(name = "summary.lm", min_args = 1, namespace = "stats")]
fn interp_summary_lm(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let obj = args.first().ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "summary.lm() requires an lm object".to_string(),
        )
    })?;

    let list = match obj {
        RValue::List(l) => l,
        _ => {
            return Err(RError::new(
                RErrorKind::Type,
                "summary.lm() requires an lm object (a list with class 'lm')".to_string(),
            ))
        }
    };

    // Extract coefficients
    let coefs = list
        .values
        .iter()
        .find(|(n, _)| n.as_deref() == Some("coefficients"))
        .map(|(_, v)| v);

    context.write("Call:\nlm(formula = ...)\n\n");

    if let Some(RValue::Vector(rv)) = coefs {
        let values = rv.to_doubles();
        let names: Vec<String> = rv
            .get_attr("names")
            .and_then(|n| match n {
                RValue::Vector(nv) => match &nv.inner {
                    Vector::Character(c) => {
                        Some(c.iter().map(|s| s.clone().unwrap_or_default()).collect())
                    }
                    _ => None,
                },
                _ => None,
            })
            .unwrap_or_default();

        context.write("Coefficients:\n");
        let max_name_len = names.iter().map(|n| n.len()).max().unwrap_or(0).max(8);
        context.write(&format!("{:>width$}  Estimate\n", "", width = max_name_len));
        for (i, val) in values.iter().enumerate() {
            let name = names.get(i).map(|s| s.as_str()).unwrap_or("???");
            let estimate = val.map_or("NA".to_string(), |v| format!("{:.6}", v));
            context.write(&format!(
                "{:>width$}  {}\n",
                name,
                estimate,
                width = max_name_len
            ));
        }
    }

    // Extract residuals for a brief summary
    let residuals = list
        .values
        .iter()
        .find(|(n, _)| n.as_deref() == Some("residuals"))
        .map(|(_, v)| v);

    if let Some(RValue::Vector(rv)) = residuals {
        let vals: Vec<f64> = rv.to_doubles().into_iter().flatten().collect();
        if !vals.is_empty() {
            let mut sorted = vals.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let min = sorted[0];
            let max = sorted[sorted.len() - 1];
            let median = if sorted.len().is_multiple_of(2) {
                (sorted[sorted.len() / 2 - 1] + sorted[sorted.len() / 2]) / 2.0
            } else {
                sorted[sorted.len() / 2]
            };
            context.write(&format!(
                "\nResiduals: Min = {:.4}, Median = {:.4}, Max = {:.4}\n",
                min, median, max
            ));
        }
    }

    Ok(obj.clone())
}

/// Extract coefficients from a model object.
///
/// Extracts the `$coefficients` component from a fitted model (e.g. lm).
///
/// @param object a fitted model object with a coefficients component
/// @return a named numeric vector of coefficients
#[cfg(feature = "linalg")]
#[builtin(min_args = 1, namespace = "stats")]
fn builtin_coef(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let obj = args.first().ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "coef() requires a model object".to_string(),
        )
    })?;

    match obj {
        RValue::List(list) => {
            for (name, val) in &list.values {
                if name.as_deref() == Some("coefficients") {
                    return Ok(val.clone());
                }
            }
            Err(RError::new(
                RErrorKind::Name,
                "object does not have a 'coefficients' component".to_string(),
            ))
        }
        _ => Err(RError::new(
            RErrorKind::Type,
            "coef() requires a list-like model object (e.g. result of lm())".to_string(),
        )),
    }
}

// endregion

// region: arrayInd

/// Convert linear indices to array (row, col) subscripts.
///
/// Given a vector of 1-based linear indices and a dimension vector,
/// returns a matrix of subscripts (one row per index, one column per dim).
///
/// @param ind integer vector of linear indices (1-based)
/// @param .dim integer vector of dimensions (e.g. c(nrow, ncol))
/// @return integer matrix of subscripts
#[builtin(name = "arrayInd", min_args = 2)]
fn builtin_array_ind(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let indices = match args.first() {
        Some(RValue::Vector(rv)) => rv.to_doubles(),
        _ => {
            return Err(RError::new(
                RErrorKind::Argument,
                "'ind' must be a numeric vector".to_string(),
            ))
        }
    };
    let dims = match args.get(1) {
        Some(RValue::Vector(rv)) => rv.to_doubles(),
        _ => {
            return Err(RError::new(
                RErrorKind::Argument,
                "'.dim' must be a numeric vector".to_string(),
            ))
        }
    };

    if dims.len() < 2 {
        return Err(RError::new(
            RErrorKind::Argument,
            "'.dim' must have at least 2 elements for matrix subscripts".to_string(),
        ));
    }

    let dim_sizes: Vec<i64> = dims
        .iter()
        .map(|d| match d {
            Some(v) => Ok(*v as i64),
            None => Err(RError::new(
                RErrorKind::Argument,
                "NA in '.dim'".to_string(),
            )),
        })
        .collect::<Result<_, _>>()?;

    let ndim = dims.len();
    let n = indices.len();
    // Result stored column-major: all rows for dim1, then all rows for dim2, ...
    let mut result: Vec<Option<i64>> = vec![None; n * ndim];

    for (idx_pos, ind_val) in indices.iter().enumerate() {
        match ind_val {
            Some(v) => {
                let mut linear = *v as i64 - 1; // convert to 0-based
                for d in 0..ndim {
                    let subscript = linear % dim_sizes[d];
                    result[d * n + idx_pos] = Some(subscript + 1); // back to 1-based
                    linear /= dim_sizes[d];
                }
            }
            None => {
                for d in 0..ndim {
                    result[d * n + idx_pos] = None;
                }
            }
        }
    }

    let mut rv = RVector::from(Vector::Integer(result.into()));
    rv.set_attr(
        "dim".to_string(),
        RValue::vec(Vector::Integer(
            vec![
                Some(i64::try_from(n).unwrap_or(0)),
                Some(i64::try_from(ndim).unwrap_or(0)),
            ]
            .into(),
        )),
    );

    Ok(RValue::Vector(rv))
}

// endregion

// region: Row/column aggregation (rowSums, colSums, rowMeans, colMeans)

/// Helper: extract matrix dimensions and data as doubles.
fn matrix_dims_and_data(args: &[RValue]) -> Option<(usize, usize, Vec<Option<f64>>)> {
    let v = match args.first()? {
        RValue::Vector(v) => v,
        RValue::List(list) => {
            // Data frame: ncol = number of list elements, nrow = length of first
            let ncol = list.values.len();
            if ncol == 0 {
                return Some((0, 0, vec![]));
            }
            let nrow = list
                .values
                .first()
                .map(|(_, v)| match v {
                    RValue::Vector(rv) => rv.len(),
                    _ => 1,
                })
                .unwrap_or(0);
            let mut data = Vec::with_capacity(nrow * ncol);
            for (_, val) in &list.values {
                if let Some(v) = val.as_vector() {
                    data.extend(v.to_doubles());
                } else {
                    data.extend(std::iter::repeat_n(None, nrow));
                }
            }
            return Some((nrow, ncol, data));
        }
        _ => return None,
    };
    let dim = v.get_attr("dim")?;
    let dim_vec = dim.as_vector()?;
    let dims = dim_vec.to_integers();
    if dims.len() != 2 {
        return None;
    }
    let nrow = dims[0].unwrap_or(0) as usize;
    let ncol = dims[1].unwrap_or(0) as usize;
    Some((nrow, ncol, v.to_doubles()))
}

/// Sum of each row of a matrix or data frame.
///
/// @param x numeric matrix or data frame
/// @param na.rm logical: remove NAs before summing?
/// @return numeric vector of length nrow(x)
/// @namespace base
#[builtin(name = "rowSums", min_args = 1)]
fn builtin_row_sums(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let (nrow, ncol, data) = matrix_dims_and_data(args).ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "'x' must be a matrix or data frame".to_string(),
        )
    })?;
    let na_rm = named
        .iter()
        .find(|(n, _)| n == "na.rm")
        .and_then(|(_, v)| v.as_vector()?.as_logical_scalar())
        .unwrap_or(false);
    let mut result = Vec::with_capacity(nrow);
    for i in 0..nrow {
        let mut sum = 0.0;
        let mut has_na = false;
        for j in 0..ncol {
            match data.get(j * nrow + i).copied().flatten() {
                Some(v) => sum += v,
                None if !na_rm => {
                    has_na = true;
                    break;
                }
                _ => {}
            }
        }
        result.push(if has_na { None } else { Some(sum) });
    }
    Ok(RValue::vec(Vector::Double(result.into())))
}

/// Sum of each column of a matrix or data frame.
///
/// @param x numeric matrix or data frame
/// @param na.rm logical: remove NAs?
/// @return numeric vector of length ncol(x)
/// @namespace base
#[builtin(name = "colSums", min_args = 1)]
fn builtin_col_sums(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let (nrow, ncol, data) = matrix_dims_and_data(args).ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "'x' must be a matrix or data frame".to_string(),
        )
    })?;
    let na_rm = named
        .iter()
        .find(|(n, _)| n == "na.rm")
        .and_then(|(_, v)| v.as_vector()?.as_logical_scalar())
        .unwrap_or(false);
    let mut result = Vec::with_capacity(ncol);
    for j in 0..ncol {
        let mut sum = 0.0;
        let mut has_na = false;
        for i in 0..nrow {
            match data.get(j * nrow + i).copied().flatten() {
                Some(v) => sum += v,
                None if !na_rm => {
                    has_na = true;
                    break;
                }
                _ => {}
            }
        }
        result.push(if has_na { None } else { Some(sum) });
    }
    Ok(RValue::vec(Vector::Double(result.into())))
}

/// Mean of each row of a matrix or data frame.
///
/// @param x numeric matrix or data frame
/// @param na.rm logical: remove NAs?
/// @return numeric vector of length nrow(x)
/// @namespace base
#[builtin(name = "rowMeans", min_args = 1)]
fn builtin_row_means(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let (nrow, ncol, data) = matrix_dims_and_data(args).ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "'x' must be a matrix or data frame".to_string(),
        )
    })?;
    let na_rm = named
        .iter()
        .find(|(n, _)| n == "na.rm")
        .and_then(|(_, v)| v.as_vector()?.as_logical_scalar())
        .unwrap_or(false);
    let mut result = Vec::with_capacity(nrow);
    for i in 0..nrow {
        let mut sum = 0.0;
        let mut count = 0usize;
        let mut has_na = false;
        for j in 0..ncol {
            match data.get(j * nrow + i).copied().flatten() {
                Some(v) => {
                    sum += v;
                    count += 1;
                }
                None if !na_rm => {
                    has_na = true;
                    break;
                }
                _ => {}
            }
        }
        result.push(if has_na || count == 0 {
            None
        } else {
            Some(sum / count as f64)
        });
    }
    Ok(RValue::vec(Vector::Double(result.into())))
}

/// Mean of each column of a matrix or data frame.
///
/// @param x numeric matrix or data frame
/// @param na.rm logical: remove NAs?
/// @return numeric vector of length ncol(x)
/// @namespace base
#[builtin(name = "colMeans", min_args = 1)]
fn builtin_col_means(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let (nrow, ncol, data) = matrix_dims_and_data(args).ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "'x' must be a matrix or data frame".to_string(),
        )
    })?;
    let na_rm = named
        .iter()
        .find(|(n, _)| n == "na.rm")
        .and_then(|(_, v)| v.as_vector()?.as_logical_scalar())
        .unwrap_or(false);
    let mut result = Vec::with_capacity(ncol);
    for j in 0..ncol {
        let mut sum = 0.0;
        let mut count = 0usize;
        let mut has_na = false;
        for i in 0..nrow {
            match data.get(j * nrow + i).copied().flatten() {
                Some(v) => {
                    sum += v;
                    count += 1;
                }
                None if !na_rm => {
                    has_na = true;
                    break;
                }
                _ => {}
            }
        }
        result.push(if has_na || count == 0 {
            None
        } else {
            Some(sum / count as f64)
        });
    }
    Ok(RValue::vec(Vector::Double(result.into())))
}

/// Drop unused factor levels.
///
/// @param x factor
/// @return factor with unused levels removed
/// @namespace base
#[builtin(min_args = 1)]
fn builtin_droplevels(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(v)) => {
            let codes = v.inner.to_integers();
            let levels = match v.get_attr("levels") {
                Some(RValue::Vector(lv)) => lv.to_characters(),
                _ => return Ok(RValue::Vector(v.clone())),
            };
            let used: std::collections::BTreeSet<usize> = codes
                .iter()
                .filter_map(|c| c.and_then(|i| usize::try_from(i).ok()))
                .collect();
            let mut new_levels = Vec::new();
            let mut old_to_new = std::collections::HashMap::new();
            for (old_idx, level) in levels.iter().enumerate() {
                if used.contains(&(old_idx + 1)) {
                    old_to_new.insert(old_idx + 1, new_levels.len() + 1);
                    new_levels.push(level.clone());
                }
            }
            let new_codes: Vec<Option<i64>> = codes
                .iter()
                .map(|c| c.and_then(|i| old_to_new.get(&(i as usize)).map(|&new| new as i64)))
                .collect();
            let mut rv = RVector::from(Vector::Integer(new_codes.into()));
            rv.set_attr(
                "levels".to_string(),
                RValue::vec(Vector::Character(new_levels.into())),
            );
            rv.set_attr(
                "class".to_string(),
                RValue::vec(Vector::Character(vec![Some("factor".to_string())].into())),
            );
            Ok(RValue::Vector(rv))
        }
        _ => Ok(args.first().cloned().unwrap_or(RValue::Null)),
    }
}

// region: sweep and kronecker

/// Sweep a summary statistic from each row or column of a matrix.
///
/// @param x a numeric matrix
/// @param MARGIN 1 for rows, 2 for columns
/// @param STATS a numeric vector of statistics to sweep out
/// @param FUN the function to use: "-" (default), "+", "*", "/"
/// @return a matrix of the same dimensions as x
/// @namespace base
#[builtin(min_args = 3)]
fn builtin_sweep(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let (nrow, ncol, data) = matrix_dims_and_data(args).ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "'x' must be a matrix or data frame".to_string(),
        )
    })?;

    // MARGIN: positional arg[1] or named
    let margin_val = named
        .iter()
        .find(|(n, _)| n == "MARGIN")
        .map(|(_, v)| v)
        .or_else(|| args.get(1));
    let margin = match margin_val {
        Some(RValue::Vector(v)) => v
            .as_double_scalar()
            .or_else(|| v.as_integer_scalar().map(|i| i as f64))
            .ok_or_else(|| {
                RError::new(
                    RErrorKind::Argument,
                    "MARGIN must be 1 (rows) or 2 (columns)".to_string(),
                )
            })? as usize,
        _ => {
            return Err(RError::new(
                RErrorKind::Argument,
                "MARGIN must be 1 (rows) or 2 (columns)".to_string(),
            ))
        }
    };
    if margin != 1 && margin != 2 {
        return Err(RError::new(
            RErrorKind::Argument,
            format!("MARGIN must be 1 (rows) or 2 (columns), got {}", margin),
        ));
    }

    // STATS: positional arg[2] or named
    let stats_val = named
        .iter()
        .find(|(n, _)| n == "STATS")
        .map(|(_, v)| v)
        .or_else(|| args.get(2));
    let stats: Vec<f64> = match stats_val {
        Some(RValue::Vector(v)) => v
            .to_doubles()
            .into_iter()
            .map(|d| d.unwrap_or(f64::NAN))
            .collect(),
        _ => {
            return Err(RError::new(
                RErrorKind::Argument,
                "STATS must be a numeric vector".to_string(),
            ))
        }
    };

    // Validate STATS length matches the swept dimension
    let expected_len = if margin == 1 { nrow } else { ncol };
    if stats.len() != expected_len {
        return Err(RError::new(
            RErrorKind::Argument,
            format!(
                "STATS has length {} but MARGIN {} has {} {}",
                stats.len(),
                margin,
                expected_len,
                if margin == 1 { "rows" } else { "columns" }
            ),
        ));
    }

    // FUN: positional arg[3] or named — default is "-"
    let fun_val = named
        .iter()
        .find(|(n, _)| n == "FUN")
        .map(|(_, v)| v)
        .or_else(|| args.get(3));
    let fun_str = match fun_val {
        Some(RValue::Vector(v)) => v.as_character_scalar().unwrap_or_else(|| "-".to_string()),
        None => "-".to_string(),
        _ => "-".to_string(),
    };

    let apply_fn: fn(f64, f64) -> f64 = match fun_str.as_str() {
        "-" => |a, b| a - b,
        "+" => |a, b| a + b,
        "*" => |a, b| a * b,
        "/" => |a, b| a / b,
        other => {
            return Err(RError::new(
                RErrorKind::Argument,
                format!(
                    "FUN must be one of \"-\", \"+\", \"*\", \"/\", got {:?}",
                    other
                ),
            ))
        }
    };

    // Sweep: data is column-major (flat[j * nrow + i] = matrix[i, j])
    let mut result: Vec<Option<f64>> = Vec::with_capacity(nrow * ncol);
    for j in 0..ncol {
        for i in 0..nrow {
            let idx = j * nrow + i;
            let val = data[idx];
            let stat = if margin == 1 { stats[i] } else { stats[j] };
            result.push(val.map(|v| apply_fn(v, stat)));
        }
    }

    let (row_names, col_names) = matrix_dimnames(args.first().unwrap_or(&RValue::Null));
    let mut rv = RVector::from(Vector::Double(result.into()));
    set_matrix_attrs(&mut rv, nrow, ncol, row_names, col_names)?;
    Ok(RValue::Vector(rv))
}

/// Kronecker product of two matrices (or vectors treated as single-column matrices).
///
/// The default FUN is "*" (standard Kronecker product). The result has
/// dimensions (nrow(A)*nrow(B)) x (ncol(A)*ncol(B)).
///
/// Can also be called via the `%x%` operator: A %x% B.
///
/// @param A numeric matrix or vector
/// @param B numeric matrix or vector
/// @param FUN the function to apply element-wise: "*" (default), "+", "-", "/"
/// @return a numeric matrix
/// @namespace base
#[builtin(min_args = 2)]
fn builtin_kronecker(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    // FUN: positional arg[2] or named — default is "*"
    let fun_val = named
        .iter()
        .find(|(n, _)| n == "FUN")
        .map(|(_, v)| v)
        .or_else(|| args.get(2));
    let fun_str = match fun_val {
        Some(RValue::Vector(v)) => v.as_character_scalar().unwrap_or_else(|| "*".to_string()),
        None => "*".to_string(),
        _ => "*".to_string(),
    };

    let apply_fn: fn(f64, f64) -> f64 = match fun_str.as_str() {
        "*" => |a, b| a * b,
        "+" => |a, b| a + b,
        "-" => |a, b| a - b,
        "/" => |a, b| a / b,
        other => {
            return Err(RError::new(
                RErrorKind::Argument,
                format!(
                    "FUN must be one of \"*\", \"+\", \"-\", \"/\", got {:?}",
                    other
                ),
            ))
        }
    };

    eval_kronecker_with_fn(
        args.first().unwrap_or(&RValue::Null),
        args.get(1).unwrap_or(&RValue::Null),
        apply_fn,
    )
}

/// Extract matrix dimensions from a value, treating plain vectors as column vectors.
fn kronecker_dims_and_data(val: &RValue) -> Result<(usize, usize, Vec<Option<f64>>), RError> {
    match val {
        RValue::Vector(v) => {
            let data = v.to_doubles();
            match v.get_attr("dim") {
                Some(dim_val) => {
                    let dim_vec = dim_val.as_vector().ok_or_else(|| {
                        RError::new(RErrorKind::Type, "invalid dim attribute".to_string())
                    })?;
                    let dims = dim_vec.to_integers();
                    if dims.len() != 2 {
                        return Err(RError::new(
                            RErrorKind::Argument,
                            "kronecker() requires matrix arguments (2-d dim)".to_string(),
                        ));
                    }
                    let nrow = usize::try_from(dims[0].unwrap_or(0))?;
                    let ncol = usize::try_from(dims[1].unwrap_or(0))?;
                    Ok((nrow, ncol, data))
                }
                // Plain vector → treat as column vector (n x 1)
                None => {
                    let n = data.len();
                    Ok((n, 1, data))
                }
            }
        }
        _ => Err(RError::new(
            RErrorKind::Argument,
            "kronecker() requires numeric matrix/vector arguments".to_string(),
        )),
    }
}

fn eval_kronecker_with_fn(
    a: &RValue,
    b: &RValue,
    fun: fn(f64, f64) -> f64,
) -> Result<RValue, RError> {
    let (a_nrow, a_ncol, a_data) = kronecker_dims_and_data(a)?;
    let (b_nrow, b_ncol, b_data) = kronecker_dims_and_data(b)?;

    let out_nrow = a_nrow * b_nrow;
    let out_ncol = a_ncol * b_ncol;
    let mut result: Vec<Option<f64>> = Vec::with_capacity(out_nrow * out_ncol);

    // Build column-major result:
    // result[(i_a * b_nrow + i_b), (j_a * b_ncol + j_b)] = a[i_a, j_a] * b[i_b, j_b]
    // In column-major flat layout: result[out_col * out_nrow + out_row]
    for j_a in 0..a_ncol {
        for j_b in 0..b_ncol {
            for i_a in 0..a_nrow {
                for i_b in 0..b_nrow {
                    let a_val = a_data[j_a * a_nrow + i_a];
                    let b_val = b_data[j_b * b_nrow + i_b];
                    result.push(match (a_val, b_val) {
                        (Some(av), Some(bv)) => Some(fun(av, bv)),
                        _ => None,
                    });
                }
            }
        }
    }

    let mut rv = RVector::from(Vector::Double(result.into()));
    set_matrix_attrs(&mut rv, out_nrow, out_ncol, None, None)?;
    Ok(RValue::Vector(rv))
}

/// Evaluate the `%x%` (Kronecker product) operator.
///
/// Called from `ops.rs` when the parser encounters `A %x% B`.
pub fn eval_kronecker(left: &RValue, right: &RValue) -> Result<RValue, RError> {
    eval_kronecker_with_fn(left, right, |a, b| a * b)
}

// endregion
