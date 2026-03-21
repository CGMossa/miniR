//! Tests for colored diagnostic output integration.
//!
//! These tests verify that errors, warnings, and messages produce correct
//! textual content through the session-scoped writers. Color itself is not
//! tested (it depends on terminal capabilities), but the content must be
//! correct regardless of whether color is enabled.

use r::Session;

#[test]
fn captured_session_disables_color() {
    let s = Session::new_with_captured_output();
    assert!(
        !s.interpreter().color_stderr(),
        "captured output sessions should have color_stderr = false"
    );
}

#[test]
fn warning_content_correct_in_captured_session() {
    let mut s = Session::new_with_captured_output();
    s.eval_source("warning('test warning')").unwrap();
    let stderr = s.captured_stderr();
    assert!(
        stderr.contains("Warning message:"),
        "warning output should contain 'Warning message:': {stderr}"
    );
    assert!(
        stderr.contains("test warning"),
        "warning output should contain the warning text: {stderr}"
    );
}

#[test]
fn message_content_correct_in_captured_session() {
    let mut s = Session::new_with_captured_output();
    s.eval_source("message('hello world')").unwrap();
    let stderr = s.captured_stderr();
    assert_eq!(stderr, "hello world\n");
}

#[test]
fn stop_error_content_correct() {
    let mut s = Session::new_with_captured_output();
    let result = s.eval_source("stop('boom')");
    assert!(result.is_err(), "stop() should produce an error");
    let err_msg = format!("{}", result.unwrap_err());
    assert!(
        err_msg.contains("boom"),
        "error message should contain 'boom': {err_msg}"
    );
}

#[test]
fn multiple_warnings_accumulate_in_captured_session() {
    let mut s = Session::new_with_captured_output();
    s.eval_source(
        r#"
        suppressWarnings(warning("ignored"))
        warning("visible1")
        warning("visible2")
        "#,
    )
    .unwrap();
    let stderr = s.captured_stderr();
    assert!(
        stderr.contains("visible1"),
        "stderr should contain first warning: {stderr}"
    );
    assert!(
        stderr.contains("visible2"),
        "stderr should contain second warning: {stderr}"
    );
    assert!(
        !stderr.contains("ignored"),
        "suppressed warning should not appear: {stderr}"
    );
}

#[test]
fn message_with_append_lf_false() {
    let mut s = Session::new_with_captured_output();
    s.eval_source("message('no newline', appendLF = FALSE)")
        .unwrap();
    let stderr = s.captured_stderr();
    assert_eq!(stderr, "no newline");
}
