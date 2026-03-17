use r::Session;

// region: merge

#[test]
fn merge_inner_join() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- data.frame(id = c("a", "b", "c"), val = c(1, 2, 3))
y <- data.frame(id = c("b", "c", "d"), score = c(10, 20, 30))
m <- merge(x, y)
stopifnot(
  nrow(m) == 2L,
  identical(m$id, c("b", "c")),
  identical(m$val, c(2, 3)),
  identical(m$score, c(10, 20))
)
"#,
    )
    .expect("merge inner join failed");
}

#[test]
fn merge_left_join() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- data.frame(id = c("a", "b"), val = c(1, 2))
y <- data.frame(id = c("b", "c"), score = c(10, 20))
m <- merge(x, y, all.x = TRUE)
stopifnot(
  nrow(m) == 2L,
  identical(m$id, c("a", "b")),
  identical(m$val, c(1, 2))
)
"#,
    )
    .expect("merge left join failed");
}

#[test]
fn merge_full_outer_join() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- data.frame(id = c("a", "b"), val = c(1, 2))
y <- data.frame(id = c("b", "c"), score = c(10, 20))
m <- merge(x, y, all = TRUE)
stopifnot(nrow(m) == 3L)
"#,
    )
    .expect("merge full outer join failed");
}

#[test]
fn merge_by_different_columns() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- data.frame(key_x = c("a", "b"), val = c(1, 2))
y <- data.frame(key_y = c("b", "c"), score = c(10, 20))
m <- merge(x, y, by.x = "key_x", by.y = "key_y")
stopifnot(
  nrow(m) == 1L,
  identical(m$val, 2),
  identical(m$score, 10)
)
"#,
    )
    .expect("merge by different columns failed");
}

// endregion

// region: subset

#[test]
fn subset_rows_by_condition() {
    let mut s = Session::new();
    s.eval_source(
        r#"
df <- data.frame(x = c(1, 2, 3, 4, 5), y = c("a", "b", "c", "d", "e"))
s <- subset(df, x > 3)
stopifnot(
  nrow(s) == 2L,
  identical(s$x, c(4, 5)),
  identical(s$y, c("d", "e"))
)
"#,
    )
    .expect("subset rows failed");
}

#[test]
fn subset_with_column_selection() {
    let mut s = Session::new();
    s.eval_source(
        r#"
df <- data.frame(x = c(1, 2, 3), y = c(4, 5, 6), z = c(7, 8, 9))
s <- subset(df, x > 1, select = c(x, z))
stopifnot(
  nrow(s) == 2L,
  identical(names(s), c("x", "z")),
  identical(s$x, c(2, 3)),
  identical(s$z, c(8, 9))
)
"#,
    )
    .expect("subset with select failed");
}

#[test]
fn subset_negative_column_selection() {
    let mut s = Session::new();
    s.eval_source(
        r#"
df <- data.frame(x = c(1, 2), y = c(3, 4), z = c(5, 6))
s <- subset(df, select = -y)
stopifnot(
  identical(names(s), c("x", "z"))
)
"#,
    )
    .expect("subset negative select failed");
}

// endregion

// region: transform

#[test]
fn transform_add_column() {
    let mut s = Session::new();
    s.eval_source(
        r#"
df <- data.frame(x = c(1, 2, 3))
t <- transform(df, y = x * 2)
stopifnot(
  identical(names(t), c("x", "y")),
  identical(t$y, c(2, 4, 6))
)
"#,
    )
    .expect("transform add column failed");
}

#[test]
fn transform_modify_column() {
    let mut s = Session::new();
    s.eval_source(
        r#"
df <- data.frame(x = c(1, 2, 3), y = c(10, 20, 30))
t <- transform(df, y = y + 1)
stopifnot(
  identical(t$y, c(11, 21, 31)),
  identical(t$x, c(1, 2, 3))
)
"#,
    )
    .expect("transform modify column failed");
}

// endregion

// region: with

#[test]
fn with_evaluates_in_df_context() {
    let mut s = Session::new();
    s.eval_source(
        r#"
df <- data.frame(x = c(1, 2, 3), y = c(10, 20, 30))
result <- with(df, x + y)
stopifnot(identical(result, c(11, 22, 33)))
"#,
    )
    .expect("with() failed");
}

