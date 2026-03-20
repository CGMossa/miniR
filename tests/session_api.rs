use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use r::{Session, SessionError};

fn temp_path(name: &str, extension: &str) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock drift")
        .as_nanos();
    std::env::temp_dir().join(format!("minir-{name}-{suffix}.{extension}"))
}

#[test]
fn sessions_keep_state_isolated_without_using_the_cli() {
    let mut first = Session::new();
    let mut second = Session::new();

    first
        .eval_source("x <- 1L")
        .expect("failed to seed first session");
    second
        .eval_source("x <- 2L")
        .expect("failed to seed second session");

    let first_x = first
        .eval_source("x")
        .expect("failed to read first x")
        .value;
    let second_x = second
        .eval_source("x")
        .expect("failed to read second x")
        .value;
    let missing_in_third = Session::new()
        .eval_source("exists(\"x\")")
        .expect("failed to query missing symbol")
        .value;

    assert_eq!(
        first_x.as_vector().and_then(|v| v.as_integer_scalar()),
        Some(1)
    );
    assert_eq!(
        second_x.as_vector().and_then(|v| v.as_integer_scalar()),
        Some(2)
    );
    assert_eq!(
        missing_in_third
            .as_vector()
            .and_then(|v| v.as_logical_scalar()),
        Some(false)
    );
}

#[test]
fn eval_file_reports_parse_errors_with_filenames() {
    let path = temp_path("session-parse-error", "R");
    fs::write(&path, "if TRUE 1\n").expect("failed to write test file");

    let err = Session::new()
        .eval_file(&path)
        .expect_err("eval_file unexpectedly succeeded");

    match err {
        SessionError::Parse(parse_error) => {
            assert_eq!(
                parse_error.filename.as_deref(),
                Some(path.to_string_lossy().as_ref())
            );
        }
        other => panic!("unexpected error: {other}"),
    }

    let _ = fs::remove_file(path);
}

#[test]
fn sync_terminal_width_sets_option() {
    let mut session = Session::new();
    session.sync_terminal_width();

    // After sync_terminal_width, getOption("width") should return a positive integer.
    let result = session
        .eval_source("getOption('width')")
        .expect("getOption('width') failed");
    let width = result
        .value
        .as_vector()
        .and_then(|v| v.as_integer_scalar())
        .expect("width should be an integer");
    assert!(
        (10..=10000).contains(&width),
        "terminal width {width} should be in [10, 10000]"
    );
}

#[test]
fn manual_width_option_overrides_sync() {
    let mut session = Session::new();
    session.sync_terminal_width();

    // Manually set width to 42
    session
        .eval_source("options(width = 42L)")
        .expect("options(width=42) failed");
    let result = session
        .eval_source("getOption('width')")
        .expect("getOption('width') failed");
    let width = result
        .value
        .as_vector()
        .and_then(|v| v.as_integer_scalar())
        .expect("width should be an integer");
    assert_eq!(width, 42, "manual width should override synced value");
}
