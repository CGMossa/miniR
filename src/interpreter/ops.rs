//! Vectorized arithmetic, comparison, logical, range, membership, and matrix
//! multiplication operators. All functions are pure — they operate on values
//! without needing interpreter state.

use ndarray::{Array2, ShapeBuilder};

use crate::interpreter::builtins;
use crate::interpreter::coerce::f64_to_i64;
use crate::interpreter::value::*;
use crate::interpreter::Interpreter;
use crate::parser::ast::{BinaryOp, SpecialOp, UnaryOp};

/// Copy attributes (dim, dimnames, names, class) from the longer operand
/// to the arithmetic result. R's rule: attrs come from the first operand
/// if lengths are equal, otherwise from the longer one.
fn propagate_attrs(result: RValue, lv: &RVector, rv: &RVector) -> RValue {
    let donor = if lv.inner.len() >= rv.inner.len() {
        lv
    } else {
        rv
    };
    let Some(attrs) = &donor.attrs else {
        return result;
    };
    match result {
        RValue::Vector(mut rv_out) => {
            // Copy dim, dimnames, names — skip class (arithmetic strips S3 class for base types)
            for key in &["dim", "dimnames", "names"] {
                if let Some(val) = attrs.get(*key) {
                    rv_out.set_attr(key.to_string(), val.clone());
                }
            }
            RValue::Vector(rv_out)
        }
        other => other,
    }
}

// region: Interpreter delegation

impl Interpreter {
    pub(super) fn eval_unary(&self, op: UnaryOp, val: &RValue) -> Result<RValue, RFlow> {
        eval_unary(op, val)
    }

    pub(super) fn eval_binary(
        &self,
        op: BinaryOp,
        left: &RValue,
        right: &RValue,
    ) -> Result<RValue, RFlow> {
        eval_binary(op, left, right)
    }
}

// endregion

// region: unary

fn eval_unary(op: UnaryOp, val: &RValue) -> Result<RValue, RFlow> {
    match op {
        UnaryOp::Neg => match val {
            RValue::Vector(v) => {
                let result = match &v.inner {
                    Vector::Double(vals) => Vector::Double(
                        vals.iter()
                            .map(|x| x.map(|f| -f))
                            .collect::<Vec<_>>()
                            .into(),
                    ),
                    Vector::Integer(vals) => Vector::Integer(
                        vals.iter()
                            .map(|x| x.map(|i| -i))
                            .collect::<Vec<_>>()
                            .into(),
                    ),
                    Vector::Logical(vals) => Vector::Integer(
                        vals.iter()
                            .map(|x| x.map(|b| if b { -1 } else { 0 }))
                            .collect::<Vec<_>>()
                            .into(),
                    ),
                    _ => {
                        return Err(RError::new(
                            RErrorKind::Type,
                            "invalid argument to unary operator",
                        )
                        .into())
                    }
                };
                Ok(RValue::vec(result))
            }
            _ => Err(RError::new(RErrorKind::Type, "invalid argument to unary operator").into()),
        },
        UnaryOp::Pos => match val {
            RValue::Vector(v) if matches!(v.inner, Vector::Raw(_)) => {
                Err(RError::new(RErrorKind::Type, "non-numeric argument to unary operator").into())
            }
            _ => Ok(val.clone()),
        },
        UnaryOp::Not => match val {
            // Bitwise NOT for raw vectors
            RValue::Vector(v) if matches!(v.inner, Vector::Raw(_)) => {
                let bytes = v.inner.to_raw();
                let result: Vec<u8> = bytes.iter().map(|b| !b).collect();
                Ok(RValue::vec(Vector::Raw(result)))
            }
            RValue::Vector(v) => {
                let logicals = v.to_logicals();
                let result: Vec<Option<bool>> = logicals.iter().map(|x| x.map(|b| !b)).collect();
                Ok(RValue::vec(Vector::Logical(result.into())))
            }
            _ => Err(RError::new(RErrorKind::Type, "invalid argument type").into()),
        },
        UnaryOp::Formula => Ok(RValue::Null), // stub for unary ~
    }
}

// endregion

// region: binary dispatch

