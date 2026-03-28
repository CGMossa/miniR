//! Stub builtins — not-yet-implemented functions that return sensible defaults
//! or fail explicitly. Also includes lightweight implementations of commonly
//! needed functions that don't warrant their own module.

use crate::interpreter::value::*;
use crate::interpreter::BuiltinContext;
use minir_macros::{builtin, interpreter_builtin, stub_builtin};

// region: Package management stubs

stub_builtin!("installed.packages");
stub_builtin!("install.packages");

// endregion

// region: C-level interface stubs

#[cfg(not(feature = "native"))]
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
fn builtin_dot_internal(_args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
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
#[interpreter_builtin(name = "hasMethod", namespace = "methods")]
fn interp_has_method(
    _args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    context
        .interpreter()
        .write_stderr("[miniR stub] hasMethod() is a no-op in miniR — always returns FALSE\n");
    Ok(RValue::vec(Vector::Logical(vec![Some(false)].into())))
}

/// Set old-style S3 class for S4 compatibility.
///
/// @param Classes character vector of class names
/// @return invisible NULL
/// @namespace methods
#[interpreter_builtin(name = "setOldClass", namespace = "methods")]
fn interp_set_old_class(
    _args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    context
        .interpreter()
        .write_stderr("[miniR stub] setOldClass() is a no-op in miniR\n");
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
    _args: &[RValue],
    _: &[(String, RValue)],
) -> Result<RValue, RError> {
    // Silently ignored — startup messages are informational only
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

// region: Namespace utilities

/// Get a namespace environment by name (string → environment).
///
/// Delegates to the same logic as `getNamespace()`: checks loaded namespaces,
/// attempts to load unloaded namespaces, and falls back to the base environment
/// for built-in namespaces.
///
/// @param ns character scalar: namespace name
/// @return environment
/// @namespace base
#[interpreter_builtin(name = "asNamespace", min_args = 1)]
fn interp_as_namespace(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let ns = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| RError::new(RErrorKind::Argument, "invalid namespace name".to_string()))?;

    // Check loaded packages first
    let loaded_ns = context.with_interpreter(|interp| {
        interp
            .loaded_namespaces
            .borrow()
            .get(&ns)
            .map(|loaded| loaded.namespace_env.clone())
    });

    if let Some(env) = loaded_ns {
        return Ok(RValue::Environment(env));
    }

    // Try to load the namespace if it's not already loaded
    let loaded_env = context.with_interpreter(|interp| interp.load_namespace(&ns).ok());

    if let Some(env) = loaded_env {
        return Ok(RValue::Environment(env));
    }

    // Fall back to base env for builtin namespaces (base, utils, stats, etc.)
    let env = context.with_interpreter(|interp| interp.base_env());
    Ok(RValue::Environment(env))
}

/// getFromNamespace — look up a name in a package namespace.
///
/// @param x character: the name to look up
/// @param ns character or namespace environment
/// @return the value of the name in the namespace
/// @namespace utils
#[interpreter_builtin(name = "getFromNamespace", min_args = 2)]
fn interp_get_from_namespace(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let name = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "getFromNamespace: 'x' must be a character string".to_string(),
            )
        })?;

    // Get namespace env — either from a string name or directly as environment
    let ns_env = match args.get(1) {
        Some(RValue::Environment(env)) => env.clone(),
        Some(val) => {
            let ns_name = val
                .as_vector()
                .and_then(|v| v.as_character_scalar())
                .ok_or_else(|| {
                    RError::new(
                        RErrorKind::Argument,
                        "getFromNamespace: 'ns' must be a string or namespace".to_string(),
                    )
                })?;
            // Look up the namespace
            let loaded = context.with_interpreter(|interp| {
                interp
                    .loaded_namespaces
                    .borrow()
                    .get(&ns_name)
                    .map(|ns| ns.namespace_env.clone())
            });
            match loaded {
                Some(env) => env,
                None => context.with_interpreter(|interp| interp.load_namespace(&ns_name))?,
            }
        }
        None => {
            return Err(RError::new(
                RErrorKind::Argument,
                "getFromNamespace: 'ns' argument is missing".to_string(),
            ))
        }
    };

    ns_env.get(&name).ok_or_else(|| {
        RError::new(
            RErrorKind::Other,
            format!("object '{name}' not found in namespace"),
        )
    })
}

/// Get the name of a namespace environment.
///
/// @param ns namespace environment
/// @return character scalar
/// @namespace base
#[builtin(name = "getNamespaceName", min_args = 1)]
fn builtin_get_namespace_name(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Environment(env)) => {
            let name = env.name().unwrap_or_default();
            Ok(RValue::vec(Vector::Character(vec![Some(name)].into())))
        }
        _ => Ok(RValue::vec(Vector::Character(
            vec![Some(String::new())].into(),
        ))),
    }
}

