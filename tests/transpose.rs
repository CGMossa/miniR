use r::Session;

#[test]
fn transpose_swaps_matrix_dimnames() {
    let mut s = Session::new();
    s.eval_source(
        r#"m <- matrix(
  1:6,
  nrow = 2,
  dimnames = list(c("r1", "r2"), c("x", "y", "z"))
)
tm <- t(m)
stopifnot(
  identical(dim(tm), c(3L, 2L)),
  identical(rownames(tm), c("x", "y", "z")),
  identical(colnames(tm), c("r1", "r2"))
)"#,
    )
    .expect("transpose tests failed");
}
