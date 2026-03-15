use std::collections::BTreeSet;

use derive_more::{Display, Error};
use ndarray::{Array2, ShapeBuilder};

use crate::interpreter::coerce::{f64_to_i32, usize_to_f64};
use crate::interpreter::value::*;
use minir_macros::builtin;

type DimNameVec = Vec<Option<String>>;
type MatrixDimNames = (Option<DimNameVec>, Option<DimNameVec>);

// region: MathError

/// Structured error type for math/linear algebra operations.
#[derive(Debug, Display, Error)]
pub enum MathError {
    #[display("matrix shape error: {}", source)]
    Shape {
        #[error(source)]
        source: ndarray::ShapeError,
    },
}

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

/// Natural logarithm.
///
/// @param x numeric vector
/// @return numeric vector of natural logarithms
#[builtin(min_args = 1)]
fn builtin_log(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    math_unary(args, f64::ln)
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
/// @param x numeric vector
/// @return numeric vector of signs
#[builtin(min_args = 1)]
fn builtin_sign(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    math_unary(args, f64::signum)
}

use super::math_unary_op as math_unary;

/// Round to the specified number of decimal places.
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
                .map(|x| x.map(|f| (f * factor).round() / factor))
                .collect();
            Ok(RValue::vec(Vector::Double(result.into())))
        }
        _ => Err(RError::new(
            RErrorKind::Argument,
            "non-numeric argument".to_string(),
        )),
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
/// @return scalar double
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

/// Sample variance (Bessel-corrected, divides by n-1).
///
/// @param x numeric vector
/// @return scalar double
#[builtin(min_args = 1)]
fn builtin_var(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(v)) => {
            let vals: Vec<f64> = v.to_doubles().into_iter().flatten().collect();
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

/// Cumulative sum.
///
/// @param x numeric vector
/// @return numeric vector of running sums
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
        _ => Err(RError::new(
            RErrorKind::Argument,
            "invalid argument".to_string(),
        )),
    }
}

/// Cumulative product.
///
/// @param x numeric vector
/// @return numeric vector of running products
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
        _ => Err(RError::new(
            RErrorKind::Argument,
            "invalid argument".to_string(),
        )),
    }
}

/// Cumulative maximum.
///
/// @param x numeric vector
/// @return numeric vector of running maxima
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
        _ => Err(RError::new(
            RErrorKind::Argument,
            "invalid argument".to_string(),
        )),
    }
}