fn eval_binary(op: BinaryOp, left: &RValue, right: &RValue) -> Result<RValue, RFlow> {
    match op {
        BinaryOp::Range => return eval_range(left, right),
        BinaryOp::Special(SpecialOp::In) => return eval_in_op(left, right),
        BinaryOp::Special(SpecialOp::MatMul) => return eval_matmul(left, right),
        _ => {}
    };

    // Get vectors for element-wise operations
    let lv = match left {
        RValue::Vector(v) => v,
        RValue::Null => return Ok(RValue::Null),
        _ => {
            return Err(RError::new(
                RErrorKind::Type,
                "non-numeric argument to binary operator".to_string(),
            )
            .into())
        }
    };
    let rv = match right {
        RValue::Vector(v) => v,
        RValue::Null => return Ok(RValue::Null),
        _ => {
            return Err(RError::new(
                RErrorKind::Type,
                "non-numeric argument to binary operator".to_string(),
            )
            .into())
        }
    };

    match op {
        BinaryOp::Range => eval_range(left, right),
        BinaryOp::Special(SpecialOp::In) => eval_in_op(left, right),
        BinaryOp::Special(SpecialOp::MatMul) => eval_matmul(left, right),
        BinaryOp::Special(_) => Ok(RValue::Null),

        // Arithmetic (vectorized with recycling) — raw vectors cannot participate
        BinaryOp::Add
        | BinaryOp::Sub
        | BinaryOp::Mul
        | BinaryOp::Div
        | BinaryOp::Pow
        | BinaryOp::Mod
        | BinaryOp::IntDiv => {
            if matches!(lv.inner, Vector::Raw(_)) || matches!(rv.inner, Vector::Raw(_)) {
                return Err(RError::new(
                    RErrorKind::Type,
                    "non-numeric argument to binary operator",
                )
                .into());
            }
            let result = eval_arith(op, &lv.inner, &rv.inner)?;
            Ok(propagate_attrs(result, lv, rv))
        }

        // Comparison (vectorized)
        BinaryOp::Eq | BinaryOp::Ne | BinaryOp::Lt | BinaryOp::Gt | BinaryOp::Le | BinaryOp::Ge => {
            eval_compare(op, lv, rv)
        }

        // Logical — for raw vectors, & and | are bitwise
        BinaryOp::And | BinaryOp::Or => {
            if matches!(lv.inner, Vector::Raw(_)) || matches!(rv.inner, Vector::Raw(_)) {
                return eval_raw_bitwise(op, &lv.inner, &rv.inner);
            }
            eval_logical_vec(op, &lv.inner, &rv.inner)
        }

        // Scalar logical
        BinaryOp::AndScalar => {
            let a = lv.as_logical_scalar();
            let b = rv.as_logical_scalar();
            match (a, b) {
                (Some(false), _) | (_, Some(false)) => {
                    Ok(RValue::vec(Vector::Logical(vec![Some(false)].into())))
                }
                (Some(true), Some(true)) => {
                    Ok(RValue::vec(Vector::Logical(vec![Some(true)].into())))
                }
                _ => Ok(RValue::vec(Vector::Logical(vec![None].into()))),
            }
        }
        BinaryOp::OrScalar => {
            let a = lv.as_logical_scalar();
            let b = rv.as_logical_scalar();
            match (a, b) {
                (Some(true), _) | (_, Some(true)) => {
                    Ok(RValue::vec(Vector::Logical(vec![Some(true)].into())))
                }
                (Some(false), Some(false)) => {
                    Ok(RValue::vec(Vector::Logical(vec![Some(false)].into())))
                }
                _ => Ok(RValue::vec(Vector::Logical(vec![None].into()))),
            }
        }

        BinaryOp::Pipe => unreachable!("pipe handled separately"),
        BinaryOp::Tilde => Ok(RValue::Null), // stub for binary ~
    }
}

// endregion

// region: arithmetic

