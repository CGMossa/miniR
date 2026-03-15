use r::Session;

#[test]
fn unname_strips_matrix_dimnames() {
    let mut s = Session::new();
    s.eval_source(
        r#"m <- matrix(1:4, nrow = 2, dimnames = list(c("r1", "r2"), c("x", "y")))
um <- unname(m)
stopifnot(is.null(rownames(um)), is.null(colnames(um)))

df <- data.frame(x = 1:2, y = 3:4)
udf <- unname(df)
stopifnot(
  is.null(names(udf)),
  identical(row.names(udf), c("1", "2")),
  is.null(colnames(udf))
)"#,
    )
    .expect("unname tests failed");
}
