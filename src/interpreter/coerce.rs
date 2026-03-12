//! Safe numeric conversion helpers.
//!
//! Only contains helpers for conversions where std's `From`/`TryFrom` traits
//! don't exist. For integer↔integer conversions, use `From`/`TryFrom` directly
//! (with `?` — `From<TryFromIntError>` is implemented for `RError`).

use super::value::{RError, RErrorKind};

// region: float → integer (no std TryFrom for f64 → int)

/// `f64 → i64` — truncation toward zero (R `as.integer()` semantics).
/// Fails for NaN, ±Inf, or values outside `i64` range.
#[inline]
pub fn f64_to_i64(v: f64) -> Result<i64, RError> {
    if v.is_nan() {
        return Err(RError::new(
            RErrorKind::Type,
            "NaN cannot be converted to integer",
        ));
    }
    if v.is_infinite() {
        return Err(RError::new(
            RErrorKind::Type,
            format!(
                "{}Inf cannot be converted to integer",
                if v < 0.0 { "-" } else { "" }
            ),
        ));
    }
    // Check range before truncating
    if v > i64::MAX as f64 || v < i64::MIN as f64 {
        return Err(RError::new(
            RErrorKind::Type,
            format!("value {v} out of integer range"),
        ));
    }
    Ok(v as i64) // intentional truncation toward zero
}

/// `f64 → usize` — truncation toward zero, then check non-negative.
#[inline]
#[allow(dead_code)]
pub fn f64_to_usize(v: f64) -> Result<usize, RError> {
    let i = f64_to_i64(v)?;
    Ok(usize::try_from(i)?)
}

/// `f64 → u64` — truncation toward zero, then check non-negative.
#[inline]
pub fn f64_to_u64(v: f64) -> Result<u64, RError> {
    let i = f64_to_i64(v)?;
    Ok(u64::try_from(i)?)
}

/// `f64 → i32` — truncation toward zero, then check range.
#[inline]
pub fn f64_to_i32(v: f64) -> Result<i32, RError> {
    let i = f64_to_i64(v)?;
    Ok(i32::try_from(i)?)
}

/// `f64 → u32` — truncation toward zero, then check range.
#[inline]
#[allow(dead_code)]
pub fn f64_to_u32(v: f64) -> Result<u32, RError> {
    let i = f64_to_i64(v)?;
    Ok(u32::try_from(i)?)
}

// endregion

// region: integer → float (lossy but always succeeds — no std From)

/// `i64 → f64` — always produces a valid f64, but may lose precision
/// for `|v| > 2^53`. This matches R semantics where `as.double()` on an
/// integer always succeeds.
#[inline]
pub fn i64_to_f64(v: i64) -> f64 {
    v as f64
}

/// `usize → f64` — always produces a valid f64, but may lose precision
/// for `v > 2^53`.
#[inline]
pub fn usize_to_f64(v: usize) -> f64 {
    v as f64
}

/// `u64 → f64` — always produces a valid f64, but may lose precision
/// for `v > 2^53`.
#[inline]
pub fn u64_to_f64(v: u64) -> f64 {
    v as f64
}

// endregion
