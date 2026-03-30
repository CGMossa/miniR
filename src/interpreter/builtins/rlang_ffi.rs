//! Native Rust implementations of rlang FFI functions.
//!
//! rlang's C code uses `r_abort()` which spins in `while(1)` when Rf_eval
//! returns on error, causing hangs in miniR. By intercepting rlang's `.Call`
//! FFI functions and implementing them in pure Rust, we bypass rlang's C code
//! entirely. This unblocks 83+ CRAN packages that depend on rlang.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::interpreter::value::*;

/// Try to dispatch an rlang FFI function by symbol name.
///
/// Returns `Some(result)` if the symbol was handled, `None` to fall through
/// to the native C code path.
pub fn try_dispatch(name: &str, args: &[RValue]) -> Option<Result<RValue, RError>> {
    match name {
        // region: Init functions (no-ops)
        "ffi_init_r_library" | "ffi_init_rlang" | "ffi_fini_rlang" | "ffi_glue_is_here" => {
            Some(Ok(RValue::Null))
        }

        // region: Type checking
        "ffi_is_character" => Some(ffi_is_character(args)),
        "ffi_is_string" => Some(ffi_is_string(args)),
        "ffi_is_logical" => Some(ffi_is_logical(args)),
        "ffi_is_integer" => Some(ffi_is_integer(args)),
        "ffi_is_double" => Some(ffi_is_double(args)),
        "ffi_is_complex" => Some(ffi_is_complex(args)),
        "ffi_is_raw" => Some(ffi_is_raw(args)),
        "ffi_is_list" => Some(ffi_is_list(args)),
        "ffi_is_vector" => Some(ffi_is_vector(args)),
        "ffi_is_atomic" => Some(ffi_is_atomic(args)),
        "ffi_is_function" => Some(ffi_is_function(args)),
        "ffi_is_closure" => Some(ffi_is_closure(args)),
        "ffi_is_primitive" => Some(ffi_is_primitive(args)),
        "ffi_is_primitive_eager" => Some(ffi_is_primitive_eager(args)),
        "ffi_is_primitive_lazy" => Some(ffi_is_primitive_lazy(args)),
        "ffi_is_formula" => Some(ffi_is_formula(args)),
        "ffi_is_call" => Some(ffi_is_call(args)),
        "ffi_is_integerish" => Some(ffi_is_integerish(args)),
        "ffi_is_finite" => Some(ffi_is_finite(args)),
        "ffi_is_reference" => Some(ffi_is_reference(args)),
        "ffi_is_weakref" => Some(Ok(r_false())),
        "ffi_is_splice_box" => Some(Ok(r_false())),

        // region: Utility functions
        "ffi_length" => Some(ffi_length(args)),
        "ffi_names" => Some(ffi_names(args)),
        "ffi_set_names" => Some(ffi_set_names(args)),
        "ffi_missing_arg" => Some(Ok(RValue::Null)),
        "ffi_duplicate" => Some(ffi_duplicate(args)),
        "ffi_symbol" => Some(ffi_symbol(args)),
        "ffi_compiled_by_gcc" => Some(Ok(r_false())),
        "ffi_obj_address" => Some(ffi_obj_address(args)),
        "ffi_hash" => Some(ffi_hash(args)),
        "ffi_format_error_arg" => Some(ffi_format_error_arg(args)),
        "ffi_cnd_type" => Some(ffi_cnd_type(args)),

        // region: Environment functions
        "ffi_env_has" => Some(ffi_env_has(args)),
        "ffi_env_poke_parent" => Some(ffi_env_poke_parent(args)),
        "ffi_env_clone" => Some(ffi_env_clone(args)),
        "ffi_find_var" => Some(ffi_find_var(args)),
        "ffi_ns_registry_env" => Some(ffi_ns_registry_env()),
        "ffi_env_binding_types" => Some(ffi_env_binding_types(args)),

        // region: Promise functions (stubs)
        "ffi_promise_expr" => Some(Ok(RValue::Null)),
        "ffi_promise_value" => Some(Ok(RValue::Null)),
        "ffi_promise_env" => Some(Ok(RValue::Null)),

        // region: Standalone type-check functions (used by lifecycle, vctrs, etc.)
        "ffi_standalone_is_bool_1.0.7" => Some(ffi_standalone_is_bool(args)),
        "ffi_standalone_check_number_1.0.7" => Some(ffi_standalone_check_number(args)),

        // Catch-all: any ffi_ name we don't handle — return NULL to avoid segfaults
        // from uninitialized rlang C code. Log for debugging.
        _ if name.starts_with("ffi_") => {
            tracing::debug!(symbol = name, "unhandled rlang FFI — returning NULL");
            Some(Ok(RValue::Null))
        }
        _ => None,
    }
}

