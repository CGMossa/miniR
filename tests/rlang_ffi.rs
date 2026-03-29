//! Tests for the native Rust rlang FFI intercept layer.
//!
//! These test that rlang's `.Call("ffi_*", ...)` functions are handled
//! natively in Rust without calling into rlang's C code.

#![cfg(feature = "native")]

use r::interpreter::builtins::rlang_ffi;
use r::interpreter::value::*;

// region: Helper

fn r_null() -> RValue {
    RValue::Null
}

fn r_int(v: i64) -> RValue {
    RValue::vec(Vector::Integer(vec![Some(v)].into()))
}

fn r_double(v: f64) -> RValue {
    RValue::vec(Vector::Double(vec![Some(v)].into()))
}

fn r_char(s: &str) -> RValue {
    RValue::vec(Vector::Character(vec![Some(s.to_string())].into()))
}

fn r_lgl(v: bool) -> RValue {
    RValue::vec(Vector::Logical(vec![Some(v)].into()))
}

fn r_char_vec(vals: &[&str]) -> RValue {
    RValue::vec(Vector::Character(
        vals.iter()
            .map(|s| Some(s.to_string()))
            .collect::<Vec<_>>()
            .into(),
    ))
}

fn r_int_vec(vals: &[i64]) -> RValue {
    RValue::vec(Vector::Integer(
        vals.iter().map(|v| Some(*v)).collect::<Vec<_>>().into(),
    ))
}

fn r_double_vec(vals: &[f64]) -> RValue {
    RValue::vec(Vector::Double(
        vals.iter().map(|v| Some(*v)).collect::<Vec<_>>().into(),
    ))
}

fn assert_is_true(result: &Result<RValue, RError>) {
    match result {
        Ok(RValue::Vector(rv)) => match &rv.inner {
            Vector::Logical(v) => assert_eq!(v[0], Some(true), "expected TRUE, got {:?}", v[0]),
            other => panic!("expected Logical vector, got {:?}", other),
        },
        other => panic!("expected Ok(Vector(Logical)), got {:?}", other),
    }
}

fn assert_is_false(result: &Result<RValue, RError>) {
    match result {
        Ok(RValue::Vector(rv)) => match &rv.inner {
            Vector::Logical(v) => assert_eq!(v[0], Some(false), "expected FALSE, got {:?}", v[0]),
            other => panic!("expected Logical vector, got {:?}", other),
        },
        other => panic!("expected Ok(Vector(Logical)), got {:?}", other),
    }
}

fn assert_is_null(result: &Result<RValue, RError>) {
    match result {
        Ok(RValue::Null) => {}
        other => panic!("expected Ok(Null), got {:?}", other),
    }
}

// endregion

// region: Init functions

#[test]
fn init_functions_return_null() {
    for name in &[
        "ffi_init_r_library",
        "ffi_init_rlang",
        "ffi_fini_rlang",
        "ffi_glue_is_here",
    ] {
        let result = rlang_ffi::try_dispatch(name, &[]);
        assert!(result.is_some(), "{name} should be intercepted");
        assert_is_null(&result.unwrap());
    }
}

// endregion

// region: Type checking

#[test]
fn ffi_is_character_basic() {
    // character vector -> TRUE
    let args = vec![r_char("hello"), r_null(), r_null(), r_null()];
    let result = rlang_ffi::try_dispatch("ffi_is_character", &args).unwrap();
    assert_is_true(&result);

    // integer vector -> FALSE
    let args = vec![r_int(42), r_null(), r_null(), r_null()];
    let result = rlang_ffi::try_dispatch("ffi_is_character", &args).unwrap();
    assert_is_false(&result);
}

#[test]
fn ffi_is_character_with_length_check() {
    let args = vec![r_char_vec(&["a", "b"]), r_int(2), r_null(), r_null()];
    let result = rlang_ffi::try_dispatch("ffi_is_character", &args).unwrap();
    assert_is_true(&result);

    let args = vec![r_char_vec(&["a", "b"]), r_int(3), r_null(), r_null()];
    let result = rlang_ffi::try_dispatch("ffi_is_character", &args).unwrap();
    assert_is_false(&result);
}

#[test]
fn ffi_is_string_basic() {
    // Single string -> TRUE
    let args = vec![r_char("hello"), r_null(), r_null()];
    let result = rlang_ffi::try_dispatch("ffi_is_string", &args).unwrap();
    assert_is_true(&result);

    // Length-2 character vector -> FALSE (not a single string)
    let args = vec![r_char_vec(&["a", "b"]), r_null(), r_null()];
    let result = rlang_ffi::try_dispatch("ffi_is_string", &args).unwrap();
    assert_is_false(&result);
}

