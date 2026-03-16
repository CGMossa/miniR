//! Atomic vector types (`Vector` enum) and element-level conversions.

use super::{Character, ComplexVec, Double, Integer, Logical};
use crate::interpreter::coerce;

/// Atomic vector types in R
#[derive(Debug, Clone)]
pub enum Vector {
    Raw(Vec<u8>),
    Logical(Logical),
    Integer(Integer),
    Double(Double),
    Complex(ComplexVec),
    Character(Character),
}

impl Vector {
    pub fn len(&self) -> usize {
        match self {
            Vector::Raw(v) => v.len(),
            Vector::Logical(v) => v.len(),
            Vector::Integer(v) => v.len(),
            Vector::Double(v) => v.len(),
            Vector::Complex(v) => v.len(),
            Vector::Character(v) => v.len(),
        }
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Build a new vector by selecting elements at the given indices (with recycling).
    /// Preserves the vector type.
    pub fn select_indices(&self, indices: &[usize]) -> Vector {
        macro_rules! sel {
            ($vals:expr, $variant:ident) => {{
                let result: Vec<_> = indices
                    .iter()
                    .map(|&i| {
                        if i < $vals.len() {
                            $vals[i].clone()
                        } else {
                            Default::default()
                        }
                    })
                    .collect();
                Vector::$variant(result.into())
            }};
        }

        match self {
            Vector::Raw(vals) => {
                let result: Vec<u8> = indices
                    .iter()
                    .map(|&i| if i < vals.len() { vals[i] } else { 0 })
                    .collect();
                Vector::Raw(result)
            }
            Vector::Double(vals) => sel!(vals, Double),
            Vector::Integer(vals) => sel!(vals, Integer),
            Vector::Logical(vals) => sel!(vals, Logical),
            Vector::Complex(vals) => sel!(vals, Complex),
            Vector::Character(vals) => sel!(vals, Character),
        }
    }

    /// Get the first element as a boolean (for conditions)
    pub fn as_logical_scalar(&self) -> Option<bool> {
        match self {
            Vector::Logical(v) => v.first().copied().flatten(),
            Vector::Integer(v) => v.first().copied().flatten().map(|i| i != 0),
            Vector::Double(v) => v.first().copied().flatten().map(|f| f != 0.0),
            Vector::Complex(v) => v
                .first()
                .copied()
                .flatten()
                .map(|c| c.re != 0.0 || c.im != 0.0),
            Vector::Raw(_) | Vector::Character(_) => None,
        }
    }

    /// Get the first element as f64
    pub fn as_double_scalar(&self) -> Option<f64> {
        match self {
            Vector::Double(v) => v.first().copied().flatten(),
            Vector::Integer(v) => v.first().copied().flatten().map(coerce::i64_to_f64),
            Vector::Logical(v) => v
                .first()
                .copied()
                .flatten()
                .map(|b| if b { 1.0 } else { 0.0 }),
            Vector::Raw(v) => v.first().map(|&b| f64::from(b)),
            Vector::Complex(_) => None, // complex cannot coerce to double without losing info
            Vector::Character(v) => v.first().cloned().flatten().and_then(|s| s.parse().ok()),
        }
    }

    /// Get the first element as i64
    pub fn as_integer_scalar(&self) -> Option<i64> {
        match self {
            Vector::Integer(v) => v.first().copied().flatten(),
            Vector::Double(v) => v
                .first()
                .copied()
                .flatten()
                .and_then(|f| coerce::f64_to_i64(f).ok()),
            Vector::Logical(v) => v.first().copied().flatten().map(|b| if b { 1 } else { 0 }),
            Vector::Raw(v) => v.first().map(|&b| i64::from(b)),
            Vector::Complex(_) => None,
            Vector::Character(v) => v.first().cloned().flatten().and_then(|s| s.parse().ok()),
        }
    }

    /// Get the first element as String
    pub fn as_character_scalar(&self) -> Option<String> {
        match self {
            Vector::Character(v) => v.first().cloned().flatten(),
            Vector::Double(v) => v.first().copied().flatten().map(format_r_double),
            Vector::Integer(v) => v.first().copied().flatten().map(|i| i.to_string()),
            Vector::Logical(v) => v.first().copied().flatten().map(|b| {
                if b {
                    "TRUE".to_string()
                } else {
                    "FALSE".to_string()
                }
            }),
            Vector::Complex(v) => v.first().copied().flatten().map(format_r_complex),
            Vector::Raw(v) => v.first().map(|b| format!("{:02x}", b)),
        }
    }

    /// Convert entire vector to doubles
    pub fn to_doubles(&self) -> Vec<Option<f64>> {
        match self {
            Vector::Double(v) => v.0.clone(),
            Vector::Integer(v) => v.iter().map(|x| x.map(coerce::i64_to_f64)).collect(),
            Vector::Logical(v) => v
                .iter()
                .map(|x| x.map(|b| if b { 1.0 } else { 0.0 }))
                .collect(),
            Vector::Raw(v) => v.iter().map(|&b| Some(f64::from(b))).collect(),
            Vector::Complex(v) => v.iter().map(|x| x.map(|c| c.re)).collect(),
            Vector::Character(v) => v
                .iter()
                .map(|x| x.as_ref().and_then(|s| s.parse().ok()))
                .collect(),
        }
    }

    /// Convert entire vector to integers
    pub fn to_integers(&self) -> Vec<Option<i64>> {
        match self {
            Vector::Integer(v) => v.0.clone(),
            Vector::Double(v) => v
                .iter()
                .map(|x| x.and_then(|f| coerce::f64_to_i64(f).ok()))
                .collect(),
            Vector::Logical(v) => v.iter().map(|x| x.map(|b| if b { 1 } else { 0 })).collect(),
            Vector::Raw(v) => v.iter().map(|&b| Some(i64::from(b))).collect(),
            Vector::Complex(v) => v
                .iter()
                .map(|x| x.and_then(|c| coerce::f64_to_i64(c.re).ok()))
                .collect(),
            Vector::Character(v) => v
                .iter()
                .map(|x| x.as_ref().and_then(|s| s.parse().ok()))
                .collect(),
        }
    }

    /// Convert entire vector to characters
    pub fn to_characters(&self) -> Vec<Option<String>> {
        match self {
            Vector::Character(v) => v.0.clone(),
            Vector::Double(v) => v.iter().map(|x| x.map(format_r_double)).collect(),
            Vector::Integer(v) => v.iter().map(|x| x.map(|i| i.to_string())).collect(),
            Vector::Logical(v) => v
                .iter()
                .map(|x| {
                    x.map(|b| {
                        if b {
                            "TRUE".to_string()
                        } else {
                            "FALSE".to_string()
                        }
                    })
                })
                .collect(),
            Vector::Raw(v) => v.iter().map(|b| Some(format!("{:02x}", b))).collect(),
            Vector::Complex(v) => v.iter().map(|x| x.map(format_r_complex)).collect(),
        }
    }

    /// Convert to logicals
    pub fn to_logicals(&self) -> Vec<Option<bool>> {
        match self {
            Vector::Logical(v) => v.0.clone(),
            Vector::Integer(v) => v.iter().map(|x| x.map(|i| i != 0)).collect(),
            Vector::Double(v) => v.iter().map(|x| x.map(|f| f != 0.0)).collect(),
            Vector::Raw(v) => v.iter().map(|&b| Some(b != 0)).collect(),
            Vector::Complex(v) => v
                .iter()
                .map(|x| x.map(|c| c.re != 0.0 || c.im != 0.0))
                .collect(),
            Vector::Character(_) => vec![None; self.len()],
        }
    }

    /// Convert entire vector to complex
    pub fn to_complex(&self) -> Vec<Option<num_complex::Complex64>> {
        match self {
            Vector::Complex(v) => v.0.clone(),
            Vector::Double(v) => v
                .iter()
                .map(|x| x.map(|f| num_complex::Complex64::new(f, 0.0)))
                .collect(),
            Vector::Integer(v) => v
                .iter()
                .map(|x| x.map(|i| num_complex::Complex64::new(coerce::i64_to_f64(i), 0.0)))
                .collect(),
            Vector::Logical(v) => v
                .iter()
                .map(|x| x.map(|b| num_complex::Complex64::new(if b { 1.0 } else { 0.0 }, 0.0)))
                .collect(),
            Vector::Raw(v) => v
                .iter()
                .map(|&b| Some(num_complex::Complex64::new(f64::from(b), 0.0)))
                .collect(),
            Vector::Character(v) => v
                .iter()
                .map(|x| {
                    x.as_ref()
                        .and_then(|s| s.parse::<f64>().ok())
                        .map(|f| num_complex::Complex64::new(f, 0.0))
                })
                .collect(),
        }
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Vector::Raw(_) => "raw",
            Vector::Logical(_) => "logical",
            Vector::Integer(_) => "integer",
            Vector::Double(_) => "double",
            Vector::Complex(_) => "complex",
            Vector::Character(_) => "character",
        }
    }

