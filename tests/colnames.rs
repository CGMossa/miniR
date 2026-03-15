use r::Session;

#[test]
fn colnames_exposes_matrix_and_data_frame_labels() {
    let mut s = Session::new();
    s.eval_source(
        r#"m <- matrix(1:4, nrow = 2, dimnames = list(c("r1", "r2"), c("x", "y")))
stopifnot(identical(colnames(m), c("x", "y")))

df <- data.frame(x = 1:2, y = 3:4)
stopifnot(identical(colnames(df), c("x", "y")))"#,
    )
    .expect("colnames tests failed");
}
