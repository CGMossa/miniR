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
fn eigen_non_symmetric_real_eigenvalues() {
    // Non-symmetric matrix with real eigenvalues should now succeed (nalgebra Schur)
    let mut r = Session::new();
    r.eval_source(
        r#"
# Non-symmetric upper-triangular matrix: eigenvalues are 1 and 1
m <- matrix(c(1, 0, 5, 1), nrow = 2)
result <- eigen(m)
stopifnot(length(result$values) == 2)
stopifnot(all(abs(result$values - 1) < 1e-10))
"#,
    )
    .expect("eigen() on non-symmetric matrix with real eigenvalues should work");
}

#[test]
fn eigen_non_symmetric_complex_eigenvalues_errors() {
    // Non-symmetric matrix with complex eigenvalues should error
    let mut r = Session::new();
    r.eval_source(
        r#"
# Rotation matrix: eigenvalues are complex (cos(theta) +/- i*sin(theta))
m <- matrix(c(0, 1, -1, 0), nrow = 2)
err <- tryCatch(eigen(m), error = function(e) conditionMessage(e))
stopifnot(is.character(err))
stopifnot(grepl("complex", err, ignore.case = TRUE))
"#,
    )
    .expect("eigen() on matrix with complex eigenvalues should error");
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

// region: nalgebra-based decomposition tests

#[test]
fn svd_square_matrix_reconstruction() {
    let mut r = Session::new();
    r.eval_source(
        r#"
# 3x3 matrix SVD with reconstruction check
m <- matrix(c(1, 4, 7, 2, 5, 8, 3, 6, 10), nrow = 3)
result <- svd(m)

# Check dimensions
stopifnot(length(result$d) == 3)
stopifnot(identical(dim(result$u), c(3L, 3L)))
stopifnot(identical(dim(result$v), c(3L, 3L)))

# Singular values should be positive and descending
stopifnot(all(result$d >= 0))
stopifnot(all(diff(result$d) <= 0))

# Reconstruction: u %*% diag(d) %*% t(v) should approximate m
reconstructed <- result$u %*% diag(result$d) %*% t(result$v)
stopifnot(all(abs(reconstructed - m) < 1e-10))
"#,
    )
    .expect("svd() square matrix reconstruction should work");
}

#[test]
fn svd_wide_matrix() {
    let mut r = Session::new();
    r.eval_source(
        r#"
# 2x4 wide matrix
m <- matrix(1:8, nrow = 2, ncol = 4)
result <- svd(m)

# d should have min(2,4) = 2 singular values
stopifnot(length(result$d) == 2)

# u should be 2x2, v should be 4x2
stopifnot(identical(dim(result$u), c(2L, 2L)))
stopifnot(identical(dim(result$v), c(4L, 2L)))

# Reconstruction check
reconstructed <- result$u %*% diag(result$d) %*% t(result$v)
stopifnot(all(abs(reconstructed - m) < 1e-10))
"#,
    )
    .expect("svd() on wide matrix should work");
}

#[test]
fn det_identity_matrix() {
    let mut r = Session::new();
    r.eval_source(
        r#"
# det(I) = 1 for any identity matrix
stopifnot(abs(det(diag(1)) - 1) < 1e-15)
stopifnot(abs(det(diag(3)) - 1) < 1e-15)
stopifnot(abs(det(diag(5)) - 1) < 1e-15)

# det of a known matrix
m <- matrix(c(1, 2, 3, 4), nrow = 2)
# det = 1*4 - 3*2 = -2
stopifnot(abs(det(m) - (-2)) < 1e-10)
"#,
    )
    .expect("det() should compute correct determinants");
}

#[test]
fn chol_symmetric_positive_definite() {
    let mut r = Session::new();
    r.eval_source(
        r#"
# Cholesky of identity is identity
stopifnot(all(abs(chol(diag(3)) - diag(3)) < 1e-15))

# Cholesky of a known SPD matrix
# A = t(R) %*% R where R = chol(A)
m <- matrix(c(4, 2, 2, 3), nrow = 2)
R <- chol(m)
stopifnot(all(abs(t(R) %*% R - m) < 1e-10))

# R should be upper triangular
stopifnot(abs(R[2, 1]) < 1e-15)
"#,
    )
    .expect("chol() should produce correct upper triangular factor");
}

#[test]
fn qr_full_rank_matrix() {
    let mut r = Session::new();
    r.eval_source(
        r#"
# Full rank 3x3 matrix
m <- matrix(c(1, 2, 3, 4, 5, 6, 7, 8, 10), nrow = 3)
result <- qr(m)
stopifnot(result$rank == 3L)

# Q should be stored
Q <- result$Q
stopifnot(identical(dim(Q), c(3L, 3L)))

# Q should be orthogonal: Q^T Q = I
QtQ <- t(Q) %*% Q
stopifnot(all(abs(QtQ - diag(3)) < 1e-10))
"#,
    )
    .expect("qr() on full-rank matrix should work");
}

#[test]
fn eigen_symmetric_matrix() {
    let mut r = Session::new();
    r.eval_source(
        r#"
# Symmetric matrix eigendecomposition
m <- matrix(c(2, 1, 1, 3), nrow = 2)
result <- eigen(m)

# Check structure
stopifnot(length(result$values) == 2)
stopifnot(identical(dim(result$vectors), c(2L, 2L)))

# Eigenvalues should be descending
stopifnot(result$values[1] >= result$values[2])

# Reconstruction: A = V diag(lambda) V^T for symmetric
V <- result$vectors
D <- diag(result$values)
reconstructed <- V %*% D %*% t(V)
stopifnot(all(abs(reconstructed - m) < 1e-10))

# Eigenvectors should be orthogonal for symmetric matrix
stopifnot(all(abs(t(V) %*% V - diag(2)) < 1e-10))
"#,
    )
    .expect("eigen() on symmetric matrix should produce correct eigendecomposition");
}

#[test]
fn solve_matrix_inverse() {
    let mut r = Session::new();
    r.eval_source(
        r#"
# solve(A) should give the inverse
A <- matrix(c(2, 1, 1, 3), nrow = 2)
Ainv <- solve(A)

# A %*% A^{-1} should be I
product <- A %*% Ainv
stopifnot(all(abs(product - diag(2)) < 1e-10))

# A^{-1} %*% A should be I
product2 <- Ainv %*% A
stopifnot(all(abs(product2 - diag(2)) < 1e-10))
"#,
    )
    .expect("solve(A) should compute the matrix inverse");
}

#[test]
fn solve_with_multiple_rhs_columns() {
    let mut r = Session::new();
    r.eval_source(
        r#"
# Solve AX = B where B has multiple columns
A <- matrix(c(2, 1, 1, 3), nrow = 2)
B <- matrix(c(1, 0, 0, 1, 1, 1), nrow = 2, ncol = 3)
X <- solve(A, B)

# Verify: A %*% X should equal B
check <- A %*% X
stopifnot(all(abs(check - B) < 1e-10))
"#,
    )
    .expect("solve(A, B) with multiple RHS columns should work");
}

// endregion

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