#[test]
fn with_complex_expression() {
    let mut s = Session::new();
    s.eval_source(
        r#"
df <- data.frame(a = c(1, 2, 3), b = c(4, 5, 6))
result <- with(df, sum(a) + sum(b))
stopifnot(result == 21)
"#,
    )
    .expect("with() complex expression failed");
}

// endregion

// region: rbind/cbind for data frames

#[test]
fn rbind_data_frames() {
    let mut s = Session::new();
    s.eval_source(
        r#"
df1 <- data.frame(x = c(1, 2), y = c("a", "b"))
df2 <- data.frame(x = c(3, 4), y = c("c", "d"))
r <- rbind(df1, df2)
stopifnot(
  nrow(r) == 4L,
  identical(r$x, c(1, 2, 3, 4)),
  identical(r$y, c("a", "b", "c", "d"))
)
"#,
    )
    .expect("rbind data frames failed");
}

#[test]
fn cbind_data_frames() {
    let mut s = Session::new();
    s.eval_source(
        r#"
df1 <- data.frame(x = c(1, 2, 3))
df2 <- data.frame(y = c(4, 5, 6))
c <- cbind(df1, df2)
stopifnot(
  nrow(c) == 3L,
  identical(names(c), c("x", "y")),
  identical(c$x, c(1, 2, 3)),
  identical(c$y, c(4, 5, 6))
)
"#,
    )
    .expect("cbind data frames failed");
}

// endregion

// region: head/tail for data frames

#[test]
#[ignore = "needs head/tail/order data frame enhancements"]
fn head_data_frame() {
    let mut s = Session::new();
    s.eval_source(
        r#"
df <- data.frame(x = 1:10, y = 11:20)
h <- head(df, 3)
stopifnot(
  nrow(h) == 3L,
  identical(h$x, 1:3),
  identical(h$y, 11:13)
)
"#,
    )
    .expect("head data frame failed");
}

#[test]
#[ignore = "needs head/tail/order data frame enhancements"]
fn tail_data_frame() {
    let mut s = Session::new();
    s.eval_source(
        r#"
df <- data.frame(x = 1:10, y = 11:20)
t <- tail(df, 3)
stopifnot(
  nrow(t) == 3L,
  identical(t$x, 8:10),
  identical(t$y, 18:20)
)
"#,
    )
    .expect("tail data frame failed");
}

#[test]
#[ignore = "needs head/tail/order data frame enhancements"]
fn head_tail_default_n() {
    let mut s = Session::new();
    s.eval_source(
        r#"
df <- data.frame(x = 1:10)
h <- head(df)
t <- tail(df)
stopifnot(
  nrow(h) == 6L,
  nrow(t) == 6L,
  identical(h$x, 1:6),
  identical(t$x, 5:10)
)
"#,
    )
    .expect("head/tail default n failed");
}

#[test]
#[ignore = "needs head/tail/order data frame enhancements"]
fn head_tail_list() {
    let mut s = Session::new();
    s.eval_source(
        r#"
l <- list(a = 1, b = 2, c = 3, d = 4, e = 5)
h <- head(l, 2)
t <- tail(l, 2)
stopifnot(
  length(h) == 2L,
  length(t) == 2L
)
"#,
    )
    .expect("head/tail list failed");
}

// endregion

// region: order

#[test]
fn order_numeric() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- c(3, 1, 4, 1, 5, 9)
o <- order(x)
stopifnot(identical(o, c(2L, 4L, 1L, 3L, 5L, 6L)))
"#,
    )
    .expect("order numeric failed");
}

#[test]
#[ignore = "needs head/tail/order data frame enhancements"]
fn order_decreasing() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- c(3, 1, 4, 1, 5)
o <- order(x, decreasing = TRUE)
stopifnot(identical(o, c(5L, 3L, 1L, 2L, 4L)))
"#,
    )
    .expect("order decreasing failed");
}

#[test]
#[ignore = "needs head/tail/order data frame enhancements"]
fn order_character() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- c("banana", "apple", "cherry")
o <- order(x)
stopifnot(identical(o, c(2L, 1L, 3L)))
"#,
    )
    .expect("order character failed");
}

#[test]
#[ignore = "needs head/tail/order data frame enhancements"]
fn order_multiple_keys() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- c(1, 1, 2, 2)
y <- c(4, 3, 2, 1)
o <- order(x, y)
stopifnot(identical(o, c(2L, 1L, 4L, 3L)))
"#,
    )
    .expect("order multiple keys failed");
}

// endregion
