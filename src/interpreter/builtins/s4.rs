//! S4 OOP system — class registry, method dispatch, and object construction.
//!
//! S4 is R's formal object system. This module implements:
//! - Class definitions with slots, inheritance, and prototypes (setClass)
//! - Object construction with slot validation (new)
//! - Inheritance-aware type checking (is)
//! - Generic functions and method dispatch (setGeneric, setMethod)
//! - Slot access and modification (slot, slot<-)

use crate::interpreter::value::*;
use crate::interpreter::{BuiltinContext, S4ClassDef, S4GenericDef};
use minir_macros::{builtin, interpreter_builtin};

// region: Helpers

/// Extract slot definitions from a `representation` or `slots` argument.
/// Accepts a named list (list(x = "numeric", y = "character")) or a named
/// character vector (c(x = "numeric", y = "character")).
fn extract_slots(val: &RValue) -> Vec<(String, String)> {
    match val {
        RValue::List(list) => list
            .values
            .iter()
            .filter_map(|(name, v)| {
                let slot_name = name.as_ref()?;
                let type_name = match v {
                    RValue::Vector(rv) => rv.as_character_scalar(),
                    _ => None,
                }
                .unwrap_or_else(|| "ANY".to_string());
                Some((slot_name.clone(), type_name))
            })
            .collect(),
        RValue::Vector(rv) => {
            // Named character vector: names are slot names, values are type names
            let names = rv
                .attrs
                .as_ref()
                .and_then(|a| a.get("names"))
                .and_then(|v| {
                    if let RValue::Vector(nv) = v {
                        if let Vector::Character(c) = &nv.inner {
                            Some(c.clone())
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                });
            if let (Some(names), Vector::Character(types)) = (names, &rv.inner) {
                names
                    .iter()
                    .zip(types.iter())
                    .filter_map(|(n, t)| {
                        let slot_name = n.as_ref()?;
                        let type_name = t.clone().unwrap_or_else(|| "ANY".to_string());
                        Some((slot_name.clone(), type_name))
                    })
                    .collect()
            } else {
                Vec::new()
            }
        }
        _ => Vec::new(),
    }
}

/// Extract a character vector from an RValue (for `contains` argument).
fn extract_character_vec(val: &RValue) -> Vec<String> {
    match val {
        RValue::Vector(rv) => rv.to_characters().into_iter().flatten().collect(),
        _ => Vec::new(),
    }
}

/// Extract prototype values from a named list or named vector.
fn extract_prototype(val: &RValue) -> Vec<(String, RValue)> {
    match val {
        RValue::List(list) => list
            .values
            .iter()
            .filter_map(|(name, v)| Some((name.as_ref()?.clone(), v.clone())))
            .collect(),
        _ => Vec::new(),
    }
}

/// Collect the full inheritance chain for a class, including the class itself.
/// Uses the S4 class registry to walk the `contains` hierarchy.
fn inheritance_chain(
    class_name: &str,
    registry: &std::collections::HashMap<String, S4ClassDef>,
) -> Vec<String> {
    let mut chain = vec![class_name.to_string()];
    let mut visited = std::collections::HashSet::new();
    visited.insert(class_name.to_string());
    let mut queue = std::collections::VecDeque::new();
    queue.push_back(class_name.to_string());

    while let Some(current) = queue.pop_front() {
        if let Some(def) = registry.get(&current) {
            for parent in &def.contains {
                if visited.insert(parent.clone()) {
                    chain.push(parent.clone());
                    queue.push_back(parent.clone());
                }
            }
        }
    }

    chain
}

/// Collect all slot definitions for a class including inherited slots.
fn all_slots_for_class(
    class_name: &str,
    registry: &std::collections::HashMap<String, S4ClassDef>,
) -> Vec<(String, String)> {
    let chain = inheritance_chain(class_name, registry);
    let mut slots = Vec::new();
    let mut seen = std::collections::HashSet::new();
    // Walk from most ancestral to most derived so derived slots override
    for class in chain.iter().rev() {
        if let Some(def) = registry.get(class) {
            for (name, typ) in &def.slots {
                if seen.insert(name.clone()) {
                    slots.push((name.clone(), typ.clone()));
                }
            }
        }
    }
    slots
}

/// Collect all prototype defaults for a class including inherited prototypes.
fn all_prototypes_for_class(
    class_name: &str,
    registry: &std::collections::HashMap<String, S4ClassDef>,
) -> Vec<(String, RValue)> {
    let chain = inheritance_chain(class_name, registry);
    let mut proto = Vec::new();
    let mut seen = std::collections::HashSet::new();
    // Walk from most ancestral to most derived so derived prototypes override
    for class in chain.iter().rev() {
        if let Some(def) = registry.get(class) {
            for (name, val) in &def.prototype {
                if seen.insert(name.clone()) {
                    proto.push((name.clone(), val.clone()));
                }
            }
        }
    }
    proto
}

// endregion

// region: setClass

/// Define an S4 class.
///
/// Registers the class in the per-interpreter S4 class registry with its
/// slot definitions, superclasses (inheritance), and prototype defaults.
///
/// @param Class character string naming the class
/// @param representation named list/vector of slot types (synonym for slots)
/// @param slots named list/vector of slot types
/// @param contains character vector of superclass names
/// @param prototype named list of default slot values
/// @param validity validity-checking function
/// @param sealed logical, whether the class definition is sealed
/// @return the class name (invisibly)
#[interpreter_builtin(name = "setClass", min_args = 1, namespace = "methods")]
fn interp_set_class_s4(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let class_name = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "setClass() requires a character string for the class name",
            )
        })?;

    // Extract slots from:
    // 1. Named arg "representation" or "slots"
    // 2. Positional arg 2 (common pattern: setClass("Name", representation(...)))
    let slots_val = named
        .iter()
        .find(|(n, _)| n == "representation" || n == "slots")
        .map(|(_, v)| v)
        .or_else(|| args.get(1));

    let slots = slots_val.map(extract_slots).unwrap_or_default();

    // Extract superclasses from "contains" named arg or positional arg 3
    let contains = named
        .iter()
        .find(|(n, _)| n == "contains")
        .map(|(_, v)| v)
        .or_else(|| args.get(2))
        .map(extract_character_vec)
        .unwrap_or_default();

    // Extract prototype defaults from "prototype" named arg
    let prototype = named
        .iter()
        .find(|(n, _)| n == "prototype")
        .map(|(_, v)| extract_prototype(v))
        .unwrap_or_default();

    // Check for virtual class
    let is_virtual = named
        .iter()
        .find(|(n, _)| n == "virtual")
        .and_then(|(_, v)| v.as_vector()?.as_logical_scalar())
        .unwrap_or(false);

    // Extract validity function
    let validity = named
        .iter()
        .find(|(n, _)| n == "validity")
        .map(|(_, v)| v.clone())
        .filter(|v| matches!(v, RValue::Function(_)));

    let class_def = S4ClassDef {
        name: class_name.clone(),
        slots,
        contains,
        prototype,
        is_virtual,
        validity,
    };

    // Store in the per-interpreter registry
    context.with_interpreter(|interp| {
        interp
            .s4_classes
            .borrow_mut()
            .insert(class_name.clone(), class_def);
    });

    Ok(RValue::vec(Vector::Character(
        vec![Some(class_name)].into(),
    )))
}

