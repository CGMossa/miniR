use r::Session;

#[test]
fn dim_reports_data_frame_shape() {
    let mut s = Session::new();
    s.eval_source(
        r#"df <- data.frame(x = 1:2, y = 3:4)
stopifnot(identical(dim(df), c(2L, 2L)))

empty <- data.frame(row.names = 1:4)
stopifnot(identical(dim(empty), c(4L, 0L)))"#,
    )
    .expect("dim tests failed");
}