/// Check if an object is a namespace environment.
///
/// Returns TRUE if the object is an environment whose name starts with
/// "namespace:" (indicating it was created by the package loader), or if it
/// matches one of the loaded namespace environments in the interpreter's
/// registry.
///
/// @param ns object to check
/// @return logical
/// @namespace base
#[interpreter_builtin(name = "isNamespace", min_args = 1)]
fn interp_is_namespace(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let is_ns = match args.first() {
        Some(RValue::Environment(env)) => {
            // Check if the environment has a "namespace:" prefix in its name
            let has_ns_name = env
                .name()
                .map(|n| n.starts_with("namespace:"))
                .unwrap_or(false);

            if has_ns_name {
                true
            } else {
                // Check if the environment is in the loaded_namespaces registry
                context.with_interpreter(|interp| {
                    interp
                        .loaded_namespaces
                        .borrow()
                        .values()
                        .any(|loaded| loaded.namespace_env.ptr_eq(env))
                })
            }
        }
        _ => false,
    };
    Ok(RValue::vec(Vector::Logical(vec![Some(is_ns)].into())))
}

/// Get the top-level environment.
///
/// @param envir starting environment
/// @return the top-level environment (global or namespace)
/// @namespace base
#[builtin(name = "topenv", min_args = 0)]
fn builtin_topenv(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Environment(env)) => Ok(RValue::Environment(env.clone())),
        _ => Ok(RValue::Null),
    }
}

// endregion

// region: Date/time constructors

/// .POSIXct — construct a POSIXct object from numeric seconds.
///
/// @param xx numeric: seconds since epoch
/// @param tz character: timezone (default "")
/// @return POSIXct object
/// @namespace base
#[builtin(name = ".POSIXct", min_args = 1)]
fn builtin_dot_posixct(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let val = args.first().cloned().unwrap_or(RValue::Null);
    match val {
        RValue::Vector(mut rv) => {
            rv.set_attr(
                "class".to_string(),
                RValue::vec(Vector::Character(
                    vec![Some("POSIXct".to_string()), Some("POSIXt".to_string())].into(),
                )),
            );
            Ok(RValue::Vector(rv))
        }
        _ => Ok(val),
    }
}

/// .POSIXlt — construct a POSIXlt object (list-based time).
///
/// @param xx numeric or list
/// @param tz character: timezone
/// @return POSIXlt object
/// @namespace base
#[builtin(name = ".POSIXlt", min_args = 1)]
fn builtin_dot_posixlt(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let val = args.first().cloned().unwrap_or(RValue::Null);
    // Just tag with class — full POSIXlt structure not implemented
    match val {
        RValue::List(mut list) => {
            let mut attrs = *list.attrs.take().unwrap_or_default();
            attrs.insert(
                "class".to_string(),
                RValue::vec(Vector::Character(
                    vec![Some("POSIXlt".to_string()), Some("POSIXt".to_string())].into(),
                )),
            );
            list.attrs = Some(Box::new(attrs));
            Ok(RValue::List(list))
        }
        _ => Ok(val),
    }
}

/// .Date — construct a Date object from numeric days since epoch.
///
/// @param xx numeric: days since 1970-01-01
/// @return Date object
/// @namespace base
#[builtin(name = ".Date", min_args = 1)]
fn builtin_dot_date(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let val = args.first().cloned().unwrap_or(RValue::Null);
    match val {
        RValue::Vector(mut rv) => {
            rv.set_attr(
                "class".to_string(),
                RValue::vec(Vector::Character(vec![Some("Date".to_string())].into())),
            );
            Ok(RValue::Vector(rv))
        }
        _ => Ok(val),
    }
}

/// .difftime — construct a difftime object.
///
/// @param xx numeric value
/// @param units character: time units
/// @return difftime object
/// @namespace base
#[builtin(name = ".difftime", min_args = 1)]
fn builtin_dot_difftime(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let val = args.first().cloned().unwrap_or(RValue::Null);
    let units = named
        .iter()
        .find(|(n, _)| n == "units")
        .and_then(|(_, v)| v.as_vector()?.as_character_scalar())
        .or_else(|| {
            args.get(1)
                .and_then(|v| v.as_vector()?.as_character_scalar())
        })
        .unwrap_or_else(|| "secs".to_string());
    match val {
        RValue::Vector(mut rv) => {
            rv.set_attr(
                "class".to_string(),
                RValue::vec(Vector::Character(vec![Some("difftime".to_string())].into())),
            );
            rv.set_attr(
                "units".to_string(),
                RValue::vec(Vector::Character(vec![Some(units)].into())),
            );
            Ok(RValue::Vector(rv))
        }
        _ => Ok(val),
    }
}

