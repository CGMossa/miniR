use r::Session;

// region: row()

#[test]
fn row_basic_matrix() {
    let mut s = Session::new();
    s.eval_source(
        r#"
m <- matrix(1:6, nrow = 2, ncol = 3)
r <- row(m)
# row(m) should be:
#   [,1] [,2] [,3]
# [1,]  1    1    1
# [2,]  2    2    2
stopifnot(identical(r[1, 1], 1L))
stopifnot(identical(r[2, 1], 2L))
stopifnot(identical(r[1, 2], 1L))
stopifnot(identical(r[2, 3], 2L))
stopifnot(identical(nrow(r), 2L))
stopifnot(identical(ncol(r), 3L))
"#,
    )
    .expect("row() basic matrix failed");
}

#[test]
fn row_single_row() {
    let mut s = Session::new();
    s.eval_source(
        r#"
m <- matrix(1:5, nrow = 1, ncol = 5)
r <- row(m)
stopifnot(all(r == 1L))
"#,
    )
    .expect("row() single row failed");
}

#[test]
fn row_single_col() {
    let mut s = Session::new();
    s.eval_source(
        r#"
m <- matrix(1:4, nrow = 4, ncol = 1)
r <- row(m)
stopifnot(identical(r[1, 1], 1L))
stopifnot(identical(r[4, 1], 4L))
"#,
    )
    .expect("row() single col failed");
}

// endregion

// region: col()

#[test]
fn col_basic_matrix() {
    let mut s = Session::new();
    s.eval_source(
        r#"
m <- matrix(1:6, nrow = 2, ncol = 3)
c <- col(m)
# col(m) should be:
#   [,1] [,2] [,3]
# [1,]  1    2    3
# [2,]  1    2    3
stopifnot(identical(c[1, 1], 1L))
stopifnot(identical(c[2, 1], 1L))
stopifnot(identical(c[1, 2], 2L))
stopifnot(identical(c[2, 3], 3L))
stopifnot(identical(nrow(c), 2L))
stopifnot(identical(ncol(c), 3L))
"#,
    )
    .expect("col() basic matrix failed");
}

#[test]
fn col_single_row() {
    let mut s = Session::new();
    s.eval_source(
        r#"
m <- matrix(1:5, nrow = 1, ncol = 5)
c <- col(m)
stopifnot(identical(c[1, 1], 1L))
stopifnot(identical(c[1, 5], 5L))
"#,
    )
    .expect("col() single row failed");
}

// endregion

// region: slice.index()

#[test]
fn slice_index_margin1() {
    let mut s = Session::new();
    s.eval_source(
        r#"
m <- matrix(1:6, nrow = 2, ncol = 3)
s <- slice.index(m, 1)
# MARGIN=1 gives row indices, same as row()
stopifnot(identical(s[1, 1], 1L))
stopifnot(identical(s[2, 1], 2L))
stopifnot(identical(s[1, 3], 1L))
stopifnot(identical(s[2, 3], 2L))
"#,
    )
    .expect("slice.index MARGIN=1 failed");
}

#[test]
fn slice_index_margin2() {
    let mut s = Session::new();
    s.eval_source(
        r#"
m <- matrix(1:6, nrow = 2, ncol = 3)
s <- slice.index(m, 2)
# MARGIN=2 gives column indices, same as col()
stopifnot(identical(s[1, 1], 1L))
stopifnot(identical(s[2, 1], 1L))
stopifnot(identical(s[1, 3], 3L))
stopifnot(identical(s[2, 3], 3L))
"#,
    )
    .expect("slice.index MARGIN=2 failed");
}

// endregion

// region: nrow() / ncol()

#[test]
fn nrow_ncol_matrix() {
    let mut s = Session::new();
    s.eval_source(
        r#"
m <- matrix(1:12, nrow = 3, ncol = 4)
stopifnot(identical(nrow(m), 3L))
stopifnot(identical(ncol(m), 4L))
"#,
    )
    .expect("nrow/ncol on matrix failed");
}

#[test]
fn nrow_ncol_dataframe() {
    let mut s = Session::new();
    s.eval_source(
        r#"
df <- data.frame(x = 1:5, y = 6:10)
stopifnot(identical(nrow(df), 5L))
stopifnot(identical(ncol(df), 2L))
"#,
    )
    .expect("nrow/ncol on data.frame failed");
}

#[test]
fn nrow_ncol_vector_returns_null() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- 1:10
stopifnot(is.null(nrow(x)))
stopifnot(is.null(ncol(x)))
"#,
    )
    .expect("nrow/ncol on vector should return NULL");
}

#[test]
fn nrow_ncol_null_returns_null() {
    let mut s = Session::new();
    s.eval_source(
        r#"
stopifnot(is.null(nrow(NULL)))
stopifnot(is.null(ncol(NULL)))
"#,
    )
    .expect("nrow/ncol on NULL should return NULL");
}

// endregion

// region: NROW() / NCOL()

#[test]
fn nrow_safe_vector() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- 1:10
stopifnot(identical(NROW(x), 10L))
stopifnot(identical(NCOL(x), 1L))
"#,
    )
    .expect("NROW/NCOL on vector failed");
}

#[test]
fn nrow_safe_matrix() {
    let mut s = Session::new();
    s.eval_source(
        r#"
m <- matrix(1:12, nrow = 3, ncol = 4)
stopifnot(identical(NROW(m), 3L))
stopifnot(identical(NCOL(m), 4L))
"#,
    )
    .expect("NROW/NCOL on matrix failed");
}