fn eval_arith(op: BinaryOp, lv: &Vector, rv: &Vector) -> Result<RValue, RFlow> {
    // Check if both are integer and op preserves integer type
    let use_integer = matches!(
        (&lv, &rv, &op),
        (
            Vector::Integer(_),
            Vector::Integer(_),
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::IntDiv | BinaryOp::Mod
        )
    );

    if use_integer {
        let li = lv.to_integers();
        let ri = rv.to_integers();
        let len = li.len().max(ri.len());
        if len == 0 {
            return Ok(RValue::vec(Vector::Integer(vec![].into())));
        }
        let result: Vec<Option<i64>> = (0..len)
            .map(|i| {
                let a = li[i % li.len()];
                let b = ri[i % ri.len()];
                match (a, b) {
                    (Some(a), Some(b)) => match op {
                        BinaryOp::Add => Some(a.wrapping_add(b)),
                        BinaryOp::Sub => Some(a.wrapping_sub(b)),
                        BinaryOp::Mul => Some(a.wrapping_mul(b)),
                        BinaryOp::IntDiv => {
                            if b != 0 {
                                Some(a / b)
                            } else {
                                None
                            }
                        }
                        BinaryOp::Mod => {
                            if b != 0 {
                                Some(a % b)
                            } else {
                                None
                            }
                        }
                        _ => None,
                    },
                    _ => None,
                }
            })
            .collect();
        return Ok(RValue::vec(Vector::Integer(result.into())));
    }

    // If either operand is complex, operate in complex space
    let use_complex = matches!(lv, Vector::Complex(_)) || matches!(rv, Vector::Complex(_));

    if use_complex {
        let lc = lv.to_complex();
        let rc = rv.to_complex();
        let len = lc.len().max(rc.len());
        if len == 0 {
            return Ok(RValue::vec(Vector::Complex(vec![].into())));
        }
        if matches!(op, BinaryOp::Mod | BinaryOp::IntDiv) {
            return Err(RError::new(
                RErrorKind::Type,
                "unimplemented complex operation".to_string(),
            )
            .into());
        }
        let result: Vec<Option<num_complex::Complex64>> = (0..len)
            .map(|i| {
                let a = lc[i % lc.len()];
                let b = rc[i % rc.len()];
                match (a, b) {
                    (Some(a), Some(b)) => Some(match op {
                        BinaryOp::Add => a + b,
                        BinaryOp::Sub => a - b,
                        BinaryOp::Mul => a * b,
                        BinaryOp::Div => a / b,
                        BinaryOp::Pow => a.powc(b),
                        _ => unreachable!(),
                    }),
                    _ => None,
                }
            })
            .collect();
        return Ok(RValue::vec(Vector::Complex(result.into())));
    }

    let ld = lv.to_doubles();
    let rd = rv.to_doubles();
    let len = ld.len().max(rd.len());
    if len == 0 {
        return Ok(RValue::vec(Vector::Double(vec![].into())));
    }

    let arith_element = |i: usize| -> Option<f64> {
        let a = ld[i % ld.len()];
        let b = rd[i % rd.len()];
        match (a, b) {
            (Some(a), Some(b)) => Some(match op {
                BinaryOp::Add => a + b,
                BinaryOp::Sub => a - b,
                BinaryOp::Mul => a * b,
                BinaryOp::Div => a / b,
                BinaryOp::Pow => a.powf(b),
                BinaryOp::Mod => a % b,
                BinaryOp::IntDiv => (a / b).floor(),
                _ => unreachable!(),
            }),
            _ => None,
        }
    };

    // Use rayon for large vectors when the parallel feature is enabled
    #[cfg(feature = "parallel")]
    if len >= 10_000 {
        use rayon::prelude::*;
        let result: Vec<Option<f64>> = (0..len).into_par_iter().map(arith_element).collect();
        return Ok(RValue::vec(Vector::Double(result.into())));
    }

    let result: Vec<Option<f64>> = (0..len).map(arith_element).collect();
    Ok(RValue::vec(Vector::Double(result.into())))
}

// endregion

// region: comparison