// endregion

// region: new

/// Create a new S4 object.
///
/// Validates slot values against the registered class definition and
/// initializes unspecified slots with prototype defaults.
///
/// @param Class character string naming the S4 class
/// @param ... slot values as named arguments
/// @return a list with slots as named elements and the class attribute set
#[interpreter_builtin(name = "new", min_args = 1, namespace = "methods")]
fn interp_new(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let class_name = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "new() requires a character string for the class name",
            )
        })?;

    context.with_interpreter(|interp| {
        let registry = interp.s4_classes.borrow();

        if let Some(class_def) = registry.get(&class_name) {
            // Check if virtual
            if class_def.is_virtual {
                return Err(RError::new(
                    RErrorKind::Other,
                    format!(
                        "trying to generate an object from a virtual class (\"{}\")",
                        class_name
                    ),
                ));
            }

            // Collect all valid slots (including inherited)
            let all_slots = all_slots_for_class(&class_name, &registry);
            let all_protos = all_prototypes_for_class(&class_name, &registry);
            let slot_names: std::collections::HashSet<&str> =
                all_slots.iter().map(|(n, _)| n.as_str()).collect();

            // Validate that all named args are valid slot names
            let mut errors = Vec::new();
            for (name, _) in named {
                if !slot_names.contains(name.as_str()) {
                    errors.push(format!("\"{}\"", name));
                }
            }
            if !errors.is_empty() {
                return Err(RError::new(
                    RErrorKind::Argument,
                    format!(
                        "invalid name{} for slot{} of class \"{}\": {}.\n  \
                         Valid slots are: {}",
                        if errors.len() > 1 { "s" } else { "" },
                        if errors.len() > 1 { "s" } else { "" },
                        class_name,
                        errors.join(", "),
                        all_slots
                            .iter()
                            .map(|(n, _)| format!("\"{}\"", n))
                            .collect::<Vec<_>>()
                            .join(", "),
                    ),
                ));
            }

            // Build the object: start with prototype defaults, then override
            // with user-supplied values
            let named_map: std::collections::HashMap<&str, &RValue> =
                named.iter().map(|(n, v)| (n.as_str(), v)).collect();

            let proto_map: std::collections::HashMap<&str, &RValue> =
                all_protos.iter().map(|(n, v)| (n.as_str(), v)).collect();

            let mut values: Vec<(Option<String>, RValue)> = Vec::new();
            for (slot_name, _slot_type) in &all_slots {
                let val = if let Some(v) = named_map.get(slot_name.as_str()) {
                    (*v).clone()
                } else if let Some(v) = proto_map.get(slot_name.as_str()) {
                    (*v).clone()
                } else {
                    // No prototype, no user value — use NULL
                    RValue::Null
                };
                values.push((Some(slot_name.clone()), val));
            }

            // Build class vector: the class itself + all ancestors
            let chain = inheritance_chain(&class_name, &registry);
            let class_vec: Vec<Option<String>> = chain.into_iter().map(Some).collect();

            let mut list = RList::new(values);
            list.set_attr(
                "class".to_string(),
                RValue::vec(Vector::Character(class_vec.into())),
            );

            // Run validity check if defined.
            // R convention: validity returns TRUE if valid, or a character
            // string describing the problem if invalid.
            if let Some(ref validity_fn) = class_def.validity {
                let obj = RValue::List(list.clone());
                let result = interp
                    .call_function(validity_fn, &[obj], &[], &interp.global_env)
                    .map_err(RError::from)?;
                if let Some(vec) = result.as_vector() {
                    match vec {
                        // Logical TRUE means valid — do nothing
                        Vector::Logical(vals) if vals.first() == Some(&Some(true)) => {}
                        // Character string means error message
                        Vector::Character(vals) => {
                            if let Some(Some(msg)) = vals.first() {
                                return Err(RError::new(
                                    RErrorKind::Other,
                                    format!("invalid class \"{}\" object: {}", class_name, msg),
                                ));
                            }
                        }
                        _ => {}
                    }
                }
            }

            Ok(RValue::List(list))
        } else {
            // No registered class — fall back to simple list construction
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
    })
}