#[test]
fn ffi_is_string_with_allowed_values() {
    let args = vec![r_char("error"), r_char_vec(&["error", "warning"]), r_null()];
    let result = rlang_ffi::try_dispatch("ffi_is_string", &args).unwrap();
    assert_is_true(&result);

    let args = vec![
        r_char("message"),
        r_char_vec(&["error", "warning"]),
        r_null(),
    ];
    let result = rlang_ffi::try_dispatch("ffi_is_string", &args).unwrap();
    assert_is_false(&result);
}

#[test]
fn ffi_is_string_empty_rejected() {
    let args = vec![r_char(""), r_null(), r_lgl(false)];
    let result = rlang_ffi::try_dispatch("ffi_is_string", &args).unwrap();
    assert_is_false(&result);
}

#[test]
fn ffi_is_logical_basic() {
    let args = vec![r_lgl(true), r_null()];
    let result = rlang_ffi::try_dispatch("ffi_is_logical", &args).unwrap();
    assert_is_true(&result);

    let args = vec![r_int(1), r_null()];
    let result = rlang_ffi::try_dispatch("ffi_is_logical", &args).unwrap();
    assert_is_false(&result);
}

#[test]
fn ffi_is_integer_basic() {
    let args = vec![r_int(42), r_null()];
    let result = rlang_ffi::try_dispatch("ffi_is_integer", &args).unwrap();
    assert_is_true(&result);

    let args = vec![r_double(7.5), r_null()];
    let result = rlang_ffi::try_dispatch("ffi_is_integer", &args).unwrap();
    assert_is_false(&result);
}

#[test]
fn ffi_is_double_basic() {
    let args = vec![r_double(7.5), r_null(), r_null()];
    let result = rlang_ffi::try_dispatch("ffi_is_double", &args).unwrap();
    assert_is_true(&result);

    let args = vec![r_int(42), r_null(), r_null()];
    let result = rlang_ffi::try_dispatch("ffi_is_double", &args).unwrap();
    assert_is_false(&result);
}

#[test]
fn ffi_is_double_finite_check() {
    let args = vec![r_double(f64::INFINITY), r_null(), r_lgl(true)];
    let result = rlang_ffi::try_dispatch("ffi_is_double", &args).unwrap();
    assert_is_false(&result);

    let args = vec![r_double(42.0), r_null(), r_lgl(true)];
    let result = rlang_ffi::try_dispatch("ffi_is_double", &args).unwrap();
    assert_is_true(&result);
}

#[test]
fn ffi_is_list_basic() {
    let list = RValue::List(RList::new(vec![(None, r_int(1)), (None, r_int(2))]));
    let args = vec![list, r_null()];
    let result = rlang_ffi::try_dispatch("ffi_is_list", &args).unwrap();
    assert_is_true(&result);

    let args = vec![r_int(42), r_null()];
    let result = rlang_ffi::try_dispatch("ffi_is_list", &args).unwrap();
    assert_is_false(&result);
}

#[test]
fn ffi_is_function_basic() {
    let args = vec![r_int(42)];
    let result = rlang_ffi::try_dispatch("ffi_is_function", &args).unwrap();
    assert_is_false(&result);
}

#[test]
fn ffi_is_atomic_basic() {
    let args = vec![r_int(42), r_null()];
    let result = rlang_ffi::try_dispatch("ffi_is_atomic", &args).unwrap();
    assert_is_true(&result);

    let args = vec![r_null(), r_null()];
    let result = rlang_ffi::try_dispatch("ffi_is_atomic", &args).unwrap();
    assert_is_true(&result);

    let list = RValue::List(RList::new(vec![]));
    let args = vec![list, r_null()];
    let result = rlang_ffi::try_dispatch("ffi_is_atomic", &args).unwrap();
    assert_is_false(&result);
}

#[test]
fn ffi_is_integerish_integer_input() {
    let args = vec![r_int(42), r_null(), r_null()];
    let result = rlang_ffi::try_dispatch("ffi_is_integerish", &args).unwrap();
    assert_is_true(&result);
}

