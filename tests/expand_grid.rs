use r::Session;

#[test]
fn expand_grid_basic_two_vectors() {
    let mut s = Session::new();
    s.eval_source(
        r#"
g <- expand.grid(x = 1:3, y = c("a", "b"))
stopifnot(is.data.frame(g))
stopifnot(nrow(g) == 6)
stopifnot(ncol(g) == 2)
stopifnot(identical(names(g), c("x", "y")))

# First factor varies fastest
stopifnot(identical(g$x, c(1L, 2L, 3L, 1L, 2L, 3L)))
stopifnot(identical(g$y, c("a", "a", "a", "b", "b", "b")))
"#,
    )
    .expect("expand.grid basic two vectors failed");
}

#[test]
fn expand_grid_three_vectors() {
    let mut s = Session::new();
    s.eval_source(
        r#"
g <- expand.grid(a = 1:2, b = 3:4, c = 5:6)
stopifnot(nrow(g) == 8)
stopifnot(ncol(g) == 3)
stopifnot(identical(names(g), c("a", "b", "c")))

# First factor (a) varies fastest, last (c) slowest
stopifnot(identical(g$a, c(1L, 2L, 1L, 2L, 1L, 2L, 1L, 2L)))
stopifnot(identical(g$b, c(3L, 3L, 4L, 4L, 3L, 3L, 4L, 4L)))
stopifnot(identical(g$c, c(5L, 5L, 5L, 5L, 6L, 6L, 6L, 6L)))
"#,
    )
    .expect("expand.grid three vectors failed");
}

#[test]
fn expand_grid_single_vector() {
    let mut s = Session::new();
    s.eval_source(
        r#"
g <- expand.grid(x = 1:4)
stopifnot(is.data.frame(g))
stopifnot(nrow(g) == 4)
stopifnot(ncol(g) == 1)
stopifnot(identical(g$x, 1:4))
"#,
    )
    .expect("expand.grid single vector failed");
}

#[test]
fn expand_grid_unnamed_args() {
    let mut s = Session::new();
    s.eval_source(
        r#"
g <- expand.grid(1:2, c("a", "b"))
stopifnot(is.data.frame(g))
stopifnot(nrow(g) == 4)
# Unnamed args get default names Var1, Var2, ...
stopifnot(identical(names(g), c("Var1", "Var2")))
"#,
    )
    .expect("expand.grid unnamed args failed");
}

#[test]
fn expand_grid_empty_input() {
    let mut s = Session::new();
    s.eval_source(
        r#"
g <- expand.grid()
stopifnot(is.data.frame(g))
stopifnot(nrow(g) == 0)
stopifnot(ncol(g) == 0)
"#,
    )
    .expect("expand.grid empty input failed");
}

#[test]
fn expand_grid_logical_vectors() {
    let mut s = Session::new();
    s.eval_source(
        r#"
g <- expand.grid(x = c(TRUE, FALSE), y = c(TRUE, FALSE))
stopifnot(nrow(g) == 4)
stopifnot(is.logical(g$x))
stopifnot(is.logical(g$y))
"#,
    )
    .expect("expand.grid logical vectors failed");
}

#[test]
fn expand_grid_double_vectors() {
    let mut s = Session::new();
    s.eval_source(
        r#"
g <- expand.grid(x = c(1.5, 2.5), y = c(10.0, 20.0))
stopifnot(nrow(g) == 4)
stopifnot(is.double(g$x))
stopifnot(identical(g$x, c(1.5, 2.5, 1.5, 2.5)))
stopifnot(identical(g$y, c(10, 10, 20, 20)))
"#,
    )
    .expect("expand.grid double vectors failed");
}

#[test]
fn expand_grid_row_names() {
    let mut s = Session::new();
    s.eval_source(
        r#"
g <- expand.grid(x = 1:2, y = 1:3)
rn <- attr(g, "row.names")
stopifnot(identical(rn, 1:6))
"#,
    )
    .expect("expand.grid row names failed");
}
