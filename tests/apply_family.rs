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

// region: vapply

#[test]
fn vapply_basic_double() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- vapply(1:5, function(x) x * 2.0, numeric(1))
stopifnot(is.double(result))
stopifnot(identical(result, c(2, 4, 6, 8, 10)))
"#,
    )
    .expect("vapply basic double failed");
}

#[test]
fn vapply_character() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- vapply(c("hello", "world"), toupper, character(1))
stopifnot(is.character(result))
stopifnot(identical(result, c("HELLO", "WORLD")))
"#,
    )
    .expect("vapply character failed");
}

#[test]
fn vapply_logical() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- vapply(c(1, -2, 3, -4), function(x) x > 0, logical(1))
stopifnot(is.logical(result))
stopifnot(identical(result, c(TRUE, FALSE, TRUE, FALSE)))
"#,
    )
    .expect("vapply logical failed");
}

#[test]
fn vapply_type_mismatch_errors() {
    let mut s = Session::new();
    let result = s.eval_source(
        r#"
# This should error: function returns character but FUN.VALUE is numeric
vapply(c("a", "b"), function(x) x, numeric(1))
"#,
    );
    assert!(result.is_err(), "vapply should error on type mismatch");
}

#[test]
fn vapply_length_mismatch_errors() {
    let mut s = Session::new();
    let result = s.eval_source(
        r#"
# This should error: function returns length-2 but FUN.VALUE is length-1
vapply(1:3, function(x) c(x, x), numeric(1))
"#,
    );
    assert!(result.is_err(), "vapply should error on length mismatch");
}

#[test]
fn vapply_empty_input() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- vapply(integer(0), function(x) x * 2.0, numeric(1))
stopifnot(is.double(result))
stopifnot(length(result) == 0)
"#,
    )
    .expect("vapply empty input failed");
}

// endregion

// region: sapply/lapply with extra args

#[test]
fn sapply_with_extra_args() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- sapply(1:3, function(x, y) x + y, 10)
stopifnot(identical(result, c(11, 12, 13)))
"#,
    )
    .expect("sapply with extra args failed");
}

#[test]
fn lapply_with_extra_args() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- lapply(1:3, function(x, y) x * y, 5)
stopifnot(is.list(result))
stopifnot(result[[1]] == 5)
stopifnot(result[[2]] == 10)
stopifnot(result[[3]] == 15)
"#,
    )
    .expect("lapply with extra args failed");
}

// endregion

// region: mapply with MoreArgs

#[test]
#[ignore = "mapply MoreArgs not yet implemented"]
fn mapply_with_more_args() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- mapply(
  function(x, y, sep) paste(x, y, sep = sep),
  c("a", "b", "c"),
  1:3,
  MoreArgs = list(sep = "-")
)
stopifnot(identical(result, c("a-1", "b-2", "c-3")))
"#,
    )
    .expect("mapply with MoreArgs failed");
}

// endregion

// region: Reduce

#[test]
fn reduce_basic_sum() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- Reduce("+", 1:5)
stopifnot(result == 15)
"#,
    )
    .expect("reduce basic sum failed");
}

#[test]
fn reduce_with_init() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- Reduce("+", 1:5, init = 100)
stopifnot(result == 115)
"#,
    )
    .expect("reduce with init failed");
}

#[test]
fn reduce_accumulate() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- Reduce("+", 1:4, accumulate = TRUE)
stopifnot(is.list(result))
stopifnot(length(result) == 4)
# Accumulate: 1, 1+2=3, 3+3=6, 6+4=10
stopifnot(result[[1]] == 1)
stopifnot(result[[2]] == 3)
stopifnot(result[[3]] == 6)
stopifnot(result[[4]] == 10)
"#,
    )
    .expect("reduce accumulate failed");
}

#[test]
fn reduce_custom_function() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- Reduce(function(a, b) a * b, 1:5)
stopifnot(result == 120)  # 5!
"#,
    )
    .expect("reduce custom function failed");
}

#[test]
fn reduce_empty_with_init() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- Reduce("+", integer(0), init = 42)
stopifnot(result == 42)
"#,
    )
    .expect("reduce empty with init failed");
}

// endregion

// region: Filter / Find / Position / Map / Negate

#[test]
fn filter_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- Filter(function(x) x > 3, 1:6)
stopifnot(identical(result, 4:6))
"#,
    )
    .expect("filter basic failed");
}

