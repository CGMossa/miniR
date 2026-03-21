use r::Session;

// region: ifelse vectorized

#[test]
fn ifelse_vectorized_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Element-wise selection from yes/no vectors
x <- ifelse(c(TRUE, FALSE, TRUE), 1:3, 4:6)
stopifnot(identical(x, c(1L, 5L, 3L)))
"#,
    )
    .unwrap();
}

#[test]
fn ifelse_na_propagation() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# NA in condition produces NA in result
x <- ifelse(c(TRUE, NA, FALSE), 10, 20)
stopifnot(x[1] == 10)
stopifnot(is.na(x[2]))
stopifnot(x[3] == 20)
"#,
    )
    .unwrap();
}

#[test]
fn ifelse_recycling() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# yes/no shorter than condition: recycled
x <- ifelse(c(TRUE, FALSE, TRUE, FALSE), c(1, 2), c(10, 20))
stopifnot(identical(x, c(1, 20, 1, 20)))
"#,
    )
    .unwrap();
}

#[test]
fn ifelse_character_vectors() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- ifelse(c(TRUE, FALSE, TRUE), c("a", "b", "c"), c("x", "y", "z"))
stopifnot(identical(x, c("a", "y", "c")))
"#,
    )
    .unwrap();
}

#[test]
fn ifelse_scalar_yes_no() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Scalar yes/no recycled across all elements
x <- ifelse(c(TRUE, FALSE, TRUE, FALSE), "yes", "no")
stopifnot(identical(x, c("yes", "no", "yes", "no")))
"#,
    )
    .unwrap();
}

#[test]
fn ifelse_all_na_condition() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- ifelse(c(NA, NA, NA), 1, 2)
stopifnot(length(x) == 3)
stopifnot(is.na(x[1]))
stopifnot(is.na(x[2]))
stopifnot(is.na(x[3]))
"#,
    )
    .unwrap();
}

#[test]
fn ifelse_logical_result_type() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# When both yes and no are logical, result should be logical
x <- ifelse(c(TRUE, FALSE), TRUE, FALSE)
stopifnot(is.logical(x))
stopifnot(identical(x, c(TRUE, FALSE)))
"#,
    )
    .unwrap();
}

#[test]
fn ifelse_integer_result_type() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# When both yes and no are integer, result should be integer
x <- ifelse(c(TRUE, FALSE), 1L, 2L)
stopifnot(is.integer(x))
stopifnot(identical(x, c(1L, 2L)))
"#,
    )
    .unwrap();
}

// endregion

// region: round half-to-even (banker's rounding)

#[test]
fn round_half_to_even_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# IEEE 754 round-half-to-even: .5 rounds to nearest even integer
stopifnot(round(0.5) == 0)    # 0 is even
stopifnot(round(1.5) == 2)    # 2 is even
stopifnot(round(2.5) == 2)    # 2 is even
stopifnot(round(3.5) == 4)    # 4 is even
stopifnot(round(4.5) == 4)    # 4 is even
stopifnot(round(5.5) == 6)    # 6 is even
"#,
    )
    .unwrap();
}

#[test]
fn round_half_to_even_negative() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Negative half-values also round to even
stopifnot(round(-0.5) == 0)   # 0 is even
stopifnot(round(-1.5) == -2)  # -2 is even
stopifnot(round(-2.5) == -2)  # -2 is even
stopifnot(round(-3.5) == -4)  # -4 is even
stopifnot(round(-4.5) == -4)  # -4 is even
"#,
    )
    .unwrap();
}

#[test]
fn round_non_half_values() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Non-halfway values round normally
stopifnot(round(0.3) == 0)
stopifnot(round(0.7) == 1)
stopifnot(round(1.2) == 1)
stopifnot(round(1.8) == 2)
stopifnot(round(-0.3) == 0)
stopifnot(round(-0.7) == -1)
"#,
    )
    .unwrap();
}

#[test]
fn round_with_digits() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Round to 1 decimal place
stopifnot(round(0.15, 1) == 0.2)
stopifnot(round(0.25, 1) == 0.2)  # half-to-even
stopifnot(round(0.35, 1) == 0.4)
stopifnot(round(0.45, 1) == 0.4)  # half-to-even