// endregion

// region: Fast subset primitives

/// .subset — fast subset without method dispatch.
///
/// @param x object to subset
/// @param ... indices
/// @return subset of x
/// @namespace base
#[builtin(name = ".subset", min_args = 1)]
fn builtin_dot_subset(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    if args.len() < 2 {
        return Ok(args.first().cloned().unwrap_or(RValue::Null));
    }
    // Delegate to regular [ indexing logic
    let obj = &args[0];
    let idx = &args[1];
    match (obj, idx) {
        (RValue::List(list), RValue::Vector(iv)) => {
            if let Some(name) = iv.as_character_scalar() {
                for (n, v) in &list.values {
                    if n.as_deref() == Some(&name) {
                        return Ok(v.clone());
                    }
                }
                Ok(RValue::Null)
            } else {
                let i = iv.as_integer_scalar().unwrap_or(0) as usize;
                if i > 0 && i <= list.values.len() {
                    Ok(RValue::List(RList::new(vec![list.values[i - 1].clone()])))
                } else {
                    Ok(RValue::Null)
                }
            }
        }
        (RValue::Vector(v), RValue::Vector(iv)) => {
            let i = iv.as_integer_scalar().unwrap_or(0) as usize;
            if i > 0 && i <= v.len() {
                Ok(crate::interpreter::indexing::extract_vector_element(
                    v,
                    i - 1,
                ))
            } else {
                Ok(RValue::Null)
            }
        }
        _ => Ok(RValue::Null),
    }
}

/// .subset2 — fast [[ without method dispatch.
///
/// @param x object
/// @param i index
/// @return element
/// @namespace base
#[builtin(name = ".subset2", min_args = 2)]
fn builtin_dot_subset2(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    // Same as [[ — extract single element
    let obj = &args[0];
    let idx = &args[1];
    match (obj, idx) {
        (RValue::List(list), RValue::Vector(iv)) => {
            if let Some(name) = iv.as_character_scalar() {
                for (n, v) in &list.values {
                    if n.as_deref() == Some(&name) {
                        return Ok(v.clone());
                    }
                }
                Ok(RValue::Null)
            } else {
                let i = iv.as_integer_scalar().unwrap_or(0) as usize;
                if i > 0 && i <= list.values.len() {
                    Ok(list.values[i - 1].1.clone())
                } else {
                    Ok(RValue::Null)
                }
            }
        }
        (RValue::Vector(v), RValue::Vector(iv)) => {
            if let Vector::Character(idx_names) = &iv.inner {
                if let Some(Some(name)) = idx_names.first() {
                    if let Some(names_attr) = v.get_attr("names") {
                        if let Some(names_vec) = names_attr.as_vector() {
                            let name_strs = names_vec.to_characters();
                            for (j, n) in name_strs.iter().enumerate() {
                                if n.as_deref() == Some(name.as_str()) && j < v.len() {
                                    return Ok(
                                        crate::interpreter::indexing::extract_vector_element(v, j),
                                    );
                                }
                            }
                        }
                    }
                    return Ok(RValue::Null);
                }
            }
            let i = iv.as_integer_scalar().unwrap_or(0) as usize;
            if i > 0 && i <= v.len() {
                Ok(crate::interpreter::indexing::extract_vector_element(
                    v,
                    i - 1,
                ))
            } else {
                Ok(RValue::Null)
            }
        }
        _ => Ok(RValue::Null),
    }
}

// endregion

// region: S4 dispatch and misc stubs

/// standardGeneric — S4 method dispatch.
///
/// @param f character: generic function name
/// @return dispatched result
/// @namespace methods
#[builtin(name = "standardGeneric", namespace = "methods", min_args = 1)]
fn builtin_standard_generic(_args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    Err(RError::new(
        RErrorKind::Other,
        "standardGeneric() dispatch not yet implemented — use S3 methods instead".to_string(),
    ))
}

/// setIs — define class inheritance relationship.
/// @namespace methods
#[interpreter_builtin(name = "setIs", namespace = "methods")]
fn interp_set_is(
    _args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    context
        .interpreter()
        .write_stderr("[miniR stub] setIs() is a no-op in miniR\n");
    Ok(RValue::Null)
}

