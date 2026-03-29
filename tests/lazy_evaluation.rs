//! Tests for lazy argument evaluation (R promise semantics).
//!
//! R uses call-by-need: function arguments are not evaluated until their value
//! is actually needed. This enables patterns like:
//! - `f(x) { 42 }; f(stop("boom"))` -- never errors because `x` is unused
//! - `substitute(x)` -- returns the unevaluated expression, not the value
//! - Default argument values that reference other parameters

use r::session::Session;

// region: Basic lazy evaluation

#[test]
fn unused_argument_not_evaluated() {
    let mut s = Session::new();
    let result = s
        .eval_source(
            r#"
        f <- function(x) 42
        f(stop("should not error"))
    "#,
        )
        .unwrap();
    assert_eq!(
        result.value.as_vector().unwrap().as_double_scalar(),
        Some(42.0)
    );
}

#[test]
fn used_argument_evaluated() {
    let mut s = Session::new();
    let result = s.eval_source(
        r#"
        f <- function(x) x + 1
        f(41)
    "#,
    );
    let val = result.unwrap().value;
    assert_eq!(val.as_vector().unwrap().as_double_scalar(), Some(42.0));
}

#[test]
fn argument_evaluated_only_once() {
    // Side effects should happen exactly once when the promise is forced
    let mut s = Session::new();
    let result = s
        .eval_source(
            r#"
        counter <- 0
        f <- function(x) { x; x; x }
        inc <- function() { counter <<- counter + 1; counter }
        f(inc())
        counter
    "#,
        )
        .unwrap();
    assert_eq!(
        result.value.as_vector().unwrap().as_double_scalar(),
        Some(1.0)
    );
}

#[test]
fn lazy_error_only_on_access() {
    // Error in argument should only propagate when accessed
    let mut s = Session::new();
    // This should succeed because x is never used
    let result = s.eval_source(
        r#"
        f <- function(x, y) y
        f(stop("boom"), 99)
    "#,
    );
    let val = result.unwrap().value;
    assert_eq!(val.as_vector().unwrap().as_double_scalar(), Some(99.0));

    // This should error because x IS used
    let result = s.eval_source(
        r#"
        g <- function(x, y) x
        g(stop("boom"), 99)
    "#,
    );
    assert!(result.is_err());
}

// endregion

// region: substitute() with promises

#[test]
fn substitute_returns_unevaluated_expression() {
    let mut s = Session::new();
    let result = s
        .eval_source(
            r#"
        f <- function(x) substitute(x)
        result <- f(a + b)
        identical(result, quote(a + b))
    "#,
        )
        .unwrap();
    assert_eq!(
        result.value.as_vector().unwrap().as_logical_scalar(),
        Some(true)
    );
}

#[test]
fn substitute_complex_expression() {
    let mut s = Session::new();
    let result = s
        .eval_source(
            r#"
        f <- function(x) substitute(x)
        result <- f(foo(1, bar = 2 + 3))
        deparse(result)
    "#,
        )
        .unwrap();
    let deparsed = result
        .value
        .as_vector()
        .unwrap()
        .as_character_scalar()
        .unwrap();
    assert_eq!(deparsed, "foo(1, bar = 2 + 3)");
}

#[test]
fn substitute_named_argument() {
    let mut s = Session::new();
    let result = s
        .eval_source(
            r#"
        f <- function(x) substitute(x)
        result <- f(x = a * b)
        identical(result, quote(a * b))
    "#,
        )
        .unwrap();
    assert_eq!(
        result.value.as_vector().unwrap().as_logical_scalar(),
        Some(true)
    );
}

// endregion

// region: Default argument values (lazy)

#[test]
fn default_arguments_are_lazy() {
    let mut s = Session::new();
    // Default value is lazy -- not evaluated unless used
    let result = s
        .eval_source(
            r#"
        f <- function(x = stop("default error")) 42
        f()  # should NOT error -- x is never used
    "#,
        )
        .unwrap();
    assert_eq!(
        result.value.as_vector().unwrap().as_double_scalar(),
        Some(42.0)
    );

    // But when the default IS used, it should be forced
    let result2 = s.eval_source(
        r#"
        g <- function(x = stop("default error")) x
        g()  # should error -- x forces the default
    "#,
    );
    assert!(result2.is_err());
}

#[test]
fn default_can_reference_other_params() {
    let mut s = Session::new();
    let result = s
        .eval_source(
            r#"
        f <- function(x, y = x + 1) y
        f(10)
    "#,
        )
        .unwrap();
    assert_eq!(
        result.value.as_vector().unwrap().as_double_scalar(),
        Some(11.0)
    );
}

#[test]
fn default_evaluated_in_call_env() {
    // Default values are evaluated in the call environment, so they can see
    // the values of other parameters that have been bound.
    let mut s = Session::new();
    let result = s
        .eval_source(
            r#"
        f <- function(n, x = seq_len(n)) x
        f(3)
    "#,
        )
        .unwrap();
    let vec = result.value.as_vector().unwrap();
    assert_eq!(vec.len(), 3);
}

// endregion

// region: Dots forwarding with promises

#[test]
fn dots_forward_promises() {
    // Promises should be forwarded through ... without being forced
    let mut s = Session::new();
    let result = s
        .eval_source(
            r#"
        inner <- function(x) x
        outer <- function(...) inner(...)
        outer(42)
    "#,
        )
        .unwrap();
    assert_eq!(
        result.value.as_vector().unwrap().as_double_scalar(),
        Some(42.0)
    );
}

#[test]
fn dots_unused_not_forced() {
    let mut s = Session::new();
    let result = s
        .eval_source(
            r#"
        sink <- function(...) 99
        sink(stop("nope"))
    "#,
        )
        .unwrap();
    assert_eq!(
        result.value.as_vector().unwrap().as_double_scalar(),
        Some(99.0)
    );
}

// endregion

// region: Recursive promise detection

#[test]
fn recursive_promise_detected() {
    // `f(x = x)` where default of x references x should error
    let mut s = Session::new();
    let result = s.eval_source(
        r#"
        f <- function(x = x) x
        f()
    "#,
    );
    assert!(result.is_err());
}

// endregion

// region: S3 dispatch with promises

#[test]
fn s3_dispatch_forces_first_arg() {
    let mut s = Session::new();
    let result = s
        .eval_source(
            r#"
        describe <- function(x, ...) UseMethod("describe")
        describe.widget <- function(x, ...) paste0("widget:", x$name)
        describe.default <- function(x, ...) "unknown"
        make_widget <- function(name) {
            w <- list(name = name)
            class(w) <- "widget"
            w
        }
        describe(make_widget("gear"))
    "#,
        )
        .unwrap();
    let desc = result
        .value
        .as_vector()
        .unwrap()
        .as_character_scalar()
        .unwrap();
    assert_eq!(desc, "widget:gear");
}

// endregion
