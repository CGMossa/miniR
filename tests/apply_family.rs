use r::Session;

// region: apply

#[test]
fn apply_rows_sum() {
    let mut s = Session::new();
    s.eval_source(
        r#"
m <- matrix(1:6, nrow = 2, ncol = 3)
# m is:
#   [,1] [,2] [,3]
# [1,]  1    3    5
# [2,]  2    4    6
row_sums <- apply(m, 1, sum)
stopifnot(identical(row_sums, c(9, 12)))
"#,
    )
    .expect("apply rows sum failed");
}

#[test]
fn apply_cols_sum() {
    let mut s = Session::new();
    s.eval_source(
        r#"
m <- matrix(1:6, nrow = 2, ncol = 3)
col_sums <- apply(m, 2, sum)
stopifnot(identical(col_sums, c(3, 7, 11)))
"#,
    )
    .expect("apply cols sum failed");
}

#[test]
fn apply_rows_mean() {
    let mut s = Session::new();
    s.eval_source(
        r#"
m <- matrix(c(1, 2, 3, 4, 5, 6), nrow = 2)
row_means <- apply(m, 1, mean)
stopifnot(identical(row_means, c(3, 4)))
"#,
    )
    .expect("apply rows mean failed");
}

#[test]
fn apply_with_extra_args() {
    let mut s = Session::new();
    s.eval_source(
        r#"
m <- matrix(c(1, 2, 3, 4), nrow = 2)
result <- apply(m, 1, paste, collapse = "-")
stopifnot(identical(result, c("1-3", "2-4")))
"#,
    )
    .expect("apply with extra args failed");
}

#[test]
fn apply_returns_list_for_varying_lengths() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# 3x2 matrix: rows are [1,4], [2,5], [3,6]
m <- matrix(1:6, nrow = 3, ncol = 2)
# f returns elements > 3: row1=[1,4]->4, row2=[2,5]->5, row3=[3,6]->c(3,6) (wait, 3>3 is F)
# Actually: row3=[3,6]->[6] since only 6>3
# All return length 1, so simplify to vector. Need truly different lengths.
# Use a function that returns variable-length results:
f <- function(x) rep(x[1], times = x[1])
result <- apply(m, 1, f)
# row1: rep(1, 1) = [1], row2: rep(2, 2) = [2,2], row3: rep(3, 3) = [3,3,3]
# Different lengths -> must return a list
stopifnot(is.list(result))
"#,
    )
    .expect("apply returns list failed");
}

#[test]
fn apply_preserves_integer_type() {
    let mut s = Session::new();
    s.eval_source(
        r#"
m <- matrix(1L:6L, nrow = 2, ncol = 3)
# Extract first element of each row — should remain integer
result <- apply(m, 1, function(x) x[1])
stopifnot(is.integer(result))
"#,
    )
    .expect("apply preserves integer type failed");
}

// endregion

// region: mapply

#[test]
fn mapply_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- mapply(function(x, y) x + y, 1:3, 10:12)
stopifnot(identical(result, c(11L, 13L, 15L)))
"#,
    )
    .expect("mapply basic failed");
}

#[test]
fn mapply_recycling() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- mapply(function(x, y) x * y, 1:4, c(10, 20))
stopifnot(identical(result, c(10, 40, 30, 80)))
"#,
    )
    .expect("mapply recycling failed");
}

#[test]
fn mapply_simplify_false() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- mapply(function(x, y) x + y, 1:3, 10:12, SIMPLIFY = FALSE)
stopifnot(is.list(result))
stopifnot(length(result) == 3)
stopifnot(result[[1]] == 11)
stopifnot(result[[2]] == 13)
stopifnot(result[[3]] == 15)
"#,
    )
    .expect("mapply simplify false failed");
}

#[test]
fn mapply_paste() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- mapply(paste, c("a", "b", "c"), 1:3, MoreArgs = NULL)
stopifnot(identical(result, c("a 1", "b 2", "c 3")))
"#,
    )
    .expect("mapply paste failed");
}

// endregion

// region: tapply

