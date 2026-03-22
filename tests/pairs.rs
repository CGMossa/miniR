use r::Session;

#[test]
fn pairs_data_frame_returns_null() {
    let mut s = Session::new();
    s.eval_source(
        r#"
df <- data.frame(x = 1:5, y = c(2.0, 4.0, 6.0, 8.0, 10.0), z = c(5, 4, 3, 2, 1))
result <- pairs(df)
stopifnot(is.null(result))
"#,
    )
    .expect("pairs() on data frame should succeed");
}

#[test]
fn pairs_matrix_returns_null() {
    let mut s = Session::new();
    s.eval_source(
        r#"
m <- matrix(1:12, nrow = 4, ncol = 3)
result <- pairs(m)
stopifnot(is.null(result))
"#,
    )
    .expect("pairs() on matrix should succeed");
}

#[test]
fn pairs_skips_non_numeric_columns() {
    let mut s = Session::new();
    // Data frame with one character column and two numeric columns.
    // pairs() should silently skip the character column and succeed.
    s.eval_source(
        r#"
df <- data.frame(a = 1:5, b = c(10, 20, 30, 40, 50))
df$label <- c("a", "b", "c", "d", "e")
result <- pairs(df)
stopifnot(is.null(result))
"#,
    )
    .expect("pairs() should skip non-numeric columns");
}

#[test]
fn pairs_errors_on_plain_vector() {
    let mut s = Session::new();
    s.eval_source(
        r#"
err <- tryCatch(pairs(1:10), error = function(e) conditionMessage(e))
stopifnot(is.character(err))
stopifnot(grepl("data frame or a matrix", err))
"#,
    )
    .expect("pairs() on a plain vector should error with informative message");
}

#[test]
fn pairs_errors_on_non_numeric_matrix() {
    let mut s = Session::new();
    s.eval_source(
        r#"
err <- tryCatch(
    pairs(matrix(c("a", "b", "c", "d"), nrow = 2, ncol = 2)),
    error = function(e) conditionMessage(e)
)
stopifnot(is.character(err))
stopifnot(grepl("requires numeric data", err))
"#,
    )
    .expect("pairs() on character matrix should error");
}

#[test]
fn pairs_errors_on_fewer_than_two_numeric_columns() {
    let mut s = Session::new();
    s.eval_source(
        r#"
df <- data.frame(x = 1:5)
df$label <- c("a", "b", "c", "d", "e")
err <- tryCatch(pairs(df), error = function(e) conditionMessage(e))
stopifnot(is.character(err))
stopifnot(grepl("at least 2 numeric", err))
"#,
    )
    .expect("pairs() with fewer than 2 numeric columns should error");
}

#[test]
fn pairs_errors_on_non_data_types() {
    let mut s = Session::new();
    // A character scalar is a character vector, which hits the vector branch
    // and errors because it has no dim attribute (not a matrix).
    s.eval_source(
        r#"
err <- tryCatch(pairs("hello"), error = function(e) conditionMessage(e))
stopifnot(is.character(err))
stopifnot(grepl("data frame or a matrix", err))
"#,
    )
    .expect("pairs() on string should error");
}

#[test]
fn pairs_logical_columns_accepted() {
    let mut s = Session::new();
    // Logical columns should be treated as numeric (coerced to 0/1).
    s.eval_source(
        r#"
df <- data.frame(x = c(TRUE, FALSE, TRUE), y = c(FALSE, TRUE, FALSE), z = 1:3)
result <- pairs(df)
stopifnot(is.null(result))
"#,
    )
    .expect("pairs() should accept logical columns");
}

#[test]
fn pairs_integer_matrix_accepted() {
    let mut s = Session::new();
    s.eval_source(
        r#"
m <- matrix(1:6, nrow = 3, ncol = 2)
result <- pairs(m)
stopifnot(is.null(result))
"#,
    )
    .expect("pairs() should accept integer matrix");
}

#[test]
fn pairs_errors_on_plain_list() {
    let mut s = Session::new();
    // A plain list (not a data frame) should error.
    s.eval_source(
        r#"
err <- tryCatch(pairs(list(1, 2, 3)), error = function(e) conditionMessage(e))
stopifnot(is.character(err))
stopifnot(grepl("data frame or a numeric matrix", err))
"#,
    )
    .expect("pairs() on plain list should error");
}

#[test]
fn pairs_errors_on_function() {
    let mut s = Session::new();
    s.eval_source(
        r#"
err <- tryCatch(pairs(mean), error = function(e) conditionMessage(e))
stopifnot(is.character(err))
stopifnot(grepl("data frame or a numeric matrix", err))
"#,
    )
    .expect("pairs() on function should error");
}

#[test]
fn pairs_prints_stub_message() {
    let mut s = Session::new_with_captured_output();
    s.eval_source(
        r#"
df <- data.frame(x = 1:5, y = c(2.0, 4.0, 6.0, 8.0, 10.0))
pairs(df)
"#,
    )
    .expect("pairs() should succeed");
    let stderr = s.captured_stderr();
    assert!(
        stderr.contains("graphics devices are not yet supported"),
        "Expected stub message on stderr, got: {stderr}"
    );
}