/// removeClass — remove a class definition.
/// @namespace methods
#[interpreter_builtin(name = "removeClass", namespace = "methods")]
fn interp_remove_class(
    _args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    context
        .interpreter()
        .write_stderr("[miniR stub] removeClass() is a no-op in miniR\n");
    Ok(RValue::Null)
}

/// resetGeneric — reset a generic function.
/// @namespace methods
#[interpreter_builtin(name = "resetGeneric", namespace = "methods")]
fn interp_reset_generic(
    _args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    context
        .interpreter()
        .write_stderr("[miniR stub] resetGeneric() is a no-op in miniR\n");
    Ok(RValue::Null)
}

/// Encoding<- — set string encoding (no-op in UTF-8-only miniR).
/// @namespace base
#[builtin(name = "Encoding<-", min_args = 2)]
fn builtin_encoding_set(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    // Return the string unchanged — miniR is always UTF-8
    Ok(args.first().cloned().unwrap_or(RValue::Null))
}

/// bindtextdomain — bind a text domain for translations (no-op).
/// @namespace base
#[interpreter_builtin(name = "bindtextdomain")]
fn interp_bindtextdomain(
    _args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    context
        .interpreter()
        .write_stderr("[miniR stub] bindtextdomain() is a no-op in miniR — no i18n support\n");
    Ok(RValue::Null)
}

/// eapply — apply function over environment bindings.
/// @namespace base
#[interpreter_builtin(name = "eapply", min_args = 2)]
fn interp_eapply(
    _args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    context
        .interpreter()
        .write_stderr("[miniR stub] eapply() is a stub in miniR — returns empty list\n");
    Ok(RValue::List(RList::new(vec![])))
}

/// unlockBinding — unlock a locked binding.
/// @namespace base
#[builtin(name = "unlockBinding", min_args = 2)]
fn builtin_unlock_binding(_args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    // Binding locks are advisory — silently succeed
    Ok(RValue::Null)
}

/// sys.status — return system status (call stack info).
/// @namespace base
#[builtin(name = "sys.status")]
fn builtin_sys_status(_args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    Ok(RValue::List(RList::new(vec![
        (
            Some("sys.calls".to_string()),
            RValue::List(RList::new(vec![])),
        ),
        (
            Some("sys.parents".to_string()),
            RValue::vec(Vector::Integer(vec![].into())),
        ),
        (
            Some("sys.frames".to_string()),
            RValue::List(RList::new(vec![])),
        ),
    ])))
}

/// file.access — check file access permissions.
/// @namespace base
#[builtin(name = "file.access", min_args = 1)]
fn builtin_file_access(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let path = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();
    let exists = std::path::Path::new(&path).exists();
    Ok(RValue::vec(Vector::Integer(
        vec![Some(if exists { 0 } else { -1 })].into(),
    )))
}

/// serialize/unserialize — R object serialization (delegates to our RDS functions).
/// @namespace base
#[builtin(name = "serialize", min_args = 2)]
fn builtin_serialize(_args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    Err(RError::new(
        RErrorKind::Other,
        "serialize() to connection not yet implemented — use saveRDS() instead".to_string(),
    ))
}

/// @namespace base
#[builtin(name = "unserialize", min_args = 1)]
fn builtin_unserialize(_args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    Err(RError::new(
        RErrorKind::Other,
        "unserialize() from connection not yet implemented — use readRDS() instead".to_string(),
    ))
}

/// tracemem/untracemem/retracemem — memory tracing (no-ops).
/// @namespace base
#[builtin(name = "tracemem", min_args = 1)]
fn builtin_tracemem(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    Ok(RValue::vec(Vector::Character(
        vec![Some(format!("<{:p}>", &args[0]))].into(),
    )))
}
#[builtin(name = "untracemem", min_args = 1)]
fn builtin_untracemem(_args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    Ok(RValue::Null)
}
#[builtin(name = "retracemem")]
fn builtin_retracemem(_args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    Ok(RValue::Null)
}

// endregion

// region: Connection stubs

stub_builtin!("rawConnection", 1, "rawConnection() not yet implemented");
stub_builtin!("textConnection", 1, "textConnection() not yet implemented");
stub_builtin!("pipe", 1, "pipe() not yet implemented");
stub_builtin!("fifo", 1, "fifo() not yet implemented");
stub_builtin!(
    "socketConnection",
    1,
    "socketConnection() not yet implemented — use make.socket() instead"
);
stub_builtin!("gzcon", 1, "gzcon() not yet implemented");
stub_builtin!("readBin", 1, "readBin() not yet implemented");
stub_builtin!("writeBin", 1, "writeBin() not yet implemented");
stub_builtin!("readChar", 1, "readChar() not yet implemented");
stub_builtin!("writeChar", 1, "writeChar() not yet implemented");
stub_builtin!("memCompress", 1, "memCompress() not yet implemented");
stub_builtin!("memDecompress", 1, "memDecompress() not yet implemented");

