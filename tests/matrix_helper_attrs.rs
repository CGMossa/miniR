use r::Session;

#[test]
fn matrix_helpers_preserve_matrix_class_and_common_labels() {
    let mut s = Session::new();
    s.eval_source(
        r#"x <- matrix(1:4, nrow = 2, dimnames = list(c("r1", "r2"), c("x", "y")))
y <- matrix(5:8, nrow = 2, dimnames = list(c("a", "b"), c("u", "v")))

cp <- crossprod(x, y)
stopifnot(
  inherits(cp, "matrix"),
  identical(rownames(cp), c("x", "y")),
  identical(colnames(cp), c("u", "v"))
)

tcp <- tcrossprod(x, y)
stopifnot(
  inherits(tcp, "matrix"),
  identical(rownames(tcp), c("r1", "r2")),
  identical(colnames(tcp), c("a", "b"))
)

xv <- c(1, 2)
names(xv) <- c("a", "b")
yv <- c(3, 4)
names(yv) <- c("u", "v")
o <- outer(xv, yv)
stopifnot(
  inherits(o, "matrix"),
  identical(rownames(o), c("a", "b")),
  identical(colnames(o), c("u", "v"))
)

d <- diag(c(10, 20))
stopifnot(inherits(d, "matrix"), identical(dim(d), c(2L, 2L)))

lt <- lower.tri(x)
ut <- upper.tri(x)
stopifnot(
  inherits(lt, "matrix"),
  inherits(ut, "matrix"),
  identical(dim(lt), c(2L, 2L)),
  identical(dim(ut), c(2L, 2L))
)"#,
    )
    .expect("matrix helper attrs tests failed");
}