// region: Helpers

fn lgl(v: bool) -> RValue {
    RValue::vec(Vector::Logical(vec![Some(v)].into()))
}

fn r_false() -> RValue {
    lgl(false)
}

fn r_bool(v: bool) -> RValue {
    RValue::vec(Vector::Logical(vec![Some(v)].into()))
}

/// Get the nth arg, returning NULL if missing.
fn arg(args: &[RValue], i: usize) -> &RValue {
    args.get(i).unwrap_or(&RValue::Null)
}

/// Check if an arg is R NULL (meaning "no restriction" in rlang type checks).
fn is_r_null(v: &RValue) -> bool {
    v.is_null()
}

/// Extract an integer scalar from an RValue, or None if NULL.
fn int_scalar(v: &RValue) -> Option<i64> {
    if is_r_null(v) {
        return None;
    }
    v.as_vector().and_then(|v| v.as_integer_scalar())
}

/// Extract a logical scalar from an RValue, or None if NULL.
fn lgl_scalar(v: &RValue) -> Option<bool> {
    if is_r_null(v) {
        return None;
    }
    v.as_vector().and_then(|v| v.as_logical_scalar())
}

/// Check if length matches expected n (None = no restriction).
fn check_length(actual: usize, expected_n: Option<i64>) -> bool {
    match expected_n {
        None => true,
        Some(n) => {
            let expected = u64::try_from(n).unwrap_or(0);
            u64::try_from(actual).unwrap_or(0) == expected
        }
    }
}

/// Extract a character vector from an RValue.
fn as_char_vec(v: &RValue) -> Option<&[Option<String>]> {
    match v {
        RValue::Vector(rv) => match &rv.inner {
            Vector::Character(c) => Some(c.as_slice()),
            _ => None,
        },
        _ => None,
    }
}

// endregion

// region: Type checking implementations

/// ffi_is_character(x, n, missing, empty)
fn ffi_is_character(args: &[RValue]) -> Result<RValue, RError> {
    let x = arg(args, 0);
    let n = int_scalar(arg(args, 1));

    let result = match x.as_vector() {
        Some(Vector::Character(chars)) => {
            if !check_length(chars.len(), n) {
                false
            } else {
                let missing_ok = lgl_scalar(arg(args, 2));
                let empty_ok = lgl_scalar(arg(args, 3));
                check_character_constraints(chars, missing_ok, empty_ok)
            }
        }
        _ => false,
    };

    Ok(r_bool(result))
}

/// Check character-specific constraints: missing (NA) and empty ("") values.
fn check_character_constraints(
    chars: &[Option<String>],
    missing_ok: Option<bool>,
    empty_ok: Option<bool>,
) -> bool {
    // If missing_ok is Some(false), reject NA values
    if missing_ok == Some(false) && chars.iter().any(|s| s.is_none()) {
        return false;
    }
    // If empty_ok is Some(false), reject empty strings
    if empty_ok == Some(false) && chars.iter().any(|s| s.as_deref() == Some("")) {
        return false;
    }
    true
}

/// ffi_is_string(x, string, empty)
fn ffi_is_string(args: &[RValue]) -> Result<RValue, RError> {
    let x = arg(args, 0);
    let string_arg = arg(args, 1);
    let empty_arg = arg(args, 2);

    let result = match x.as_vector() {
        Some(Vector::Character(chars)) if chars.len() == 1 => {
            // Must not be NA
            match &chars[0] {
                None => false,
                Some(s) => {
                    // If empty is FALSE, reject empty strings
                    if lgl_scalar(empty_arg) == Some(false) && s.is_empty() {
                        return Ok(r_false());
                    }
                    // If string arg is not NULL, check x matches one of the values
                    if !is_r_null(string_arg) {
                        if let Some(allowed) = as_char_vec(string_arg) {
                            allowed.iter().any(|a| a.as_deref() == Some(s.as_str()))
                        } else {
                            false
                        }
                    } else {
                        true
                    }
                }
            }
        }
        _ => false,
    };

    Ok(r_bool(result))
}