#[test]
fn ffi_is_integerish_double_whole_numbers() {
    let args = vec![r_double_vec(&[1.0, 2.0, 3.0]), r_null(), r_null()];
    let result = rlang_ffi::try_dispatch("ffi_is_integerish", &args).unwrap();
    assert_is_true(&result);
}

#[test]
fn ffi_is_integerish_double_non_whole() {
    let args = vec![r_double(7.5), r_null(), r_null()];
    let result = rlang_ffi::try_dispatch("ffi_is_integerish", &args).unwrap();
    assert_is_false(&result);
}

#[test]
fn ffi_is_finite_all_finite() {
    let args = vec![r_double_vec(&[1.0, 2.0, 3.0])];
    let result = rlang_ffi::try_dispatch("ffi_is_finite", &args).unwrap();
    assert_is_true(&result);
}

#[test]
fn ffi_is_finite_with_inf() {
    let args = vec![r_double_vec(&[1.0, f64::INFINITY])];
    let result = rlang_ffi::try_dispatch("ffi_is_finite", &args).unwrap();
    assert_is_false(&result);
}

#[test]
fn ffi_is_call_language_object() {
    let lang = RValue::Language(Language::new(r::parser::ast::Expr::Symbol("x".to_string())));
    let args = vec![lang, r_null(), r_null(), r_null()];
    let result = rlang_ffi::try_dispatch("ffi_is_call", &args).unwrap();
    assert_is_true(&result);
}

#[test]
fn ffi_is_call_non_language() {
    let args = vec![r_int(42), r_null(), r_null(), r_null()];
    let result = rlang_ffi::try_dispatch("ffi_is_call", &args).unwrap();
    assert_is_false(&result);
}

#[test]
fn ffi_is_weakref_always_false() {
    let result = rlang_ffi::try_dispatch("ffi_is_weakref", &[r_int(1)]).unwrap();
    assert_is_false(&result);
}

#[test]
fn ffi_is_splice_box_always_false() {
    let result = rlang_ffi::try_dispatch("ffi_is_splice_box", &[r_int(1)]).unwrap();
    assert_is_false(&result);
}

// endregion

// region: Utility functions

#[test]
fn ffi_length_returns_correct_length() {
    let args = vec![r_int_vec(&[1, 2, 3])];
    let result = rlang_ffi::try_dispatch("ffi_length", &args)
        .unwrap()
        .unwrap();
    match &result {
        RValue::Vector(rv) => match &rv.inner {
            Vector::Integer(v) => assert_eq!(v.get_opt(0), Some(3)),
            other => panic!("expected Integer, got {:?}", other),
        },
        other => panic!("expected Vector, got {:?}", other),
    }
}

#[test]
fn ffi_names_vector_with_names() {
    let mut rv = RVector::from(Vector::Integer(vec![Some(1i64), Some(2)].into()));
    rv.set_attr("names".to_string(), r_char_vec(&["a", "b"]));
    let args = vec![RValue::Vector(rv)];
    let result = rlang_ffi::try_dispatch("ffi_names", &args)
        .unwrap()
        .unwrap();
    match &result {
        RValue::Vector(rv) => match &rv.inner {
            Vector::Character(c) => {
                assert_eq!(c[0], Some("a".to_string()));
                assert_eq!(c[1], Some("b".to_string()));
            }
            other => panic!("expected Character, got {:?}", other),
        },
        other => panic!("expected Vector, got {:?}", other),
    }
}

#[test]
fn ffi_names_without_names_returns_null() {
    let args = vec![r_int_vec(&[1, 2])];
    let result = rlang_ffi::try_dispatch("ffi_names", &args).unwrap();
    assert_is_null(&result);
}

#[test]
fn ffi_duplicate_clones_value() {
    let args = vec![r_int(42), r_lgl(false)];
    let result = rlang_ffi::try_dispatch("ffi_duplicate", &args)
        .unwrap()
        .unwrap();
    match &result {
        RValue::Vector(rv) => match &rv.inner {
            Vector::Integer(v) => assert_eq!(v.get_opt(0), Some(42)),
            other => panic!("expected Integer, got {:?}", other),
        },
        other => panic!("expected Vector, got {:?}", other),
    }
}

#[test]
fn ffi_symbol_creates_language_object() {
    let args = vec![r_char("x")];
    let result = rlang_ffi::try_dispatch("ffi_symbol", &args)
        .unwrap()
        .unwrap();
    match &result {
        RValue::Language(lang) => match lang.inner.as_ref() {
            r::parser::ast::Expr::Symbol(s) => assert_eq!(s, "x"),
            other => panic!("expected Symbol, got {:?}", other),
        },
        other => panic!("expected Language, got {:?}", other),
    }
}

