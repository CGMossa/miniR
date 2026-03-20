use r::Session;

// region: sweep tests

#[test]
fn sweep_subtract_row_stats() {
    let mut r = Session::new();
    r.eval_source(
        r#"
# 3x2 matrix, column-major: col1 = c(1,2,3), col2 = c(4,5,6)
m <- matrix(1:6, nrow = 3, ncol = 2)
row_means <- c(2.5, 3.5, 4.5)  # mean of each row
result <- sweep(m, 1, row_means, "-")
# row 1: 1 - 2.5 = -1.5, 4 - 2.5 = 1.5
# row 2: 2 - 3.5 = -1.5, 5 - 3.5 = 1.5
# row 3: 3 - 4.5 = -1.5, 6 - 4.5 = 1.5
stopifnot(result[1, 1] == -1.5)
stopifnot(result[1, 2] == 1.5)
stopifnot(result[2, 1] == -1.5)
stopifnot(result[3, 2] == 1.5)
"#,
    )
    .expect("sweep with row subtraction should work");
}

#[test]
fn sweep_subtract_col_stats() {
    let mut r = Session::new();
    r.eval_source(
        r#"
m <- matrix(1:6, nrow = 3, ncol = 2)
col_means <- c(2.0, 5.0)  # mean of each column
result <- sweep(m, 2, col_means, "-")
# col 1: 1 - 2 = -1, 2 - 2 = 0, 3 - 2 = 1
# col 2: 4 - 5 = -1, 5 - 5 = 0, 6 - 5 = 1
stopifnot(result[1, 1] == -1)
stopifnot(result[2, 1] == 0)
stopifnot(result[3, 1] == 1)
stopifnot(result[1, 2] == -1)
stopifnot(result[2, 2] == 0)
stopifnot(result[3, 2] == 1)
"#,
    )
    .expect("sweep with column subtraction should work");
}

#[test]
fn sweep_default_fun_is_minus() {
    let mut r = Session::new();
    r.eval_source(
        r#"
m <- matrix(c(10, 20, 30, 40), nrow = 2)
stats <- c(1, 2)
result <- sweep(m, 1, stats)
# Default FUN is "-"
stopifnot(result[1, 1] == 9)
stopifnot(result[2, 1] == 18)
stopifnot(result[1, 2] == 29)
stopifnot(result[2, 2] == 38)
"#,
    )
    .expect("sweep default FUN should be subtraction");
}

#[test]
fn sweep_multiply() {
    let mut r = Session::new();
    r.eval_source(
        r#"
m <- matrix(c(1, 2, 3, 4, 5, 6), nrow = 2)
col_scales <- c(10, 100, 1000)
result <- sweep(m, 2, col_scales, "*")
stopifnot(result[1, 1] == 10)
stopifnot(result[2, 1] == 20)
stopifnot(result[1, 2] == 300)
stopifnot(result[2, 2] == 400)
stopifnot(result[1, 3] == 5000)
stopifnot(result[2, 3] == 6000)
"#,
    )
    .expect("sweep with multiplication should work");
}

#[test]
fn sweep_divide() {
    let mut r = Session::new();
    r.eval_source(
        r#"
m <- matrix(c(10, 20, 30, 60), nrow = 2)
row_divs <- c(10, 20)
result <- sweep(m, 1, row_divs, "/")
stopifnot(result[1, 1] == 1)
stopifnot(result[2, 1] == 1)
stopifnot(result[1, 2] == 3)
stopifnot(result[2, 2] == 3)
"#,
    )
    .expect("sweep with division should work");
}

#[test]
fn sweep_add() {
    let mut r = Session::new();
    r.eval_source(
        r#"
m <- matrix(c(1, 2, 3, 4), nrow = 2)
result <- sweep(m, 1, c(10, 20), "+")
stopifnot(result[1, 1] == 11)
stopifnot(result[2, 1] == 22)
stopifnot(result[1, 2] == 13)
stopifnot(result[2, 2] == 24)
"#,
    )
    .expect("sweep with addition should work");
}

#[test]
fn sweep_preserves_dimnames() {
    let mut r = Session::new();
    r.eval_source(
        r#"
m <- matrix(1:4, nrow = 2, dimnames = list(c("r1", "r2"), c("c1", "c2")))
result <- sweep(m, 1, c(0, 0), "-")
dn <- dimnames(result)
stopifnot(dn[[1]][1] == "r1")
stopifnot(dn[[1]][2] == "r2")
stopifnot(dn[[2]][1] == "c1")
stopifnot(dn[[2]][2] == "c2")
"#,
    )
    .expect("sweep should preserve dimnames");
}

#[test]
fn sweep_wrong_stats_length_errors() {
    let mut r = Session::new();
    r.eval_source(
        r#"
m <- matrix(1:6, nrow = 3, ncol = 2)
err <- tryCatch(sweep(m, 1, c(1, 2), "-"), error = function(e) conditionMessage(e))
stopifnot(is.character(err))
stopifnot(grepl("length", err, ignore.case = TRUE))
"#,
    )
    .expect("sweep with wrong STATS length should error");
}

#[test]
fn sweep_invalid_margin_errors() {
    let mut r = Session::new();
    r.eval_source(
        r#"
m <- matrix(1:4, nrow = 2)
err <- tryCatch(sweep(m, 3, c(1, 2), "-"), error = function(e) conditionMessage(e))
stopifnot(is.character(err))
stopifnot(grepl("MARGIN", err))
"#,
    )
    .expect("sweep with invalid MARGIN should error");
}

// endregion

// region: kronecker tests

