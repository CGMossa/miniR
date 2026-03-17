//! Cryptographic digest builtins (SHA-256, SHA-512, BLAKE3).
//!
//! Provides `digest(x, algo)` for hashing character strings, `md5(x)` as
//! an error stub directing users to SHA-256, and BLAKE3 builtins for fast
//! hashing of strings, raw vectors, and files.

#[cfg(feature = "digest")]
use sha2::{Digest, Sha256, Sha512};

use crate::interpreter::value::*;
use minir_macros::builtin;

// region: helpers

/// Extract a character or raw input from the first argument.
///
/// Returns `Ok(bytes)` for the input data, or the appropriate error/NA result.
fn extract_input_bytes(args: &[RValue]) -> Result<Result<Vec<u8>, RValue>, RError> {
    match args.first() {
        Some(RValue::Vector(rv)) => match &**rv {
            Vector::Raw(bytes) => Ok(Ok(bytes.clone())),
            Vector::Character(chars) => match chars.first() {
                Some(Some(s)) => Ok(Ok(s.as_bytes().to_vec())),
                Some(None) => Ok(Err(RValue::vec(Vector::Character(vec![None].into())))),
                None => Err(RError::new(
                    RErrorKind::Argument,
                    "argument must be a non-empty character vector".to_string(),
                )),
            },
            _ => match rv.to_characters().first() {
                Some(Some(s)) => Ok(Ok(s.as_bytes().to_vec())),
                Some(None) => Ok(Err(RValue::vec(Vector::Character(vec![None].into())))),
                None => Err(RError::new(
                    RErrorKind::Argument,
                    "argument must be a non-empty character or raw vector".to_string(),
                )),
            },
        },
        _ => Err(RError::new(
            RErrorKind::Argument,
            "argument must be a character or raw vector".to_string(),
        )),
    }
}

// endregion

// region: digest

/// Compute a cryptographic hash of a character string.
///
/// Supports SHA-256 (default), SHA-512, and BLAKE3 (when the blake3 feature
/// is enabled). Returns the hash as a lowercase hex string, matching the
/// output format of R's `digest` package.
///
/// @param x character scalar to hash
/// @param algo algorithm name: "sha256" (default), "sha512", or "blake3"
/// @return character scalar containing the hex digest
#[cfg(feature = "digest")]
#[builtin(min_args = 1, namespace = "digest")]
fn builtin_digest(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let input = match extract_input_bytes(args)? {
        Ok(bytes) => bytes,
        Err(na) => return Ok(na),
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
            let result = Sha256::digest(&input);
            format!("{:x}", result)
        }
        "sha512" => {
            let result = Sha512::digest(&input);
            format!("{:x}", result)
        }
        #[cfg(feature = "blake3")]
        "blake3" => {
            let result = blake3::hash(&input);
            result.to_hex().to_string()
        }
        other => {
            #[cfg(feature = "blake3")]
            let supported = "\"sha256\", \"sha512\", or \"blake3\"";
            #[cfg(not(feature = "blake3"))]
            let supported = "\"sha256\" or \"sha512\"";
            return Err(RError::new(
                RErrorKind::Argument,
                format!(
                    "unsupported algorithm {:?} \u{2014} use {}",
                    other, supported
                ),
            ));
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
#[cfg(feature = "digest")]
#[builtin(min_args = 0, namespace = "digest")]
fn builtin_md5(_args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    Err(RError::new(
        RErrorKind::Other,
        "md5() is not available \u{2014} MD5 is cryptographically broken. \
         Use digest(x, algo=\"sha256\") for secure hashing."
            .to_string(),
    ))
}

// endregion

// region: blake3

/// Compute a BLAKE3 hash of a character string or raw vector.
///
/// Returns the hash as a 64-character lowercase hex string.
///
/// @param x character scalar or raw vector to hash
/// @return character scalar containing the 64-char hex digest
#[cfg(feature = "blake3")]
#[builtin(min_args = 1)]
fn builtin_blake3(args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    let input = match extract_input_bytes(args)? {
        Ok(bytes) => bytes,
        Err(na) => return Ok(na),
    };
    let hash = blake3::hash(&input);
    Ok(RValue::vec(Vector::Character(
        vec![Some(hash.to_hex().to_string())].into(),
    )))
}

/// Compute a BLAKE3 hash and return as a 32-byte raw vector.
///
/// @param x character scalar or raw vector to hash
/// @return raw vector of 32 bytes (the BLAKE3 digest)
#[cfg(feature = "blake3")]
#[builtin(name = "blake3_raw", min_args = 1)]
fn builtin_blake3_raw(args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    let input = match extract_input_bytes(args)? {
        Ok(bytes) => bytes,
        Err(na) => return Ok(na),
    };
    let hash = blake3::hash(&input);
    Ok(RValue::vec(Vector::Raw(hash.as_bytes().to_vec())))
}

/// Compute a BLAKE3 hash of a file's contents using streaming I/O.
///
/// Reads the file in chunks to avoid loading the entire file into memory,
/// making it efficient for large files.
///
/// @param path character scalar: path to the file to hash
/// @return character scalar containing the 64-char hex digest
#[cfg(feature = "blake3")]
#[builtin(name = "blake3_file", min_args = 1)]
fn builtin_blake3_file(args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    let path = match args.first() {
        Some(RValue::Vector(rv)) => rv.as_character_scalar().ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "blake3_file() requires a character scalar path".to_string(),
            )
        })?,
        _ => {
            return Err(RError::new(
                RErrorKind::Argument,
                "blake3_file() requires a character scalar path".to_string(),
            ))
        }
    };

    let file = std::fs::File::open(&path).map_err(|e| {
        RError::new(
            RErrorKind::Other,
            format!("cannot open file {:?}: {}", path, e),
        )
    })?;

    let mut reader = std::io::BufReader::new(file);
    let mut hasher = blake3::Hasher::new();
    std::io::copy(&mut reader, &mut hasher).map_err(|e| {
        RError::new(
            RErrorKind::Other,
            format!("error reading file {:?}: {}", path, e),
        )
    })?;

    let hash = hasher.finalize();
    Ok(RValue::vec(Vector::Character(
        vec![Some(hash.to_hex().to_string())].into(),
    )))
}

// endregion