// endregion

// region: Interactive/session utilities

/// interactive — check if R is running interactively.
///
/// @return logical scalar
/// @namespace base
#[builtin(name = "interactive")]
fn builtin_interactive(_args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    // Return TRUE for REPL, FALSE for scripts — for now always FALSE
    // since we can't distinguish from a plain builtin
    Ok(RValue::vec(Vector::Logical(vec![Some(false)].into())))
}

/// eval.parent — evaluate expression in the calling environment.
///
/// @param expr expression to evaluate
/// @param n number of frames to go up (default 1)
/// @return result of evaluation
/// @namespace base
#[builtin(name = "eval.parent", min_args = 1)]
fn builtin_eval_parent(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    // Simplified — just return the first arg (can't access call stack from plain builtin)
    Ok(args.first().cloned().unwrap_or(RValue::Null))
}

/// length<- — set the length of a vector.
///
/// @param x vector
/// @param value new length
/// @return vector with adjusted length (truncated or extended with NA)
/// @namespace base
#[builtin(name = "length<-", min_args = 2)]
fn builtin_length_set(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let new_len = args
        .get(1)
        .and_then(|v| v.as_vector()?.as_integer_scalar())
        .unwrap_or(0) as usize;

    match args.first() {
        Some(RValue::Vector(v)) => {
            let mut doubles = v.to_doubles();
            doubles.resize(new_len, None);
            Ok(RValue::vec(Vector::Double(doubles.into())))
        }
        Some(RValue::List(list)) => {
            let mut values = list.values.clone();
            values.resize(new_len, (None, RValue::Null));
            Ok(RValue::List(RList::new(values)))
        }
        _ => Ok(RValue::Null),
    }
}

/// levels<- — set factor levels.
///
/// @param x factor
/// @param value new levels
/// @return factor with updated levels
/// @namespace base
#[builtin(name = "levels<-", min_args = 2)]
fn builtin_levels_set(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(v)) => {
            let mut rv = v.clone();
            let new_levels = args.get(1).cloned().unwrap_or(RValue::Null);
            rv.set_attr("levels".to_string(), new_levels);
            Ok(RValue::Vector(rv))
        }
        _ => Ok(args.first().cloned().unwrap_or(RValue::Null)),
    }
}

/// file.append — append contents of one file to another.
///
/// @param file1 destination file
/// @param file2 source file
/// @return logical scalar
/// @namespace base
#[builtin(name = "file.append", min_args = 2)]
fn builtin_file_append(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let file1 = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();
    let file2 = args
        .get(1)
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();
    let result = (|| {
        let content = std::fs::read(&file2).ok()?;
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new().append(true).open(&file1).ok()?;
        f.write_all(&content).ok()
    })();
    Ok(RValue::vec(Vector::Logical(
        vec![Some(result.is_some())].into(),
    )))
}

/// Cstack_info — C stack info (returns dummy values).
/// @namespace base
#[builtin(name = "Cstack_info")]
fn builtin_cstack_info(_args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let mut rv = RVector::from(Vector::Integer(
        vec![Some(8388608), Some(16384), Some(0)].into(),
    ));
    rv.set_attr(
        "names".to_string(),
        RValue::vec(Vector::Character(
            vec![
                Some("size".to_string()),
                Some("current".to_string()),
                Some("direction".to_string()),
            ]
            .into(),
        )),
    );
    Ok(RValue::Vector(rv))
}

/// extSoftVersion — external software version info.
/// @namespace base
#[builtin(name = "extSoftVersion")]
fn builtin_ext_soft_version(_args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let mut rv = RVector::from(Vector::Character(
        vec![
            Some("".to_string()),
            Some("1.1.1".to_string()),
            Some("".to_string()),
            Some("".to_string()),
        ]
        .into(),
    ));
    rv.set_attr(
        "names".to_string(),
        RValue::vec(Vector::Character(
            vec![
                Some("zlib".to_string()),
                Some("bzlib".to_string()),
                Some("xz".to_string()),
                Some("PCRE".to_string()),
            ]
            .into(),
        )),
    );
    Ok(RValue::Vector(rv))
}

// endregion

// region: Dynamic loading stubs (C/Fortran shared libs — only when native feature is off)