// endregion

// region: is

/// Check if an object is an instance of a class (S4-compatible).
///
/// Checks the class attribute and then walks the S4 inheritance chain
/// from the class registry to determine if the object inherits from
/// the specified class.
///
/// @param object any R object
/// @param class2 character string naming the class to check
/// @return TRUE if the object inherits from class2, FALSE otherwise
#[interpreter_builtin(min_args = 1, namespace = "methods")]
fn interp_is(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let object = args
        .first()
        .ok_or_else(|| RError::new(RErrorKind::Argument, "is() requires at least one argument"))?;

    let class2 = match args.get(1) {
        Some(v) => v
            .as_vector()
            .and_then(|v| v.as_character_scalar())
            .ok_or_else(|| {
                RError::new(
                    RErrorKind::Argument,
                    "is() requires a character string for class2",
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

    // Direct class match
    if classes.iter().any(|c| c == &class2) {
        return Ok(RValue::vec(Vector::Logical(vec![Some(true)].into())));
    }

    // Walk the inheritance chain from the S4 registry
    let result = context.with_interpreter(|interp| {
        let registry = interp.s4_classes.borrow();
        for obj_class in &classes {
            let chain = inheritance_chain(obj_class, &registry);
            if chain.iter().any(|c| c == &class2) {
                return true;
            }
        }
        false
    });

    Ok(RValue::vec(Vector::Logical(vec![Some(result)].into())))
}

// endregion

// region: setGeneric / setMethod

/// Define an S4 generic function.
///
/// Registers the generic in the per-interpreter S4 generic registry and
/// creates a dispatching function binding in the calling environment.
///
/// @param name character string naming the generic
/// @param def default function definition
/// @return the generic name (invisibly)
#[interpreter_builtin(name = "setGeneric", min_args = 1, namespace = "methods")]
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
                "setGeneric() requires a character string for the generic name",
            )
        })?;

    // Look for the definition in positional arg 2 or named "def"
    let def = args.get(1).cloned().or_else(|| {
        named
            .iter()
            .find(|(n, _)| n == "def")
            .map(|(_, v)| v.clone())
    });

    let default_fn = def.filter(|v| matches!(v, RValue::Function(_)));

    // Register the generic in the interpreter
    context.with_interpreter(|interp| {
        interp.s4_generics.borrow_mut().insert(
            name.clone(),
            S4GenericDef {
                name: name.clone(),
                default: default_fn.clone(),
            },
        );
    });

    // Create a dispatching function and bind it in the environment.
    // If we have a default, bind that as the callable. The S4 dispatch
    // logic in call_eval will check the method table before falling back.
    if let Some(func) = default_fn {
        context.env().set(name.clone(), func);
    }

    Ok(RValue::vec(Vector::Character(vec![Some(name)].into())))
}

