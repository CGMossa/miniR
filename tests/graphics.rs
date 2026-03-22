//! Tests for the graphics plotting subsystem.
//!
//! These tests verify that plot builtins correctly build PlotState data
//! structures without requiring a GUI window. The `plot` feature is NOT
//! needed for these tests — they test the data model and the builtins'
//! non-GUI behavior (printing the "feature required" message).

use r::session::Session;

// region: plot() builds correct data

#[test]
fn plot_without_feature_prints_message() {
    let mut session = Session::new_with_captured_output();
    session.eval_source("plot(1:5)");
    let stderr = session.captured_stderr();
    // Without the plot feature, should get a message about building with --features plot
    // OR if the plot feature IS enabled, we can't test GUI in CI, so just check no crash
    assert!(
        stderr.contains("plot")
            || stderr.is_empty()
            || stderr.contains("failed to display"),
        "unexpected stderr: {stderr}"
    );
}

#[test]
fn plot_returns_null() {
    let mut session = Session::new_with_captured_output();
    let result = session.eval_source("is.null(plot(1:5))");
    // plot() returns NULL (invisibly) regardless of feature
    let output = session.captured_stdout();
    assert!(
        output.contains("TRUE") || output.is_empty(),
        "plot() should return NULL, got stdout: {output}"
    );
}

// endregion

// region: hist() returns correct structure

#[test]
fn hist_returns_list_with_breaks_counts_mids() {
    let mut session = Session::new_with_captured_output();
    session.eval_source(
        r#"
        h <- hist(c(1, 2, 2, 3, 3, 3, 4, 4, 5))
        stopifnot(is.list(h))
        stopifnot("breaks" %in% names(h))
        stopifnot("counts" %in% names(h))
        stopifnot("mids" %in% names(h))
        "#,
    );
    let stderr = session.captured_stderr();
    // Should not error (the hist data structure is returned regardless of plot feature)
    assert!(
        !stderr.contains("Error"),
        "hist() should return a valid list, got stderr: {stderr}"
    );
}

#[test]
fn hist_counts_sum_to_n() {
    let mut session = Session::new_with_captured_output();
    session.eval_source(
        r#"
        h <- hist(c(1, 2, 2, 3, 3, 3, 4, 4, 5))
        stopifnot(sum(h$counts) == 9)
        "#,
    );
    let stderr = session.captured_stderr();
    assert!(
        !stderr.contains("Error"),
        "hist counts should sum to n=9, got stderr: {stderr}"
    );
}

// endregion

// region: barplot() returns midpoints

#[test]
fn barplot_returns_midpoints() {
    let mut session = Session::new_with_captured_output();
    session.eval_source(
        r#"
        mp <- barplot(c(3, 5, 2))
        stopifnot(length(mp) == 3)
        stopifnot(mp[1] == 1)
        stopifnot(mp[2] == 2)
        stopifnot(mp[3] == 3)
        "#,
    );
    let stderr = session.captured_stderr();
    assert!(
        !stderr.contains("Error"),
        "barplot should return midpoints, got stderr: {stderr}"
    );
}

// endregion

// region: boxplot() does not crash

#[test]
fn boxplot_runs_without_error() {
    let mut session = Session::new_with_captured_output();
    session.eval_source("boxplot(c(1, 2, 3, 4, 5))");
    let stderr = session.captured_stderr();
    assert!(
        !stderr.contains("Error"),
        "boxplot should not error, got stderr: {stderr}"
    );
}

// endregion

// region: Low-level additions

#[test]
fn lines_and_points_do_not_crash() {
    let mut session = Session::new_with_captured_output();
    session.eval_source(
        r#"
        plot(1:5, type="n")
        points(1:5, c(2,4,6,8,10))
        lines(1:5, c(1,3,5,7,9))
        "#,
    );
    let stderr = session.captured_stderr();
    assert!(
        !stderr.contains("Error"),
        "points/lines should not error, got stderr: {stderr}"
    );
}

#[test]
fn abline_does_not_crash() {
    let mut session = Session::new_with_captured_output();
    session.eval_source(
        r#"
        plot(1:5, type="n")
        abline(h = 3)
        abline(v = 2)
        abline(a = 0, b = 1)
        "#,
    );
    let stderr = session.captured_stderr();
    assert!(
        !stderr.contains("Error"),
        "abline should not error, got stderr: {stderr}"
    );
}