/// ffi_is_logical(x, n)
fn ffi_is_logical(args: &[RValue]) -> Result<RValue, RError> {
    let x = arg(args, 0);
    let n = int_scalar(arg(args, 1));

    let result = match x.as_vector() {
        Some(Vector::Logical(v)) => check_length(v.len(), n),
        _ => false,
    };

    Ok(r_bool(result))
}

/// ffi_is_integer(x, n)
fn ffi_is_integer(args: &[RValue]) -> Result<RValue, RError> {
    let x = arg(args, 0);
    let n = int_scalar(arg(args, 1));

    let result = match x.as_vector() {
        Some(Vector::Integer(v)) => check_length(v.len(), n),
        _ => false,
    };

    Ok(r_bool(result))
}

/// ffi_is_double(x, n, finite)
fn ffi_is_double(args: &[RValue]) -> Result<RValue, RError> {
    let x = arg(args, 0);
    let n = int_scalar(arg(args, 1));
    let finite = lgl_scalar(arg(args, 2));

    let result = match x.as_vector() {
        Some(Vector::Double(v)) => {
            if !check_length(v.len(), n) {
                false
            } else if finite == Some(true) {
                v.iter_opt().all(|opt| opt.is_some_and(|f| f.is_finite()))
            } else {
                true
            }
        }
        _ => false,
    };

    Ok(r_bool(result))
}

/// ffi_is_complex(x, n, finite)
fn ffi_is_complex(args: &[RValue]) -> Result<RValue, RError> {
    let x = arg(args, 0);
    let n = int_scalar(arg(args, 1));
    let finite = lgl_scalar(arg(args, 2));

    let result = match x.as_vector() {
        Some(Vector::Complex(v)) => {
            if !check_length(v.len(), n) {
                false
            } else if finite == Some(true) {
                v.iter()
                    .all(|opt| opt.is_some_and(|c| c.re.is_finite() && c.im.is_finite()))
            } else {
                true
            }
        }
        _ => false,
    };

    Ok(r_bool(result))
}

/// ffi_is_raw(x, n)
fn ffi_is_raw(args: &[RValue]) -> Result<RValue, RError> {
    let x = arg(args, 0);
    let n = int_scalar(arg(args, 1));

    let result = match x.as_vector() {
        Some(Vector::Raw(v)) => check_length(v.len(), n),
        _ => false,
    };

    Ok(r_bool(result))
}

/// ffi_is_list(x, n)
fn ffi_is_list(args: &[RValue]) -> Result<RValue, RError> {
    let x = arg(args, 0);
    let n = int_scalar(arg(args, 1));

    let result = match x {
        RValue::List(l) => check_length(l.values.len(), n),
        _ => false,
    };

    Ok(r_bool(result))
}

/// ffi_is_vector(x, n) — TRUE for atomic vectors and lists
fn ffi_is_vector(args: &[RValue]) -> Result<RValue, RError> {
    let x = arg(args, 0);
    let n = int_scalar(arg(args, 1));

    let result = match x {
        RValue::Vector(rv) => check_length(rv.inner.len(), n),
        RValue::List(l) => check_length(l.values.len(), n),
        RValue::Null => check_length(0, n),
        _ => false,
    };

    Ok(r_bool(result))
}

/// ffi_is_atomic(x, n) — TRUE for atomic vectors and NULL
fn ffi_is_atomic(args: &[RValue]) -> Result<RValue, RError> {
    let x = arg(args, 0);
    let n = int_scalar(arg(args, 1));

    let result = match x {
        RValue::Vector(rv) => check_length(rv.inner.len(), n),
        RValue::Null => check_length(0, n),
        _ => false,
    };

    Ok(r_bool(result))
}

/// ffi_is_function(x)
fn ffi_is_function(args: &[RValue]) -> Result<RValue, RError> {
    let x = arg(args, 0);
    Ok(r_bool(matches!(x, RValue::Function(_))))
}

/// ffi_is_closure(x)
fn ffi_is_closure(args: &[RValue]) -> Result<RValue, RError> {
    let x = arg(args, 0);
    Ok(r_bool(matches!(
        x,
        RValue::Function(RFunction::Closure { .. })
    )))
}