/// Register an S4 method.
///
/// Stores the method in the per-interpreter S4 method dispatch table,
/// keyed by (generic_name, signature). Falls back to binding the method
/// under the generic name if no dispatch table entry can be created.
///
/// @param f character string naming the generic
/// @param signature character vector or string specifying the method signature
/// @param def function implementing the method
/// @return the function name (invisibly)
#[interpreter_builtin(name = "setMethod", min_args = 1, namespace = "methods")]
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
                "setMethod() requires a character string for the function name",
            )
        })?;

    // Extract signature from positional arg 2 or named "signature"
    let sig_val = args.get(1).cloned().or_else(|| {
        named
            .iter()
            .find(|(n, _)| n == "signature")
            .map(|(_, v)| v.clone())
    });

    let signature: Vec<String> = sig_val
        .as_ref()
        .map(extract_character_vec)
        .unwrap_or_default();

    // Extract definition from positional arg 3 or named "def"
    let def = args.get(2).cloned().or_else(|| {
        named
            .iter()
            .find(|(n, _)| n == "def")
            .map(|(_, v)| v.clone())
    });

    if let Some(func) = def.filter(|v| matches!(v, RValue::Function(_))) {
        context.with_interpreter(|interp| {
            // Store in the method dispatch table
            interp
                .s4_methods
                .borrow_mut()
                .insert((f.clone(), signature), func.clone());

            // Also bind under the generic name if there is no existing binding,
            // or if no generic was registered (backwards compat)
            let generics = interp.s4_generics.borrow();
            if !generics.contains_key(&f) {
                drop(generics);
                context.env().set(f.clone(), func);
            }
        });
    }

    Ok(RValue::vec(Vector::Character(vec![Some(f)].into())))
}

// endregion

// region: isVirtualClass / validObject / setValidity / showClass / existsMethod

/// Check if a class is virtual.
///
/// Looks up the class in the S4 registry and returns its virtual status.
///
/// @param Class character string naming the class
/// @return TRUE if the class is virtual, FALSE otherwise
#[interpreter_builtin(name = "isVirtualClass", min_args = 1, namespace = "methods")]
fn interp_is_virtual_class(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let class_name = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();

    let is_virtual = context.with_interpreter(|interp| {
        interp
            .s4_classes
            .borrow()
            .get(&class_name)
            .is_some_and(|def| def.is_virtual)
    });

    Ok(RValue::vec(Vector::Logical(vec![Some(is_virtual)].into())))
}

