//! Tests for plot() formula interface and log-scale axes.

use r::Session;

// region: plot.formula

#[test]
fn plot_formula_with_data_frame() {
    let mut s = Session::new();
    s.eval_source(
        r#"
df <- data.frame(height = c(1.0, 2.0, 3.0), weight = c(10.0, 20.0, 30.0))
result <- plot(weight ~ height, data = df)

# result is an invisible list with x, y, xlab, ylab, log, main
stopifnot(is.list(result))
stopifnot(identical(result$xlab, "height"))
stopifnot(identical(result$ylab, "weight"))
stopifnot(identical(result$x, c(1.0, 2.0, 3.0)))
stopifnot(identical(result$y, c(10.0, 20.0, 30.0)))
stopifnot(identical(result$log, ""))
"#,
    )
    .expect("plot formula with data frame failed");
}

#[test]
fn plot_formula_without_data_uses_environment() {
    let mut s = Session::new();
    s.eval_source(
        r#"
xx <- c(1.0, 2.0, 3.0)
yy <- c(4.0, 5.0, 6.0)
result <- plot(yy ~ xx)

stopifnot(is.list(result))
stopifnot(identical(result$xlab, "xx"))
stopifnot(identical(result$ylab, "yy"))
stopifnot(identical(result$x, c(1.0, 2.0, 3.0)))
stopifnot(identical(result$y, c(4.0, 5.0, 6.0)))
"#,
    )
    .expect("plot formula without data failed");
}

#[test]
fn plot_formula_missing_column_error() {
    let mut s = Session::new();
    let result = s.eval_source(
        r#"
df <- data.frame(a = c(1, 2), b = c(3, 4))
plot(z ~ a, data = df)
"#,
    );
    assert!(result.is_err(), "should error on missing column 'z'");
    let err_msg = format!("{:?}", result.unwrap_err());
    assert!(
        err_msg.contains("z") && err_msg.contains("not found"),
        "error should mention 'z' not found, got: {}",
        err_msg
    );
}

#[test]
fn plot_formula_missing_variable_error() {
    let mut s = Session::new();
    let result = s.eval_source(
        r#"
xx <- c(1, 2, 3)
plot(nonexistent ~ xx)
"#,
    );
    assert!(
        result.is_err(),
        "should error on missing variable 'nonexistent'"
    );
}

#[test]
fn plot_formula_custom_labels() {
    let mut s = Session::new();
    s.eval_source(
        r#"
df <- data.frame(x = c(1.0, 2.0), y = c(3.0, 4.0))
result <- plot(y ~ x, data = df, xlab = "Custom X", ylab = "Custom Y", main = "My Plot")

stopifnot(identical(result$xlab, "Custom X"))
stopifnot(identical(result$ylab, "Custom Y"))
stopifnot(identical(result$main, "My Plot"))
"#,
    )
    .expect("plot formula with custom labels failed");
}

// endregion

// region: plot(x, y) standard interface

#[test]
fn plot_xy_vectors() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- plot(c(1.0, 2.0, 3.0), c(4.0, 5.0, 6.0))
stopifnot(is.list(result))
stopifnot(identical(result$x, c(1.0, 2.0, 3.0)))
stopifnot(identical(result$y, c(4.0, 5.0, 6.0)))
stopifnot(identical(result$xlab, "x"))
stopifnot(identical(result$ylab, "y"))
"#,
    )
    .expect("plot(x, y) failed");
}

#[test]
fn plot_single_vector_uses_index() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- plot(c(10.0, 20.0, 30.0))
stopifnot(is.list(result))
# When only x is given, R uses 1:n as x and the input as y
stopifnot(identical(result$x, c(1.0, 2.0, 3.0)))
stopifnot(identical(result$y, c(10.0, 20.0, 30.0)))
stopifnot(identical(result$xlab, "Index"))
stopifnot(identical(result$ylab, "x"))
"#,
    )
    .expect("plot(x) single vector failed");
}

// endregion

// region: log-scale axes

#[test]
fn plot_log_x() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- c(1.0, exp(1), exp(2))
y <- c(10.0, 20.0, 30.0)
result <- plot(x, y, log = "x")