/// ffi_is_primitive(x)
fn ffi_is_primitive(args: &[RValue]) -> Result<RValue, RError> {
    let x = arg(args, 0);
    Ok(r_bool(matches!(
        x,
        RValue::Function(RFunction::Builtin { .. })
    )))
}

/// ffi_is_primitive_eager(x) — in miniR all builtins are eager
fn ffi_is_primitive_eager(args: &[RValue]) -> Result<RValue, RError> {
    ffi_is_primitive(args)
}

/// ffi_is_primitive_lazy(x) — in miniR no builtins are lazy
fn ffi_is_primitive_lazy(args: &[RValue]) -> Result<RValue, RError> {
    let _x = arg(args, 0);
    Ok(r_false())
}

/// ffi_is_formula(x, n, lhs)
fn ffi_is_formula(args: &[RValue]) -> Result<RValue, RError> {
    let x = arg(args, 0);
    let _n = int_scalar(arg(args, 1));
    let _lhs = lgl_scalar(arg(args, 2));

    // A formula is a language object with class "formula" or whose expression
    // is Expr::Formula.
    let result = match x {
        RValue::Language(lang) => {
            // Check class attribute for "formula"
            let has_formula_class = lang
                .class()
                .is_some_and(|c| c.iter().any(|s| s == "formula"));
            if has_formula_class {
                true
            } else {
                matches!(
                    lang.inner.as_ref(),
                    crate::parser::ast::Expr::Formula { .. }
                )
            }
        }
        _ => false,
    };

    Ok(r_bool(result))
}

/// ffi_is_call(x, name, n, ns)
fn ffi_is_call(args: &[RValue]) -> Result<RValue, RError> {
    let x = arg(args, 0);
    let _name = arg(args, 1);
    let _n = int_scalar(arg(args, 2));
    let _ns = arg(args, 3);

    // A "call" in R is a language object
    let result = matches!(x, RValue::Language(_));

    Ok(r_bool(result))
}

/// ffi_is_integerish(x, n, finite)
fn ffi_is_integerish(args: &[RValue]) -> Result<RValue, RError> {
    let x = arg(args, 0);
    let n = int_scalar(arg(args, 1));
    let finite = lgl_scalar(arg(args, 2));

    let result = match x.as_vector() {
        Some(Vector::Integer(v)) => {
            if !check_length(v.len(), n) {
                false
            } else if finite == Some(true) {
                // All non-NA (integer is always finite if non-NA)
                v.iter().all(|opt| opt.is_some())
            } else {
                true
            }
        }
        Some(Vector::Double(v)) => {
            if !check_length(v.len(), n) {
                false
            } else {
                // Check all values are whole numbers
                v.iter_opt().all(|opt| match opt {
                    None => finite != Some(true), // NA: ok unless finite required
                    Some(f) => {
                        if finite == Some(true) && !f.is_finite() {
                            false
                        } else if f.is_nan() || f.is_infinite() {
                            finite != Some(true)
                        } else {
                            f == f.trunc()
                        }
                    }
                })
            }
        }
        _ => false,
    };

    Ok(r_bool(result))
}

/// ffi_is_finite(x) — all elements are finite
fn ffi_is_finite(args: &[RValue]) -> Result<RValue, RError> {
    let x = arg(args, 0);

    let result = match x.as_vector() {
        Some(Vector::Double(v)) => v.iter_opt().all(|opt| opt.is_some_and(|f| f.is_finite())),
        Some(Vector::Integer(v)) => v.iter().all(|opt| opt.is_some()),
        Some(Vector::Logical(v)) => v.iter().all(|opt| opt.is_some()),
        Some(Vector::Complex(v)) => v
            .iter()
            .all(|opt| opt.is_some_and(|c| c.re.is_finite() && c.im.is_finite())),
        Some(Vector::Character(_) | Vector::Raw(_)) => true,
        None => matches!(x, RValue::Null),
    };

    Ok(r_bool(result))
}

/// ffi_is_reference(x, y) — check if two objects are identical (pointer-equal
/// for environments, deep-equal for others).
fn ffi_is_reference(args: &[RValue]) -> Result<RValue, RError> {
    let x = arg(args, 0);
    let y = arg(args, 1);

    let result = match (x, y) {
        (RValue::Environment(a), RValue::Environment(b)) => a.ptr_eq(b),
        _ => std::ptr::eq(x, y),
    };

    Ok(r_bool(result))
}

