//! S4 OOP stubs — partial support so packages using S4 classes don't crash
//! immediately. These are informative stubs that print notes about limited
//! support rather than silently succeeding or failing with cryptic errors.
//!
//! S4 is R's formal object system. Full support requires a class registry,
//! method dispatch tables, validity checking, and inheritance resolution.
//! These stubs provide enough to let S4-using packages load and run simple
//! code paths while clearly communicating the limitations.

use crate::interpreter::value::*;
use crate::interpreter::BuiltinContext;
use minir_macros::{builtin, interpreter_builtin};

/// Define an S4 class (stub).
///
/// Stores the class name but does not fully implement S4 class semantics.
/// Prints a note that S4 classes are partially supported.
///
/// @param Class character string naming the class
/// @param representation named list of slot types (ignored in stub)
/// @param contains character vector of superclasses (ignored in stub)
/// @param ... additional arguments (ignored)
/// @return the class name (invisibly)
#[builtin(name = "setClass", min_args = 1)]
fn builtin_set_class_s4(args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    let class_name = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "setClass() requires a character string for the class name".to_string(),
            )
        })?;

    eprintln!(
        "Note: S4 classes are partially supported. Registering class '{}' as a stub.",
        class_name
    );

    Ok(RValue::vec(Vector::Character(
        vec![Some(class_name)].into(),
    )))
}

/// Define an S4 generic function (stub).
///
/// Creates a regular function binding for the generic. Does not implement
/// full S4 method dispatch — the supplied default definition is bound
/// directly as a plain function.
///
/// @param name character string naming the generic
/// @param def default function definition
/// @return the generic name (invisibly)
#[interpreter_builtin(name = "setGeneric", min_args = 1)]
fn interp_set_generic(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let name = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "setGeneric() requires a character string for the generic name".to_string(),
            )
        })?;

    // Look for the definition in positional arg 2 or named "def"
    let def = args.get(1).cloned().or_else(|| {
        named
            .iter()
            .find(|(n, _)| n == "def")
            .map(|(_, v)| v.clone())
    });

    if let Some(func @ RValue::Function(_)) = def {
        context.env().set(name.clone(), func);
    }

    eprintln!(
        "Note: setGeneric('{}') registered as a plain function (S4 dispatch is not yet supported).",
        name
    );

    Ok(RValue::vec(Vector::Character(vec![Some(name)].into())))
}

/// Register an S4 method (stub).
///
/// Binds the method function under a mangled name (f.signature) in the
/// calling environment. Does not implement full S4 method dispatch —
/// the method will only be callable directly, not via automatic dispatch.
///
/// @param f character string naming the generic
/// @param signature character vector or string specifying the method signature
/// @param def function implementing the method
/// @return the function name (invisibly)
#[interpreter_builtin(name = "setMethod", min_args = 1)]
fn interp_set_method(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let f = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "setMethod() requires a character string for the function name".to_string(),
            )
        })?;

    // Look for the definition in positional arg 3 or named "def"
    let def = args.get(2).cloned().or_else(|| {
        named
            .iter()
            .find(|(n, _)| n == "def")
            .map(|(_, v)| v.clone())
    });

    if let Some(func @ RValue::Function(_)) = def {
        // Bind under the generic name so it at least works as a fallback
        context.env().set(f.clone(), func);
    }

    eprintln!(
        "Note: setMethod('{}') registered as a plain function (S4 dispatch is not yet supported).",
        f
    );

    Ok(RValue::vec(Vector::Character(vec![Some(f)].into())))
}

/// Create a new S4 object (stub).
///
/// Creates a list with the specified class attribute set. Named arguments
/// become named elements (slots) of the list.
///
/// @param Class character string naming the S4 class
/// @param ... slot values as named arguments
/// @return a list with class attribute set to Class
#[builtin(name = "new", min_args = 1)]
fn builtin_new(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let class_name = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "new() requires a character string for the class name".to_string(),
            )
        })?;

    // Remaining positional args become unnamed slots; named args become named slots
    let mut values: Vec<(Option<String>, RValue)> = Vec::new();
    for arg in args.iter().skip(1) {
        values.push((None, arg.clone()));
    }
    for (name, val) in named {
        values.push((Some(name.clone()), val.clone()));
    }

    let mut list = RList::new(values);
    list.set_attr(
        "class".to_string(),
        RValue::vec(Vector::Character(vec![Some(class_name)].into())),
    );

    Ok(RValue::List(list))
}

/// Check if an object is an instance of a class (S4-compatible).
///
/// Uses the class attribute to check inheritance, similar to `inherits()`
/// but following S4 conventions.
///
/// @param object any R object
/// @param class2 character string naming the class to check
/// @return TRUE if the object inherits from class2, FALSE otherwise
#[builtin(min_args = 1)]
fn builtin_is(args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    let object = args.first().ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "is() requires at least one argument".to_string(),
        )
    })?;

    let class2 = match args.get(1) {
        Some(v) => v
            .as_vector()
            .and_then(|v| v.as_character_scalar())
            .ok_or_else(|| {
                RError::new(
                    RErrorKind::Argument,
                    "is() requires a character string for class2".to_string(),
                )
            })?,
        // With one argument, is() returns the class — match R behavior
        None => {
            let classes = get_class(object);
            if classes.is_empty() {
                return Ok(RValue::vec(Vector::Character(vec![None].into())));
            }
            return Ok(RValue::vec(Vector::Character(
                classes.into_iter().map(Some).collect::<Vec<_>>().into(),
            )));
        }
    };

    let classes = get_class(object);
    let result = classes.iter().any(|c| c == &class2);
    Ok(RValue::vec(Vector::Logical(vec![Some(result)].into())))
}