/// Cumulative minimum.
///
/// @param x numeric vector
/// @return numeric vector of running minima
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
/// @param times number of times to repeat (default 1)
/// @return vector with elements repeated
#[builtin(min_args = 1)]
fn builtin_rep(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let times = named
        .iter()
        .find(|(n, _)| n == "times")
        .and_then(|(_, v)| v.as_vector()?.as_integer_scalar())
        .or_else(|| args.get(1)?.as_vector()?.as_integer_scalar())
        .unwrap_or(1);
    let times = usize::try_from(times)?;

    match args.first() {
        Some(RValue::Vector(v)) => match &v.inner {
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
/// @return sorted vector
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
/// @param x logical vector
/// @return integer vector of 1-based indices where x is TRUE
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
                        Some(Some(i64::try_from(i).unwrap_or(0) + 1))
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
/// @param x a vector
/// @param values values to append
/// @return concatenated character vector
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

/// Return the first n elements of a vector.
///
/// @param x a vector
/// @param n number of elements to return (default 6)
/// @return vector of the first n elements
#[builtin(min_args = 1)]
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
#[builtin(min_args = 1)]
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
/// @return numeric vector of length 2: c(min, max)
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

/// Lagged differences.
///
/// @param x numeric vector
/// @param lag the lag to use (default 1)
/// @return numeric vector of length(x) - lag
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
    match args.first() {
        Some(RValue::Vector(v)) => {
            let vals = v.to_doubles();
            if vals.len() <= lag {
                return Ok(RValue::vec(Vector::Double(vec![].into())));
            }
            let result: Vec<Option<f64>> = (lag..vals.len())
                .map(|i| match (vals[i - lag], vals[i]) {
                    (Some(a), Some(b)) => Some(b - a),
                    _ => None,
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

fn names_attr(value: &RValue) -> Option<DimNameVec> {
    match value {
        RValue::Vector(rv) => rv.get_attr("names").and_then(super::coerce_name_values),
        RValue::List(list) => list.get_attr("names").and_then(super::coerce_name_values),
        _ => None,
    }
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
        Some(val) => rvalue_to_array2(val)?,
        None => Array2::eye(n),
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
            return Err(RError::other(
                "solve(): matrix is singular (or very close to singular). \
                 Check that your matrix has full rank — its determinant is effectively zero",
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
            return Err(RError::new(
                RErrorKind::Argument,
                "outer() requires numeric vectors for X and Y".to_string(),
            ))
        }
    };
    let y_vec = match args.get(1) {
        Some(RValue::Vector(rv)) => rv.to_doubles(),
        _ => {
            return Err(RError::new(
                RErrorKind::Argument,
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
            return Err(RError::new(
                RErrorKind::Argument,
                format!(
                    "outer() with FUN = \"{}\" is not supported. \
                 Supported operators: \"*\", \"+\", \"-\", \"/\", \"^\", \"%%\", \"%/%\"",
                    other
                ),
            ));
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
    set_matrix_attrs(&mut rv, nx, ny, names_attr(&args[0]), names_attr(&args[1]))?;
    Ok(RValue::Vector(rv))
}
/// `det(x)` — matrix determinant via Gaussian elimination with partial pivoting.
#[builtin(min_args = 1)]
fn builtin_det(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let a = rvalue_to_array2(args.first().unwrap_or(&RValue::Null))?;
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

    let mut m = a;
    let mut det = 1.0;
    for col in 0..n {
        // Partial pivot
        let mut max_row = col;
        let mut max_val = m[[col, col]].abs();
        for row in (col + 1)..n {
            let val = m[[row, col]].abs();
            if val > max_val {
                max_val = val;
                max_row = row;
            }
        }
        if max_val < 1e-15 {
            return Ok(RValue::vec(Vector::Double(vec![Some(0.0)].into())));
        }
        if max_row != col {
            for j in 0..n {
                let tmp = m[[col, j]];
                m[[col, j]] = m[[max_row, j]];
                m[[max_row, j]] = tmp;
            }
            det = -det;
        }
        det *= m[[col, col]];
        for row in (col + 1)..n {
            let factor = m[[row, col]] / m[[col, col]];
            for j in col..n {
                m[[row, j]] -= factor * m[[col, j]];
            }
        }
    }
    Ok(RValue::vec(Vector::Double(vec![Some(det)].into())))
}

/// `chol(x)` — Cholesky decomposition (upper triangular R such that x = R'R).
#[builtin(min_args = 1)]
fn builtin_chol(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let a = rvalue_to_array2(args.first().unwrap_or(&RValue::Null))?;
    let n = a.nrows();
    if n != a.ncols() {
        return Err(RError::new(
            RErrorKind::Argument,
            format!("'x' must be a square matrix, got {}x{}", n, a.ncols()),
        ));
    }

    let mut r = Array2::<f64>::zeros((n, n));
    for i in 0..n {
        for j in i..n {
            let mut sum = a[[i, j]];
            for k in 0..i {
                sum -= r[[k, i]] * r[[k, j]];
            }
            if i == j {
                if sum <= 0.0 {
                    return Err(RError::other(
                        "the leading minor of order ".to_string()
                            + &(i + 1).to_string()
                            + " is not positive — matrix is not positive definite",
                    ));
                }
                r[[i, j]] = sum.sqrt();
            } else {
                r[[i, j]] = sum / r[[i, i]];
            }
        }
    }
    Ok(array2_to_rvalue(&r))
}

// region: QR, SVD, Eigen decompositions

/// `qr(x)` — QR decomposition via Householder reflections.
///
/// Returns a list with class "qr" containing:
/// - `$qr`: the compact QR matrix (R stored in upper triangle, Householder
///    vectors in lower triangle)
/// - `$rank`: integer rank estimate
/// - `$pivot`: integer vector 1:ncol (no column pivoting)
#[builtin(min_args = 1)]
fn builtin_qr(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let a = rvalue_to_array2(args.first().unwrap_or(&RValue::Null))?;
    let m = a.nrows();
    let n = a.ncols();

    // Work on a mutable copy — will hold the compact QR representation
    let mut qr = a;
    let k = m.min(n);

    for j in 0..k {
        // Compute the norm of the j-th column below the diagonal
        let mut col_norm_sq = 0.0;
        for i in j..m {
            col_norm_sq += qr[[i, j]] * qr[[i, j]];
        }
        let col_norm = col_norm_sq.sqrt();

        if col_norm < 1e-15 {
            continue;
        }

        // Choose sign to avoid cancellation
        let sign = if qr[[j, j]] >= 0.0 { 1.0 } else { -1.0 };
        let alpha = -sign * col_norm;

        // Householder vector v = x - alpha*e1, stored in-place below diagonal
        qr[[j, j]] -= alpha;

        // Normalise the Householder vector for numerical stability
        let mut v_norm_sq = 0.0;
        for i in j..m {
            v_norm_sq += qr[[i, j]] * qr[[i, j]];
        }
        if v_norm_sq < 1e-30 {
            qr[[j, j]] = alpha;
            continue;
        }
        let inv_v_norm_sq = 1.0 / v_norm_sq;

        // Apply Householder reflection to remaining columns:
        // A[j:m, j+1:n] -= 2 * v * (v^T A) / (v^T v)
        for col in (j + 1)..n {
            let mut dot = 0.0;
            for i in j..m {
                dot += qr[[i, j]] * qr[[i, col]];
            }
            let factor = 2.0 * dot * inv_v_norm_sq;
            for i in j..m {
                qr[[i, col]] -= factor * qr[[i, j]];
            }
        }

        // Store the diagonal element of R
        qr[[j, j]] = alpha;
    }

    // Estimate rank from diagonal of R
    let tol = f64::EPSILON * (m.max(n) as f64) * {
        let mut max_diag = 0.0f64;
        for i in 0..k {
            max_diag = max_diag.max(qr[[i, i]].abs());
        }
        max_diag
    };
    let mut rank = 0i64;
    for i in 0..k {
        if qr[[i, i]].abs() > tol {
            rank += 1;
        }
    }

    // Pivot vector: 1:ncol (no pivoting)
    let pivot: Vec<Option<i64>> = (1..=i64::try_from(n)?).map(Some).collect();

    let qr_val = array2_to_rvalue(&qr);

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
    ]);
    list.set_attr(
        "class".to_string(),
        RValue::vec(Vector::Character(vec![Some("qr".to_string())].into())),
    );
    Ok(RValue::List(list))
}

/// `svd(x)` — Singular Value Decomposition via one-sided Jacobi rotations.
///
/// Returns a list with:
/// - `$d`: numeric vector of singular values (descending)
/// - `$u`: left singular vectors (m x min(m,n) matrix)
/// - `$v`: right singular vectors (n x min(m,n) matrix)
#[builtin(min_args = 1)]
fn builtin_svd(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let a = rvalue_to_array2(args.first().unwrap_or(&RValue::Null))?;
    let m = a.nrows();
    let n = a.ncols();

    if m == 0 || n == 0 {
        return Err(RError::new(
            RErrorKind::Argument,
            "0 extent dimensions are not allowed".to_string(),
        ));
    }

    // Work on B = A^T A (n x n) for one-sided Jacobi
    let at = a.t();
    let mut b = at.dot(&a);

    // V accumulates right singular vectors (n x n)
    let mut v = Array2::<f64>::zeros((n, n));
    for i in 0..n {
        v[[i, i]] = 1.0;
    }

    // Jacobi iterations on B = A^T A to diagonalize it
    let max_iter = 100 * n * n;
    let tol = 1e-12;
    for _ in 0..max_iter {
        let mut off_diag = 0.0;
        for i in 0..n {
            for j in (i + 1)..n {
                off_diag += b[[i, j]] * b[[i, j]];
            }
        }
        if off_diag.sqrt()
            < tol * {
                let mut diag_norm = 0.0;
                for i in 0..n {
                    diag_norm += b[[i, i]] * b[[i, i]];
                }
                diag_norm.sqrt()
            }
        {
            break;
        }

        for p in 0..n {
            for q in (p + 1)..n {
                let b_pq = b[[p, q]];
                if b_pq.abs() < 1e-15 {
                    continue;
                }
                let b_pp = b[[p, p]];
                let b_qq = b[[q, q]];

                // Compute Jacobi rotation angle
                let tau = (b_qq - b_pp) / (2.0 * b_pq);
                let t = if tau >= 0.0 {
                    1.0 / (tau + (1.0 + tau * tau).sqrt())
                } else {
                    -1.0 / (-tau + (1.0 + tau * tau).sqrt())
                };
                let c = 1.0 / (1.0 + t * t).sqrt();
                let s = t * c;

                // Apply rotation to B: B <- J^T B J
                // Update rows p and q of B
                for i in 0..n {
                    let b_ip = b[[i, p]];
                    let b_iq = b[[i, q]];
                    b[[i, p]] = c * b_ip - s * b_iq;
                    b[[i, q]] = s * b_ip + c * b_iq;
                }
                // Update columns p and q of B
                for j in 0..n {
                    let b_pj = b[[p, j]];
                    let b_qj = b[[q, j]];
                    b[[p, j]] = c * b_pj - s * b_qj;
                    b[[q, j]] = s * b_pj + c * b_qj;
                }

                // Accumulate in V
                for i in 0..n {
                    let v_ip = v[[i, p]];
                    let v_iq = v[[i, q]];
                    v[[i, p]] = c * v_ip - s * v_iq;
                    v[[i, q]] = s * v_ip + c * v_iq;
                }
            }
        }
    }

    // Singular values are sqrt of diagonal of B (eigenvalues of A^T A)
    let k = m.min(n);
    let mut sigma: Vec<f64> = (0..n).map(|i| b[[i, i]].max(0.0).sqrt()).collect();

    // Sort singular values in descending order and permute V accordingly
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a_idx, &b_idx| {
        sigma[b_idx]
            .partial_cmp(&sigma[a_idx])
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let sorted_sigma: Vec<f64> = order.iter().map(|&i| sigma[i]).collect();
    sigma = sorted_sigma;

    let mut v_sorted = Array2::<f64>::zeros((n, n));
    for (new_j, &old_j) in order.iter().enumerate() {
        for i in 0..n {
            v_sorted[[i, new_j]] = v[[i, old_j]];
        }
    }

    // Compute U = A V Sigma^{-1} (only first k columns)
    let mut u = Array2::<f64>::zeros((m, k));
    for j in 0..k {
        if sigma[j] > 1e-15 {
            let inv_sigma = 1.0 / sigma[j];
            for i in 0..m {
                let mut sum = 0.0;
                for l in 0..n {
                    sum += a[[i, l]] * v_sorted[[l, j]];
                }
                u[[i, j]] = sum * inv_sigma;
            }
        }
    }

    // Truncate to min(m,n) singular values and V columns
    let d_vals: Vec<Option<f64>> = sigma[..k].iter().copied().map(Some).collect();

    let mut v_out = Array2::<f64>::zeros((n, k));
    for j in 0..k {
        for i in 0..n {
            v_out[[i, j]] = v_sorted[[i, j]];
        }
    }

    Ok(RValue::List(RList::new(vec![
        (
            Some("d".to_string()),
            RValue::vec(Vector::Double(d_vals.into())),
        ),
        (Some("u".to_string()), array2_to_rvalue(&u)),
        (Some("v".to_string()), array2_to_rvalue(&v_out)),
    ])))
}

/// `eigen(x)` — Eigenvalue decomposition for symmetric matrices via Jacobi
/// iteration.
///
/// Returns a list with:
/// - `$values`: numeric vector of eigenvalues (descending)
/// - `$vectors`: matrix of eigenvectors (columns)
///
/// Currently only supports real symmetric matrices. Non-symmetric input
/// produces an informative error.
#[builtin(min_args = 1)]
fn builtin_eigen(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let a = rvalue_to_array2(args.first().unwrap_or(&RValue::Null))?;
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
                array2_to_rvalue(&Array2::<f64>::zeros((0, 0))),
            ),
        ])));
    }

    // Check symmetry
    let sym_tol = 1e-10;
    for i in 0..n {
        for j in (i + 1)..n {
            if (a[[i, j]] - a[[j, i]]).abs() > sym_tol * (a[[i, j]].abs() + a[[j, i]].abs() + 1.0) {
                return Err(RError::other(
                    "only real symmetric matrices are supported in eigen() currently — \
                     the input matrix is not symmetric. Consider using (x + t(x))/2 \
                     if you want to symmetrize it."
                        .to_string(),
                ));
            }
        }
    }

    // Jacobi eigenvalue algorithm for symmetric matrices
    let mut s = a;
    let mut eigvecs = Array2::<f64>::zeros((n, n));
    for i in 0..n {
        eigvecs[[i, i]] = 1.0;
    }

    let max_iter = 100 * n * n;
    let tol = 1e-12;

    for _ in 0..max_iter {
        // Find largest off-diagonal element
        let mut off_diag = 0.0;
        for i in 0..n {
            for j in (i + 1)..n {
                off_diag += s[[i, j]] * s[[i, j]];
            }
        }
        if off_diag.sqrt() < tol {
            break;
        }

        for p in 0..n {
            for q in (p + 1)..n {
                let s_pq = s[[p, q]];
                if s_pq.abs() < 1e-15 {
                    continue;
                }

                let tau = (s[[q, q]] - s[[p, p]]) / (2.0 * s_pq);
                let t = if tau >= 0.0 {
                    1.0 / (tau + (1.0 + tau * tau).sqrt())
                } else {
                    -1.0 / (-tau + (1.0 + tau * tau).sqrt())
                };
                let c = 1.0 / (1.0 + t * t).sqrt();
                let sv = t * c;

                // Apply Givens rotation
                for i in 0..n {
                    let s_ip = s[[i, p]];
                    let s_iq = s[[i, q]];
                    s[[i, p]] = c * s_ip - sv * s_iq;
                    s[[i, q]] = sv * s_ip + c * s_iq;
                }
                for j in 0..n {
                    let s_pj = s[[p, j]];
                    let s_qj = s[[q, j]];
                    s[[p, j]] = c * s_pj - sv * s_qj;
                    s[[q, j]] = sv * s_pj + c * s_qj;
                }

                // Accumulate eigenvectors
                for i in 0..n {
                    let v_ip = eigvecs[[i, p]];
                    let v_iq = eigvecs[[i, q]];
                    eigvecs[[i, p]] = c * v_ip - sv * v_iq;
                    eigvecs[[i, q]] = sv * v_ip + c * v_iq;
                }
            }
        }
    }

    // Extract eigenvalues from diagonal
    let mut eigen_pairs: Vec<(f64, usize)> = (0..n).map(|i| (s[[i, i]], i)).collect();
    // Sort descending by eigenvalue
    eigen_pairs.sort_by(|a_pair, b_pair| {
        b_pair
            .0
            .partial_cmp(&a_pair.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let values: Vec<Option<f64>> = eigen_pairs.iter().map(|&(val, _)| Some(val)).collect();

    let mut vectors = Array2::<f64>::zeros((n, n));
    for (new_j, &(_, old_j)) in eigen_pairs.iter().enumerate() {
        for i in 0..n {
            vectors[[i, new_j]] = eigvecs[[i, old_j]];
        }
    }

    Ok(RValue::List(RList::new(vec![
        (
            Some("values".to_string()),
            RValue::vec(Vector::Double(values.into())),
        ),
        (Some("vectors".to_string()), array2_to_rvalue(&vectors)),
    ])))
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