/// Validate an S4 object.
///
/// Runs the validity function registered for the object's class, if any.
///
/// @param object an S4 object
/// @return the object if valid, error otherwise
#[interpreter_builtin(name = "validObject", min_args = 1, namespace = "methods")]
fn interp_valid_object(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let object = args.first().cloned().ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "validObject() requires an object argument",
        )
    })?;

    let classes = get_class(&object);
    if let Some(class_name) = classes.first() {
        context.with_interpreter(|interp| {
            let registry = interp.s4_classes.borrow();
            if let Some(def) = registry.get(class_name) {
                if let Some(ref validity_fn) = def.validity {
                    let validity_fn = validity_fn.clone();
                    drop(registry);
                    let result = interp
                        .call_function(
                            &validity_fn,
                            std::slice::from_ref(&object),
                            &[],
                            &interp.global_env,
                        )
                        .map_err(RError::from)?;
                    if let Some(vec) = result.as_vector() {
                        match vec {
                            // Logical TRUE means valid
                            Vector::Logical(vals) if vals.first() == Some(&Some(true)) => {}
                            // Character string means error message
                            Vector::Character(vals) => {
                                if let Some(Some(msg)) = vals.first() {
                                    return Err(RError::new(
                                        RErrorKind::Other,
                                        format!("invalid class \"{}\" object: {}", class_name, msg),
                                    ));
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            Ok(())
        })?;
    }

    Ok(object)
}

/// Set a validity method for an S4 class.
///
/// Stores the validity function in the class registry entry.
///
/// @param Class character string naming the class
/// @param method validity-checking function
/// @return the class name
#[interpreter_builtin(name = "setValidity", min_args = 1, namespace = "methods")]
fn interp_set_validity(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let class_name = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "setValidity() requires a character string for the class name",
            )
        })?;

    let method = args.get(1).cloned().or_else(|| {
        named
            .iter()
            .find(|(n, _)| n == "method")
            .map(|(_, v)| v.clone())
    });

    if let Some(func) = method.filter(|v| matches!(v, RValue::Function(_))) {
        context.with_interpreter(|interp| {
            let mut registry = interp.s4_classes.borrow_mut();
            if let Some(def) = registry.get_mut(&class_name) {
                def.validity = Some(func);
            }
        });
    }

    Ok(RValue::vec(Vector::Character(
        vec![Some(class_name)].into(),
    )))
}

/// Display information about an S4 class.
///
/// Shows class details from the registry including slots and inheritance.
///
/// @param Class character string naming the class
/// @return NULL, invisibly
#[interpreter_builtin(name = "showClass", min_args = 1, namespace = "methods")]
fn interp_show_class(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let class_name = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "showClass() requires a character string for the class name",
            )
        })?;

    context.with_interpreter(|interp| {
        let registry = interp.s4_classes.borrow();
        if let Some(def) = registry.get(&class_name) {
            interp.write_stderr(&format!("Class \"{}\"\n", class_name));
            if !def.slots.is_empty() {
                interp.write_stderr("Slots:\n");
                for (name, typ) in &def.slots {
                    interp.write_stderr(&format!("  Name: {}  Class: {}\n", name, typ));
                }
            }
            if !def.contains.is_empty() {
                interp.write_stderr(&format!("Extends: {}\n", def.contains.join(", ")));
            }
            if def.is_virtual {
                interp.write_stderr("(virtual class)\n");
            }
        } else {
            interp.write_stderr(&format!(
                "Class \"{}\" (not registered in S4 class registry)\n",
                class_name
            ));
        }
    });

    Ok(RValue::Null)
}

/// Check if a method exists for a given generic and signature.
///
/// Looks up the method in the S4 dispatch table.
///
/// @param f character string naming the generic function
/// @param signature character string or vector for the method signature
/// @return TRUE if a method exists, FALSE otherwise
#[interpreter_builtin(name = "existsMethod", min_args = 1, namespace = "methods")]
fn interp_exists_method(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let f = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();

    let sig_val = args.get(1);
    let signature: Vec<String> = sig_val.map(extract_character_vec).unwrap_or_default();

    let exists = context.with_interpreter(|interp| {
        let methods = interp.s4_methods.borrow();
        methods.contains_key(&(f, signature))
    });

    Ok(RValue::vec(Vector::Logical(vec![Some(exists)].into())))
}

// endregion

// region: slot / slot<-

/// Extract a slot from an S4 object.
///
/// Extracts a named element from the underlying list, equivalent to the
/// `@` operator.
///
/// @param object an S4 object (list with class attribute)
/// @param name character string naming the slot
/// @return the slot value, or an error if the slot doesn't exist
#[builtin(min_args = 2, namespace = "methods")]
fn builtin_slot(args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    let object = args
        .first()
        .ok_or_else(|| RError::new(RErrorKind::Argument, "slot() requires an object argument"))?;

    let slot_name = args
        .get(1)
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "slot() requires a character string for the slot name",
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
            "slot() requires an S4 object (list with class attribute)",
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
#[builtin(name = "slot<-", min_args = 3, namespace = "methods")]
fn builtin_slot_set(args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    let object = args
        .first()
        .ok_or_else(|| RError::new(RErrorKind::Argument, "slot<-() requires an object argument"))?;

    let slot_name = args
        .get(1)
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "slot<-() requires a character string for the slot name",
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
            "slot<-() requires an S4 object (list with class attribute)",
        )),
    }
}

// endregion

// region: representation

/// Create a named character vector describing S4 slot types.
///
/// This is used as the `representation` or `slots` argument to `setClass()`.
/// Each named argument specifies a slot name and its type as a character string.
///
/// @param ... named arguments where names are slot names and values are type strings
/// @return a named character vector
#[builtin(min_args = 0, namespace = "methods")]
fn builtin_representation(_args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let names: Vec<Option<String>> = named.iter().map(|(n, _)| Some(n.clone())).collect();
    let values: Vec<Option<String>> = named
        .iter()
        .map(|(_, v)| {
            v.as_vector()
                .and_then(|rv| rv.as_character_scalar())
                .or_else(|| Some(format!("{}", v)))
        })
        .collect();

    let mut rv = RVector::from(Vector::Character(values.into()));
    rv.set_attr(
        "names".to_string(),
        RValue::vec(Vector::Character(names.into())),
    );
    Ok(RValue::Vector(rv))
}

// endregion
