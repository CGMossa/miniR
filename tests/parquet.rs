#![cfg(feature = "parquet")]

use r::Session;

// region: read/write roundtrip tests

#[test]
fn parquet_write_and_read_roundtrip() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
df <- data.frame(
    name = c("Alice", "Bob", "Charlie"),
    age = c(30L, 25L, 35L),
    score = c(95.5, 87.3, 92.1),
    active = c(TRUE, FALSE, TRUE)
)
f <- tempfile(fileext = ".parquet")
write.parquet(df, f)
df2 <- read.parquet(f)
stopifnot(is.data.frame(df2))
stopifnot(ncol(df2) == 4)
stopifnot(nrow(df2) == 3)
stopifnot(identical(df2$name, c("Alice", "Bob", "Charlie")))
stopifnot(identical(df2$age, c(30L, 25L, 35L)))
stopifnot(identical(df2$active, c(TRUE, FALSE, TRUE)))
stopifnot(abs(df2$score[1] - 95.5) < 1e-10)
stopifnot(abs(df2$score[2] - 87.3) < 1e-10)
stopifnot(abs(df2$score[3] - 92.1) < 1e-10)
"#,
    );
}

#[test]
fn parquet_column_selection() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
df <- data.frame(a = 1:3, b = c("x", "y", "z"), c = c(1.1, 2.2, 3.3))
f <- tempfile(fileext = ".parquet")
write.parquet(df, f)

# Read only selected columns
df2 <- read.parquet(f, columns = c("a", "c"))
stopifnot(is.data.frame(df2))
stopifnot(ncol(df2) == 2)
stopifnot(nrow(df2) == 3)
stopifnot("a" %in% names(df2))
stopifnot("c" %in% names(df2))
stopifnot(!("b" %in% names(df2)))
"#,
    );
}

#[test]
fn parquet_na_values() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
x <- c(1L, NA, 3L)
y <- c("hello", NA, "world")
z <- c(1.5, 2.5, NA)
w <- c(TRUE, NA, FALSE)
df <- data.frame(x = x, y = y, z = z, w = w)
f <- tempfile(fileext = ".parquet")
write.parquet(df, f)
df2 <- read.parquet(f)
stopifnot(is.na(df2$x[2]))
stopifnot(!is.na(df2$x[1]))
stopifnot(df2$x[1] == 1L)
stopifnot(df2$x[3] == 3L)
stopifnot(is.na(df2$y[2]))
stopifnot(df2$y[1] == "hello")
stopifnot(is.na(df2$z[3]))
stopifnot(abs(df2$z[1] - 1.5) < 1e-10)
stopifnot(is.na(df2$w[2]))
stopifnot(df2$w[1] == TRUE)
stopifnot(df2$w[3] == FALSE)
"#,
    );
}

#[test]
fn parquet_empty_data_frame() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
df <- data.frame(a = integer(0), b = character(0))
f <- tempfile(fileext = ".parquet")
write.parquet(df, f)
df2 <- read.parquet(f)
stopifnot(is.data.frame(df2))
stopifnot(nrow(df2) == 0)
"#,
    );
}

#[test]
fn parquet_column_names_preserved() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
df <- data.frame(
    first.name = c("A", "B"),
    last.name = c("C", "D"),
    age = c(1L, 2L)
)
f <- tempfile(fileext = ".parquet")
write.parquet(df, f)
df2 <- read.parquet(f)
stopifnot(identical(names(df2), c("first.name", "last.name", "age")))
"#,
    );
}

#[test]
fn parquet_read_nonexistent_file_errors() {
    let mut s = Session::new();
    let result = s.eval_source(r#"read.parquet("/nonexistent/file.parquet")"#);
    // Should produce an error, not a panic
    assert!(
        result.is_err(),
        "reading a nonexistent file should return an error"
    );
}

#[test]
fn parquet_write_non_dataframe_errors() {
    let mut s = Session::new();
    let result = s.eval_source(
        r#"
f <- tempfile(fileext = ".parquet")
write.parquet(42, f)
"#,
    );
    assert!(
        result.is_err(),
        "writing a non-data.frame should return an error"
    );
}

#[test]
fn parquet_invalid_column_selection_errors() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
df <- data.frame(a = 1:3, b = 4:6)
f <- tempfile(fileext = ".parquet")
write.parquet(df, f)
"#,
    );
    let result = s.eval_source(r#"read.parquet(f, columns = c("a", "nonexistent"))"#);
    assert!(
        result.is_err(),
        "selecting nonexistent columns should return an error"
    );
}

// endregion