#[test]
fn nrow_safe_null() {
    let mut s = Session::new();
    s.eval_source(
        r#"
stopifnot(identical(NROW(NULL), 0L))
stopifnot(identical(NCOL(NULL), 0L))
"#,
    )
    .expect("NROW/NCOL on NULL failed");
}

#[test]
fn nrow_safe_dataframe() {
    let mut s = Session::new();
    s.eval_source(
        r#"
df <- data.frame(a = 1:3, b = 4:6)
stopifnot(identical(NROW(df), 3L))
stopifnot(identical(NCOL(df), 2L))
"#,
    )
    .expect("NROW/NCOL on data.frame failed");
}

// endregion

// region: which() with arr.ind

#[test]
fn which_arr_ind_matrix() {
    let mut s = Session::new();
    s.eval_source(
        r#"
m <- matrix(c(TRUE, FALSE, FALSE, TRUE, TRUE, FALSE), nrow = 2, ncol = 3)
# m is:
#   [,1]  [,2]  [,3]
# [1,] TRUE  FALSE TRUE
# [2,] FALSE TRUE  FALSE
w <- which(m, arr.ind = TRUE)
stopifnot(is.matrix(w))
stopifnot(nrow(w) == 3)
stopifnot(ncol(w) == 2)
# TRUE positions: (1,1), (2,2), (1,3) in column-major order
stopifnot(w[1, 1] == 1)  # row of first TRUE
stopifnot(w[1, 2] == 1)  # col of first TRUE
stopifnot(w[2, 1] == 2)  # row of second TRUE
stopifnot(w[2, 2] == 2)  # col of second TRUE
stopifnot(w[3, 1] == 1)  # row of third TRUE
stopifnot(w[3, 2] == 3)  # col of third TRUE
"#,
    )
    .expect("which with arr.ind on matrix failed");
}

#[test]
fn which_arr_ind_has_dimnames() {
    let mut s = Session::new();
    s.eval_source(
        r#"
m <- matrix(c(TRUE, FALSE, FALSE, TRUE), nrow = 2)
w <- which(m, arr.ind = TRUE)
dn <- dimnames(w)
stopifnot(identical(dn[[2]], c("row", "col")))
"#,
    )
    .expect("which arr.ind should have row/col dimnames");
}

#[test]
fn which_no_arr_ind_returns_vector() {
    let mut s = Session::new();
    s.eval_source(
        r#"
m <- matrix(c(TRUE, FALSE, FALSE, TRUE, TRUE, FALSE), nrow = 2, ncol = 3)
w <- which(m)
stopifnot(is.null(dim(w)))
stopifnot(identical(w, c(1L, 4L, 5L)))
"#,
    )
    .expect("which without arr.ind should return vector");
}

// endregion

// region: is.matrix()

#[test]
fn is_matrix_true_for_matrix() {
    let mut s = Session::new();
    s.eval_source(
        r#"
m <- matrix(1:4, nrow = 2)
stopifnot(is.matrix(m))
"#,
    )
    .expect("is.matrix should be TRUE for matrix");
}

#[test]
fn is_matrix_false_for_vector() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- 1:10
stopifnot(!is.matrix(x))
"#,
    )
    .expect("is.matrix should be FALSE for vector");
}

#[test]
fn is_matrix_false_for_dataframe() {
    let mut s = Session::new();
    s.eval_source(
        r#"
df <- data.frame(a = 1:3)
stopifnot(!is.matrix(df))
"#,
    )
    .expect("is.matrix should be FALSE for data.frame");
}

#[test]
fn is_matrix_false_for_null() {
    let mut s = Session::new();
    s.eval_source(
        r#"
stopifnot(!is.matrix(NULL))
"#,
    )
    .expect("is.matrix should be FALSE for NULL");
}

// endregion

// region: as.matrix()

#[test]
fn as_matrix_vector() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- 1:5
m <- as.matrix(x)
stopifnot(is.matrix(m))
stopifnot(nrow(m) == 5)
stopifnot(ncol(m) == 1)
"#,
    )
    .expect("as.matrix on vector failed");
}

#[test]
fn as_matrix_dataframe() {
    let mut s = Session::new();
    s.eval_source(
        r#"
df <- data.frame(a = c(1, 2), b = c(3, 4))
m <- as.matrix(df)
stopifnot(is.matrix(m))
stopifnot(nrow(m) == 2)
stopifnot(ncol(m) == 2)
stopifnot(m[1, 1] == 1)
stopifnot(m[2, 2] == 4)
"#,
    )
    .expect("as.matrix on data.frame failed");
}

// endregion

// region: row()/col() combined with arithmetic

#[test]
fn row_col_arithmetic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
m <- matrix(1:6, nrow = 2, ncol = 3)
# Upper triangular mask: col(m) >= row(m)
# Result is a logical vector in column-major order (dim not preserved by comparison ops)
mask <- col(m) >= row(m)
# Column-major: (1,1)=T, (2,1)=F, (1,2)=T, (2,2)=T, (1,3)=T, (2,3)=T
stopifnot(identical(mask[1], TRUE))   # col 1 >= row 1
stopifnot(identical(mask[2], FALSE))  # col 1 < row 2
stopifnot(identical(mask[3], TRUE))   # col 2 >= row 1
stopifnot(identical(mask[4], TRUE))   # col 2 >= row 2
"#,
    )
    .expect("row/col arithmetic failed");
}

// endregion
