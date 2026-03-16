use r::Session;

#[test]
fn det_singular_matrix_returns_zero() {
    let mut r = Session::new();
    r.eval_source(
        r#"
# Singular matrix: row 2 = 2 * row 1
m <- matrix(c(1, 2, 2, 4), nrow = 2)
d <- det(m)
stopifnot(d == 0)
"#,
    )
    .expect("det() on singular matrix should return 0");
}

#[test]
fn solve_singular_matrix_errors() {
    let mut r = Session::new();
    r.eval_source(
        r#"
m <- matrix(c(1, 2, 2, 4), nrow = 2)
err <- tryCatch(solve(m), error = function(e) conditionMessage(e))
stopifnot(is.character(err))
stopifnot(grepl("singular", err, ignore.case = TRUE))
"#,
    )
    .expect("solve() on singular matrix should produce an error message");
}

#[test]
fn chol_non_positive_definite_errors() {
    let mut r = Session::new();
    r.eval_source(
        r#"
# Not positive definite: eigenvalues include negatives
m <- matrix(c(1, 3, 3, 1), nrow = 2)
err <- tryCatch(chol(m), error = function(e) conditionMessage(e))
stopifnot(is.character(err))
stopifnot(grepl("not positive", err, ignore.case = TRUE))
"#,
    )
    .expect("chol() on non-positive-definite matrix should error");
}

#[test]
fn eigen_non_symmetric_matrix_errors() {
    let mut r = Session::new();
    r.eval_source(
        r#"
# Non-symmetric matrix
m <- matrix(c(1, 0, 5, 1), nrow = 2)
err <- tryCatch(eigen(m), error = function(e) conditionMessage(e))
stopifnot(is.character(err))
stopifnot(grepl("symmetric", err, ignore.case = TRUE))
"#,
    )
    .expect("eigen() on non-symmetric matrix should error");
}

#[test]
fn qr_rank_deficient_matrix() {
    let mut r = Session::new();
    r.eval_source(
        r#"
# Rank-deficient 3x3 matrix: col 3 = col 1 + col 2
m <- matrix(c(1, 0, 0,
              0, 1, 0,
              1, 1, 0), nrow = 3)
result <- qr(m)
stopifnot(result$rank == 2L)
"#,
    )
    .expect("qr()$rank should detect rank deficiency");
}

#[test]
fn svd_rectangular_matrix() {
    let mut r = Session::new();
    r.eval_source(
        r#"
# 3x2 rectangular matrix
m <- matrix(c(1, 2, 3, 4, 5, 6), nrow = 3, ncol = 2)
result <- svd(m)

# d should have min(3,2) = 2 singular values
stopifnot(length(result$d) == 2)
stopifnot(all(result$d > 0))

# u should be 3x2, v should be 2x2
stopifnot(identical(dim(result$u), c(3L, 2L)))
stopifnot(identical(dim(result$v), c(2L, 2L)))

# Reconstruction: u %*% diag(d) %*% t(v) should approximate m
reconstructed <- result$u %*% diag(result$d) %*% t(result$v)
stopifnot(all(abs(reconstructed - m) < 1e-10))
"#,
    )
    .expect("svd() on rectangular matrix should work");
}

#[test]
fn transpose_preserves_integer_type() {
    let mut r = Session::new();
    r.eval_source(
        r#"
m <- matrix(1L:6L, nrow = 2, ncol = 3)
tm <- t(m)
stopifnot(is.integer(tm))
stopifnot(identical(dim(tm), c(3L, 2L)))
"#,
    )
    .expect("t() should preserve integer type");
}

#[test]
fn crossprod_equals_t_x_matmul_x() {
    let mut r = Session::new();
    r.eval_source(
        r#"
x <- matrix(c(1, 2, 3, 4, 5, 6), nrow = 3, ncol = 2)
cp <- crossprod(x)
manual <- t(x) %*% x

stopifnot(identical(dim(cp), c(2L, 2L)))
stopifnot(identical(dim(manual), c(2L, 2L)))
stopifnot(all(abs(cp - manual) < 1e-10))
"#,
    )
    .expect("crossprod(x) should equal t(x) %*% x");
}

#[test]
fn matmul_dimensions() {
    let mut r = Session::new();
    r.eval_source(
        r#"
# 2x3 %*% 3x2 should give 2x2
a <- matrix(1:6, nrow = 2, ncol = 3)
b <- matrix(1:6, nrow = 3, ncol = 2)
result <- a %*% b

stopifnot(identical(dim(result), c(2L, 2L)))

# Verify values: manual computation
# a = [[1,3,5],[2,4,6]], b = [[1,4],[2,5],[3,6]]
# result[1,1] = 1*1 + 3*2 + 5*3 = 22
# result[2,1] = 2*1 + 4*2 + 6*3 = 28
# result[1,2] = 1*4 + 3*5 + 5*6 = 49
# result[2,2] = 2*4 + 4*5 + 6*6 = 64
stopifnot(result[1,1] == 22)
stopifnot(result[2,1] == 28)
stopifnot(result[1,2] == 49)
stopifnot(result[2,2] == 64)
"#,
    )
    .expect("matrix multiplication dimensions and values should be correct");
}

#[test]
fn solve_linear_system() {
    let mut r = Session::new();
    r.eval_source(
        r#"
# Solve Ax = b where A = [[2,1],[5,3]] (column-major), b = [4,7]
# A = matrix(c(2,5,1,3), nrow=2) => col1=[2,5], col2=[1,3]
# 2*x1 + 1*x2 = 4, 5*x1 + 3*x2 = 7  =>  x1 = 5, x2 = -6
A <- matrix(c(2, 5, 1, 3), nrow = 2)
b <- c(4, 7)
x <- solve(A, b)

stopifnot(abs(x[1] - 5.0) < 1e-10)
stopifnot(abs(x[2] - (-6.0)) < 1e-10)

# Verify: A %*% x should equal b
check <- A %*% x
stopifnot(abs(check[1] - 4.0) < 1e-10)
stopifnot(abs(check[2] - 7.0) < 1e-10)
"#,
    )
    .expect("solve(A, b) should correctly solve a linear system");
}