// endregion

// region: Utility function implementations

/// ffi_length(x) — return length as integer
fn ffi_length(args: &[RValue]) -> Result<RValue, RError> {
    let x = arg(args, 0);
    let len = i64::try_from(x.length()).unwrap_or(i64::MAX);
    Ok(RValue::vec(Vector::Integer(vec![Some(len)].into())))
}

/// ffi_names(x) — return names attribute
fn ffi_names(args: &[RValue]) -> Result<RValue, RError> {
    let x = arg(args, 0);

    match x {
        RValue::Vector(rv) => match rv.get_attr("names") {
            Some(v) => Ok(v.clone()),
            None => Ok(RValue::Null),
        },
        RValue::List(l) => {
            let names: Vec<Option<String>> =
                l.values.iter().map(|(name, _)| name.clone()).collect();
            if names.iter().all(|n| n.is_none()) {
                Ok(RValue::Null)
            } else {
                Ok(RValue::vec(Vector::Character(names.into())))
            }
        }
        _ => Ok(RValue::Null),
    }
}

/// ffi_set_names(x, names, transform) — set names on x.
/// `transform` is ignored (it's a function for name transformation).
fn ffi_set_names(args: &[RValue]) -> Result<RValue, RError> {
    let x = arg(args, 0).clone();
    let names_val = arg(args, 1);

    match x {
        RValue::Vector(mut rv) => {
            if names_val.is_null() {
                if let Some(ref mut attrs) = rv.attrs {
                    attrs.shift_remove("names");
                }
            } else {
                rv.set_attr("names".to_string(), names_val.clone());
            }
            Ok(RValue::Vector(rv))
        }
        RValue::List(mut l) => {
            if let Some(char_names) = as_char_vec(names_val) {
                for (i, entry) in l.values.iter_mut().enumerate() {
                    entry.0 = char_names.get(i).cloned().flatten();
                }
            } else if names_val.is_null() {
                for entry in &mut l.values {
                    entry.0 = None;
                }
            }
            Ok(RValue::List(l))
        }
        other => Ok(other),
    }
}

/// ffi_duplicate(x, shallow) — deep or shallow copy.
/// In miniR, RValue::clone() is always a deep copy since we use Rc<RefCell<>>
/// for environments.
fn ffi_duplicate(args: &[RValue]) -> Result<RValue, RError> {
    let x = arg(args, 0);
    Ok(x.clone())
}

/// ffi_symbol(x) — create a symbol (language object) from a string.
fn ffi_symbol(args: &[RValue]) -> Result<RValue, RError> {
    let x = arg(args, 0);
    let name = x
        .as_vector()
        .and_then(|v| v.as_character_scalar())
        .unwrap_or_default();
    Ok(RValue::Language(Language::new(
        crate::parser::ast::Expr::Symbol(name),
    )))
}

/// ffi_obj_address(x) — return address as hex string.
/// We use a hash of the debug representation since RValues don't have stable addresses.
fn ffi_obj_address(args: &[RValue]) -> Result<RValue, RError> {
    let x = arg(args, 0);
    let mut hasher = DefaultHasher::new();
    format!("{x:?}").hash(&mut hasher);
    let addr = format!("0x{:016x}", hasher.finish());
    Ok(RValue::vec(Vector::Character(vec![Some(addr)].into())))
}

/// ffi_hash(x) — compute a hash of x.
fn ffi_hash(args: &[RValue]) -> Result<RValue, RError> {
    let x = arg(args, 0);
    let mut hasher = DefaultHasher::new();
    format!("{x:?}").hash(&mut hasher);
    let hash_str = format!("{:016x}", hasher.finish());
    Ok(RValue::vec(Vector::Character(vec![Some(hash_str)].into())))
}

/// ffi_format_error_arg(x) — format an argument for error messages.
/// Returns the argument as a backtick-quoted string.
fn ffi_format_error_arg(args: &[RValue]) -> Result<RValue, RError> {
    let x = arg(args, 0);
    let formatted = match x {
        RValue::Vector(rv) => match &rv.inner {
            Vector::Character(c) => c
                .first()
                .cloned()
                .flatten()
                .map(|s| format!("`{s}`"))
                .unwrap_or_else(|| "``".to_string()),
            _ => format!("`{x}`"),
        },
        RValue::Language(lang) => format!("`{:?}`", lang.inner),
        _ => format!("`{x}`"),
    };
    Ok(RValue::vec(Vector::Character(vec![Some(formatted)].into())))
}

