use r::Session;

// region: sign(0) returns 0, not 1

#[test]
fn sign_zero_double() {
    let mut s = Session::new();
    s.eval_source(
        r#"
stopifnot(sign(0) == 0)
stopifnot(sign(0.0) == 0)
"#,
    )
    .unwrap();
}

#[test]
fn sign_zero_integer() {
    let mut s = Session::new();
    s.eval_source(
        r#"
stopifnot(sign(0L) == 0L)
stopifnot(is.integer(sign(0L)))
"#,
    )
    .unwrap();
}

#[test]
fn sign_positive_negative() {
    let mut s = Session::new();
    s.eval_source(
        r#"
stopifnot(sign(5) == 1)
stopifnot(sign(-5) == -1)
stopifnot(sign(5L) == 1L)
stopifnot(sign(-5L) == -1L)
stopifnot(is.integer(sign(5L)))
"#,
    )
    .unwrap();
}

#[test]
fn sign_vector() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- sign(c(-3, -1, 0, 1, 3))
stopifnot(length(x) == 5)
stopifnot(x[1] == -1)
stopifnot(x[2] == -1)
stopifnot(x[3] == 0)
stopifnot(x[4] == 1)
stopifnot(x[5] == 1)
"#,
    )
    .unwrap();
}

#[test]
fn sign_nan() {
    let mut s = Session::new();
    s.eval_source(
        r#"
stopifnot(is.nan(sign(NaN)))
"#,
    )
    .unwrap();
}

// endregion

// region: match.arg() returns first choice with default

#[test]
fn match_arg_default_returns_first() {
    let mut s = Session::new();
    s.eval_source(
        r#"
f <- function(method = c("exact", "partial", "none")) {
    match.arg(method)
}
stopifnot(f() == "exact")
"#,
    )
    .unwrap();
}

#[test]
fn match_arg_explicit_value() {
    let mut s = Session::new();
    s.eval_source(
        r#"
f <- function(method = c("exact", "partial", "none")) {
    match.arg(method)
}
stopifnot(f("partial") == "partial")
stopifnot(f("none") == "none")
"#,
    )
    .unwrap();
}

#[test]
fn match_arg_with_explicit_choices() {
    let mut s = Session::new();
    s.eval_source(
        r#"
stopifnot(match.arg("b", c("a", "b", "c")) == "b")
stopifnot(match.arg(NULL, c("a", "b", "c")) == "a")
"#,
    )
    .unwrap();
}

#[test]
fn match_arg_partial_match() {
    let mut s = Session::new();
    s.eval_source(
        r#"
stopifnot(match.arg("ex", c("exact", "partial", "none")) == "exact")
stopifnot(match.arg("p", c("exact", "partial", "none")) == "partial")
"#,
    )
    .unwrap();
}

// endregion

// region: rm(x) supports bare symbol names (NSE)

#[test]
fn rm_bare_symbol() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- 42
stopifnot(exists("x"))
rm(x)
stopifnot(!exists("x"))
"#,
    )
    .unwrap();
}

#[test]
fn rm_multiple_bare_symbols() {
    let mut s = Session::new();
    s.eval_source(
        r#"
a <- 1
b <- 2
c_val <- 3
rm(a, b, c_val)
stopifnot(!exists("a"))
stopifnot(!exists("b"))
stopifnot(!exists("c_val"))
"#,
    )
    .unwrap();
}

#[test]
fn rm_string_still_works() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- 42
rm("x")
stopifnot(!exists("x"))
"#,
    )
    .unwrap();
}

#[test]
fn rm_list_argument() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- 1
y <- 2
rm(list = c("x", "y"))
stopifnot(!exists("x"))
stopifnot(!exists("y"))
"#,
    )
    .unwrap();
}

// endregion

// region: format(digits=) respects digits parameter

#[test]
fn format_digits_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# format with 3 significant digits
x <- format(3.14159, digits = 3)
stopifnot(x == "3.14")
"#,
    )
    .unwrap();
}

#[test]
fn format_digits_large_number() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- format(1234.5678, digits = 4)
stopifnot(x == "1235")
"#,
    )
    .unwrap();
}

#[test]
fn format_digits_small_number() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- format(0.001234, digits = 3)
stopifnot(x == "0.00123")
"#,
    )
    .unwrap();
}

// endregion

// region: aggregate() formula interface

#[test]
fn aggregate_formula_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
df <- data.frame(
    y = c(1, 2, 3, 4, 5, 6),
    x = c("a", "b", "a", "b", "a", "b")
)
result <- aggregate(y ~ x, data = df, FUN = mean)
stopifnot(is.data.frame(result))
stopifnot("x" %in% names(result))
stopifnot("y" %in% names(result))
# Group "a" has values 1, 3, 5 -> mean = 3
# Group "b" has values 2, 4, 6 -> mean = 4
a_row <- result$y[result$x == "a"]
b_row <- result$y[result$x == "b"]
stopifnot(a_row == 3)
stopifnot(b_row == 4)
"#,
    )
    .unwrap();
}

#[test]
fn aggregate_formula_sum() {
    let mut s = Session::new();
    s.eval_source(
        r#"
df <- data.frame(
    value = c(10, 20, 30, 40),
    group = c("x", "y", "x", "y")
)
result <- aggregate(value ~ group, data = df, FUN = sum)
stopifnot(is.data.frame(result))
x_sum <- result$value[result$group == "x"]
y_sum <- result$value[result$group == "y"]
stopifnot(x_sum == 40)
stopifnot(y_sum == 60)
"#,
    )
    .unwrap();
}

#[test]
fn aggregate_standard_still_works() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- c(1, 2, 3, 4, 5, 6)
by <- list(group = c("a", "b", "a", "b", "a", "b"))
result <- aggregate(x, by, mean)
stopifnot(is.data.frame(result))
"#,
    )
    .unwrap();
}

// endregion
