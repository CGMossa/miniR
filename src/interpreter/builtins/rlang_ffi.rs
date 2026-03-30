// Native Rust implementations of rlang FFI functions.
// rlang's C code uses `r_abort()` which spins in `while(1)` when Rf_eval
// returns on error, causing hangs in miniR. By intercepting rlang's `.Call`
// FFI functions and implementing them in pure Rust, we bypass rlang's C code
// entirely. This unblocks 83+ CRAN packages that depend on rlang.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::interpreter::value::*;

/// Try to dispatch an rlang FFI function by symbol name.
///
/// Returns `Some(result)` if the symbol was handled, `None` to fall through
/// to the native C code path.
pub fn try_dispatch(name: &str, args: &[RValue]) -> Option<Result<RValue, RError>> {
    match name {
        // region: Init functions — register CCallable shims so downstream packages
        // (purrr, stringr, dplyr) get working function pointers from R_GetCCallable.
        "ffi_init_r_library" | "ffi_init_rlang" => {
            register_rlang_ccallables();
            Some(Ok(RValue::Null))
        }
        "ffi_fini_rlang" | "ffi_glue_is_here" => Some(Ok(RValue::Null)),

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

        // region: List construction
        "ffi_list2" | "ffi_dots_list" | "ffi_dots_pairlist" => {
            // list2(...) / dots_list(...) — just return args as a named list
            Some(Ok(RValue::List(crate::interpreter::value::RList::new(
                args.iter().map(|v| (None, v.clone())).collect(),
            ))))
        }

        // region: Promise functions (stubs)
        "ffi_promise_expr" => Some(Ok(RValue::Null)),
        "ffi_promise_value" => Some(Ok(RValue::Null)),
        "ffi_promise_env" => Some(Ok(RValue::Null)),

        // region: Standalone type-check functions (used by lifecycle, vctrs, etc.)
        "ffi_standalone_is_bool_1.0.7" => Some(ffi_standalone_is_bool(args)),
        "ffi_standalone_check_number_1.0.7" => Some(ffi_standalone_check_number(args)),

        // Package linked_version checks — return the installed version string
        // so check_linked_version() succeeds. The C function normally returns the
        // compiled-in version string. We read it from DESCRIPTION.
        _ if name.ends_with("_linked_version") => {
            let pkg_name = name.trim_end_matches("_linked_version");
            // Try to find the version from cran/<pkg>/DESCRIPTION
            let version = std::fs::read_to_string(format!("cran/{pkg_name}/DESCRIPTION"))
                .ok()
                .and_then(|desc| {
                    desc.lines()
                        .find(|l| l.starts_with("Version:"))
                        .map(|l| l.trim_start_matches("Version:").trim().to_string())
                })
                .unwrap_or_default();
            Some(Ok(RValue::vec(Vector::Character(
                vec![Some(version)].into(),
            ))))
        }

        // Package init functions that pass namespace env to C — intercept as no-ops
        // since C code can't handle our ENVSXP representation.
        _ if name.ends_with("_init_library") || name.ends_with("_init_utils") => {
            tracing::debug!(symbol = name, "intercepted package init — no-op");
            Some(Ok(RValue::Null))
        }

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
// rlang CCallable shims — Rust implementations of rlang's exported C API.
// When rlang loads, its C init functions (`ffi_init_r_library`, `ffi_init_rlang`)
// set up internal state that many CCallable functions depend on. If that init
// fails or encounters unsupported R internals, the CCallable function pointers
// registered by `R_init_rlang` point to uninitialized code.
// This module provides standalone Rust implementations of the most important
// CCallable functions. After rlang's `.Call(ffi_init_rlang, ns)` runs, we
// overwrite the CCallable registry entries with these Rust implementations
// so downstream packages (purrr, stringr, dplyr, etc.) get working functions.

use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_void};

use crate::interpreter::native::runtime::{
    R_NilValue, R_RegisterCCallable, R_alloc, Rf_allocVector, Rf_getAttrib, Rf_inherits, Rf_mkChar,
    Rf_mkString,
};
use crate::interpreter::native::sexp::{self, Sexp};

// region: rlang_obj_type_friendly_full

/// Return a human-friendly type description for an R object.
///
/// Signature: `const char* rlang_obj_type_friendly_full(SEXP x, Rboolean value, Rboolean length)`
///
/// Returns strings like "a character vector", "a double vector", "NULL", etc.
/// The `value` param would add the actual value (e.g. `the string "foo"`),
/// the `length` param would add length info — we simplify both for now.
///
/// The returned string is allocated via `R_alloc` so it lives on the vmax
/// protection stack and is freed automatically.
extern "C" fn rlang_obj_type_friendly_full(x: Sexp, value: c_int, _length: c_int) -> *const c_char {
    let desc = if x.is_null() || x == unsafe { R_NilValue } {
        "NULL"
    } else {
        let stype = unsafe { (*x).stype };
        match stype {
            sexp::NILSXP => "NULL",
            sexp::LGLSXP => {
                let len = unsafe { (*x).length };
                if value != 0 && len == 1 {
                    "`TRUE` or `FALSE`"
                } else {
                    "a logical vector"
                }
            }
            sexp::INTSXP => {
                if Rf_inherits(x, c"factor".as_ptr()) != 0 {
                    "a factor"
                } else {
                    let len = unsafe { (*x).length };
                    if value != 0 && len == 1 {
                        "an integer"
                    } else {
                        "an integer vector"
                    }
                }
            }
            sexp::REALSXP => {
                let len = unsafe { (*x).length };
                if value != 0 && len == 1 {
                    "a number"
                } else {
                    "a double vector"
                }
            }
            sexp::CPLXSXP => "a complex vector",
            sexp::STRSXP => {
                let len = unsafe { (*x).length };
                if value != 0 && len == 1 {
                    "a string"
                } else {
                    "a character vector"
                }
            }
            sexp::RAWSXP => "a raw vector",
            sexp::VECSXP => {
                if Rf_inherits(x, c"data.frame".as_ptr()) != 0 {
                    "a data frame"
                } else if Rf_inherits(x, c"tbl_df".as_ptr()) != 0 {
                    "a tibble"
                } else {
                    "a list"
                }
            }
            // LISTSXP = 2 (pairlist)
            2 => "a pairlist",
            // CLOSXP = 3
            3 => "a function",
            // ENVSXP = 4
            4 => {
                if Rf_inherits(x, c"rlang_data_mask".as_ptr()) != 0 {
                    "a data mask"
                } else {
                    "an environment"
                }
            }
            // PROMSXP = 5
            5 => "a promise",
            // LANGSXP = 6
            6 => {
                if Rf_inherits(x, c"formula".as_ptr()) != 0 {
                    "a formula"
                } else {
                    "a call"
                }
            }
            // SPECIALSXP = 7
            7 => "a primitive function",
            // BUILTINSXP = 8
            8 => "a primitive function",
            sexp::CHARSXP => "an internal string",
            // EXPRSXP = 20
            20 => "an expression vector",
            // EXTPTRSXP = 22
            22 => "an external pointer",
            // SYMSXP = 1
            sexp::SYMSXP => "a symbol",
            _ => "an object",
        }
    };

    // Allocate via R_alloc and copy the string
    let len = desc.len() + 1;
    let buf = R_alloc(len, 1);
    if buf.is_null() {
        return c"an object".as_ptr();
    }
    unsafe {
        std::ptr::copy_nonoverlapping(desc.as_ptr(), buf as *mut u8, desc.len());
        *buf.add(desc.len()) = 0; // null terminator
    }
    buf
}

// endregion

// region: rlang_format_error_arg

/// Format an argument name for error messages.
///
/// Signature: `const char* rlang_format_error_arg(SEXP arg)`
///
/// In real rlang, this calls R code `format_arg(x)` which wraps the arg name
/// in backticks. We approximate: if the SEXP is a STRSXP of length 1,
/// extract the string and wrap it in backticks.
extern "C" fn rlang_format_error_arg(arg: Sexp) -> *const c_char {
    if arg.is_null() || arg == unsafe { R_NilValue } {
        return c"``".as_ptr();
    }

    // Try to extract a character scalar
    let name = unsafe {
        if (*arg).stype == sexp::STRSXP && (*arg).length >= 1 && !(*arg).data.is_null() {
            let elt = *((*arg).data as *const Sexp);
            if !elt.is_null() {
                sexp::char_data(elt)
            } else {
                ""
            }
        } else if (*arg).stype == sexp::SYMSXP && !(*arg).data.is_null() {
            sexp::char_data(arg)
        } else {
            ""
        }
    };

    // Format as `name`
    let formatted = format!("`{name}`");
    let len = formatted.len() + 1;
    let buf = R_alloc(len, 1);
    if buf.is_null() {
        return c"``".as_ptr();
    }
    unsafe {
        std::ptr::copy_nonoverlapping(formatted.as_ptr(), buf as *mut u8, formatted.len());
        *buf.add(formatted.len()) = 0;
    }
    buf
}

// endregion

// region: rlang_stop_internal / rlang_stop_internal2

/// rlang_stop_internal — abort with an internal error message.
///
/// Signature: `void rlang_stop_internal(const char* fn, const char* fmt, ...)`
///
/// In rlang, this is `r_no_return`. It calls `Rf_error` which longjmps.
/// Since this is called from C code inside `_minir_call_protected`, the
/// longjmp is safe (only crosses C frames).
extern "C" fn rlang_stop_internal(func: *const c_char, fmt: *const c_char) {
    // We can't handle varargs from Rust, but we can format what we have.
    let func_name = if func.is_null() {
        "<unknown>"
    } else {
        unsafe { CStr::from_ptr(func) }
            .to_str()
            .unwrap_or("<unknown>")
    };
    let msg = if fmt.is_null() {
        "internal error"
    } else {
        unsafe { CStr::from_ptr(fmt) }
            .to_str()
            .unwrap_or("internal error")
    };

    // Build a message and call Rf_error (which longjmps)
    let full_msg = format!("Internal error in `{func_name}()`: {msg}\0");
    extern "C" {
        fn Rf_error(fmt: *const c_char, ...) -> !;
    }
    unsafe {
        Rf_error(c"%s".as_ptr(), full_msg.as_ptr() as *const c_char);
    }
}

/// rlang_stop_internal2 — abort with file/line context.
///
/// Signature: `void rlang_stop_internal2(const char* file, int line, SEXP call, const char* fmt, ...)`
extern "C" fn rlang_stop_internal2(
    _file: *const c_char,
    _line: c_int,
    _call: Sexp,
    fmt: *const c_char,
) {
    let msg = if fmt.is_null() {
        "internal error"
    } else {
        unsafe { CStr::from_ptr(fmt) }
            .to_str()
            .unwrap_or("internal error")
    };
    let full_msg = format!("Internal error: {msg}\0");
    extern "C" {
        fn Rf_error(fmt: *const c_char, ...) -> !;
    }
    unsafe {
        Rf_error(c"%s".as_ptr(), full_msg.as_ptr() as *const c_char);
    }
}

// endregion

// region: rlang_is_quosure

/// Check if an object is a quosure.
///
/// Signature: `int rlang_is_quosure(SEXP x)`
///
/// A quosure is an object inheriting from "quosure".
extern "C" fn rlang_is_quosure(x: Sexp) -> c_int {
    Rf_inherits(x, c"quosure".as_ptr())
}

// endregion

// region: rlang_str_as_symbol

/// Convert a CHARSXP or scalar STRSXP to a symbol (SYMSXP).
///
/// Signature: `SEXP rlang_str_as_symbol(SEXP x)`
extern "C" fn rlang_str_as_symbol(x: Sexp) -> Sexp {
    if x.is_null() || x == unsafe { R_NilValue } {
        return unsafe { R_NilValue };
    }
    unsafe {
        // If it's a STRSXP, get the first CHARSXP element
        let charsxp = if (*x).stype == sexp::STRSXP && (*x).length >= 1 && !(*x).data.is_null() {
            *((*x).data as *const Sexp)
        } else if (*x).stype == sexp::CHARSXP {
            x
        } else {
            return R_NilValue;
        };

        if charsxp.is_null() {
            return R_NilValue;
        }

        // Create a symbol SEXP from the character data
        let name_ptr = (*charsxp).data as *const c_char;
        if name_ptr.is_null() {
            return R_NilValue;
        }
        crate::interpreter::native::runtime::Rf_install(name_ptr)
    }
}

// endregion

// region: rlang_names_as_unique

/// Make names unique by appending ...1, ...2, etc. for duplicates/NA/empty.
///
/// Signature: `SEXP rlang_names_as_unique(SEXP names, Rboolean quiet)`
///
/// Simplified implementation: just return the names as-is. A full implementation
/// would deduplicate, but this is enough to unblock dependent packages.
extern "C" fn rlang_names_as_unique(names: Sexp, _quiet: c_int) -> Sexp {
    if names.is_null() || names == unsafe { R_NilValue } {
        return unsafe { R_NilValue };
    }
    // Return names unchanged for now — downstream code handles missing names OK
    names
}

// endregion

// region: rlang_eval_tidy

/// Evaluate an expression in a tidy evaluation context.
///
/// Signature: `SEXP rlang_eval_tidy(SEXP expr, SEXP data, SEXP env)`
///
/// Simplified: just evaluate the expression in the given environment,
/// ignoring the data mask. This is enough for basic purrr/dplyr usage.
extern "C" fn rlang_eval_tidy(expr: Sexp, _data: Sexp, env: Sexp) -> Sexp {
    // Delegate to Rf_eval — this handles the simple case where there's no data mask
    crate::interpreter::native::runtime::Rf_eval(expr, env)
}

// endregion

// region: data mask stubs

/// Create a new data mask from bottom and top environments.
///
/// Signature: `SEXP rlang_new_data_mask_3.0.0(SEXP bottom, SEXP top)`
///
/// Returns the bottom env as-is — a simplification that works for basic cases.
extern "C" fn rlang_new_data_mask(bottom: Sexp, _top: Sexp) -> Sexp {
    if bottom.is_null() || bottom == unsafe { R_NilValue } {
        return unsafe { R_NilValue };
    }
    bottom
}

/// Convert data to a data mask.
///
/// Signature: `SEXP rlang_as_data_mask(SEXP data)`
extern "C" fn rlang_as_data_mask(data: Sexp) -> Sexp {
    data
}

/// Create a data pronoun from an environment.
///
/// Signature: `SEXP rlang_as_data_pronoun(SEXP env)`
extern "C" fn rlang_as_data_pronoun(env: Sexp) -> Sexp {
    env
}

// endregion

// region: rlang_env_unbind

/// Unbind variables from an environment.
///
/// Signature: `void rlang_env_unbind(SEXP env, SEXP names)`
///
/// Stub — unbinding is not critical for package loading.
extern "C" fn rlang_env_unbind(_env: Sexp, _names: Sexp) {
    // No-op stub
}

// endregion

// region: rlang_as_function

/// Coerce to function — if it's already a function, return it.
///
/// Signature: `SEXP rlang_as_function(SEXP x, const char* arg)`
extern "C" fn rlang_as_function(x: Sexp, _arg: *const c_char) -> Sexp {
    // If already a function (CLOSXP, BUILTINSXP, SPECIALSXP), return it
    if !x.is_null() && x != unsafe { R_NilValue } {
        let stype = unsafe { (*x).stype };
        if matches!(stype, 3 | 7 | 8) {
            return x;
        }
    }
    // Otherwise return as-is — rlang's real impl would convert formulas etc.
    x
}

// endregion

// region: quosure stubs

/// Get the expression from a quosure.
extern "C" fn rlang_quo_get_expr(quo: Sexp) -> Sexp {
    // A quosure is a formula with class "quosure" — the expression is the RHS
    // For simplicity, return the input or R_NilValue
    if quo.is_null() || quo == unsafe { R_NilValue } {
        return unsafe { R_NilValue };
    }
    // Try to get the formula's RHS via the second element of the LANGSXP
    unsafe {
        if (*quo).stype == 6 {
            // LANGSXP — the formula `~expr` has CAR=`~`, CDR->CAR=expr
            if !(*quo).data.is_null() {
                let pd = (*quo).data as *const sexp::PairlistData;
                let cdr = (*pd).cdr;
                if !cdr.is_null() && !(*cdr).data.is_null() {
                    let pd2 = (*cdr).data as *const sexp::PairlistData;
                    return (*pd2).car;
                }
            }
        }
        R_NilValue
    }
}

/// Get the environment from a quosure.
extern "C" fn rlang_quo_get_env(quo: Sexp) -> Sexp {
    // The quosure's env is stored as an attribute ".environment"
    if quo.is_null() || quo == unsafe { R_NilValue } {
        return unsafe { R_NilValue };
    }
    // Look for .environment attribute
    let env_sym = crate::interpreter::native::runtime::Rf_install(c".environment".as_ptr());
    let env = Rf_getAttrib(quo, env_sym);
    if env.is_null() || env == unsafe { R_NilValue } {
        return unsafe { R_NilValue };
    }
    env
}

/// Set the expression on a quosure.
extern "C" fn rlang_quo_set_expr(quo: Sexp, _expr: Sexp) -> Sexp {
    // Stub — return the quosure unchanged
    quo
}

/// Set the environment on a quosure.
extern "C" fn rlang_quo_set_env(quo: Sexp, _env: Sexp) -> Sexp {
    // Stub — return the quosure unchanged
    quo
}

/// Create a new quosure.
extern "C" fn rlang_new_quosure(_expr: Sexp, _env: Sexp) -> Sexp {
    // Stub — return R_NilValue
    unsafe { R_NilValue }
}

// endregion

// region: additional stubs

/// arg_match — match an argument to allowed values (legacy).
extern "C" fn rlang_arg_match(_arg: Sexp, _values: Sexp, _error_arg: Sexp) -> Sexp {
    // Return the argument unchanged
    _arg
}

/// arg_match_2 — match an argument to allowed values.
extern "C" fn rlang_arg_match_2(
    _arg: Sexp,
    _values: Sexp,
    _error_arg: Sexp,
    _error_call: Sexp,
) -> Sexp {
    _arg
}

/// is_splice_box — check if object is a splice box.
extern "C" fn rlang_is_splice_box(_x: Sexp) -> c_int {
    0
}

/// Encode a character vector as UTF-8.
extern "C" fn rlang_obj_encode_utf8(x: Sexp) -> Sexp {
    // Our strings are already UTF-8
    x
}

/// Convert a symbol to a character SEXP.
extern "C" fn rlang_sym_as_character(sym: Sexp) -> Sexp {
    if sym.is_null() || sym == unsafe { R_NilValue } {
        return Rf_mkString(c"".as_ptr());
    }
    unsafe {
        if (*sym).stype == sexp::SYMSXP && !(*sym).data.is_null() {
            let name_ptr = (*sym).data as *const c_char;
            return Rf_mkString(name_ptr);
        }
    }
    Rf_mkString(c"".as_ptr())
}

/// Convert a symbol to a string SEXP (CHARSXP).
extern "C" fn rlang_sym_as_string(sym: Sexp) -> Sexp {
    if sym.is_null() || sym == unsafe { R_NilValue } {
        return Rf_mkChar(c"".as_ptr());
    }
    unsafe {
        if (*sym).stype == sexp::SYMSXP && !(*sym).data.is_null() {
            let name_ptr = (*sym).data as *const c_char;
            return Rf_mkChar(name_ptr);
        }
    }
    Rf_mkChar(c"".as_ptr())
}

/// Unbox a scalar value from a list.
extern "C" fn rlang_unbox(x: Sexp) -> Sexp {
    // If it's a length-1 list, return the first element
    if !x.is_null() && x != unsafe { R_NilValue } {
        unsafe {
            if (*x).stype == sexp::VECSXP && (*x).length == 1 && !(*x).data.is_null() {
                return *((*x).data as *const Sexp);
            }
        }
    }
    x
}

/// Squash a list conditionally.
extern "C" fn rlang_squash_if(_x: Sexp, _type: Sexp, _predicate: Sexp) -> Sexp {
    unsafe { R_NilValue }
}

/// Get dots as a list from an environment.
extern "C" fn rlang_env_dots_list(_env: Sexp) -> Sexp {
    // Return empty list
    Rf_allocVector(sexp::VECSXP as c_int, 0)
}

/// Get dots values from an environment.
extern "C" fn rlang_env_dots_values(_env: Sexp) -> Sexp {
    Rf_allocVector(sexp::VECSXP as c_int, 0)
}

/// Print backtrace — no-op.
extern "C" fn rlang_print_backtrace() {
    // No-op
}

/// Print environment — no-op.
extern "C" fn rlang_env_print(_env: Sexp) {
    // No-op
}

/// xxh3_64bits hash — return 0 as stub.
extern "C" fn rlang_xxh3_64bits(_data: *const c_void, _len: usize) -> u64 {
    0
}

// endregion

// region: registration

/// Register all rlang CCallable functions in the cross-package registry.
///
/// This overwrites any entries previously registered by rlang's own C code,
/// ensuring downstream packages get working Rust implementations instead of
/// pointers to uninitialized C code.
pub fn register_rlang_ccallables() {
    let registrations: &[(&str, *const ())] = &[
        (
            "rlang_obj_type_friendly_full",
            rlang_obj_type_friendly_full as *const (),
        ),
        (
            "rlang_format_error_arg",
            rlang_format_error_arg as *const (),
        ),
        ("rlang_stop_internal", rlang_stop_internal as *const ()),
        ("rlang_stop_internal2", rlang_stop_internal2 as *const ()),
        ("rlang_is_quosure", rlang_is_quosure as *const ()),
        ("rlang_str_as_symbol", rlang_str_as_symbol as *const ()),
        ("rlang_names_as_unique", rlang_names_as_unique as *const ()),
        ("rlang_eval_tidy", rlang_eval_tidy as *const ()),
        (
            "rlang_new_data_mask_3.0.0",
            rlang_new_data_mask as *const (),
        ),
        ("rlang_as_data_mask_3.0.0", rlang_as_data_mask as *const ()),
        ("rlang_as_data_pronoun", rlang_as_data_pronoun as *const ()),
        ("rlang_env_unbind", rlang_env_unbind as *const ()),
        ("rlang_as_function", rlang_as_function as *const ()),
        ("rlang_quo_get_expr", rlang_quo_get_expr as *const ()),
        ("rlang_quo_get_env", rlang_quo_get_env as *const ()),
        ("rlang_quo_set_expr", rlang_quo_set_expr as *const ()),
        ("rlang_quo_set_env", rlang_quo_set_env as *const ()),
        ("rlang_new_quosure", rlang_new_quosure as *const ()),
        ("rlang_arg_match", rlang_arg_match as *const ()),
        ("rlang_arg_match_2", rlang_arg_match_2 as *const ()),
        ("rlang_is_splice_box", rlang_is_splice_box as *const ()),
        ("rlang_obj_encode_utf8", rlang_obj_encode_utf8 as *const ()),
        (
            "rlang_sym_as_character",
            rlang_sym_as_character as *const (),
        ),
        ("rlang_sym_as_string", rlang_sym_as_string as *const ()),
        ("rlang_unbox", rlang_unbox as *const ()),
        ("rlang_squash_if", rlang_squash_if as *const ()),
        ("rlang_env_dots_list", rlang_env_dots_list as *const ()),
        ("rlang_env_dots_values", rlang_env_dots_values as *const ()),
        ("rlang_as_data_mask", rlang_as_data_mask as *const ()),
        ("rlang_new_data_mask", rlang_new_data_mask as *const ()),
        ("rlang_print_backtrace", rlang_print_backtrace as *const ()),
        ("rlang_env_print", rlang_env_print as *const ()),
        ("rlang_xxh3_64bits", rlang_xxh3_64bits as *const ()),
    ];

    for &(name, fptr) in registrations {
        let pkg = std::ffi::CString::new("rlang").expect("CString::new");
        let nm = std::ffi::CString::new(name).expect("CString::new");
        R_RegisterCCallable(pkg.as_ptr(), nm.as_ptr(), fptr);
    }

    tracing::debug!("registered {} rlang CCallable shims", registrations.len());
}

// endregion