#[test]
fn ffi_compiled_by_gcc_returns_false() {
    let result = rlang_ffi::try_dispatch("ffi_compiled_by_gcc", &[]).unwrap();
    assert_is_false(&result);
}

#[test]
fn ffi_missing_arg_returns_null() {
    let result = rlang_ffi::try_dispatch("ffi_missing_arg", &[]).unwrap();
    assert_is_null(&result);
}

#[test]
fn ffi_obj_address_returns_string() {
    let args = vec![r_int(42)];
    let result = rlang_ffi::try_dispatch("ffi_obj_address", &args)
        .unwrap()
        .unwrap();
    match &result {
        RValue::Vector(rv) => match &rv.inner {
            Vector::Character(c) => {
                assert!(c[0].as_ref().unwrap().starts_with("0x"));
            }
            other => panic!("expected Character, got {:?}", other),
        },
        other => panic!("expected Vector, got {:?}", other),
    }
}

#[test]
fn ffi_hash_returns_hex_string() {
    let args = vec![r_int(42)];
    let result = rlang_ffi::try_dispatch("ffi_hash", &args).unwrap().unwrap();
    match &result {
        RValue::Vector(rv) => match &rv.inner {
            Vector::Character(c) => {
                let hash = c[0].as_ref().unwrap();
                assert_eq!(hash.len(), 16, "hash should be 16 hex chars");
            }
            other => panic!("expected Character, got {:?}", other),
        },
        other => panic!("expected Vector, got {:?}", other),
    }
}

#[test]
fn ffi_format_error_arg_backtick_quotes() {
    let args = vec![r_char("myarg")];
    let result = rlang_ffi::try_dispatch("ffi_format_error_arg", &args)
        .unwrap()
        .unwrap();
    match &result {
        RValue::Vector(rv) => match &rv.inner {
            Vector::Character(c) => {
                assert_eq!(c[0].as_deref(), Some("`myarg`"));
            }
            other => panic!("expected Character, got {:?}", other),
        },
        other => panic!("expected Vector, got {:?}", other),
    }
}

#[test]
fn ffi_cnd_type_error() {
    let mut list = RList::new(vec![(Some("message".to_string()), r_char("boom"))]);
    list.set_attr(
        "class".to_string(),
        r_char_vec(&["simpleError", "error", "condition"]),
    );
    let args = vec![RValue::List(list)];
    let result = rlang_ffi::try_dispatch("ffi_cnd_type", &args)
        .unwrap()
        .unwrap();
    match &result {
        RValue::Vector(rv) => match &rv.inner {
            Vector::Character(c) => assert_eq!(c[0].as_deref(), Some("error")),
            other => panic!("expected Character, got {:?}", other),
        },
        other => panic!("expected Vector, got {:?}", other),
    }
}

#[test]
fn ffi_cnd_type_warning() {
    let mut list = RList::new(vec![]);
    list.set_attr(
        "class".to_string(),
        r_char_vec(&["simpleWarning", "warning", "condition"]),
    );
    let args = vec![RValue::List(list)];
    let result = rlang_ffi::try_dispatch("ffi_cnd_type", &args)
        .unwrap()
        .unwrap();
    match &result {
        RValue::Vector(rv) => match &rv.inner {
            Vector::Character(c) => assert_eq!(c[0].as_deref(), Some("warning")),
            other => panic!("expected Character, got {:?}", other),
        },
        other => panic!("expected Vector, got {:?}", other),
    }
}

// endregion

// region: Environment functions

#[test]
fn ffi_env_has_finds_bindings() {
    let env = r::interpreter::environment::Environment::new_global();
    env.set("x".to_string(), r_int(42));

    let args = vec![
        RValue::Environment(env),
        r_char_vec(&["x", "y"]),
        r_lgl(false),
    ];
    let result = rlang_ffi::try_dispatch("ffi_env_has", &args)
        .unwrap()
        .unwrap();
    match &result {
        RValue::Vector(rv) => match &rv.inner {
            Vector::Logical(v) => {
                assert_eq!(v[0], Some(true), "x should be found");
                assert_eq!(v[1], Some(false), "y should not be found");
            }
            other => panic!("expected Logical, got {:?}", other),
        },
        other => panic!("expected Vector, got {:?}", other),
    }
}