#[test]
fn tapply_basic_sum() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- c(1, 2, 3, 4, 5, 6)
groups <- c("a", "b", "a", "b", "a", "b")
result <- tapply(x, groups, sum)
# tapply returns a named vector; group order is first-seen: a, b
stopifnot(names(result)[1] == "a")
stopifnot(names(result)[2] == "b")
stopifnot(result[1] == 9)
stopifnot(result[2] == 12)
"#,
    )
    .expect("tapply basic sum failed");
}

#[test]
fn tapply_mean() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- c(10, 20, 30, 40)
g <- c("x", "y", "x", "y")
result <- tapply(x, g, mean)
# first-seen order: x, y
stopifnot(result[1] == 20)
stopifnot(result[2] == 30)
"#,
    )
    .expect("tapply mean failed");
}

#[test]
fn tapply_preserves_group_order() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- c(1, 2, 3, 4, 5, 6)
groups <- c("b", "a", "b", "a", "b", "a")
result <- tapply(x, groups, sum)
# first-seen order: b then a
stopifnot(names(result)[1] == "b")
stopifnot(names(result)[2] == "a")
stopifnot(result[1] == 9)
stopifnot(result[2] == 12)
"#,
    )
    .expect("tapply group order failed");
}

// endregion

// region: by

#[test]
fn by_vector() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- c(1, 2, 3, 4)
g <- c("a", "b", "a", "b")
result <- by(x, g, sum)
# by returns a named list; access by position
stopifnot(result[[1]] == 4)
stopifnot(result[[2]] == 6)
"#,
    )
    .expect("by vector failed");
}

#[test]
fn by_dataframe() {
    let mut s = Session::new();
    s.eval_source(
        r#"
df <- data.frame(x = c(1, 2, 3, 4), g = c("a", "b", "a", "b"))
result <- by(df, df$g, function(d) sum(d$x))
# by returns a named list; access by position
stopifnot(result[[1]] == 4)
stopifnot(result[[2]] == 6)
"#,
    )
    .expect("by dataframe failed");
}

// endregion

// region: split / unsplit

#[test]
fn split_vector() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- c(1, 2, 3, 4, 5, 6)
f <- c("a", "b", "a", "b", "a", "b")
result <- split(x, f)
stopifnot(is.list(result))
stopifnot(identical(result[["a"]], c(1, 3, 5)))
stopifnot(identical(result[["b"]], c(2, 4, 6)))
"#,
    )
    .expect("split vector failed");
}

#[test]
fn split_preserves_order() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- c(10, 20, 30, 40)
f <- c("b", "a", "b", "a")
result <- split(x, f)
# first-seen order: b then a
stopifnot(names(result)[1] == "b")
stopifnot(names(result)[2] == "a")
stopifnot(identical(result[["b"]], c(10, 30)))
stopifnot(identical(result[["a"]], c(20, 40)))
"#,
    )
    .expect("split preserves order failed");
}

#[test]
fn split_dataframe() {
    let mut s = Session::new();
    s.eval_source(
        r#"
df <- data.frame(x = c(1, 2, 3, 4), g = c("a", "b", "a", "b"))
result <- split(df, df$g)
stopifnot(is.list(result))
stopifnot(is.data.frame(result[["a"]]))
stopifnot(nrow(result[["a"]]) == 2)
stopifnot(identical(result[["a"]]$x, c(1, 3)))
"#,
    )
    .expect("split dataframe failed");
}

#[test]
fn unsplit_vector() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- c(1, 2, 3, 4, 5, 6)
f <- c("a", "b", "a", "b", "a", "b")
s <- split(x, f)
result <- unsplit(s, f)
stopifnot(identical(result, x))
"#,
    )
    .expect("unsplit vector failed");
}

#[test]
fn split_unsplit_roundtrip() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- c(10, 20, 30, 40, 50)
f <- c("x", "y", "x", "y", "x")
parts <- split(x, f)
restored <- unsplit(parts, f)
stopifnot(identical(restored, x))
"#,
    )
    .expect("split/unsplit roundtrip failed");
}

// endregion

// region: aggregate