/// Check if a class is virtual (stub).
///
/// Always returns FALSE since virtual classes are not yet supported.
///
/// @param Class character string naming the class
/// @return FALSE
#[builtin(name = "isVirtualClass", min_args = 1)]
fn builtin_is_virtual_class(
    _args: &[RValue],
    _named: &[(String, RValue)],
) -> Result<RValue, RError> {
    Ok(RValue::vec(Vector::Logical(vec![Some(false)].into())))
}

/// Validate an S4 object (stub).
///
/// Returns the object unchanged since validity methods are not yet supported.
///
/// @param object an S4 object
/// @return the object, unchanged
#[builtin(name = "validObject", min_args = 1)]
fn builtin_valid_object(args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    args.first().cloned().ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "validObject() requires an object argument".to_string(),
        )
    })
}

/// Set a validity method for an S4 class (stub).
///
/// Accepts and ignores the validity method. Returns the class name.
///
/// @param Class character string naming the class
/// @param method validity-checking function (ignored in stub)
/// @return the class name
#[builtin(name = "setValidity", min_args = 1)]
fn builtin_set_validity(args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    let class_name = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "setValidity() requires a character string for the class name".to_string(),
            )
        })?;

    Ok(RValue::vec(Vector::Character(
        vec![Some(class_name)].into(),
    )))
}

/// Display information about an S4 class (stub).
///
/// Prints the class name. Full class metadata display is not yet supported.
///
/// @param Class character string naming the class
/// @return NULL, invisibly
#[builtin(name = "showClass", min_args = 1)]
fn builtin_show_class(args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    let class_name = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "showClass() requires a character string for the class name".to_string(),
            )
        })?;

    eprintln!(
        "Class \"{}\" (S4 class details not yet available)",
        class_name
    );
    Ok(RValue::Null)
}

/// Check if a method exists for a given generic and signature (stub).
///
/// Always returns FALSE since the S4 method registry is not implemented.
///
/// @param f character string naming the generic function
/// @param signature character string or vector for the method signature
/// @return FALSE
#[builtin(name = "existsMethod", min_args = 1)]
fn builtin_exists_method(_args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    Ok(RValue::vec(Vector::Logical(vec![Some(false)].into())))
}

/// Extract a slot from an S4 object.
///
/// Extracts a named element from the underlying list, equivalent to the
/// `@` operator.
///
/// @param object an S4 object (list with class attribute)
/// @param name character string naming the slot
/// @return the slot value, or an error if the slot doesn't exist
#[builtin(min_args = 2)]
fn builtin_slot(args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    let object = args.first().ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "slot() requires an object argument".to_string(),
        )
    })?;

    let slot_name = args
        .get(1)
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "slot() requires a character string for the slot name".to_string(),
            )
        })?;

    match object {
        RValue::List(list) => {
            for (name, val) in &list.values {
                if name.as_deref() == Some(&slot_name) {
                    return Ok(val.clone());
                }
            }
            Err(RError::new(
                RErrorKind::Name,
                format!(
                    "no slot of name \"{}\" for this object of class \"{}\"",
                    slot_name,
                    get_class(object).first().unwrap_or(&"unknown".to_string())
                ),
            ))
        }
        _ => Err(RError::new(
            RErrorKind::Type,
            "slot() requires an S4 object (list with class attribute)".to_string(),
        )),
    }
}

/// Set a slot on an S4 object (replacement function).
///
/// Sets a named element on the underlying list, equivalent to `@<-`.
///
/// @param object an S4 object (list with class attribute)
/// @param name character string naming the slot
/// @param value the new value for the slot
/// @return the modified object
#[builtin(name = "slot<-", min_args = 3)]
fn builtin_slot_set(args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    let object = args.first().ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "slot<-() requires an object argument".to_string(),
        )
    })?;

    let slot_name = args
        .get(1)
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "slot<-() requires a character string for the slot name".to_string(),
            )
        })?;

    let value = args.get(2).cloned().unwrap_or(RValue::Null);

    match object {
        RValue::List(list) => {
            let mut new_list = list.clone();
            let mut found = false;
            for entry in &mut new_list.values {
                if entry.0.as_deref() == Some(&slot_name) {
                    entry.1 = value.clone();
                    found = true;
                    break;
                }
            }
            if !found {
                new_list.values.push((Some(slot_name.to_string()), value));
            }
            Ok(RValue::List(new_list))
        }
        _ => Err(RError::new(
            RErrorKind::Type,
            "slot<-() requires an S4 object (list with class attribute)".to_string(),
        )),
    }
}
