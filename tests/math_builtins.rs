use r::Session;

// region: quantile

#[test]
fn quantile_default_probs() {
    let mut s = Session::new();
    s.eval_source(
        r#"
q <- quantile(c(1, 2, 3, 4, 5))
stopifnot(length(q) == 5)
stopifnot(q[1] == 1)    # 0%
stopifnot(q[2] == 2)    # 25%
stopifnot(q[3] == 3)    # 50%
stopifnot(q[4] == 4)    # 75%
stopifnot(q[5] == 5)    # 100%

# Check names
n <- names(q)
stopifnot(n[1] == "0%")
stopifnot(n[2] == "25%")
stopifnot(n[3] == "50%")
stopifnot(n[4] == "75%")
stopifnot(n[5] == "100%")
"#,
    )
    .unwrap();
}

#[test]
fn quantile_custom_probs() {
    let mut s = Session::new();
    s.eval_source(
        r#"
q <- quantile(1:10, probs = c(0.1, 0.5, 0.9))
stopifnot(length(q) == 3)
# Type 7: h = (n-1)*p
# n=10, p=0.1: h=0.9, floor=0, ceil=1: 1 + 0.9*(2-1) = 1.9
stopifnot(abs(q[1] - 1.9) < 1e-10)
# p=0.5: h=4.5, floor=4, ceil=5: 5 + 0.5*(6-5) = 5.5
stopifnot(abs(q[2] - 5.5) < 1e-10)
# p=0.9: h=8.1, floor=8, ceil=9: 9 + 0.1*(10-9) = 9.1
stopifnot(abs(q[3] - 9.1) < 1e-10)
"#,
    )
    .unwrap();
}

#[test]
fn quantile_single_value() {
    let mut s = Session::new();
    s.eval_source(
        r#"
q <- quantile(42)
stopifnot(q[1] == 42)
stopifnot(q[3] == 42)
stopifnot(q[5] == 42)
"#,
    )
    .unwrap();
}

#[test]
fn quantile_na_rm() {
    let mut s = Session::new();
    // Without na.rm should error
    let result = s.eval_source("quantile(c(1, NA, 3))");
    assert!(result.is_err());

    // With na.rm=TRUE should work
    s.eval_source(
        r#"
q <- quantile(c(1, NA, 3), na.rm = TRUE)
stopifnot(q[1] == 1)  # 0%: min
stopifnot(q[5] == 3)  # 100%: max
"#,
    )
    .unwrap();
}

#[test]
fn quantile_probs_out_of_range() {
    let mut s = Session::new();
    let result = s.eval_source("quantile(1:5, probs = c(-0.1))");
    assert!(result.is_err());
    let result = s.eval_source("quantile(1:5, probs = c(1.5))");
    assert!(result.is_err());
}

// endregion

// region: rep with each and length.out

#[test]
fn rep_each() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- rep(1:3, each = 2)
stopifnot(identical(x, c(1L, 1L, 2L, 2L, 3L, 3L)))
"#,
    )
    .unwrap();
}

#[test]
fn rep_each_with_times() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# each=2 first, then times=3 repeats the whole thing
x <- rep(1:2, each = 2, times = 3)
stopifnot(length(x) == 12)
stopifnot(identical(x, c(1L, 1L, 2L, 2L, 1L, 1L, 2L, 2L, 1L, 1L, 2L, 2L)))
"#,
    )
    .unwrap();
}

#[test]
fn rep_length_out() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Truncate
x <- rep(1:5, length.out = 3)
stopifnot(identical(x, 1:3))

# Extend by cycling
y <- rep(1:3, length.out = 7)
stopifnot(identical(y, c(1L, 2L, 3L, 1L, 2L, 3L, 1L)))
"#,
    )
    .unwrap();
}

#[test]
fn rep_each_and_length_out() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- rep(1:3, each = 2, length.out = 4)
stopifnot(identical(x, c(1L, 1L, 2L, 2L)))
"#,
    )
    .unwrap();
}

#[test]
fn rep_times_still_works() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Original times behavior should still work
x <- rep(c(1, 2, 3), times = 2)
stopifnot(identical(x, c(1, 2, 3, 1, 2, 3)))

# Positional arg for times
y <- rep(c(1, 2), 3)
stopifnot(identical(y, c(1, 2, 1, 2, 1, 2)))
"#,
    )
    .unwrap();
}

#[test]
fn rep_character_each() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- rep(c("a", "b"), each = 3)
stopifnot(identical(x, c("a", "a", "a", "b", "b", "b")))
"#,
    )
    .unwrap();
}

#[test]
fn rep_logical_each() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- rep(c(TRUE, FALSE), each = 2)
stopifnot(identical(x, c(TRUE, TRUE, FALSE, FALSE)))
"#,
    )
    .unwrap();
}

// endregion

// region: sort with na.last

#[test]
fn sort_na_last_true() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- sort(c(3, NA, 1, NA, 2), na.last = TRUE)
stopifnot(length(x) == 5)
stopifnot(x[1] == 1)
stopifnot(x[2] == 2)
stopifnot(x[3] == 3)
stopifnot(is.na(x[4]))
stopifnot(is.na(x[5]))
"#,
    )
    .unwrap();
}

