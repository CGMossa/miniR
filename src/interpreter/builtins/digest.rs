//! Cryptographic digest builtins (SHA-256, SHA-512).
//!
//! Provides `digest(x, algo)` for hashing character strings and `md5(x)` as
//! an error stub directing users to SHA-256.

use sha2::{Digest, Sha256, Sha512};

use crate::interpreter::value::*;
use minir_macros::builtin;

/// Compute a cryptographic hash of a character string.
///
/// Supports SHA-256 (default) and SHA-512. Returns the hash as a lowercase
/// hex string, matching the output format of R's `digest` package.
///
/// @param x character scalar to hash
/// @param algo algorithm name: "sha256" (default) or "sha512"
/// @return character scalar containing the hex digest
#[builtin(min_args = 1, namespace = "digest")]
fn builtin_digest(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let input = match args.first() {
        Some(RValue::Vector(rv)) => match rv.to_characters().first() {
            Some(Some(s)) => s.clone(),
            Some(None) => return Ok(RValue::vec(Vector::Character(vec![None].into()))),
            None => {
                return Err(RError::new(
                    RErrorKind::Argument,
                    "argument must be a non-empty character vector".to_string(),
                ))
            }
        },
        _ => {
            return Err(RError::new(
                RErrorKind::Argument,
                "argument must be a character vector".to_string(),
            ))
        }
    };

    let algo = named
        .iter()
        .find(|(n, _)| n == "algo")
        .and_then(|(_, v)| v.as_vector().and_then(|v| v.as_character_scalar()))
        .or_else(|| {
            args.get(1)
                .and_then(|v| v.as_vector().and_then(|v| v.as_character_scalar()))
        })
        .unwrap_or_else(|| "sha256".to_string());

    let hex = match algo.as_str() {
        "sha256" => {
            let result = Sha256::digest(input.as_bytes());
            format!("{:x}", result)
        }
        "sha512" => {
            let result = Sha512::digest(input.as_bytes());
            format!("{:x}", result)
        }
        other => {
            return Err(RError::new(
                RErrorKind::Argument,
                format!(
                    "unsupported algorithm {:?} — use \"sha256\" or \"sha512\"",
                    other
                ),
            ))
        }
    };

    Ok(RValue::vec(Vector::Character(vec![Some(hex)].into())))
}

/// MD5 is deprecated — error stub suggesting SHA-256.
///
/// MD5 is cryptographically broken and should not be used for any purpose.
/// This function always errors with a suggestion to use `digest(x, algo="sha256")`.
///
/// @param x ignored
/// @return always errors
#[builtin(min_args = 0, namespace = "digest")]
fn builtin_md5(_args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    Err(RError::new(
        RErrorKind::Other,
        "md5() is not available — MD5 is cryptographically broken. \
         Use digest(x, algo=\"sha256\") for secure hashing."
            .to_string(),
    ))
}