stopifnot(identical(result$log, "x"))
# x values should be log-transformed: log(1)=0, log(e)=1, log(e^2)=2
stopifnot(abs(result$x[1] - 0.0) < 1e-10)
stopifnot(abs(result$x[2] - 1.0) < 1e-10)
stopifnot(abs(result$x[3] - 2.0) < 1e-10)
# y values should be unchanged
stopifnot(identical(result$y, c(10.0, 20.0, 30.0)))
"#,
    )
    .expect("plot with log='x' failed");
}

#[test]
fn plot_log_y() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- c(1.0, 2.0, 3.0)
y <- c(1.0, exp(1), exp(2))
result <- plot(x, y, log = "y")

stopifnot(identical(result$log, "y"))
# x unchanged
stopifnot(identical(result$x, c(1.0, 2.0, 3.0)))
# y log-transformed
stopifnot(abs(result$y[1] - 0.0) < 1e-10)
stopifnot(abs(result$y[2] - 1.0) < 1e-10)
stopifnot(abs(result$y[3] - 2.0) < 1e-10)
"#,
    )
    .expect("plot with log='y' failed");
}

#[test]
fn plot_log_xy() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- c(exp(1), exp(2))
y <- c(exp(3), exp(4))
result <- plot(x, y, log = "xy")

stopifnot(identical(result$log, "xy"))
stopifnot(abs(result$x[1] - 1.0) < 1e-10)
stopifnot(abs(result$x[2] - 2.0) < 1e-10)
stopifnot(abs(result$y[1] - 3.0) < 1e-10)
stopifnot(abs(result$y[2] - 4.0) < 1e-10)
"#,
    )
    .expect("plot with log='xy' failed");
}

#[test]
fn plot_log_invalid_spec_errors() {
    let mut s = Session::new();
    let result = s.eval_source(
        r#"
plot(c(1, 2), c(3, 4), log = "z")
"#,
    );
    assert!(result.is_err(), "invalid log spec should produce an error");
    let err_msg = format!("{:?}", result.unwrap_err());
    assert!(
        err_msg.contains("invalid") && err_msg.contains("log"),
        "error should mention invalid log argument, got: {}",
        err_msg
    );
}

#[test]
fn plot_log_non_positive_produces_na() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- c(-1.0, 0.0, 1.0, exp(1))
y <- c(1.0, 2.0, 3.0, 4.0)
result <- plot(x, y, log = "x")

# Non-positive values become NA after log transform
stopifnot(is.na(result$x[1]))
stopifnot(is.na(result$x[2]))
stopifnot(abs(result$x[3] - 0.0) < 1e-10)
stopifnot(abs(result$x[4] - 1.0) < 1e-10)
"#,
    )
    .expect("plot log with non-positive values failed");
}

// endregion

// region: formula + log combined

#[test]
fn plot_formula_with_log() {
    let mut s = Session::new();
    s.eval_source(
        r#"
df <- data.frame(dose = c(1.0, exp(1), exp(2)), response = c(10.0, 20.0, 30.0))
result <- plot(response ~ dose, data = df, log = "x")

stopifnot(identical(result$xlab, "dose"))
stopifnot(identical(result$ylab, "response"))
stopifnot(identical(result$log, "x"))
# dose (x) is log-transformed
stopifnot(abs(result$x[1] - 0.0) < 1e-10)
stopifnot(abs(result$x[2] - 1.0) < 1e-10)
stopifnot(abs(result$x[3] - 2.0) < 1e-10)
# response (y) unchanged
stopifnot(identical(result$y, c(10.0, 20.0, 30.0)))
"#,
    )
    .expect("plot formula with log failed");
}

// endregion

// region: plot returns invisibly

#[test]
fn plot_returns_invisibly() {
    let mut s = Session::new();
    let result = s
        .eval_source(
            r#"
plot(c(1.0, 2.0), c(3.0, 4.0))
"#,
        )
        .expect("plot should succeed");
    assert!(!result.visible, "plot() should return its value invisibly");
}

// endregion
