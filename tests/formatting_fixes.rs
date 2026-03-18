//! Tests for substitute(), print.matrix, print.factor, summary quartiles, and str() for lists.

use r::Session;

// region: substitute

#[test]
fn substitute_captures_unevaluated_expression() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        f <- function(x) deparse(substitute(x))
        result <- f(a + b)
        stopifnot(result == "a + b")
        "#,
    )
    .unwrap();
}

#[test]
fn substitute_direct_call() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        result <- deparse(substitute(1 + 2))
        stopifnot(result == "1 + 2")
        "#,
    )
    .unwrap();
}

#[test]
fn substitute_symbol_lookup() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        f <- function(x) substitute(x)
        expr <- f(hello)
        stopifnot(deparse(expr) == "hello")
        "#,
    )
    .unwrap();
}

#[test]
fn substitute_complex_expression() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        f <- function(x) deparse(substitute(x))
        result <- f(sqrt(a^2 + b^2))
        stopifnot(result == "sqrt(a ^ 2 + b ^ 2)")
        "#,
    )
    .unwrap();
}

#[test]
fn substitute_with_named_arg() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        f <- function(x) deparse(substitute(x))
        result <- f(x = a * b)
        stopifnot(result == "a * b")
        "#,
    )
    .unwrap();
}

// endregion

// region: print.matrix

#[test]
fn print_matrix_2x3() {
    let mut s = Session::new_with_captured_output();
    s.eval_source("m <- matrix(1:6, nrow=2, ncol=3); print(m)")
        .unwrap();
    let output = s.captured_stdout();
    // Should contain column headers and row labels
    assert!(output.contains("[,1]"), "should have column header [,1]");
    assert!(output.contains("[,2]"), "should have column header [,2]");
    assert!(output.contains("[,3]"), "should have column header [,3]");
    assert!(output.contains("[1,]"), "should have row label [1,]");
    assert!(output.contains("[2,]"), "should have row label [2,]");
    // Matrix 1:6 in 2x3 is column-major: col1=[1,2], col2=[3,4], col3=[5,6]
    assert!(output.contains("1"), "should contain element 1");
    assert!(output.contains("6"), "should contain element 6");
}

#[test]
fn print_matrix_with_dimnames() {
    let mut s = Session::new_with_captured_output();
    s.eval_source(
        r#"
        m <- matrix(c(1,2,3,4), nrow=2)
        rownames(m) <- c("r1", "r2")
        colnames(m) <- c("c1", "c2")
        print(m)
        "#,
    )
    .unwrap();
    let output = s.captured_stdout();
    assert!(output.contains("c1"), "should have column name c1");
    assert!(output.contains("c2"), "should have column name c2");
    assert!(output.contains("r1"), "should have row name r1");
    assert!(output.contains("r2"), "should have row name r2");
}

// endregion

// region: print.factor

#[test]
fn print_factor_shows_labels() {
    let mut s = Session::new_with_captured_output();
    s.eval_source(r#"f <- factor(c("a", "b", "a", "c")); print(f)"#)
        .unwrap();
    let output = s.captured_stdout();
    // Should show level labels, not integer codes
    assert!(output.contains("a"), "should contain label 'a'");
    assert!(output.contains("b"), "should contain label 'b'");
    assert!(output.contains("c"), "should contain label 'c'");
    assert!(output.contains("Levels:"), "should show Levels line");
    // Should NOT show raw integers like "[1] 1 2 1 3"
    assert!(
        !output.starts_with("[1] 1 2 1 3"),
        "should not show integer codes"
    );
}

#[test]
fn print_factor_levels_line() {
    let mut s = Session::new_with_captured_output();
    s.eval_source(r#"f <- factor(c("x", "y", "z")); print(f)"#)
        .unwrap();
    let output = s.captured_stdout();
    assert!(
        output.contains("Levels: x y z"),
        "should show Levels: x y z, got: {}",
        output
    );
}

// endregion

// region: summary with quartiles

#[test]
fn summary_has_quartiles() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        s <- summary(1:100)
        nms <- names(s)
        stopifnot("Min." %in% nms)
        stopifnot("1st Qu." %in% nms)
        stopifnot("Median" %in% nms)
        stopifnot("Mean" %in% nms)
        stopifnot("3rd Qu." %in% nms)
        stopifnot("Max." %in% nms)
        "#,
    )
    .unwrap();
}

#[test]
fn summary_quartile_values() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        s <- summary(c(1, 2, 3, 4, 5))
        # Min = 1, Q1 = 2, Median = 3, Mean = 3, Q3 = 4, Max = 5
        stopifnot(s[1] == 1)    # Min
        stopifnot(s[2] == 2)    # 1st Qu.
        stopifnot(s[3] == 3)    # Median
        stopifnot(s[4] == 3)    # Mean
        stopifnot(s[5] == 4)    # 3rd Qu.
        stopifnot(s[6] == 5)    # Max
        "#,
    )
    .unwrap();
}

// endregion

// region: str() for lists

#[test]
fn str_list_shows_elements() {
    let mut s = Session::new_with_captured_output();
    s.eval_source(r#"str(list(a = 1, b = "hello", c = TRUE))"#)
        .unwrap();
    let output = s.captured_stdout();
    assert!(output.contains("List of 3"), "should show List of 3");
    assert!(output.contains("$ a"), "should show $ a");
    assert!(output.contains("$ b"), "should show $ b");
    assert!(output.contains("$ c"), "should show $ c");
}

#[test]
fn str_nested_list() {
    let mut s = Session::new_with_captured_output();
    s.eval_source(r#"str(list(x = list(y = 1, z = 2)))"#)
        .unwrap();
    let output = s.captured_stdout();
    assert!(output.contains("List of 1"), "should show List of 1");
    assert!(output.contains("$ x"), "should show $ x");
    assert!(output.contains("$ y"), "should show $ y");
    assert!(output.contains("$ z"), "should show $ z");
}

// endregion