#[test]
fn kronecker_2x2_identity() {
    let mut r = Session::new();
    r.eval_source(
        r#"
# Kronecker product of I2 with a 2x2 matrix
I2 <- diag(2)
B <- matrix(c(1, 2, 3, 4), nrow = 2)
K <- kronecker(I2, B)
# Result should be 4x4 block diagonal
stopifnot(nrow(K) == 4)
stopifnot(ncol(K) == 4)
# Top-left block = B
stopifnot(K[1, 1] == 1)
stopifnot(K[2, 1] == 2)
stopifnot(K[1, 2] == 3)
stopifnot(K[2, 2] == 4)
# Top-right block = 0
stopifnot(K[1, 3] == 0)
stopifnot(K[2, 3] == 0)
stopifnot(K[1, 4] == 0)
stopifnot(K[2, 4] == 0)
# Bottom-left block = 0
stopifnot(K[3, 1] == 0)
stopifnot(K[4, 1] == 0)
stopifnot(K[3, 2] == 0)
stopifnot(K[4, 2] == 0)
# Bottom-right block = B
stopifnot(K[3, 3] == 1)
stopifnot(K[4, 3] == 2)
stopifnot(K[3, 4] == 3)
stopifnot(K[4, 4] == 4)
"#,
    )
    .expect("Kronecker product with identity should produce block diagonal");
}

#[test]
fn kronecker_basic() {
    let mut r = Session::new();
    r.eval_source(
        r#"
A <- matrix(c(1, 2, 3, 4), nrow = 2)
B <- matrix(c(0, 5, 6, 7), nrow = 2)
K <- kronecker(A, B)
# A is: [1 3; 2 4], B is: [0 6; 5 7]
# K[1,1] = A[1,1]*B[1,1] = 1*0 = 0
# K[2,1] = A[1,1]*B[2,1] = 1*5 = 5
# K[3,1] = A[2,1]*B[1,1] = 2*0 = 0
# K[4,1] = A[2,1]*B[2,1] = 2*5 = 10
stopifnot(nrow(K) == 4)
stopifnot(ncol(K) == 4)
stopifnot(K[1, 1] == 0)
stopifnot(K[2, 1] == 5)
stopifnot(K[3, 1] == 0)
stopifnot(K[4, 1] == 10)
"#,
    )
    .expect("basic Kronecker product should work");
}

#[test]
fn kronecker_operator_percent_x_percent() {
    let mut r = Session::new();
    r.eval_source(
        r#"
A <- matrix(c(1, 0, 0, 1), nrow = 2)
B <- matrix(c(2, 3, 4, 5), nrow = 2)
K1 <- kronecker(A, B)
K2 <- A %x% B
# Both should give the same result
stopifnot(all(K1 == K2))
"#,
    )
    .expect("%x% operator should give same result as kronecker()");
}

#[test]
fn kronecker_non_square() {
    let mut r = Session::new();
    r.eval_source(
        r#"
# A is 2x3, B is 3x2 => result is 6x6
A <- matrix(1:6, nrow = 2, ncol = 3)
B <- matrix(1:6, nrow = 3, ncol = 2)
K <- kronecker(A, B)
stopifnot(nrow(K) == 6)
stopifnot(ncol(K) == 6)
# K[1,1] = A[1,1]*B[1,1] = 1*1 = 1
stopifnot(K[1, 1] == 1)
"#,
    )
    .expect("Kronecker product of non-square matrices should work");
}

#[test]
fn kronecker_scalar() {
    let mut r = Session::new();
    r.eval_source(
        r#"
# Scalar times matrix: should just scale the matrix
A <- matrix(3, nrow = 1, ncol = 1)
B <- matrix(c(1, 2, 3, 4), nrow = 2)
K <- kronecker(A, B)
stopifnot(nrow(K) == 2)
stopifnot(ncol(K) == 2)
stopifnot(K[1, 1] == 3)
stopifnot(K[2, 1] == 6)
stopifnot(K[1, 2] == 9)
stopifnot(K[2, 2] == 12)
"#,
    )
    .expect("Kronecker product with scalar matrix should scale");
}

#[test]
fn kronecker_vector_treated_as_column() {
    let mut r = Session::new();
    r.eval_source(
        r#"
# Plain vectors should be treated as column vectors
a <- c(1, 2)
b <- c(3, 4)
K <- kronecker(a, b)
# a is 2x1, b is 2x1 => result is 4x1
stopifnot(nrow(K) == 4)
stopifnot(ncol(K) == 1)
stopifnot(K[1, 1] == 3)   # 1*3
stopifnot(K[2, 1] == 4)   # 1*4
stopifnot(K[3, 1] == 6)   # 2*3
stopifnot(K[4, 1] == 8)   # 2*4
"#,
    )
    .expect("Kronecker product of vectors should work");
}

#[test]
fn kronecker_with_addition_fun() {
    let mut r = Session::new();
    r.eval_source(
        r#"
A <- matrix(c(10, 20), nrow = 1)
B <- matrix(c(1, 2), nrow = 1)
K <- kronecker(A, B, FUN = "+")
# Result is 1x4: 10+1, 10+2, 20+1, 20+2
stopifnot(nrow(K) == 1)
stopifnot(ncol(K) == 4)
stopifnot(K[1, 1] == 11)
stopifnot(K[1, 2] == 12)
stopifnot(K[1, 3] == 21)
stopifnot(K[1, 4] == 22)
"#,
    )
    .expect("Kronecker product with addition FUN should work");
}

#[test]
fn kronecker_result_is_matrix() {
    let mut r = Session::new();
    r.eval_source(
        r#"
A <- matrix(1:4, nrow = 2)
B <- matrix(1:4, nrow = 2)
K <- kronecker(A, B)
stopifnot(is.matrix(K))
d <- dim(K)
stopifnot(d[1] == 4)
stopifnot(d[2] == 4)
"#,
    )
    .expect("Kronecker result should be a matrix with dim attribute");
}

// endregion