#[test]
fn filter_empty_result() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- Filter(function(x) x > 100, 1:5)
stopifnot(is.null(result))
"#,
    )
    .expect("filter empty result failed");
}

#[test]
fn find_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- Find(function(x) x > 3, 1:6)
stopifnot(result == 4)
"#,
    )
    .expect("find basic failed");
}

#[test]
fn find_from_right() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- Find(function(x) x > 3, 1:6, right = TRUE)
stopifnot(result == 6)
"#,
    )
    .expect("find from right failed");
}

#[test]
fn find_not_found() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- Find(function(x) x > 100, 1:5)
stopifnot(is.null(result))
"#,
    )
    .expect("find not found failed");
}

#[test]
fn position_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- Position(function(x) x > 3, 1:6)
stopifnot(result == 4L)
"#,
    )
    .expect("position basic failed");
}

#[test]
fn position_from_right() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- Position(function(x) x > 3, 1:6, right = TRUE)
stopifnot(result == 6L)
"#,
    )
    .expect("position from right failed");
}

#[test]
fn position_not_found() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- Position(function(x) x > 100, 1:5)
stopifnot(is.null(result))
"#,
    )
    .expect("position not found failed");
}

#[test]
fn map_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- Map(function(x, y) x + y, 1:3, 10:12)
stopifnot(is.list(result))
stopifnot(result[[1]] == 11)
stopifnot(result[[2]] == 13)
stopifnot(result[[3]] == 15)
"#,
    )
    .expect("map basic failed");
}

#[test]
#[ignore = "Map with 3+ args not yet implemented"]
fn map_three_args() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- Map(function(x, y, z) x + y + z, 1:3, 10:12, 100:102)
stopifnot(is.list(result))
stopifnot(result[[1]] == 111)
stopifnot(result[[2]] == 123)
stopifnot(result[[3]] == 135)
"#,
    )
    .expect("map three args failed");
}

#[test]
fn negate_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
is_positive <- function(x) x > 0
is_non_positive <- Negate(is_positive)
result <- Filter(is_non_positive, c(-2, -1, 0, 1, 2))
stopifnot(identical(result, c(-2, -1, 0)))
"#,
    )
    .expect("negate basic failed");
}

// endregion

// region: rapply

#[test]
fn rapply_unlist_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- list(a = 1, b = list(c = 2, d = 3))
result <- rapply(x, function(v) v * 10, how = "unlist")
# Flattened results: 10, 20, 30
stopifnot(length(result) == 3)
stopifnot(result[1] == 10)
stopifnot(result[2] == 20)
stopifnot(result[3] == 30)
"#,
    )
    .expect("rapply unlist basic failed");
}

#[test]
fn rapply_replace_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- list(a = 1, b = list(c = 2, d = 3))
result <- rapply(x, function(v) v * 10, how = "replace")
stopifnot(is.list(result))
stopifnot(result$a == 10)
stopifnot(is.list(result$b))
stopifnot(result$b$c == 20)
stopifnot(result$b$d == 30)
"#,
    )
    .expect("rapply replace basic failed");
}

#[test]
fn rapply_list_mode() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- list(a = 1, b = list(c = 2, d = 3))
result <- rapply(x, function(v) v + 100, how = "list")
stopifnot(is.list(result))
stopifnot(length(result) == 3)
stopifnot(result[[1]] == 101)
stopifnot(result[[2]] == 102)
stopifnot(result[[3]] == 103)
"#,
    )
    .expect("rapply list mode failed");
}

#[test]
fn rapply_with_classes() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- list(a = 1L, b = "hello", c = list(d = 2L, e = "world"))
# Only apply to integer elements
result <- rapply(x, function(v) v * 10, classes = "integer", how = "replace")
stopifnot(result$a == 10)
stopifnot(result$b == "hello")  # unchanged -- not integer
stopifnot(result$c$d == 20)
stopifnot(result$c$e == "world")  # unchanged -- not integer
"#,
    )
    .expect("rapply with classes failed");
}

#[test]
fn rapply_with_deflt() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- list(a = 1L, b = "hello", c = 2L)
# Only apply to integers; non-matching get deflt = NA
result <- rapply(x, function(v) v * 10, classes = "integer", deflt = NA, how = "unlist")
# result should have 3 elements: 10, NA, 20
stopifnot(length(result) == 3)
stopifnot(result[1] == 10)
stopifnot(is.na(result[2]))
stopifnot(result[3] == 20)
"#,
    )
    .expect("rapply with deflt failed");
}

// endregion
