use r::Session;

// region: rank() tests

#[test]
fn rank_default_average_ties() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# rank() with ties.method = "average" (default)
x <- c(3, 1, 4, 1, 5)
r <- rank(x)
stopifnot(identical(r, c(3, 1.5, 4, 1.5, 5)))
"#,
    )
    .expect("rank() average ties failed");
}

#[test]
fn rank_no_ties() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# No ties: ranks should be 1..n in sorted order
x <- c(10, 30, 20)
r <- rank(x)
stopifnot(identical(r, c(1, 3, 2)))
"#,
    )
    .expect("rank() no ties failed");
}

#[test]
fn rank_first_ties() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# ties.method = "first" preserves original order
x <- c(3, 1, 4, 1, 5)
r <- rank(x, ties.method = "first")
stopifnot(identical(r, c(3, 1, 4, 2, 5)))
"#,
    )
    .expect("rank() first ties failed");
}

#[test]
fn rank_min_ties() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# ties.method = "min": tied values get the minimum rank
x <- c(3, 1, 4, 1, 5)
r <- rank(x, ties.method = "min")
stopifnot(identical(r, c(3, 1, 4, 1, 5)))
"#,
    )
    .expect("rank() min ties failed");
}

#[test]
fn rank_max_ties() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# ties.method = "max": tied values get the maximum rank
x <- c(3, 1, 4, 1, 5)
r <- rank(x, ties.method = "max")
stopifnot(identical(r, c(3, 2, 4, 2, 5)))
"#,
    )
    .expect("rank() max ties failed");
}

#[test]
fn rank_empty_vector() {
    let mut s = Session::new();
    s.eval_source(
        r#"
r <- rank(c())
stopifnot(length(r) == 0)
"#,
    )
    .expect("rank() empty vector failed");
}

#[test]
fn rank_single_element() {
    let mut s = Session::new();
    s.eval_source(
        r#"
r <- rank(c(42))
stopifnot(identical(r, 1))
"#,
    )
    .expect("rank() single element failed");
}

#[test]
fn rank_na_values_sort_last() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# NA values sort last by default
x <- c(2, NA, 1)
r <- rank(x)
stopifnot(r[1] == 2)
stopifnot(r[3] == 1)
stopifnot(r[2] == 3)
"#,
    )
    .expect("rank() NA handling failed");
}

#[test]
fn rank_all_tied() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# All values the same: average ties
x <- c(5, 5, 5)
r <- rank(x)
stopifnot(identical(r, c(2, 2, 2)))
"#,
    )
    .expect("rank() all tied failed");
}

#[test]
fn rank_invalid_ties_method_errors() {
    let mut s = Session::new();
    s.eval_source(
        r#"
err <- tryCatch(rank(c(1,2), ties.method = "bad"), error = function(e) conditionMessage(e))
stopifnot(is.character(err))
stopifnot(grepl("ties.method", err))
"#,
    )
    .expect("rank() invalid ties.method should error");
}

// endregion

// region: aperm() tests

#[test]
fn aperm_2d_is_transpose() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# For a 2D array, aperm() should be equivalent to t()
m <- matrix(1:6, nrow = 2, ncol = 3)
p <- aperm(m)
tm <- t(m)
stopifnot(identical(dim(p), dim(tm)))
stopifnot(identical(as.double(p), as.double(tm)))
"#,
    )
    .expect("aperm() 2D transpose failed");
}

#[test]
fn aperm_2d_identity_perm() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# perm = c(1,2) should return the same array
m <- matrix(1:6, nrow = 2, ncol = 3)
p <- aperm(m, c(1L, 2L))
stopifnot(identical(dim(p), c(2L, 3L)))
stopifnot(identical(as.double(p), as.double(m)))
"#,
    )
    .expect("aperm() identity permutation failed");
}

#[test]
fn aperm_2d_with_dimnames() {
    let mut s = Session::new();
    s.eval_source(
        r#"
m <- matrix(1:6, nrow = 2, dimnames = list(c("r1", "r2"), c("a", "b", "c")))
p <- aperm(m)
stopifnot(identical(dim(p), c(3L, 2L)))
stopifnot(identical(rownames(p), c("a", "b", "c")))
stopifnot(identical(colnames(p), c("r1", "r2")))
"#,
    )
    .expect("aperm() 2D with dimnames failed");
}

#[test]
fn aperm_3d_reverse_dims() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# 3D array: 2x3x4, default perm reverses to 4x3x2
a <- array(1:24, dim = c(2L, 3L, 4L))
p <- aperm(a)
stopifnot(identical(dim(p), c(4L, 3L, 2L)))

# Check specific values: a[1,1,1] = 1, a[2,3,4] = 24
# After reversal, p[1,1,1] should be a[1,1,1] = 1
# and p[4,3,2] should be a[2,3,4] = 24
stopifnot(p[1] == 1)
"#,
    )
    .expect("aperm() 3D default reverse failed");
}

#[test]
fn aperm_3d_explicit_perm() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# 3D array: 2x3x4 with perm = c(2, 3, 1) -> 3x4x2
a <- array(1:24, dim = c(2L, 3L, 4L))
p <- aperm(a, c(2L, 3L, 1L))
stopifnot(identical(dim(p), c(3L, 4L, 2L)))
"#,
    )
    .expect("aperm() 3D explicit perm failed");
}

#[test]
fn aperm_no_dim_errors() {
    let mut s = Session::new();
    s.eval_source(
        r#"
err <- tryCatch(aperm(c(1,2,3)), error = function(e) conditionMessage(e))
stopifnot(is.character(err))
stopifnot(grepl("dim", err))
"#,
    )
    .expect("aperm() on vector without dim should error");
}

#[test]
fn aperm_invalid_perm_length_errors() {
    let mut s = Session::new();
    s.eval_source(
        r#"
m <- matrix(1:6, nrow = 2)
err <- tryCatch(aperm(m, c(1L, 2L, 3L)), error = function(e) conditionMessage(e))
stopifnot(is.character(err))
stopifnot(grepl("length", err))
"#,
    )
    .expect("aperm() with wrong perm length should error");
}

// endregion