#[test]
fn ffi_env_has_inherit_walks_parent() {
    let parent = r::interpreter::environment::Environment::new_global();
    parent.set("x".to_string(), r_int(1));
    let child = r::interpreter::environment::Environment::new_child(&parent);

    // Without inherit: x not found in child
    let args = vec![
        RValue::Environment(child.clone()),
        r_char("x"),
        r_lgl(false),
    ];
    let result = rlang_ffi::try_dispatch("ffi_env_has", &args)
        .unwrap()
        .unwrap();
    match &result {
        RValue::Vector(rv) => match &rv.inner {
            Vector::Logical(v) => assert_eq!(v[0], Some(false)),
            _ => panic!("expected Logical"),
        },
        _ => panic!("expected Vector"),
    }

    // With inherit: x found via parent
    let args = vec![RValue::Environment(child), r_char("x"), r_lgl(true)];
    let result = rlang_ffi::try_dispatch("ffi_env_has", &args)
        .unwrap()
        .unwrap();
    match &result {
        RValue::Vector(rv) => match &rv.inner {
            Vector::Logical(v) => assert_eq!(v[0], Some(true)),
            _ => panic!("expected Logical"),
        },
        _ => panic!("expected Vector"),
    }
}

#[test]
fn ffi_env_clone_copies_bindings() {
    let env = r::interpreter::environment::Environment::new_global();
    env.set("x".to_string(), r_int(42));

    let args = vec![RValue::Environment(env.clone())];
    let result = rlang_ffi::try_dispatch("ffi_env_clone", &args)
        .unwrap()
        .unwrap();
    match &result {
        RValue::Environment(cloned) => {
            // Cloned env should have x
            assert!(cloned.get("x").is_some());
            // But should be a different env
            assert!(!env.ptr_eq(cloned));
        }
        other => panic!("expected Environment, got {:?}", other),
    }
}

#[test]
fn ffi_find_var_in_env() {
    let env = r::interpreter::environment::Environment::new_global();
    env.set("myvar".to_string(), r_int(99));

    let args = vec![r_char("myvar"), RValue::Environment(env)];
    let result = rlang_ffi::try_dispatch("ffi_find_var", &args)
        .unwrap()
        .unwrap();
    match &result {
        RValue::Vector(rv) => match &rv.inner {
            Vector::Integer(v) => assert_eq!(v.get_opt(0), Some(99)),
            other => panic!("expected Integer, got {:?}", other),
        },
        other => panic!("expected Vector, got {:?}", other),
    }
}

#[test]
fn ffi_find_var_missing_returns_null() {
    let env = r::interpreter::environment::Environment::new_global();
    let args = vec![r_char("nonexistent"), RValue::Environment(env)];
    let result = rlang_ffi::try_dispatch("ffi_find_var", &args).unwrap();
    assert_is_null(&result);
}

#[test]
fn ffi_ns_registry_env_returns_environment() {
    let result = rlang_ffi::try_dispatch("ffi_ns_registry_env", &[])
        .unwrap()
        .unwrap();
    assert!(matches!(result, RValue::Environment(_)));
}

#[test]
fn ffi_env_poke_parent_changes_parent() {
    let parent1 = r::interpreter::environment::Environment::new_global();
    let parent2 = r::interpreter::environment::Environment::new_global();
    let child = r::interpreter::environment::Environment::new_child(&parent1);

    let args = vec![
        RValue::Environment(child.clone()),
        RValue::Environment(parent2.clone()),
    ];
    let result = rlang_ffi::try_dispatch("ffi_env_poke_parent", &args).unwrap();
    assert_is_null(&result);

    // Verify parent was changed
    let new_parent = child.parent().unwrap();
    assert!(new_parent.ptr_eq(&parent2));
}

// endregion

// region: Promise stubs

#[test]
fn promise_functions_return_null() {
    for name in &["ffi_promise_expr", "ffi_promise_value", "ffi_promise_env"] {
        let result = rlang_ffi::try_dispatch(name, &[r_null()]);
        assert!(result.is_some(), "{name} should be intercepted");
        assert_is_null(&result.unwrap());
    }
}

// endregion

// region: Dispatch fallthrough

#[test]
fn unknown_ffi_returns_none() {
    let result = rlang_ffi::try_dispatch("ffi_unknown_function", &[]);
    assert!(
        result.is_none(),
        "unknown FFI should fall through to C code"
    );
}

// endregion