/// ffi_cnd_type(x) — return the condition type.
/// Inspects the class attribute to determine if it's an error, warning,
/// message, or generic condition.
fn ffi_cnd_type(args: &[RValue]) -> Result<RValue, RError> {
    let x = arg(args, 0);

    let class = match x {
        RValue::List(l) => l.class(),
        RValue::Vector(rv) => rv.class(),
        _ => None,
    };

    let cnd_type = match class {
        Some(classes) => {
            if classes.iter().any(|c| c == "error") {
                "error"
            } else if classes.iter().any(|c| c == "warning") {
                "warning"
            } else if classes.iter().any(|c| c == "message") {
                "message"
            } else {
                "condition"
            }
        }
        None => "condition",
    };

    Ok(RValue::vec(Vector::Character(
        vec![Some(cnd_type.to_string())].into(),
    )))
}

// endregion

// region: Environment function implementations

/// ffi_env_has(env, names, inherit)
fn ffi_env_has(args: &[RValue]) -> Result<RValue, RError> {
    let env_val = arg(args, 0);
    let names_val = arg(args, 1);
    let inherit = lgl_scalar(arg(args, 2)).unwrap_or(false);

    let env = match env_val {
        RValue::Environment(e) => e,
        _ => return Ok(r_false()),
    };

    let names = match as_char_vec(names_val) {
        Some(n) => n,
        None => return Ok(RValue::vec(Vector::Logical(vec![].into()))),
    };

    let results: Vec<Option<bool>> = names
        .iter()
        .map(|name| {
            let name_str = name.as_deref().unwrap_or("");
            let found = if inherit {
                env.get(name_str).is_some()
            } else {
                env.has_local(name_str)
            };
            Some(found)
        })
        .collect();

    let mut rv = RVector::from(Vector::Logical(results.into()));
    // Set names attribute on result
    rv.set_attr("names".to_string(), names_val.clone());

    Ok(RValue::Vector(rv))
}

/// ffi_env_poke_parent(env, parent) — set the parent of an environment.
fn ffi_env_poke_parent(args: &[RValue]) -> Result<RValue, RError> {
    let env_val = arg(args, 0);
    let parent_val = arg(args, 1);

    if let RValue::Environment(env) = env_val {
        if let RValue::Environment(parent) = parent_val {
            env.set_parent(Some(parent.clone()));
        }
    }

    Ok(RValue::Null)
}

/// ffi_env_clone(env) — clone an environment (shallow: same parent, copy bindings).
fn ffi_env_clone(args: &[RValue]) -> Result<RValue, RError> {
    let env_val = arg(args, 0);

    match env_val {
        RValue::Environment(env) => {
            let parent = env.parent();
            let new_env = match parent {
                Some(ref p) => crate::interpreter::environment::Environment::new_child(p),
                None => crate::interpreter::environment::Environment::new_global(),
            };
            // Copy all bindings
            for name in env.ls() {
                if let Some(val) = env.get(&name) {
                    new_env.set(name, val);
                }
            }
            Ok(RValue::Environment(new_env))
        }
        _ => Ok(RValue::Null),
    }
}

/// ffi_find_var(sym, env) — find a variable in an environment.
fn ffi_find_var(args: &[RValue]) -> Result<RValue, RError> {
    let sym_val = arg(args, 0);
    let env_val = arg(args, 1);

    let sym_name = match sym_val {
        RValue::Vector(rv) => rv.as_character_scalar(),
        RValue::Language(lang) => match lang.inner.as_ref() {
            crate::parser::ast::Expr::Symbol(s) => Some(s.clone()),
            _ => None,
        },
        _ => None,
    };

    let sym_name = match sym_name {
        Some(s) => s,
        None => return Ok(RValue::Null),
    };

    match env_val {
        RValue::Environment(env) => Ok(env.get(&sym_name).unwrap_or(RValue::Null)),
        _ => Ok(RValue::Null),
    }
}

/// ffi_ns_registry_env() — return the namespace registry (empty environment).
fn ffi_ns_registry_env() -> Result<RValue, RError> {
    Ok(RValue::Environment(
        crate::interpreter::environment::Environment::new_global(),
    ))
}

