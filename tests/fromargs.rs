//! Tests for the FromArgs derive macro and trait system.
//!
//! Tests the new argument types: Option<T>, RArg<T>, Dots, #[name = "..."].

use r::interpreter::environment::Environment;
use r::interpreter::value::vector::Vector;
use r::interpreter::value::{coerce_arg_three_way, CoerceArg, Dots, RArg, RValue};
use r::session::Session;

// region: CoerceArg impls

#[test]
fn coerce_option_none_for_null() {
    // Option<f64> coerces NULL to None
    let result = Option::<f64>::coerce(&RValue::Null, "x");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), None);
}

#[test]
fn coerce_option_some_for_value() {
    let val = RValue::vec(Vector::Double(vec![Some(42.0)].into()));
    let result = Option::<f64>::coerce(&val, "x");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Some(42.0));
}

#[test]
fn coerce_usize_positive() {
    let val = RValue::vec(Vector::Integer(vec![Some(5)].into()));
    let result = usize::coerce(&val, "n");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 5);
}

#[test]
fn coerce_usize_negative_fails() {
    let val = RValue::vec(Vector::Integer(vec![Some(-1)].into()));
    let result = usize::coerce(&val, "n");
    assert!(result.is_err());
}

#[test]
fn coerce_vector() {
    let val = RValue::vec(Vector::Double(vec![Some(1.0), Some(2.0), Some(3.0)].into()));
    let result = Vector::coerce(&val, "x");
    assert!(result.is_ok());
    match result.unwrap() {
        Vector::Double(d) => assert_eq!(d.len(), 3),
        _ => panic!("expected Double vector"),
    }
}

#[test]
fn coerce_vector_null_fails() {
    let result = Vector::coerce(&RValue::Null, "x");
    assert!(result.is_err());
}

#[test]
fn coerce_environment() {
    let env = Environment::new_global();
    let val = RValue::Environment(env);
    let result = Environment::coerce(&val, "envir");
    assert!(result.is_ok());
}

#[test]
fn coerce_environment_non_env_fails() {
    let val = RValue::vec(Vector::Double(vec![Some(1.0)].into()));
    let result = Environment::coerce(&val, "envir");
    assert!(result.is_err());
}

// endregion

// region: RArg<T> three-way state

#[test]
fn rarg_missing() {
    let result: RArg<f64> = coerce_arg_three_way(None, "x").unwrap();
    assert!(result.is_missing());
    assert!(!result.is_null());
    assert_eq!(result.value(), None);
}

#[test]
fn rarg_null() {
    let result: RArg<f64> = coerce_arg_three_way(Some(&RValue::Null), "x").unwrap();
    assert!(!result.is_missing());
    assert!(result.is_null());
    assert_eq!(result.value(), None);
}

#[test]
fn rarg_value() {
    let val = RValue::vec(Vector::Double(vec![Some(42.0)].into()));
    let result: RArg<f64> = coerce_arg_three_way(Some(&val), "x").unwrap();
    assert!(!result.is_missing());
    assert!(!result.is_null());
    assert_eq!(result.value(), Some(&42.0));
}

#[test]
fn rarg_unwrap_or() {
    assert_eq!(RArg::<f64>::Missing.unwrap_or(0.0), 0.0);
    assert_eq!(RArg::<f64>::Null.unwrap_or(0.0), 0.0);
    assert_eq!(RArg::Value(42.0).unwrap_or(0.0), 42.0);
}

#[test]
fn rarg_optional() {
    assert_eq!(RArg::<f64>::Missing.optional(), None);
    assert_eq!(RArg::<f64>::Null.optional(), None);
    assert_eq!(RArg::Value(42.0).optional(), Some(42.0));
}

#[test]
fn rarg_or_default() {
    // Missing → Value(default)
    match RArg::<f64>::Missing.or_default(1.0) {
        RArg::Value(v) => assert_eq!(v, 1.0),
        _ => panic!("expected Value after or_default on Missing"),
    }
    // Null stays Null
    assert!(RArg::<f64>::Null.or_default(1.0).is_null());
    // Value stays Value
    assert_eq!(RArg::Value(42.0).or_default(1.0).value(), Some(&42.0));
}

#[test]
fn rarg_map() {
    let mapped = RArg::Value(2.0).map(|x| x * 3.0);
    assert_eq!(mapped.value(), Some(&6.0));
    assert!(RArg::<f64>::Missing.map(|x| x * 3.0).is_missing());
    assert!(RArg::<f64>::Null.map(|x| x * 3.0).is_null());
}

// endregion

// region: Dots

#[test]
fn dots_empty() {
    let dots = Dots::default();
    assert!(dots.is_empty());
    assert_eq!(dots.len(), 0);
}

#[test]
fn dots_iterate() {
    let dots = Dots(vec![
        RValue::vec(Vector::Double(vec![Some(1.0)].into())),
        RValue::vec(Vector::Double(vec![Some(2.0)].into())),
    ]);
    assert_eq!(dots.len(), 2);

    let count = dots.iter().count();
    assert_eq!(count, 2);

    // IntoIterator
    let count = (&dots).into_iter().count();
    assert_eq!(count, 2);
}

// endregion

// region: End-to-end via R evaluation

#[test]
fn existing_builtins_still_work() {
    // Verify existing fn-macro builtins are unaffected by the trait changes
    let mut session = Session::new();

    // formatC — 5 params, named args, defaults
    let result = session
        .eval_source(r#"formatC(3.14159, width = 10, format = "f", digits = 2)"#)
        .expect("formatC")
        .value;
    let s = result.as_vector().unwrap().as_character_scalar().unwrap();
    assert!(s.contains("3.14"), "formatC result: {}", s);

    // rgb — named args with reordering
    let result = session.eval_source(r#"rgb(0, 0, 1)"#).expect("rgb").value;
    let s = result.as_vector().unwrap().as_character_scalar().unwrap();
    assert_eq!(s, "#0000FF");

    // grep — mixed positional and named
    let result = session
        .eval_source(r#"grep("b", c("abc", "def", "bcd"), value = TRUE)"#)
        .expect("grep")
        .value;
    let chars = result.as_vector().unwrap().to_characters();
    assert_eq!(
        chars,
        vec![Some("abc".to_string()), Some("bcd".to_string())]
    );
}

// endregion
