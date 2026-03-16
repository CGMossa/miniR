use r::Session;

// region: sprintf vectorization

#[test]
fn sprintf_vectorized_over_single_arg() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Basic vectorization: sprintf("%d items", 1:3) should return c("1 items", "2 items", "3 items")
result <- sprintf("%d items", 1:3)
stopifnot(identical(result, c("1 items", "2 items", "3 items")))
"#,
    )
    .expect("sprintf should vectorize over a single vector argument");
}

#[test]
fn sprintf_vectorized_over_multiple_args() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Multiple vectorized args with recycling
result <- sprintf("%s has %d items", c("Alice", "Bob"), 1:4)
stopifnot(identical(result, c("Alice has 1 items", "Bob has 2 items",
                               "Alice has 3 items", "Bob has 4 items")))
"#,
    )
    .expect("sprintf should recycle shorter vector args");
}

#[test]
fn sprintf_scalar_still_works() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Scalar args should still work exactly as before
stopifnot(sprintf("hello %s, you have %d items", "world", 42) ==
          "hello world, you have 42 items")
stopifnot(sprintf("pi is %.2f", 3.14159) == "pi is 3.14")
stopifnot(sprintf("100%%") == "100%")
"#,
    )
    .expect("sprintf scalar usage should still work");
}

#[test]
fn sprintf_empty_arg_returns_empty() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# If any argument has length 0, return character(0)
result <- sprintf("%d", integer(0))
stopifnot(identical(result, character(0)))
"#,
    )
    .expect("sprintf with length-0 arg should return character(0)");
}

#[test]
fn sprintf_vectorized_float_format() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- sprintf("%.1f", c(1.5, 2.5, 3.5))
stopifnot(identical(result, c("1.5", "2.5", "3.5")))
"#,
    )
    .expect("sprintf should vectorize over float arguments");
}

#[test]
fn sprintf_vectorized_string_format() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- sprintf("Hello, %s!", c("Alice", "Bob", "Charlie"))
stopifnot(identical(result, c("Hello, Alice!", "Hello, Bob!", "Hello, Charlie!")))
"#,
    )
    .expect("sprintf should vectorize over string arguments");
}

// endregion

// region: formatC

#[test]
fn formatc_integer_format() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- formatC(42, format = "d")
stopifnot(result == "42")
"#,
    )
    .expect("formatC should format integers with format='d'");
}

#[test]
fn formatc_fixed_decimal() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- formatC(3.14159, format = "f", digits = 2)
stopifnot(result == "3.14")
"#,
    )
    .expect("formatC should format fixed-point decimals with format='f'");
}

#[test]
fn formatc_scientific_notation() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- formatC(0.001234, format = "e", digits = 3)
stopifnot(result == "1.234e-03")
"#,
    )
    .expect("formatC should format scientific notation with format='e'");
}

#[test]
fn formatc_width_padding() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Right-justified (default) with width
result <- formatC(42, width = 6, format = "d")
stopifnot(result == "    42")

# Left-justified with flag="-"
result2 <- formatC(42, width = 6, format = "d", flag = "-")
stopifnot(result2 == "42    ")
"#,
    )
    .expect("formatC should support width and left/right justification");
}

#[test]
fn formatc_zero_pad() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- formatC(42, width = 6, format = "d", flag = "0")
stopifnot(result == "000042")
"#,
    )
    .expect("formatC should zero-pad with flag='0'");
}

#[test]
fn formatc_sign_flag() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- formatC(42, format = "d", flag = "+")
stopifnot(result == "+42")

result2 <- formatC(-42, format = "d", flag = "+")
stopifnot(result2 == "-42")
"#,
    )
    .expect("formatC should always show sign with flag='+'");
}

#[test]
fn formatc_string_format() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- formatC("hello", width = 10, format = "s")
stopifnot(result == "     hello")

result2 <- formatC("hello", width = 10, format = "s", flag = "-")
stopifnot(result2 == "hello     ")
"#,
    )
    .expect("formatC should format strings with format='s'");
}

#[test]
fn formatc_vectorized() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- formatC(c(1, 22, 333), width = 5, format = "d")
stopifnot(identical(result, c("    1", "   22", "  333")))
"#,
    )
    .expect("formatC should vectorize over input");
}

#[test]
fn formatc_general_format() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# g format should pick the shorter of f/e
result <- formatC(12345.6, format = "g", digits = 4)
stopifnot(nchar(result) > 0)  # just check it doesn't error
"#,
    )
    .expect("formatC should handle format='g'");
}

// endregion

// region: format.pval

#[test]
fn format_pval_very_small() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- format.pval(1e-20)
# Should start with "< " for very small p-values
stopifnot(startsWith(result, "<"))
"#,
    )
    .expect("format.pval should show '< eps' for very small p-values");
}

#[test]
fn format_pval_normal_value() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- format.pval(0.05, digits = 2)
# Should be a normal numeric string
stopifnot(nchar(result) > 0)
stopifnot(!startsWith(result, "<"))
"#,
    )
    .expect("format.pval should format normal p-values as numbers");
}

#[test]
fn format_pval_custom_eps() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# With a high eps, a moderate p-value should still show "< eps"
result <- format.pval(0.01, eps = 0.05)
stopifnot(startsWith(result, "<"))
"#,
    )
    .expect("format.pval should respect custom eps threshold");
}

#[test]
fn format_pval_vectorized() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- format.pval(c(0.5, 1e-20, 0.001))
stopifnot(length(result) == 3)
# First should be normal, second should be "< ..."
stopifnot(!startsWith(result[1], "<"))
stopifnot(startsWith(result[2], "<"))
"#,
    )
    .expect("format.pval should vectorize over input");
}

// endregion

// region: prettyNum

#[test]
fn prettynum_big_mark() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- prettyNum(1234567, big.mark = ",")
stopifnot(result == "1,234,567")
"#,
    )
    .expect("prettyNum should insert comma thousand separators");
}

#[test]
fn prettynum_no_marks() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- prettyNum(1234567)
stopifnot(result == "1234567")
"#,
    )
    .expect("prettyNum with no marks should return unchanged string");
}

#[test]
fn prettynum_small_number() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Numbers with 3 or fewer digits shouldn't get a mark
result <- prettyNum(123, big.mark = ",")
stopifnot(result == "123")
"#,
    )
    .expect("prettyNum should not insert marks in small numbers");
}

#[test]
fn prettynum_negative_number() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- prettyNum(-1234567, big.mark = ",")
stopifnot(result == "-1,234,567")
"#,
    )
    .expect("prettyNum should handle negative numbers");
}

#[test]
fn prettynum_with_decimal() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- prettyNum("1234567.89", big.mark = ",")
stopifnot(result == "1,234,567.89")
"#,
    )
    .expect("prettyNum should handle decimal numbers");
}

#[test]
fn prettynum_vectorized() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- prettyNum(c(1000, 2000000, 300), big.mark = ",")
stopifnot(identical(result, c("1,000", "2,000,000", "300")))
"#,
    )
    .expect("prettyNum should vectorize over input");
}

#[test]
fn prettynum_space_separator() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- prettyNum(1234567, big.mark = " ")
stopifnot(result == "1 234 567")
"#,
    )
    .expect("prettyNum should support space as separator");
}

// endregion
