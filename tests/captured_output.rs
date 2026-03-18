//! Tests for per-interpreter captured output (Session::new_with_captured_output).
//!
//! Verifies that print(), cat(), message(), warning(), and str() write to the
//! interpreter's per-session writers instead of process-global stdio.

use r::Session;

#[test]
fn cat_captured() {
    let mut s = Session::new_with_captured_output();
    s.eval_source("cat('hello')").unwrap();
    assert_eq!(s.captured_stdout(), "hello");
}

#[test]
fn cat_with_newline() {
    let mut s = Session::new_with_captured_output();
    s.eval_source("cat('hello\\n')").unwrap();
    assert_eq!(s.captured_stdout(), "hello\n");
}

#[test]
fn cat_sep() {
    let mut s = Session::new_with_captured_output();
    s.eval_source("cat(1, 2, 3, sep = ', ')").unwrap();
    assert_eq!(s.captured_stdout(), "1, 2, 3");
}

#[test]
fn print_captured() {
    let mut s = Session::new_with_captured_output();
    s.eval_source("print(42)").unwrap();
    assert_eq!(s.captured_stdout(), "[1] 42\n");
}

#[test]
fn message_captured_to_stderr() {
    let mut s = Session::new_with_captured_output();
    s.eval_source("message('oops')").unwrap();
    assert_eq!(s.captured_stderr(), "oops\n");
    // stdout should be empty
    assert_eq!(s.captured_stdout(), "");
}

#[test]
fn warning_captured_to_stderr() {
    let mut s = Session::new_with_captured_output();
    s.eval_source("warning('careful')").unwrap();
    assert_eq!(s.captured_stderr(), "Warning message:\ncareful\n");
    assert_eq!(s.captured_stdout(), "");
}

#[test]
fn str_captured() {
    let mut s = Session::new_with_captured_output();
    s.eval_source("str(1:3)").unwrap();
    let out = s.captured_stdout();
    assert!(
        out.contains("int"),
        "str() output should contain type info: {out}"
    );
    assert!(
        out.contains("[1:3]"),
        "str() output should contain length: {out}"
    );
}

#[test]
fn multiple_outputs_accumulate() {
    let mut s = Session::new_with_captured_output();
    s.eval_source("cat('a')").unwrap();
    s.eval_source("cat('b')").unwrap();
    s.eval_source("cat('c')").unwrap();
    assert_eq!(s.captured_stdout(), "abc");
}

#[test]
fn isolated_sessions_dont_interfere() {
    let mut s1 = Session::new_with_captured_output();
    let mut s2 = Session::new_with_captured_output();
    s1.eval_source("cat('session1')").unwrap();
    s2.eval_source("cat('session2')").unwrap();
    assert_eq!(s1.captured_stdout(), "session1");
    assert_eq!(s2.captured_stdout(), "session2");
}

#[test]
fn non_captured_session_returns_empty() {
    let s = Session::new();
    assert_eq!(s.captured_stdout(), "");
    assert_eq!(s.captured_stderr(), "");
}