    /// Convert entire vector to raw bytes (truncating to 0-255)
    pub fn to_raw(&self) -> Vec<u8> {
        match self {
            Vector::Raw(v) => v.clone(),
            Vector::Integer(v) => v
                .iter()
                .map(|x| x.map(|i| (i & 0xff) as u8).unwrap_or(0))
                .collect(),
            Vector::Double(v) => v
                .iter()
                .map(|x| x.map(|f| (f as i64 & 0xff) as u8).unwrap_or(0))
                .collect(),
            Vector::Logical(v) => v.iter().map(|x| x.map(u8::from).unwrap_or(0)).collect(),
            Vector::Complex(v) => v
                .iter()
                .map(|x| x.map(|c| (c.re as i64 & 0xff) as u8).unwrap_or(0))
                .collect(),
            Vector::Character(_) => vec![0; self.len()],
        }
    }
}

/// Format an f64 the way R does (integer-valued doubles without decimal point, Inf, NaN, etc.)
pub fn format_r_double(f: f64) -> String {
    if f.is_nan() {
        "NaN".to_string()
    } else if f.is_infinite() {
        if f > 0.0 {
            "Inf".to_string()
        } else {
            "-Inf".to_string()
        }
    } else if f == f.floor() && f.abs() < 1e15 {
        // Safe: we checked f is finite, integer-valued, and within range
        format!("{}", coerce::f64_to_i64(f).unwrap_or(0))
    } else {
        format!("{}", f)
    }
}

/// Format a complex number the way R does.
pub fn format_r_complex(c: num_complex::Complex64) -> String {
    let re = format_r_double(c.re);
    if c.im >= 0.0 || c.im.is_nan() {
        format!("{}+{}i", re, format_r_double(c.im))
    } else {
        format!("{}{}i", re, format_r_double(c.im))
    }
}
