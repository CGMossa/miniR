use r::Session;

// region: data frame input

#[test]
fn pairs_data_frame_basic() {
    let mut r = Session::new_with_captured_output();
    r.eval_source(
        r#"
df <- data.frame(x = c(1, 2, 3), y = c(4, 5, 6), z = c(7, 8, 9))
pairs(df)
"#,
    )
    .expect("pairs() should work on a 3-column data frame");
    let out = r.captured_stdout();
    assert!(out.contains("Scatterplot Matrix"), "should contain title");
    assert!(out.contains("x vs y"), "should contain x vs y pair");
    assert!(out.contains("x vs z"), "should contain x vs z pair");
    assert!(out.contains("y vs z"), "should contain y vs z pair");
    assert!(out.contains("3 points"), "each pair should have 3 points");
}

#[test]
fn pairs_data_frame_skips_non_numeric() {
    let mut r = Session::new_with_captured_output();
    r.eval_source(
        r#"
df <- data.frame(a = c(1, 2), b = c("x", "y"), c = c(3, 4), stringsAsFactors = FALSE)
pairs(df)
"#,
    )
    .expect("pairs() should skip non-numeric columns");
    let out = r.captured_stdout();
    assert!(
        out.contains("a vs c"),
        "should pair the two numeric columns"
    );
    // Should not reference column b
    assert!(
        !out.contains("vs b"),
        "should not include non-numeric column b"
    );
    assert!(
        !out.contains("b vs"),
        "should not include non-numeric column b"
    );
}

#[test]
fn pairs_data_frame_with_na() {
    let mut r = Session::new_with_captured_output();
    r.eval_source(
        r#"
df <- data.frame(x = c(1, NA, 3), y = c(4, 5, NA))
pairs(df)
"#,
    )
    .expect("pairs() should handle NAs");
    let out = r.captured_stdout();
    // Only points where both x and y are non-NA count
    assert!(out.contains("1 points"), "only 1 complete pair (1,4)");
}

#[test]
fn pairs_data_frame_too_few_numeric_cols() {
    let mut r = Session::new();
    let result = r.eval_source(
        r#"
df <- data.frame(x = c(1, 2, 3), name = c("a", "b", "c"), stringsAsFactors = FALSE)
pairs(df)
"#,
    );
    assert!(result.is_err(), "should error with only 1 numeric column");
}

// endregion

// region: matrix input

#[test]
fn pairs_matrix_basic() {
    let mut r = Session::new_with_captured_output();
    r.eval_source(
        r#"
m <- matrix(c(1,2,3, 4,5,6, 7,8,9), nrow = 3, ncol = 3)
pairs(m)
"#,
    )
    .expect("pairs() should work on a numeric matrix");
    let out = r.captured_stdout();
    assert!(out.contains("Scatterplot Matrix"), "should contain title");
    // Default column names V1, V2, V3
    assert!(out.contains("V1 vs V2"), "should contain V1 vs V2");
    assert!(out.contains("V1 vs V3"), "should contain V1 vs V3");
    assert!(out.contains("V2 vs V3"), "should contain V2 vs V3");
}

#[test]
fn pairs_matrix_with_colnames() {
    let mut r = Session::new_with_captured_output();
    r.eval_source(
        r#"
m <- matrix(c(1,2,3, 4,5,6), nrow = 3, ncol = 2)
colnames(m) <- c("height", "weight")
pairs(m)
"#,
    )
    .expect("pairs() should use dimnames column names");
    let out = r.captured_stdout();
    assert!(
        out.contains("height vs weight"),
        "should use column names from dimnames"
    );
}

// endregion

// region: error cases

#[test]
fn pairs_error_on_plain_vector() {
    let mut r = Session::new();
    let result = r.eval_source("pairs(c(1, 2, 3))");
    assert!(result.is_err(), "should error on a plain vector");
}

#[test]
fn pairs_error_on_null() {
    let mut r = Session::new();
    let result = r.eval_source("pairs(NULL)");
    assert!(result.is_err(), "should error on NULL");
}

#[test]
fn pairs_error_no_args() {
    let mut r = Session::new();
    let result = r.eval_source("pairs()");
    assert!(result.is_err(), "should error with no arguments");
}

// endregion

// region: legend output

#[test]
fn pairs_shows_legend() {
    let mut r = Session::new_with_captured_output();
    r.eval_source(
        r#"
df <- data.frame(a = c(1, 2), b = c(3, 4))
pairs(df)
"#,
    )
    .expect("pairs() should succeed");
    let out = r.captured_stdout();
    assert!(out.contains("legend:"), "should show legend");
    assert!(
        out.contains("a vs b"),
        "legend should contain the pair label"
    );
}

// endregion

// region: pair count

#[test]
fn pairs_correct_number_of_series() {
    let mut r = Session::new_with_captured_output();
    // 4 numeric columns -> 4*3/2 = 6 pairs
    r.eval_source(
        r#"
df <- data.frame(a = 1:5, b = 6:10, c = 11:15, d = 16:20)
pairs(df)
"#,
    )
    .expect("pairs() should succeed with 4 columns");
    let out = r.captured_stdout();
    // Count the number of [points] lines
    let point_lines: Vec<&str> = out.lines().filter(|l| l.contains("[points]")).collect();
    assert_eq!(
        point_lines.len(),
        6,
        "4 columns should produce 6 pairs, got: {:?}",
        point_lines
    );
}

#[test]
fn pairs_returns_null_invisibly() {
    let mut r = Session::new_with_captured_output();
    r.eval_source(
        r#"
df <- data.frame(x = c(1, 2), y = c(3, 4))
result <- pairs(df)
stopifnot(is.null(result))
"#,
    )
    .expect("pairs() should return NULL");

    // Test that pairs() itself returns invisibly
    let result = r
        .eval_source(
            r#"
pairs(data.frame(a = 1:3, b = 4:6))
"#,
        )
        .expect("pairs() should succeed");
    assert!(
        !result.visible,
        "pairs() should mark the return value invisible"
    );
}

// endregion