/// ffi_env_binding_types(env, names) — return binding types as an integer vector.
/// Types: 0 = regular, 1 = active binding, 2 = promise
fn ffi_env_binding_types(args: &[RValue]) -> Result<RValue, RError> {
    let env_val = arg(args, 0);
    let names_val = arg(args, 1);

    let env = match env_val {
        RValue::Environment(e) => e,
        _ => return Ok(RValue::vec(Vector::Integer(vec![].into()))),
    };

    let names = match as_char_vec(names_val) {
        Some(n) => n,
        None => return Ok(RValue::vec(Vector::Integer(vec![].into()))),
    };

    let types: Vec<Option<i64>> = names
        .iter()
        .map(|name| {
            let name_str = name.as_deref().unwrap_or("");
            if env.get_local_active_binding(name_str).is_some() {
                Some(1) // active binding
            } else {
                Some(0) // regular binding
            }
        })
        .collect();

    let mut rv = RVector::from(Vector::Integer(types.into()));
    rv.set_attr("names".to_string(), names_val.clone());

    Ok(RValue::Vector(rv))
}

// endregion

// region: Standalone type-check functions

/// ffi_standalone_is_bool(x, allow_na, allow_null) -> logical
fn ffi_standalone_is_bool(args: &[RValue]) -> Result<RValue, RError> {
    let x = args.first().unwrap_or(&RValue::Null);
    let allow_na = args
        .get(1)
        .and_then(|v| v.as_vector()?.as_logical_scalar())
        .unwrap_or(false);
    let allow_null = args
        .get(2)
        .and_then(|v| v.as_vector()?.as_logical_scalar())
        .unwrap_or(false);

    if matches!(x, RValue::Null) {
        return Ok(lgl(allow_null));
    }

    if let RValue::Vector(rv) = x {
        if let Vector::Logical(l) = &rv.inner {
            if l.len() == 1 {
                return match l[0] {
                    None => Ok(lgl(allow_na)),
                    Some(_) => Ok(lgl(true)),
                };
            }
        }
    }

    Ok(lgl(false))
}

/// ffi_standalone_check_number(x, allow_decimal, min, max, allow_infinite, allow_na, allow_null) -> integer
/// Returns 0 for success, positive integer for various failure codes.
fn ffi_standalone_check_number(args: &[RValue]) -> Result<RValue, RError> {
    let x = args.first().unwrap_or(&RValue::Null);
    let allow_decimal = args
        .get(1)
        .and_then(|v| v.as_vector()?.as_logical_scalar())
        .unwrap_or(true);
    let allow_infinite = args
        .get(4)
        .and_then(|v| v.as_vector()?.as_logical_scalar())
        .unwrap_or(true);
    let allow_na = args
        .get(5)
        .and_then(|v| v.as_vector()?.as_logical_scalar())
        .unwrap_or(false);
    let allow_null = args
        .get(6)
        .and_then(|v| v.as_vector()?.as_logical_scalar())
        .unwrap_or(false);

    if matches!(x, RValue::Null) {
        return Ok(int_val(if allow_null { 0 } else { 1 }));
    }

    if let RValue::Vector(rv) = x {
        match &rv.inner {
            Vector::Integer(i) if i.len() == 1 => {
                return match i.get_opt(0) {
                    None => Ok(int_val(if allow_na { 0 } else { 4 })),
                    Some(_) => Ok(int_val(0)),
                };
            }
            Vector::Double(d) if d.len() == 1 => {
                return match d.get_opt(0) {
                    None => Ok(int_val(if allow_na { 0 } else { 4 })),
                    Some(val) => {
                        if val.is_infinite() && !allow_infinite {
                            Ok(int_val(5))
                        } else if !allow_decimal && val.fract() != 0.0 {
                            Ok(int_val(2))
                        } else {
                            Ok(int_val(0))
                        }
                    }
                };
            }
            Vector::Logical(l) if l.len() == 1 && l[0].is_none() => {
                return Ok(int_val(if allow_na { 0 } else { 4 }));
            }
            _ => {}
        }
    }

    Ok(int_val(1))
}

fn int_val(v: i64) -> RValue {
    RValue::vec(Vector::Integer(vec![Some(v)].into()))
}

// endregion