#[test]
fn aggregate_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- c(1, 2, 3, 4, 5, 6)
by_list <- list(g = c("a", "b", "a", "b", "a", "b"))
result <- aggregate(x, by_list, sum)
stopifnot(is.data.frame(result))
# Check the grouping column
stopifnot("g" %in% names(result))
stopifnot("x" %in% names(result))
"#,
    )
    .expect("aggregate basic failed");
}

#[test]
fn aggregate_multiple_groups() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- c(1, 2, 3, 4)
by_list <- list(g1 = c("a", "a", "b", "b"), g2 = c("x", "y", "x", "y"))
result <- aggregate(x, by_list, sum)
stopifnot(is.data.frame(result))
stopifnot(nrow(result) == 4)
"#,
    )
    .expect("aggregate multiple groups failed");
}

#[test]
fn aggregate_single_vector_group() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- c(10, 20, 30, 40)
g <- c("a", "b", "a", "b")
result <- aggregate(x, list(group = g), mean)
stopifnot(is.data.frame(result))
"#,
    )
    .expect("aggregate single vector group failed");
}

// endregion

// region: outer

#[test]
fn outer_default_multiply() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- outer(1:3, 1:2)
stopifnot(is.matrix(result))
stopifnot(nrow(result) == 3)
stopifnot(ncol(result) == 2)
stopifnot(result[1, 1] == 1)
stopifnot(result[2, 1] == 2)
stopifnot(result[3, 1] == 3)
stopifnot(result[1, 2] == 2)
stopifnot(result[2, 2] == 4)
stopifnot(result[3, 2] == 6)
"#,
    )
    .expect("outer default multiply failed");
}

#[test]
fn outer_with_add() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- outer(1:3, 10:11, "+")
stopifnot(result[1, 1] == 11)
stopifnot(result[2, 1] == 12)
stopifnot(result[3, 1] == 13)
stopifnot(result[1, 2] == 12)
stopifnot(result[2, 2] == 13)
stopifnot(result[3, 2] == 14)
"#,
    )
    .expect("outer with add failed");
}

#[test]
fn outer_with_custom_fun() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- outer(1:3, 1:2, function(x, y) x^2 + y)
stopifnot(result[1, 1] == 2)   # 1^2 + 1
stopifnot(result[2, 1] == 5)   # 2^2 + 1
stopifnot(result[3, 1] == 10)  # 3^2 + 1
stopifnot(result[1, 2] == 3)   # 1^2 + 2
stopifnot(result[2, 2] == 6)   # 2^2 + 2
stopifnot(result[3, 2] == 11)  # 3^2 + 2
"#,
    )
    .expect("outer with custom fun failed");
}

#[test]
fn outer_with_paste() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- outer(c("a", "b"), c("x", "y"), paste)
stopifnot(is.matrix(result))
stopifnot(result[1, 1] == "a x")
stopifnot(result[2, 1] == "b x")
stopifnot(result[1, 2] == "a y")
stopifnot(result[2, 2] == "b y")
"#,
    )
    .expect("outer with paste failed");
}

// endregion

// region: Vectorize

#[test]
fn vectorize_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
f <- function(x) x^2
vf <- Vectorize(f)
result <- vf(c(1, 2, 3))
stopifnot(identical(result, c(1, 4, 9)))
"#,
    )
    .expect("vectorize basic failed");
}

#[test]
fn vectorize_two_args() {
    let mut s = Session::new();
    s.eval_source(
        r#"
f <- function(x, y) x + y
vf <- Vectorize(f)
result <- vf(c(1, 2, 3), c(10, 20, 30))
stopifnot(identical(result, c(11, 22, 33)))
"#,
    )
    .expect("vectorize two args failed");
}

#[test]
fn vectorize_simplify_false() {
    let mut s = Session::new();
    s.eval_source(
        r#"
f <- function(x) x * 2
vf <- Vectorize(f, SIMPLIFY = FALSE)
result <- vf(c(1, 2, 3))
stopifnot(is.list(result))
stopifnot(result[[1]] == 2)
stopifnot(result[[2]] == 4)
stopifnot(result[[3]] == 6)
"#,
    )
    .expect("vectorize simplify false failed");
}

// endregion
