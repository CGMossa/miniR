use r::Session;

#[test]
fn as_vector_strips_vector_attributes() {
    let mut s = Session::new();
    s.eval_source(
        r#"x <- structure(1:3, names = c("a", "b", "c"), class = "foo");
y <- as.vector(x);
stopifnot(is.null(attributes(y)))"#,
    )
    .expect("as.vector should strip attributes");
}