#[test]
fn sort_na_last_false() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- sort(c(3, NA, 1, NA, 2), na.last = FALSE)
stopifnot(length(x) == 5)
stopifnot(is.na(x[1]))
stopifnot(is.na(x[2]))
stopifnot(x[3] == 1)
stopifnot(x[4] == 2)
stopifnot(x[5] == 3)
"#,
    )
    .unwrap();
}

#[test]
fn sort_na_last_na_removes() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Default behavior: na.last=NA removes NAs
x <- sort(c(3, NA, 1, NA, 2))
stopifnot(length(x) == 3)
stopifnot(x[1] == 1)
stopifnot(x[2] == 2)
stopifnot(x[3] == 3)
"#,
    )
    .unwrap();
}

#[test]
fn sort_decreasing_with_na() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- sort(c(3, NA, 1, 2), decreasing = TRUE, na.last = TRUE)
stopifnot(x[1] == 3)
stopifnot(x[2] == 2)
stopifnot(x[3] == 1)
stopifnot(is.na(x[4]))
"#,
    )
    .unwrap();
}

#[test]
fn sort_integer_na_last() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- sort(c(3L, NA, 1L, 2L), na.last = TRUE)
stopifnot(x[1] == 1L)
stopifnot(x[2] == 2L)
stopifnot(x[3] == 3L)
stopifnot(is.na(x[4]))
"#,
    )
    .unwrap();
}

#[test]
fn sort_character_na_last() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- sort(c("c", NA, "a", "b"), na.last = TRUE)
stopifnot(x[1] == "a")
stopifnot(x[2] == "b")
stopifnot(x[3] == "c")
stopifnot(is.na(x[4]))
"#,
    )
    .unwrap();
}

// endregion

// region: diff with differences

#[test]
fn diff_differences_2() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# diff(1:5) = c(1, 1, 1, 1)
# diff(diff(1:5)) = c(0, 0, 0)
x <- diff(1:5, differences = 2)
stopifnot(length(x) == 3)
stopifnot(x[1] == 0)
stopifnot(x[2] == 0)
stopifnot(x[3] == 0)
"#,
    )
    .unwrap();
}

#[test]
fn diff_differences_3() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# diff(c(1, 4, 9, 16, 25)) = c(3, 5, 7, 9)
# diff(c(3, 5, 7, 9)) = c(2, 2, 2)
# diff(c(2, 2, 2)) = c(0, 0)
x <- diff(c(1, 4, 9, 16, 25), differences = 3)
stopifnot(length(x) == 2)
stopifnot(x[1] == 0)
stopifnot(x[2] == 0)
"#,
    )
    .unwrap();
}

#[test]
fn diff_default_differences() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Default differences=1 should still work
x <- diff(c(1, 3, 6, 10))
stopifnot(length(x) == 3)
stopifnot(x[1] == 2)
stopifnot(x[2] == 3)
stopifnot(x[3] == 4)
"#,
    )
    .unwrap();
}

#[test]
fn diff_lag_and_differences() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# lag=2, differences=1: x[i] - x[i-2]
x <- diff(1:6, lag = 2)
stopifnot(length(x) == 4)
stopifnot(x[1] == 2)
stopifnot(x[2] == 2)

# lag=2, differences=2: apply lag-2 diff twice
y <- diff(1:8, lag = 2, differences = 2)
stopifnot(length(y) == 4)
stopifnot(y[1] == 0)
"#,
    )
    .unwrap();
}

#[test]
fn diff_differences_zero_error() {
    let mut s = Session::new();
    let result = s.eval_source("diff(1:5, differences = 0)");
    assert!(result.is_err());
}

#[test]
fn diff_too_many_differences_returns_empty() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- diff(1:3, differences = 10)
stopifnot(length(x) == 0)
"#,
    )
    .unwrap();
}

// endregion

// region: ordered

#[test]
fn ordered_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- ordered(c("low", "medium", "high", "low"), levels = c("low", "medium", "high"))
stopifnot(is.ordered(x))
stopifnot(is.factor(x))
lv <- levels(x)
stopifnot(lv[1] == "low")
stopifnot(lv[2] == "medium")
stopifnot(lv[3] == "high")
stopifnot(nlevels(x) == 3)
"#,
    )
    .unwrap();
}

#[test]
fn ordered_with_labels() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- ordered(c("a", "b", "c"), levels = c("a", "b", "c"), labels = c("Alpha", "Beta", "Gamma"))
stopifnot(is.ordered(x))
lv <- levels(x)
stopifnot(lv[1] == "Alpha")
stopifnot(lv[2] == "Beta")
stopifnot(lv[3] == "Gamma")
"#,
    )
    .unwrap();
}

#[test]
fn ordered_class_attribute() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- ordered(c("a", "b"))
cls <- class(x)
stopifnot(length(cls) == 2)
stopifnot(cls[1] == "ordered")
stopifnot(cls[2] == "factor")
"#,
    )
    .unwrap();
}

#[test]
fn ordered_no_explicit_levels() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Without explicit levels, should use sorted unique values
x <- ordered(c("b", "a", "c", "a"))
stopifnot(is.ordered(x))
lv <- levels(x)
stopifnot(lv[1] == "a")
stopifnot(lv[2] == "b")
stopifnot(lv[3] == "c")
"#,
    )
    .unwrap();
}

// endregion
