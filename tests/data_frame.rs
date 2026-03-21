use r::Session;

#[test]
fn data_frame_recycles_and_honors_row_names() {
    let mut s = Session::new();
    s.eval_source(
        r#"recycled <- data.frame(x = c("A", "B"), y = "C")
stopifnot(
  nrow(recycled) == 2L,
  identical(recycled$y, c("C", "C")),
  identical(row.names(recycled), c("1", "2"))
)

named_rows <- data.frame(x = c(a = 1, b = 2))
stopifnot(
  identical(row.names(named_rows), c("a", "b")),
  is.null(names(named_rows$x))
)

auto_rows <- data.frame(x = c(a = 1, b = 2), row.names = NULL)
stopifnot(identical(row.names(auto_rows), c("1", "2")))

empty <- data.frame(row.names = 1:4)
stopifnot(
  nrow(empty) == 4L,
  length(names(empty)) == 0L,
  identical(row.names(empty), c("1", "2", "3", "4"))
)

from_matrix <- data.frame(matrix(1:4, nrow = 2, dimnames = list(c("r1", "r2"), c("x", "y"))))
stopifnot(
  identical(names(from_matrix), c("x", "y")),
  identical(from_matrix$y, c(3L, 4L)),
  identical(row.names(from_matrix), c("r1", "r2"))
)

from_list <- data.frame(list(a = 1:2, b = 3:4))
stopifnot(identical(names(from_list), c("a", "b")))

factored <- data.frame(x = c("b", "a"), stringsAsFactors = TRUE)
stopifnot(is.factor(factored$x))"#,
    )
    .expect("data frame tests failed");
}

#[test]
fn data_frame_rejects_incompatible_row_counts() {
    let mut s = Session::new();
    let err = s
        .eval_source("data.frame(x = 1:3, y = 1:2)")
        .expect_err("data.frame with incompatible row counts should fail");

    assert!(
        err.to_string()
            .contains("arguments imply differing number of rows"),
        "unexpected error: {err}"
    );
}

/// miniR enhancement: named columns are visible to subsequent column expressions,
/// like dplyr::tibble() (GNU R data.frame() does NOT support this).
#[test]
fn data_frame_forward_references() {
    let mut s = Session::new();
    s.eval_source(
        r#"
df <- data.frame(x = 1:5, xx = x * x)
stopifnot(
  identical(df$x,  1:5),
  identical(df$xx, c(1L, 4L, 9L, 16L, 25L))
)
"#,
    )
    .unwrap();
}

#[test]
fn data_frame_forward_ref_multiple_columns() {
    let mut s = Session::new();
    s.eval_source(
        r#"
df <- data.frame(a = 1:3, b = a + 10L, c = a + b)
stopifnot(
  identical(df$a, 1:3),
  identical(df$b, c(11L, 12L, 13L)),
  identical(df$c, c(12L, 14L, 16L))
)
"#,
    )
    .unwrap();
}

#[test]
fn data_frame_forward_ref_does_not_leak() {
    // Column bindings should not persist in the caller's environment.
    let mut s = Session::new();
    let result = s.eval_source(
        r#"
df <- data.frame(secret = 1:3, derived = secret * 2L)
secret
"#,
    );
    assert!(
        result.is_err(),
        "column binding should not leak into caller env"
    );
}
