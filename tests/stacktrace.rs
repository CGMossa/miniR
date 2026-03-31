//! Tests for the stacktrace system (Layer 1: R-level traceback).

use std::io::Write;

use r::Session;

/// Nested function calls produce a traceback on error.
#[test]
fn traceback_nested_calls() {
    let mut session = Session::new();
    let result = session.eval_source(
        r#"
        f <- function() stop("boom")
        g <- function() f()
        h <- function() g()
        h()
        "#,
    );
    assert!(result.is_err());

    let tb = session.interpreter().format_traceback();
    assert!(tb.is_some(), "traceback should be captured on error");
    let tb = tb.unwrap();

    // Should show the call chain: h() -> g() -> f() -> stop("boom")
    assert!(tb.contains("f()"), "traceback should contain f(): {tb}");
    assert!(tb.contains("g()"), "traceback should contain g(): {tb}");
    assert!(tb.contains("h()"), "traceback should contain h(): {tb}");
}

/// traceback() builtin returns a character vector with the call stack.
#[test]
fn traceback_builtin_returns_calls() {
    let mut session = Session::new_with_captured_output();

    // First trigger an error
    let _ = session.eval_source(
        r#"
        inner <- function() stop("test error")
        outer <- function() inner()
        outer()
        "#,
    );

    // Now call traceback()
    let result = session.eval_source("traceback()");
    assert!(result.is_ok());

    let stdout = session.captured_stdout();
    assert!(
        stdout.contains("inner()"),
        "traceback() output should contain inner(): {stdout}"
    );
    assert!(
        stdout.contains("outer()"),
        "traceback() output should contain outer(): {stdout}"
    );
}

/// Traceback persists across evals (like R's traceback()) until a new error replaces it.
#[test]
fn traceback_persists_across_evals() {
    let mut session = Session::new();

    // Trigger an error
    let _ = session.eval_source("f <- function() stop('x'); f()");
    assert!(session.interpreter().format_traceback().is_some());

    // Successful eval should NOT clear the traceback (matches R behavior)
    let _ = session.eval_source("1 + 1");
    assert!(
        session.interpreter().format_traceback().is_some(),
        "traceback should persist after successful eval (like R)"
    );

    // A new error replaces the old traceback
    let _ = session.eval_source("g <- function() stop('y'); g()");
    let tb = session.interpreter().format_traceback().unwrap();
    assert!(
        tb.contains("g()"),
        "new error should replace old traceback: {tb}"
    );
}

/// stopifnot() failure produces a traceback.
#[test]
fn traceback_stopifnot() {
    let mut session = Session::new();
    let result = session.eval_source(
        r#"
        check <- function(x) stopifnot(x > 0)
        validate <- function(x) check(x)
        validate(-1)
        "#,
    );
    assert!(result.is_err());

    let tb = session.interpreter().format_traceback();
    assert!(tb.is_some());
    let tb = tb.unwrap();
    assert!(
        tb.contains("check"),
        "traceback should contain check(): {tb}"
    );
    assert!(
        tb.contains("validate"),
        "traceback should contain validate(): {tb}"
    );
}

/// Error with no function calls produces no traceback (top-level error).
#[test]
fn no_traceback_for_toplevel_error() {
    let mut session = Session::new();
    let _ = session.eval_source("stop('top level')");

    // Top-level stop() has no call frames (stop is a builtin, not a closure)
    let tb = session.interpreter().format_traceback();
    assert!(
        tb.is_none(),
        "top-level error should have no traceback (no closure frames)"
    );
}

/// source()'d file errors show file:line in traceback.
#[test]
fn traceback_source_file_line() {
    let mut session = Session::new();

    // Create a temp file with R code that errors on a known line
    let dir = std::env::temp_dir().join("minir_test_stacktrace");
    std::fs::create_dir_all(&dir).unwrap();
    let script = dir.join("test_trace.R");
    let mut f = std::fs::File::create(&script).unwrap();
    writeln!(f, "inner <- function() stop('boom')").unwrap();
    writeln!(f, "outer <- function() inner()").unwrap();
    writeln!(f, "outer()").unwrap();
    drop(f);

    let result = session.eval_file(&script);
    assert!(result.is_err());

    let tb = session.interpreter().format_traceback();
    assert!(tb.is_some(), "traceback should exist");
    let tb = tb.unwrap();

    // Should contain file:line info
    assert!(
        tb.contains("test_trace.R:"),
        "traceback should contain filename:line: {tb}"
    );

    // Clean up
    std::fs::remove_file(&script).ok();
    std::fs::remove_dir(&dir).ok();
}
