//! Tests for plot() formula interface and log-scale axes.

use r::Session;

#[test]
fn plot_formula_with_data_frame() {
    let mut s = Session::new();
    s.eval_source(
        r#"
df <- data.frame(height = c(1.0, 2.0, 3.0), weight = c(10.0, 20.0, 30.0))
plot(weight ~ height, data = df)
"#,
    )
    .expect("plot formula with data frame should not error");
}

#[test]
fn plot_formula_without_data_uses_environment() {
    let mut s = Session::new();
    s.eval_source(
        r#"
xx <- c(1.0, 2.0, 3.0)
yy <- c(10.0, 20.0, 30.0)
plot(yy ~ xx)
"#,
    )
    .expect("plot formula with env lookup should not error");
}

#[test]
fn plot_formula_missing_column_error() {
    let mut s = Session::new();
    let result = s.eval_source(
        r#"
df <- data.frame(height = c(1.0, 2.0, 3.0))
plot(weight ~ height, data = df)
"#,
    );
    assert!(
        result.is_err(),
        "plot formula with missing column should error"
    );
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("weight") && err.contains("not found"),
        "error should mention missing variable: {err}"
    );
}

#[test]
fn plot_formula_missing_variable_error() {
    let mut s = Session::new();
    let result = s.eval_source("plot(nonexistent_y ~ nonexistent_x)");
    assert!(
        result.is_err(),
        "plot formula with missing env var should error"
    );
}

#[test]
fn plot_log_x() {
    let mut s = Session::new();
    s.eval_source("plot(c(1, 10, 100), c(1, 2, 3), log = 'x')")
        .expect("plot with log='x' should not error");
}

#[test]
fn plot_log_y() {
    let mut s = Session::new();
    s.eval_source("plot(1:3, c(1, 10, 100), log = 'y')")
        .expect("plot with log='y' should not error");
}

#[test]
fn plot_log_xy() {
    let mut s = Session::new();
    s.eval_source("plot(c(1, 10, 100), c(1, 10, 100), log = 'xy')")
        .expect("plot with log='xy' should not error");
}

#[test]
fn plot_log_invalid_spec_errors() {
    let mut s = Session::new();
    let result = s.eval_source("plot(1:3, 1:3, log = 'z')");
    assert!(result.is_err(), "invalid log spec should error");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("invalid"),
        "error should mention invalid: {err}"
    );
}

#[test]
fn plot_standard_still_works() {
    let mut s = Session::new();
    s.eval_source("plot(1:10)")
        .expect("standard plot(x) should still work");
}

#[test]
fn plot_xy_still_works() {
    let mut s = Session::new();
    s.eval_source("plot(1:5, c(2, 4, 6, 8, 10))")
        .expect("standard plot(x, y) should still work");
}
