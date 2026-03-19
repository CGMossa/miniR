//! Stub builtins — not-yet-implemented functions that return sensible defaults
//! or fail explicitly. Also includes lightweight implementations of commonly
//! needed functions that don't warrant their own module.

use crate::interpreter::value::*;
use minir_macros::{builtin, stub_builtin};

// region: Package management stubs

stub_builtin!("installed.packages");
stub_builtin!("install.packages");

// endregion

// region: C-level interface stubs

/// .Call — stub for C-level function calls. Returns NULL with a warning.
/// Many CRAN packages use .Call for compiled code we can't execute.
///
/// @param .NAME external function reference
/// @param ... arguments passed to C
/// @return NULL
/// @namespace base
#[builtin(name = ".Call")]
fn builtin_dot_call(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let name = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_else(|| "<native>".to_string());
    Err(RError::new(
        RErrorKind::Other,
        format!(".Call(\"{name}\") is not available — miniR cannot call compiled C/C++ code"),
    ))
}

/// .Internal — stub for R internal functions.
///
/// @param call the internal function call
/// @return error
/// @namespace base
#[builtin(name = ".Internal")]
fn builtin_dot_internal(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let _ = args;
    Err(RError::new(
        RErrorKind::Other,
        ".Internal() is not available in miniR".to_string(),
    ))
}

/// .External — stub for external C calls.
///
/// @param .NAME external function reference
/// @param ... arguments
/// @return error
/// @namespace base
#[builtin(name = ".External")]
fn builtin_dot_external(_args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    Err(RError::new(
        RErrorKind::Other,
        ".External() is not available — miniR cannot call compiled C code".to_string(),
    ))
}

/// .External2 — stub.
#[builtin(name = ".External2")]
fn builtin_dot_external2(_args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    Err(RError::new(
        RErrorKind::Other,
        ".External2() is not available in miniR".to_string(),
    ))
}

// endregion

// region: S4 class lookup

/// Look up an S4 class definition by name.
///
/// @param Class character string naming the class
/// @param where environment to search in (ignored)
/// @return the class definition or NULL
/// @namespace methods
#[builtin(name = "getClassDef", namespace = "methods")]
fn builtin_get_class_def(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let _name = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();
    // Return NULL — we don't have a global class definition registry accessible here.
    // The per-interpreter S4 registry is on the Interpreter struct, not accessible
    // from a plain builtin. This is enough to not crash packages that call getClassDef.
    Ok(RValue::Null)
}

/// Check if a method exists for an S4 generic.
///
/// @param f character: generic function name
/// @param signature character: method signature
/// @return logical
/// @namespace methods
#[builtin(name = "hasMethod", namespace = "methods")]
fn builtin_has_method(_args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    Ok(RValue::vec(Vector::Logical(vec![Some(false)].into())))
}

/// Set old-style S3 class for S4 compatibility.
///
/// @param Classes character vector of class names
/// @return invisible NULL
/// @namespace methods
#[builtin(name = "setOldClass", namespace = "methods")]
fn builtin_set_old_class(_args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    Ok(RValue::Null)
}

// endregion

// region: Common utilities that packages expect

/// .Deprecated — warn that a function is deprecated.
///
/// @param new replacement function name
/// @param package package name
/// @param msg custom message
/// @namespace base
#[builtin(name = ".Deprecated")]
fn builtin_deprecated(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let new = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();
    if !new.is_empty() {
        // Just warn, don't error
        return Ok(RValue::Null);
    }
    Ok(RValue::Null)
}

/// .Defunct — error that a function is defunct.
///
/// @param new replacement function name
/// @param package package name
/// @param msg custom message
/// @namespace base
#[builtin(name = ".Defunct")]
fn builtin_defunct(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let msg = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_else(|| "this function is defunct".to_string());
    Err(RError::other(msg))
}

/// packageStartupMessage — like message() but suppressable by suppressPackageStartupMessages.
///
/// @param ... message parts
/// @namespace base
#[builtin(name = "packageStartupMessage")]
fn builtin_package_startup_message(
    args: &[RValue],
    _: &[(String, RValue)],
) -> Result<RValue, RError> {
    // Just concatenate and ignore — startup messages are noise
    let _ = args;
    Ok(RValue::Null)
}

/// suppressPackageStartupMessages — suppress package startup messages.
///
/// @param expr expression to evaluate
/// @return result of expr
/// @namespace base
#[builtin(name = "suppressPackageStartupMessages", min_args = 1)]
fn builtin_suppress_pkg_startup(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    Ok(args.first().cloned().unwrap_or(RValue::Null))
}

/// is.R — check if running in R (always TRUE for miniR).
///
/// @return TRUE
/// @namespace base
#[builtin(name = "is.R")]
fn builtin_is_r(_args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    Ok(RValue::vec(Vector::Logical(vec![Some(true)].into())))
}

/// getRversion — return the R version as a character string.
///
/// @return character scalar version string
/// @namespace base
#[builtin(name = "getRversion")]
fn builtin_get_rversion(_args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    Ok(RValue::vec(Vector::Character(
        vec![Some("4.4.0".to_string())].into(),
    )))
}

/// numeric_version — create a version object (returns as character for now).
///
/// @param x character string version
/// @return version object (character scalar)
/// @namespace base
#[builtin(name = "numeric_version", min_args = 1)]
fn builtin_numeric_version(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    // Return as-is — version comparison not implemented but the object exists
    Ok(args.first().cloned().unwrap_or(RValue::Null))
}

/// package_version — create a package version object.
///
/// @param x character string version
/// @return version object
/// @namespace base
#[builtin(name = "package_version", min_args = 1)]
fn builtin_package_version(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    Ok(args.first().cloned().unwrap_or(RValue::Null))
}

/// OlsonNames — return timezone names.
///
/// @return character vector of timezone names
/// @namespace base
#[builtin(name = "OlsonNames")]
fn builtin_olson_names(_args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    Ok(RValue::vec(Vector::Character(
        vec![
            Some("UTC".to_string()),
            Some("GMT".to_string()),
            Some("US/Eastern".to_string()),
            Some("US/Central".to_string()),
            Some("US/Mountain".to_string()),
            Some("US/Pacific".to_string()),
            Some("Europe/London".to_string()),
            Some("Europe/Berlin".to_string()),
            Some("Europe/Paris".to_string()),
            Some("Asia/Tokyo".to_string()),
            Some("Australia/Sydney".to_string()),
        ]
        .into(),
    )))
}

// endregion

// region: Connection stubs

stub_builtin!("rawConnection", 1, "rawConnection() not yet implemented");
stub_builtin!("textConnection", 1, "textConnection() not yet implemented");

// endregion

// region: TLS stub (when tls feature is disabled)

#[cfg(not(feature = "tls"))]
stub_builtin!(
    "url",
    1,
    "url() requires the 'tls' feature — rebuild miniR with --features tls"
);

// endregion

stub_builtin!("arity", 1);