#[cfg(not(feature = "native"))]
stub_builtin!(
    "dyn.load",
    1,
    "dyn.load() not available — miniR cannot load compiled shared libraries"
);
#[cfg(not(feature = "native"))]
stub_builtin!("dyn.unload", 1, "dyn.unload() not available");
#[cfg(not(feature = "native"))]
stub_builtin!(
    "library.dynam",
    1,
    "library.dynam() not available — miniR cannot load compiled code"
);
#[cfg(not(feature = "native"))]
stub_builtin!(
    "library.dynam.unload",
    1,
    "library.dynam.unload() not available"
);

#[cfg(not(feature = "native"))]
/// is.loaded — check if a C symbol is loaded (always FALSE).
/// @namespace base
#[builtin(name = "is.loaded", min_args = 1)]
fn builtin_is_loaded(_args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    Ok(RValue::vec(Vector::Logical(vec![Some(false)].into())))
}

#[cfg(not(feature = "native"))]
/// getNativeSymbolInfo — get info about a loaded symbol (always errors).
/// @namespace base
#[builtin(name = "getNativeSymbolInfo", min_args = 1)]
fn builtin_get_native_symbol_info(
    args: &[RValue],
    _: &[(String, RValue)],
) -> Result<RValue, RError> {
    let name = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();
    Err(RError::new(
        RErrorKind::Other,
        format!("no such symbol '{name}' — miniR cannot load native code"),
    ))
}

// endregion

// region: Debugging stubs

/// debugonce — set a one-time debug flag (no-op).
/// @namespace base
#[interpreter_builtin(name = "debugonce")]
fn interp_debugonce(
    _args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    context
        .interpreter()
        .write_stderr("[miniR stub] debugonce() is a no-op in miniR — no debugger\n");
    Ok(RValue::Null)
}

/// trace — set tracing on a function (no-op stub).
/// @namespace base
#[interpreter_builtin(name = "trace")]
fn interp_trace(
    _args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    context
        .interpreter()
        .write_stderr("[miniR stub] trace() is a no-op in miniR — no debugger\n");
    Ok(RValue::Null)
}

/// untrace — remove tracing (no-op stub).
/// @namespace base
#[interpreter_builtin(name = "untrace")]
fn interp_untrace(
    _args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    context
        .interpreter()
        .write_stderr("[miniR stub] untrace() is a no-op in miniR — no debugger\n");
    Ok(RValue::Null)
}

/// browseEnv — open environment browser (no-op stub).
/// @namespace utils
#[interpreter_builtin(name = "browseEnv", namespace = "utils")]
fn interp_browse_env(
    _args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    context
        .interpreter()
        .write_stderr("[miniR stub] browseEnv() is a no-op in miniR\n");
    Ok(RValue::Null)
}

// endregion

// region: Connection management stubs

stub_builtin!("sink", 0, "sink() not yet implemented");
stub_builtin!("flush", 1, "flush() not yet implemented for connections");
stub_builtin!(
    "showConnections",
    0,
    "showConnections() not yet implemented"
);
stub_builtin!(
    "getAllConnections",
    0,
    "getAllConnections() not yet implemented"
);
stub_builtin!("pushBack", 1, "pushBack() not yet implemented");
stub_builtin!("pushBackLength", 1);
stub_builtin!("clearPushBack", 1);
stub_builtin!("seek", 1, "seek() not yet implemented");
stub_builtin!("truncate", 1, "truncate() not yet implemented");
stub_builtin!("isSeekable", 1);
stub_builtin!("isIncomplete", 1);
stub_builtin!("summary.connection", 1);

// endregion

// region: Filesystem stubs

/// Sys.readlink — read a symbolic link target.
/// @namespace base
#[builtin(name = "Sys.readlink", min_args = 1)]
fn builtin_sys_readlink(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let path = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();
    let target = std::fs::read_link(&path)
        .ok()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    Ok(RValue::vec(Vector::Character(vec![Some(target)].into())))
}

/// file.link — create a hard link.
/// @namespace base
#[builtin(name = "file.link", min_args = 2)]
fn builtin_file_link(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let from = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();
    let to = args
        .get(1)
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();
    let ok = std::fs::hard_link(&from, &to).is_ok();
    Ok(RValue::vec(Vector::Logical(vec![Some(ok)].into())))
}

/// file.symlink — create a symbolic link.
/// @namespace base
#[builtin(name = "file.symlink", min_args = 2)]
fn builtin_file_symlink(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let from = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();
    let to = args
        .get(1)
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();
    #[cfg(unix)]
    let ok = std::os::unix::fs::symlink(&from, &to).is_ok();
    #[cfg(not(unix))]
    let ok = false;
    Ok(RValue::vec(Vector::Logical(vec![Some(ok)].into())))
}