# Round to 2 decimal places
stopifnot(round(0.125, 2) == 0.12)  # half-to-even
stopifnot(round(0.135, 2) == 0.14)
"#,
    )
    .unwrap();
}

#[test]
fn round_vectorized() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# round() works on vectors
x <- round(c(0.5, 1.5, 2.5, 3.5))
stopifnot(identical(x, c(0, 2, 2, 4)))
"#,
    )
    .unwrap();
}

#[test]
fn round_with_na() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- round(c(1.5, NA, 2.5))
stopifnot(x[1] == 2)
stopifnot(is.na(x[2]))
stopifnot(x[3] == 2)
"#,
    )
    .unwrap();
}

#[test]
fn round_named_digits_arg() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# digits as named argument
stopifnot(round(3.14159, digits = 2) == 3.14)
stopifnot(round(3.14159, digits = 4) == 3.1416)
"#,
    )
    .unwrap();
}

#[test]
fn round_negative_digits() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Negative digits rounds to tens, hundreds, etc.
stopifnot(round(123, -1) == 120)
stopifnot(round(155, -1) == 160)
stopifnot(round(1500, -3) == 2000)  # half-to-even: 2000 is even thousands
"#,
    )
    .unwrap();
}

// endregion

// region: log with base argument

#[test]
fn log_natural_default() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Default is natural log
stopifnot(abs(log(1) - 0) < 1e-10)
stopifnot(abs(log(exp(1)) - 1) < 1e-10)
stopifnot(abs(log(exp(5)) - 5) < 1e-10)
"#,
    )
    .unwrap();
}

#[test]
fn log_base_10_positional() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# log(x, base) with base as second positional arg
stopifnot(abs(log(100, 10) - 2) < 1e-10)
stopifnot(abs(log(1000, 10) - 3) < 1e-10)
stopifnot(abs(log(10, 10) - 1) < 1e-10)
"#,
    )
    .unwrap();
}

#[test]
fn log_base_2_positional() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# log(x, 2) should equal log2(x)
stopifnot(abs(log(8, 2) - 3) < 1e-10)
stopifnot(abs(log(16, 2) - 4) < 1e-10)
stopifnot(abs(log(1024, 2) - 10) < 1e-10)
"#,
    )
    .unwrap();
}

#[test]
fn log_base_named_arg() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# log(x, base=b) with named argument
stopifnot(abs(log(100, base = 10) - 2) < 1e-10)
stopifnot(abs(log(8, base = 2) - 3) < 1e-10)
stopifnot(abs(log(27, base = 3) - 3) < 1e-10)
"#,
    )
    .unwrap();
}

#[test]
fn log_vectorized_with_base() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# log with base works on vectors
x <- log(c(10, 100, 1000), 10)
stopifnot(abs(x[1] - 1) < 1e-10)
stopifnot(abs(x[2] - 2) < 1e-10)
stopifnot(abs(x[3] - 3) < 1e-10)
"#,
    )
    .unwrap();
}

#[test]
fn log_special_values() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# log(0) = -Inf
stopifnot(log(0) == -Inf)

# log(Inf) = Inf
stopifnot(log(Inf) == Inf)

# log(1) = 0 for any base
stopifnot(log(1, 10) == 0)
stopifnot(log(1, 2) == 0)
"#,
    )
    .unwrap();
}

#[test]
fn log_na_propagation() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- log(c(1, NA, 100), 10)
stopifnot(abs(x[1] - 0) < 1e-10)
stopifnot(is.na(x[2]))
stopifnot(abs(x[3] - 2) < 1e-10)
"#,
    )
    .unwrap();
}

#[test]
fn log2_and_log10_convenience() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# log2 and log10 should match log with corresponding base
stopifnot(abs(log2(8) - log(8, 2)) < 1e-10)
stopifnot(abs(log10(100) - log(100, 10)) < 1e-10)
"#,
    )
    .unwrap();
}

// endregion
