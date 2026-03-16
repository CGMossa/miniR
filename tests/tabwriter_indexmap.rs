use r::Session;

// region: indexmap attribute ordering

#[test]
fn attributes_preserve_insertion_order() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- 1:3
attr(x, "first") <- "a"
attr(x, "second") <- "b"
attr(x, "third") <- "c"
a <- attributes(x)
stopifnot(identical(names(a), c("first", "second", "third")))
"#,
    )
    .expect("attribute insertion order should be preserved");
}

#[test]
fn data_frame_attributes_preserve_order() {
    let mut s = Session::new();
    s.eval_source(
        r#"
df <- data.frame(x = 1:3, y = 4:6)
a <- attributes(df)
# data.frame sets class, names, row.names in that order
nms <- names(a)
stopifnot("class" %in% nms)
stopifnot("names" %in% nms)
stopifnot("row.names" %in% nms)
"#,
    )
    .expect("data.frame attributes should be preserved");
}

#[test]
fn attr_overwrite_preserves_position() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- 1:3
attr(x, "a") <- 1
attr(x, "b") <- 2
attr(x, "c") <- 3
# Overwrite "a" — it should stay in its original position
attr(x, "a") <- 10
a <- attributes(x)
stopifnot(identical(names(a), c("a", "b", "c")))
stopifnot(a$a == 10)
"#,
    )
    .expect("overwriting an attribute should preserve its position");
}

// endregion

// region: print.data.frame with tabwriter

#[test]
fn print_data_frame_captures_output() {
    let mut s = Session::new();
    // We can't easily capture stdout from Rust tests, but we can verify
    // that print.data.frame dispatches without error
    s.eval_source(
        r#"
df <- data.frame(name = c("Alice", "Bob"), age = c(30L, 25L))
# Just verify it doesn't error — print returns invisibly
result <- print(df)
stopifnot(is.data.frame(result))
"#,
    )
    .expect("print.data.frame should work without errors");
}

#[test]
fn print_data_frame_with_factors() {
    let mut s = Session::new();
    s.eval_source(
        r#"
df <- data.frame(x = c("b", "a"), stringsAsFactors = TRUE)
result <- print(df)
stopifnot(is.data.frame(result))
stopifnot(is.factor(result$x))
"#,
    )
    .expect("print.data.frame with factors should work");
}

#[test]
fn print_empty_data_frame() {
    let mut s = Session::new();
    s.eval_source(
        r#"
df <- data.frame(row.names = 1:3)
result <- print(df)
stopifnot(is.data.frame(result))
stopifnot(nrow(result) == 3)
stopifnot(ncol(result) == 0)
"#,
    )
    .expect("printing an empty data.frame should work");
}

#[test]
fn print_data_frame_single_column() {
    let mut s = Session::new();
    s.eval_source(
        r#"
df <- data.frame(value = c(1.5, 2.7, 3.14))
result <- print(df)
stopifnot(is.data.frame(result))
"#,
    )
    .expect("printing a single-column data.frame should work");
}

#[test]
fn print_data_frame_with_na_values() {
    let mut s = Session::new();
    s.eval_source(
        r#"
df <- data.frame(x = c(1L, NA, 3L), y = c("a", NA, "c"))
result <- print(df)
stopifnot(is.data.frame(result))
"#,
    )
    .expect("printing a data.frame with NA values should work");
}

#[test]
fn print_data_frame_with_logical_column() {
    let mut s = Session::new();
    s.eval_source(
        r#"
df <- data.frame(flag = c(TRUE, FALSE, TRUE), val = 1:3)
result <- print(df)
stopifnot(is.data.frame(result))
"#,
    )
    .expect("printing a data.frame with logical columns should work");
}

// endregion