/// Sys.chmod — change file permissions (Unix only).
/// @namespace base
#[interpreter_builtin(name = "Sys.chmod", min_args = 1)]
fn interp_sys_chmod(
    _args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    context
        .interpreter()
        .write_stderr("[miniR stub] Sys.chmod() is a no-op in miniR\n");
    Ok(RValue::Null)
}

/// Sys.umask — get/set file creation mask (stub).
/// @namespace base
#[builtin(name = "Sys.umask")]
fn builtin_sys_umask(_args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    Ok(RValue::vec(Vector::Integer(vec![Some(0o022)].into())))
}

/// Sys.setFileTime — set file modification time.
/// @namespace base
#[builtin(name = "Sys.setFileTime", min_args = 2)]
fn builtin_sys_set_file_time(_args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    // Stub — setting file times requires platform-specific APIs
    Ok(RValue::vec(Vector::Logical(vec![Some(false)].into())))
}

// endregion

// region: Task callbacks (no-op stubs)

/// getTaskCallbackNames — list registered task callbacks.
/// @namespace base
#[builtin(name = "getTaskCallbackNames")]
fn builtin_get_task_callback_names(
    _args: &[RValue],
    _: &[(String, RValue)],
) -> Result<RValue, RError> {
    Ok(RValue::vec(Vector::Character(vec![].into())))
}

/// addTaskCallback — register a task callback (no-op).
/// @namespace base
#[builtin(name = "addTaskCallback", min_args = 1)]
fn builtin_add_task_callback(_args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    Ok(RValue::vec(Vector::Logical(vec![Some(true)].into())))
}

/// removeTaskCallback — remove a task callback (no-op).
/// @namespace base
#[builtin(name = "removeTaskCallback", min_args = 1)]
fn builtin_remove_task_callback(
    _args: &[RValue],
    _: &[(String, RValue)],
) -> Result<RValue, RError> {
    Ok(RValue::vec(Vector::Logical(vec![Some(true)].into())))
}

// endregion

// region: Misc stubs

/// gc.time — get garbage collection timing (always zero).
/// @namespace base
#[builtin(name = "gc.time")]
fn builtin_gc_time(_args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    Ok(RValue::vec(Vector::Double(
        vec![Some(0.0), Some(0.0), Some(0.0)].into(),
    )))
}

/// mem.limits — get memory limits (dummy values).
/// @namespace base
#[builtin(name = "mem.limits")]
fn builtin_mem_limits(_args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    Ok(RValue::vec(Vector::Double(
        vec![Some(f64::INFINITY), Some(f64::INFINITY)].into(),
    )))
}

/// memory.profile — profile memory usage (dummy).
/// @namespace base
#[builtin(name = "memory.profile")]
fn builtin_memory_profile(_args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    Ok(RValue::vec(Vector::Integer(vec![].into())))
}

/// pos.to.env — convert search path position to environment.
/// @namespace base
#[builtin(name = "pos.to.env", min_args = 1)]
fn builtin_pos_to_env(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let _pos = args
        .first()
        .and_then(|v| v.as_vector()?.as_integer_scalar())
        .unwrap_or(1);
    // Can't return proper environment from plain builtin — return NULL
    Ok(RValue::Null)
}

/// setNames — set names on an object and return it.
///
/// @param object any R object
/// @param nm character vector of names
/// @return the object with names set
/// @namespace stats
// CRAN: used by 100+ packages (stats::setNames, base::setNames)
#[builtin(name = "setNames", namespace = "stats", min_args = 2)]
fn builtin_set_names(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let obj = args.first().cloned().unwrap_or(RValue::Null);
    let names = args.get(1).cloned().unwrap_or(RValue::Null);
    match obj {
        RValue::Vector(mut rv) => {
            rv.set_attr("names".to_string(), names);
            Ok(RValue::Vector(rv))
        }
        RValue::List(mut list) => {
            if let Some(names_vec) = names.as_vector() {
                let name_strs = names_vec.to_characters();
                for (i, (n, _)) in list.values.iter_mut().enumerate() {
                    if let Some(new_name) = name_strs.get(i) {
                        *n = new_name.clone();
                    }
                }
            }
            Ok(RValue::List(list))
        }
        _ => Ok(obj),
    }
}

/// globalVariables — declare global variables (no-op, suppresses R CMD check notes).
///
/// @param names character vector of variable names
/// @param package package name (ignored)
/// @param add logical (ignored)
/// @return invisible NULL
/// @namespace utils
// CRAN: used by many packages to suppress "no visible binding" notes
#[builtin(name = "globalVariables", namespace = "utils")]
fn builtin_global_variables(_args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    Ok(RValue::Null)
}

