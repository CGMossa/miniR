use r::Session;

// region: Formula interface

#[test]
fn plot_formula_with_data_frame() {
    let mut s = Session::new();
    // plot(y ~ x, data=df) should not error — it extracts columns from the data frame
    s.eval_source(
        r#"
        df <- data.frame(x = 1:5, y = c(2, 4, 6, 8, 10))
        plot(y ~ x, data = df)
        "#,
    )
    .expect("plot(y ~ x, data=df) should succeed");
}

#[test]
fn plot_formula_without_data_looks_up_env() {
    let mut s = Session::new();
    // When no data arg, plot(y ~ x) should look up x and y in the environment
    s.eval_source(
        r#"
        x <- c(1, 2, 3, 4, 5)
        y <- c(10, 20, 30, 40, 50)
        plot(y ~ x)
        "#,
    )
    .expect("plot(y ~ x) with env variables should succeed");
}

#[test]
fn plot_formula_missing_column_in_data() {
    let mut s = Session::new();
    let result = s.eval_source(
        r#"
        df <- data.frame(x = 1:3, z = 4:6)
        plot(y ~ x, data = df)
        "#,
    );
    assert!(result.is_err(), "should error when column 'y' not in data");
    let err = format!("{}", result.unwrap_err());
    assert!(
        err.contains("not found in data"),
        "error should mention column not found, got: {}",
        err
    );
}

#[test]
fn plot_formula_missing_var_in_env() {
    let mut s = Session::new();
    let result = s.eval_source(
        r#"
        x <- 1:5
        plot(y ~ x)
        "#,
    );
    assert!(result.is_err(), "should error when 'y' not in environment");
    let err = format!("{}", result.unwrap_err());
    assert!(
        err.contains("not found"),
        "error should mention object not found, got: {}",
        err
    );
}

#[test]
fn plot_formula_data_not_data_frame() {
    let mut s = Session::new();
    let result = s.eval_source(
        r#"
        plot(y ~ x, data = 42)
        "#,
    );
    assert!(
        result.is_err(),
        "should error when data is not a data frame"
    );
    let err = format!("{}", result.unwrap_err());
    assert!(
        err.contains("data frame") || err.contains("list"),
        "error should mention data frame, got: {}",
        err
    );
}

// endregion

// region: Log-scale axes

#[test]
fn plot_log_x() {
    let mut s = Session::new();
    // log="x" should not error for positive data
    s.eval_source(
        r#"
        plot(c(1, 10, 100), c(1, 2, 3), log = "x")
        "#,
    )
    .expect("plot with log='x' should succeed");
}

#[test]
fn plot_log_y() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        plot(1:3, c(1, 10, 100), log = "y")
        "#,
    )
    .expect("plot with log='y' should succeed");
}

#[test]
fn plot_log_xy() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        plot(c(1, 10, 100), c(1, 10, 100), log = "xy")
        "#,
    )
    .expect("plot with log='xy' should succeed");
}

#[test]
fn plot_log_yx() {
    let mut s = Session::new();
    // "yx" should be equivalent to "xy"
    s.eval_source(
        r#"
        plot(c(1, 10), c(1, 10), log = "yx")
        "#,
    )
    .expect("plot with log='yx' should succeed");
}

#[test]
fn plot_log_invalid_spec() {
    let mut s = Session::new();
    let result = s.eval_source(
        r#"
        plot(1:3, 1:3, log = "z")
        "#,
    );
    assert!(
        result.is_err(),
        "log='z' should error — only 'x' and 'y' are valid"
    );
    let err = format!("{}", result.unwrap_err());
    assert!(
        err.contains("invalid 'log'"),
        "error should mention invalid log, got: {}",
        err
    );
}

#[test]
fn plot_log_empty_string_is_default() {
    let mut s = Session::new();
    // log="" is the default — should work fine
    s.eval_source(
        r#"
        plot(1:3, 1:3, log = "")
        "#,
    )
    .expect("plot with log='' should succeed (no transform)");
}

// endregion

// region: Combined features

#[test]
fn plot_formula_with_log() {
    let mut s = Session::new();
    // Both formula and log together
    s.eval_source(
        r#"
        df <- data.frame(x = c(1, 10, 100), y = c(2, 20, 200))
        plot(y ~ x, data = df, log = "xy")
        "#,
    )
    .expect("plot(y ~ x, data=df, log='xy') should succeed");
}

#[test]
fn plot_standard_call_still_works() {
    let mut s = Session::new();
    // Basic plot(x, y) should still work
    s.eval_source(
        r#"
        plot(1:10, 1:10)
        "#,
    )
    .expect("standard plot(x, y) should still work");
}

#[test]
fn plot_no_args_still_works() {
    let mut s = Session::new();
    // plot() with no args should not crash
    s.eval_source("plot()")
        .expect("plot() with no args should not crash");
}

// endregion
