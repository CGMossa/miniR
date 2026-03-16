//! Cryptographic hash digest builtins.
//!
//! Provides `digest()` for computing SHA-256/SHA-512 hashes and `md5()` as a
//! stub that directs users toward SHA-256.

use crate::interpreter::value::*;
use minir_macros::builtin;
use sha2::{Digest, Sha256, Sha512};

/// Compute a cryptographic hash digest of a value.
///
/// @param x character string or raw vector to hash
/// @param algo hash algorithm: "sha256" (default), "sha512"
/// @return character string containing the hex-encoded hash
#[builtin(min_args = 1)]
fn builtin_digest(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let algo = named
        .iter()
        .find(|(n, _)| n == "algo")
        .and_then(|(_, v)| v.as_vector()?.as_character_scalar())
        .unwrap_or_else(|| "sha256".to_string());

    let bytes = extract_bytes(&args[0])?;

    let hex = match algo.as_str() {
        "sha256" => {
            let mut hasher = Sha256::new();
            hasher.update(&bytes);
            format_hex(&hasher.finalize())
        }
        "sha512" => {
            let mut hasher = Sha512::new();
            hasher.update(&bytes);
            format_hex(&hasher.finalize())
        }
        other => {
            return Err(RError::new(
                RErrorKind::Argument,
                format!(
                    "unsupported algorithm '{}'. Supported algorithms: \"sha256\", \"sha512\"",
                    other
                ),
            ));
        }
    };

    Ok(RValue::vec(Vector::Character(vec![Some(hex)].into())))
}

/// MD5 hash (stub) --- returns an error directing users to sha256.
///
/// MD5 is cryptographically broken and should not be used for any
/// security-sensitive purpose. Use `digest(x, algo = "sha256")` instead.
///
/// @param x value to hash (ignored)
/// @return error
#[builtin(min_args = 1)]
fn builtin_md5(args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    let _ = args;
    Err(RError::new(
        RErrorKind::Other,
        "md5() is not supported because MD5 is cryptographically broken. \
         Use digest(x, algo = \"sha256\") for a secure hash instead."
            .to_string(),
    ))
}

/// Extract bytes from a character string or raw vector for hashing.
fn extract_bytes(value: &RValue) -> Result<Vec<u8>, RError> {
    match value {
        RValue::Vector(rv) => match &rv.inner {
            Vector::Raw(bytes) => Ok(bytes.clone()),
            Vector::Character(chars) => {
                let s = chars.first().cloned().flatten().ok_or_else(|| {
                    RError::new(
                        RErrorKind::Argument,
                        "NA character value cannot be hashed".to_string(),
                    )
                })?;
                Ok(s.into_bytes())
            }
            other => Err(RError::new(
                RErrorKind::Argument,
                format!(
                    "digest() requires a character string or raw vector, got {}",
                    other.type_name()
                ),
            )),
        },
        _ => Err(RError::new(
            RErrorKind::Argument,
            format!(
                "digest() requires a character string or raw vector, got {}",
                value.type_name()
            ),
        )),
    }
}

/// Format a byte slice as a lowercase hex string.
fn format_hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut hex = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        let _ = write!(hex, "{:02x}", b);
    }
    hex
}