fn eval_compare(op: BinaryOp, lv: &RVector, rv: &RVector) -> Result<RValue, RFlow> {
    // Raw comparison: compares byte values
    if matches!(lv.inner, Vector::Raw(_)) || matches!(rv.inner, Vector::Raw(_)) {
        let lb = lv.to_raw();
        let rb = rv.to_raw();
        let len = lb.len().max(rb.len());
        if len == 0 {
            return Ok(RValue::vec(Vector::Logical(vec![].into())));
        }
        let result: Vec<Option<bool>> = (0..len)
            .map(|i| {
                let a = lb[i % lb.len()];
                let b = rb[i % rb.len()];
                Some(match op {
                    BinaryOp::Eq => a == b,
                    BinaryOp::Ne => a != b,
                    BinaryOp::Lt => a < b,
                    BinaryOp::Gt => a > b,
                    BinaryOp::Le => a <= b,
                    BinaryOp::Ge => a >= b,
                    _ => unreachable!(),
                })
            })
            .collect();
        return Ok(RValue::vec(Vector::Logical(result.into())));
    }

    // Complex comparison: only == and != are defined
    if matches!(lv.inner, Vector::Complex(_)) || matches!(rv.inner, Vector::Complex(_)) {
        if !matches!(op, BinaryOp::Eq | BinaryOp::Ne) {
            return Err(RError::new(
                RErrorKind::Type,
                "invalid comparison with complex values".to_string(),
            )
            .into());
        }
        let lc = lv.to_complex();
        let rc = rv.to_complex();
        let len = lc.len().max(rc.len());
        let result: Vec<Option<bool>> = (0..len)
            .map(|i| {
                let a = lc[i % lc.len()];
                let b = rc[i % rc.len()];
                match (a, b) {
                    (Some(a), Some(b)) => Some(match op {
                        BinaryOp::Eq => a == b,
                        BinaryOp::Ne => a != b,
                        _ => unreachable!(),
                    }),
                    _ => None,
                }
            })
            .collect();
        return Ok(RValue::vec(Vector::Logical(result.into())));
    }

    // If either is character, compare as strings
    if matches!(lv.inner, Vector::Character(_)) || matches!(rv.inner, Vector::Character(_)) {
        let lc = lv.to_characters();
        let rc = rv.to_characters();
        let len = lc.len().max(rc.len());
        let result: Vec<Option<bool>> = (0..len)
            .map(|i| {
                let a = &lc[i % lc.len()];
                let b = &rc[i % rc.len()];
                match (a, b) {
                    (Some(a), Some(b)) => Some(match op {
                        BinaryOp::Eq => a == b,
                        BinaryOp::Ne => a != b,
                        BinaryOp::Lt => a < b,
                        BinaryOp::Gt => a > b,
                        BinaryOp::Le => a <= b,
                        BinaryOp::Ge => a >= b,
                        _ => unreachable!(),
                    }),
                    _ => None,
                }
            })
            .collect();
        return Ok(RValue::vec(Vector::Logical(result.into())));
    }

    let ld = lv.to_doubles();
    let rd = rv.to_doubles();
    let len = ld.len().max(rd.len());
    if len == 0 {
        return Ok(RValue::vec(Vector::Logical(vec![].into())));
    }

    let result: Vec<Option<bool>> = (0..len)
        .map(|i| {
            let a = ld[i % ld.len()];
            let b = rd[i % rd.len()];
            match (a, b) {
                (Some(a), Some(b)) => Some(match op {
                    BinaryOp::Eq => a == b,
                    BinaryOp::Ne => a != b,
                    BinaryOp::Lt => a < b,
                    BinaryOp::Gt => a > b,
                    BinaryOp::Le => a <= b,
                    BinaryOp::Ge => a >= b,
                    _ => unreachable!(),
                }),
                _ => None,
            }
        })
        .collect();
    Ok(RValue::vec(Vector::Logical(result.into())))
}

// endregion

// region: logical

fn eval_logical_vec(op: BinaryOp, lv: &Vector, rv: &Vector) -> Result<RValue, RFlow> {
    let ll = lv.to_logicals();
    let rl = rv.to_logicals();
    let len = ll.len().max(rl.len());

    let result: Vec<Option<bool>> = (0..len)
        .map(|i| {
            let a = ll[i % ll.len()];
            let b = rl[i % rl.len()];
            match op {
                BinaryOp::And => match (a, b) {
                    (Some(false), _) | (_, Some(false)) => Some(false),
                    (Some(true), Some(true)) => Some(true),
                    _ => None,
                },
                BinaryOp::Or => match (a, b) {
                    (Some(true), _) | (_, Some(true)) => Some(true),
                    (Some(false), Some(false)) => Some(false),
                    _ => None,
                },
                _ => unreachable!(),
            }
        })
        .collect();
    Ok(RValue::vec(Vector::Logical(result.into())))
}