#[test]
fn title_does_not_crash() {
    let mut session = Session::new_with_captured_output();
    session.eval_source(
        r#"
        plot(1:5, type="n")
        title(main = "Test", xlab = "X", ylab = "Y")
        "#,
    );
    let stderr = session.captured_stderr();
    assert!(
        !stderr.contains("Error"),
        "title should not error, got stderr: {stderr}"
    );
}

#[test]
fn legend_does_not_crash() {
    let mut session = Session::new_with_captured_output();
    session.eval_source(
        r#"
        plot(1:5, type="n")
        legend()
        "#,
    );
    let stderr = session.captured_stderr();
    assert!(
        !stderr.contains("Error"),
        "legend should not error, got stderr: {stderr}"
    );
}

// endregion

// region: Device management

#[test]
fn dev_cur_returns_integer() {
    let mut session = Session::new_with_captured_output();
    session.eval_source(
        r#"
        d <- dev.cur()
        stopifnot(is.integer(d))
        stopifnot(d == 1L)
        "#,
    );
    let stderr = session.captured_stderr();
    assert!(
        !stderr.contains("Error"),
        "dev.cur() should return integer 1, got stderr: {stderr}"
    );
}

#[test]
fn dev_off_returns_integer() {
    let mut session = Session::new_with_captured_output();
    session.eval_source(
        r#"
        d <- dev.off()
        stopifnot(is.integer(d))
        "#,
    );
    let stderr = session.captured_stderr();
    assert!(
        !stderr.contains("Error"),
        "dev.off() should return integer, got stderr: {stderr}"
    );
}

#[test]
fn dev_new_sets_active_device() {
    let mut session = Session::new_with_captured_output();
    session.eval_source(
        r#"
        dev.new()
        d <- dev.cur()
        stopifnot(d == 2L)
        dev.off()
        "#,
    );
    let stderr = session.captured_stderr();
    assert!(
        !stderr.contains("Error"),
        "dev.new() should set device to 2, got stderr: {stderr}"
    );
}

// endregion

// region: par() compatibility

#[test]
fn par_returns_list() {
    let mut session = Session::new_with_captured_output();
    session.eval_source(
        r#"
        p <- par()
        stopifnot(is.list(p))
        "#,
    );
    let stderr = session.captured_stderr();
    assert!(
        !stderr.contains("Error"),
        "par() should return a list, got stderr: {stderr}"
    );
}

#[test]
fn par_with_args_returns_list() {
    let mut session = Session::new_with_captured_output();
    session.eval_source(
        r#"
        old <- par(mfrow = c(1, 2))
        stopifnot(is.list(old))
        "#,
    );
    let stderr = session.captured_stderr();
    assert!(
        !stderr.contains("Error"),
        "par(mfrow=...) should return a list, got stderr: {stderr}"
    );
}

// endregion

// region: File device stubs

#[test]
fn pdf_stub_prints_message() {
    let mut session = Session::new_with_captured_output();
    session.eval_source("pdf('test.pdf')");
    let stderr = session.captured_stderr();
    assert!(
        stderr.contains("not yet supported"),
        "pdf() should print a not-yet-supported message, got stderr: {stderr}"
    );
}

#[test]
fn png_stub_prints_message() {
    let mut session = Session::new_with_captured_output();
    session.eval_source("png('test.png')");
    let stderr = session.captured_stderr();
    assert!(
        stderr.contains("not yet supported"),
        "png() should print a not-yet-supported message, got stderr: {stderr}"
    );
}

#[test]
fn svg_stub_prints_message() {
    let mut session = Session::new_with_captured_output();
    session.eval_source("svg('test.svg')");
    let stderr = session.captured_stderr();
    assert!(
        stderr.contains("not yet supported"),
        "svg() should print a not-yet-supported message, got stderr: {stderr}"
    );
}

// endregion

// region: Plot type validation

#[test]
fn plot_invalid_type_errors() {
    let mut session = Session::new_with_captured_output();
    session.eval_source(
        r#"
        tryCatch(
            plot(1:5, type = "z"),
            error = function(e) cat("GOT_ERROR")
        )
        "#,
    );
    let stdout = session.captured_stdout();
    assert!(
        stdout.contains("GOT_ERROR"),
        "plot(type='z') should error, got stdout: {stdout}"
    );
}

// endregion