/// withAutoprint — evaluate expressions with auto-printing (stub).
/// @namespace base
#[builtin(name = "withAutoprint")]
fn builtin_with_autoprint(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    Ok(args.first().cloned().unwrap_or(RValue::Null))
}

/// signature — create an S4 method signature.
///
/// @param ... named arguments specifying class for each formal
/// @return named character vector
/// @namespace methods
// GNU-R-methods: used by S4 setMethod calls
#[builtin(name = "signature", namespace = "methods")]
fn builtin_signature(_args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let names: Vec<Option<String>> = named.iter().map(|(n, _)| Some(n.clone())).collect();
    let values: Vec<Option<String>> = named
        .iter()
        .map(|(_, v)| v.as_vector().and_then(|vec| vec.as_character_scalar()))
        .collect();
    let mut rv = RVector::from(Vector::Character(values.into()));
    rv.set_attr(
        "names".to_string(),
        RValue::vec(Vector::Character(names.into())),
    );
    Ok(RValue::Vector(rv))
}

/// prototype — create an S4 class prototype (returns a list).
///
/// @param ... named default values for slots
/// @return named list
/// @namespace methods
// GNU-R-methods: used in setClass() calls
#[builtin(name = "prototype", namespace = "methods")]
fn builtin_prototype(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let mut values: Vec<(Option<String>, RValue)> = named
        .iter()
        .map(|(n, v)| (Some(n.clone()), v.clone()))
        .collect();
    // Also include positional args
    for arg in args {
        values.push((None, arg.clone()));
    }
    Ok(RValue::List(RList::new(values)))
}

/// lengths — get lengths of list elements.
///
/// @param x list or vector
/// @return integer vector of lengths
/// @namespace base
// CRAN: used by many packages (base::lengths)
#[builtin(min_args = 1)]
fn builtin_lengths(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::List(list)) => {
            let lens: Vec<Option<i64>> = list
                .values
                .iter()
                .map(|(_, v)| {
                    Some(match v {
                        RValue::Vector(rv) => rv.len() as i64,
                        RValue::List(l) => l.values.len() as i64,
                        RValue::Null => 0,
                        _ => 1,
                    })
                })
                .collect();
            Ok(RValue::vec(Vector::Integer(lens.into())))
        }
        Some(RValue::Vector(v)) => {
            // For atomic vectors, each element has length 1
            let lens: Vec<Option<i64>> = (0..v.len()).map(|_| Some(1)).collect();
            Ok(RValue::vec(Vector::Integer(lens.into())))
        }
        _ => Ok(RValue::vec(Vector::Integer(vec![].into()))),
    }
}

/// commandArgs — return command-line arguments.
///
/// @param trailingOnly if TRUE, return only args after --args
/// @return character vector
/// @namespace base
#[builtin(name = "commandArgs")]
fn builtin_command_args(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let trailing_only = args
        .first()
        .and_then(|v| v.as_vector()?.as_logical_scalar())
        .unwrap_or(false);
    let cli_args: Vec<String> = std::env::args().collect();
    let result = if trailing_only {
        // Return args after "--args"
        if let Some(pos) = cli_args.iter().position(|a| a == "--args") {
            cli_args[pos + 1..].to_vec()
        } else {
            vec![]
        }
    } else {
        cli_args
    };
    Ok(RValue::vec(Vector::Character(
        result.into_iter().map(Some).collect::<Vec<_>>().into(),
    )))
}

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
/// setHook — set a hook on a package event (no-op in miniR).
/// @param hookName the hook name
/// @param value the hook function
/// @param action character: "append", "prepend", "replace"
/// @return invisible NULL
/// @namespace base
#[builtin(name = "setHook")]
fn builtin_set_hook(_args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    Ok(RValue::Null)
}

/// packageEvent — create a package event name string.
/// @param pkgname character: package name
/// @param event character: event type
/// @return character string
/// @namespace base
#[builtin(name = "packageEvent", min_args = 1)]
fn builtin_package_event(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let pkg = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();
    let event = args
        .get(1)
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_else(|| "onLoad".to_string());
    Ok(RValue::vec(Vector::Character(
        vec![Some(format!("{event}:{pkg}"))].into(),
    )))
}

/// getHook — get a hook (returns NULL, no hooks in miniR).
/// @namespace base
#[builtin(name = "getHook", min_args = 1)]
fn builtin_get_hook(_args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    Ok(RValue::Null)
}