/// Bitwise &/| for raw vectors — returns raw
fn eval_raw_bitwise(op: BinaryOp, lv: &Vector, rv: &Vector) -> Result<RValue, RFlow> {
    let lb = lv.to_raw();
    let rb = rv.to_raw();
    let len = lb.len().max(rb.len());
    if len == 0 {
        return Ok(RValue::vec(Vector::Raw(vec![])));
    }
    let result: Vec<u8> = (0..len)
        .map(|i| {
            let a = lb[i % lb.len()];
            let b = rb[i % rb.len()];
            match op {
                BinaryOp::And => a & b,
                BinaryOp::Or => a | b,
                _ => unreachable!(),
            }
        })
        .collect();
    Ok(RValue::vec(Vector::Raw(result)))
}

// endregion

// region: range and membership

fn eval_range(left: &RValue, right: &RValue) -> Result<RValue, RFlow> {
    let from = match left {
        RValue::Vector(v) => f64_to_i64(v.as_double_scalar().unwrap_or(0.0))?,
        _ => 0,
    };
    let to = match right {
        RValue::Vector(v) => f64_to_i64(v.as_double_scalar().unwrap_or(0.0))?,
        _ => 0,
    };

    let result: Vec<Option<i64>> = if from <= to {
        (from..=to).map(Some).collect()
    } else {
        (to..=from).rev().map(Some).collect()
    };
    Ok(RValue::vec(Vector::Integer(result.into())))
}

fn eval_in_op(left: &RValue, right: &RValue) -> Result<RValue, RFlow> {
    match (left, right) {
        (RValue::Vector(lv), RValue::Vector(rv)) => {
            // If either side is character, compare as strings
            if matches!(lv.inner, Vector::Character(_)) || matches!(rv.inner, Vector::Character(_))
            {
                let table = rv.to_characters();
                let vals = lv.to_characters();
                let result: Vec<Option<bool>> =
                    vals.iter().map(|x| Some(table.contains(x))).collect();
                return Ok(RValue::vec(Vector::Logical(result.into())));
            }
            // Otherwise compare as doubles (handles int/double/logical correctly)
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
        _ => Ok(RValue::vec(Vector::Logical(vec![Some(false)].into()))),
    }
}

// endregion

// region: matrix multiplication

/// Matrix multiplication using ndarray
fn eval_matmul(left: &RValue, right: &RValue) -> Result<RValue, RFlow> {
    fn to_matrix(val: &RValue) -> Result<(Array2<f64>, usize, usize), RError> {
        let (data, dim_attr) = match val {
            RValue::Vector(rv) => (rv.to_doubles(), rv.get_attr("dim")),
            _ => {
                return Err(RError::new(
                    RErrorKind::Type,
                    "requires numeric/complex matrix/vector arguments".to_string(),
                ))
            }
        };
        let (nrow, ncol) = match dim_attr {
            Some(RValue::Vector(rv)) => match &rv.inner {
                Vector::Integer(d) if d.len() >= 2 => (
                    usize::try_from(d[0].unwrap_or(0))?,
                    usize::try_from(d[1].unwrap_or(0))?,
                ),
                _ => (data.len(), 1), // treat as column vector
            },
            _ => (data.len(), 1), // treat as column vector
        };
        let flat: Vec<f64> = data.iter().map(|x| x.unwrap_or(f64::NAN)).collect();
        // ndarray uses row-major by default, R uses column-major
        let arr = Array2::from_shape_vec((nrow, ncol).f(), flat)
            .map_err(|source| -> RError { builtins::math::MathError::Shape { source }.into() })?;
        Ok((arr, nrow, ncol))
    }

    let (a, _arows, acols) = to_matrix(left)?;
    let (b, brows, bcols) = to_matrix(right)?;

    if acols != brows {
        return Err(RError::other(format!(
            "non-conformable arguments: {}x{} vs {}x{}",
            a.nrows(),
            acols,
            brows,
            bcols
        ))
        .into());
    }

    let c = a.dot(&b);
    let (rrows, rcols) = (c.nrows(), c.ncols());

    // Convert back to column-major R vector
    let mut result = Vec::with_capacity(rrows * rcols);
    for j in 0..rcols {
        for i in 0..rrows {
            result.push(Some(c[[i, j]]));
        }
    }

    let mut rv = RVector::from(Vector::Double(result.into()));
    rv.set_attr(
        "dim".to_string(),
        RValue::vec(Vector::Integer(
            vec![Some(i64::try_from(rrows)?), Some(i64::try_from(rcols)?)].into(),
        )),
    );
    Ok(RValue::Vector(rv))
}

// endregion
