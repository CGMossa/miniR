use r::Session;

// region: trimws

#[test]
fn trimws_default_both() {
    let mut s = Session::new();
    s.eval_source(
        r#"
stopifnot(identical(trimws("  hello  "), "hello"))
stopifnot(identical(trimws("\thello\n"), "hello"))
stopifnot(identical(trimws("  \t\r\n  hello  \t\r\n  "), "hello"))
"#,
    )
    .unwrap();
}

#[test]
fn trimws_left_only() {
    let mut s = Session::new();
    s.eval_source(
        r#"
stopifnot(identical(trimws("  hello  ", which = "left"), "hello  "))
stopifnot(identical(trimws("\thello\n", which = "left"), "hello\n"))
"#,
    )
    .unwrap();
}

#[test]
fn trimws_right_only() {
    let mut s = Session::new();
    s.eval_source(
        r#"
stopifnot(identical(trimws("  hello  ", which = "right"), "  hello"))
stopifnot(identical(trimws("\thello\n", which = "right"), "\thello"))
"#,
    )
    .unwrap();
}

#[test]
fn trimws_vectorized() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- trimws(c("  a  ", "  b  ", "  c  "))
stopifnot(identical(result, c("a", "b", "c")))
"#,
    )
    .unwrap();
}

#[test]
fn trimws_na_propagation() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- trimws(c("  hello  ", NA, "  world  "))
stopifnot(identical(result[1], "hello"))
stopifnot(is.na(result[2]))
stopifnot(identical(result[3], "world"))
"#,
    )
    .unwrap();
}

#[test]
fn trimws_custom_whitespace() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Trim only dots from both sides
result <- trimws("...hello...", whitespace = "[.]")
stopifnot(identical(result, "hello"))
"#,
    )
    .unwrap();
}

#[test]
fn trimws_empty_string() {
    let mut s = Session::new();
    s.eval_source(
        r#"
stopifnot(identical(trimws(""), ""))
stopifnot(identical(trimws("   "), ""))
"#,
    )
    .unwrap();
}

// endregion

// region: startsWith / endsWith

#[test]
fn starts_with_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
stopifnot(startsWith("hello world", "hello"))
stopifnot(!startsWith("hello world", "world"))
stopifnot(startsWith("", ""))
"#,
    )
    .unwrap();
}

#[test]
fn starts_with_vectorized_x() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- startsWith(c("apple", "banana", "avocado"), "a")
stopifnot(identical(result, c(TRUE, FALSE, TRUE)))
"#,
    )
    .unwrap();
}

#[test]
fn starts_with_vectorized_prefix() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Vectorized over both x and prefix (same length)
result <- startsWith(c("apple", "banana", "cherry"), c("app", "ban", "che"))
stopifnot(identical(result, c(TRUE, TRUE, TRUE)))

# Mismatched lengths with recycling
result2 <- startsWith(c("apple", "banana", "cherry"), c("app", "che"))
# Recycling: "apple"/"app" -> TRUE, "banana"/"che" -> FALSE, "cherry"/"app" -> FALSE
stopifnot(identical(result2, c(TRUE, FALSE, FALSE)))
"#,
    )
    .unwrap();
}

#[test]
fn starts_with_na_propagation() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- startsWith(c("hello", NA, "world"), "h")
stopifnot(identical(result[1], TRUE))
stopifnot(is.na(result[2]))
stopifnot(identical(result[3], FALSE))
"#,
    )
    .unwrap();
}

#[test]
fn ends_with_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
stopifnot(endsWith("hello world", "world"))
stopifnot(!endsWith("hello world", "hello"))
stopifnot(endsWith("", ""))
"#,
    )
    .unwrap();
}

#[test]
fn ends_with_vectorized_x() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- endsWith(c("apple", "banana", "mango"), "o")
stopifnot(identical(result, c(FALSE, FALSE, TRUE)))
"#,
    )
    .unwrap();
}

#[test]
fn ends_with_vectorized_suffix() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- endsWith(c("apple", "banana", "cherry"), c("le", "na", "ry"))
stopifnot(identical(result, c(TRUE, TRUE, TRUE)))
"#,
    )
    .unwrap();
}

#[test]
fn ends_with_na_propagation() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- endsWith(c("hello", NA, "world"), "d")
stopifnot(identical(result[1], FALSE))
stopifnot(is.na(result[2]))
stopifnot(identical(result[3], TRUE))
"#,
    )
    .unwrap();
}

// endregion

// region: chartr

#[test]
fn chartr_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
stopifnot(identical(chartr("aeiou", "AEIOU", "hello"), "hEllO"))
stopifnot(identical(chartr("abc", "xyz", "abcabc"), "xyzxyz"))
"#,
    )
    .unwrap();
}

#[test]
fn chartr_vectorized() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- chartr("aeiou", "AEIOU", c("hello", "world"))
stopifnot(identical(result, c("hEllO", "wOrld")))
"#,
    )
    .unwrap();
}

#[test]
fn chartr_na_propagation() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- chartr("a", "A", c("abc", NA, "aaa"))
stopifnot(identical(result[1], "Abc"))
stopifnot(is.na(result[2]))
stopifnot(identical(result[3], "AAA"))
"#,
    )
    .unwrap();
}

#[test]
fn chartr_no_match() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Characters not in old are left unchanged
stopifnot(identical(chartr("xyz", "XYZ", "hello"), "hello"))
"#,
    )
    .unwrap();
}

// endregion

// region: strtoi

#[test]
fn strtoi_decimal() {
    let mut s = Session::new();
    s.eval_source(
        r#"
stopifnot(identical(strtoi("42"), 42L))
stopifnot(identical(strtoi("-10"), -10L))
stopifnot(identical(strtoi("0"), 0L))
"#,
    )
    .unwrap();
}

#[test]
fn strtoi_hex() {
    let mut s = Session::new();
    s.eval_source(
        r#"
stopifnot(identical(strtoi("ff", base = 16L), 255L))
stopifnot(identical(strtoi("FF", base = 16L), 255L))
stopifnot(identical(strtoi("10", base = 16L), 16L))
"#,
    )
    .unwrap();
}

#[test]
fn strtoi_binary() {
    let mut s = Session::new();
    s.eval_source(
        r#"
stopifnot(identical(strtoi("1010", base = 2L), 10L))
stopifnot(identical(strtoi("11111111", base = 2L), 255L))
"#,
    )
    .unwrap();
}

#[test]
fn strtoi_octal() {
    let mut s = Session::new();
    s.eval_source(
        r#"
stopifnot(identical(strtoi("77", base = 8L), 63L))
stopifnot(identical(strtoi("10", base = 8L), 8L))
"#,
    )
    .unwrap();
}

#[test]
fn strtoi_vectorized() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- strtoi(c("10", "20", "30"))
stopifnot(identical(result, c(10L, 20L, 30L)))
"#,
    )
    .unwrap();
}

#[test]
fn strtoi_na_propagation() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Invalid strings become NA
result <- strtoi(c("10", NA, "xyz", "30"))
stopifnot(identical(result[1], 10L))
stopifnot(is.na(result[2]))
stopifnot(is.na(result[3]))
stopifnot(identical(result[4], 30L))
"#,
    )
    .unwrap();
}

#[test]
fn strtoi_empty_string_is_na() {
    let mut s = Session::new();
    s.eval_source(
        r#"
stopifnot(is.na(strtoi("")))
"#,
    )
    .unwrap();
}

#[test]
fn strtoi_whitespace_trimmed() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# strtoi trims whitespace before parsing
stopifnot(identical(strtoi("  42  "), 42L))
"#,
    )
    .unwrap();
}

// endregion

// region: sprintf

#[test]
fn sprintf_basic_formats() {
    let mut s = Session::new();
    s.eval_source(
        r#"
stopifnot(identical(sprintf("%d", 42L), "42"))
stopifnot(identical(sprintf("%s", "hello"), "hello"))
stopifnot(identical(sprintf("%.2f", 3.14159), "3.14"))
stopifnot(identical(sprintf("%%"), "%"))
"#,
    )
    .unwrap();
}

#[test]
fn sprintf_multiple_args() {
    let mut s = Session::new();
    s.eval_source(
        r#"
stopifnot(identical(
    sprintf("%s has %d items", "Alice", 5L),
    "Alice has 5 items"
))
"#,
    )
    .unwrap();
}

#[test]
fn sprintf_vectorized() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- sprintf("%d items", 1:3)
stopifnot(identical(result, c("1 items", "2 items", "3 items")))
"#,
    )
    .unwrap();
}

#[test]
fn sprintf_recycling() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- sprintf("%s has %d items", c("Alice", "Bob"), 1:4)
stopifnot(identical(result, c(
    "Alice has 1 items", "Bob has 2 items",
    "Alice has 3 items", "Bob has 4 items"
)))
"#,
    )
    .unwrap();
}

#[test]
fn sprintf_empty_arg_returns_empty() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- sprintf("%d", integer(0))
stopifnot(identical(result, character(0)))
"#,
    )
    .unwrap();
}

#[test]
fn sprintf_width_and_padding() {
    let mut s = Session::new();
    s.eval_source(
        r#"
stopifnot(identical(sprintf("%05d", 42L), "00042"))
stopifnot(identical(sprintf("%-10s", "hi"), "hi        "))
"#,
    )
    .unwrap();
}

// endregion

// region: formatC

#[test]
fn format_c_integer() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- formatC(42, format = "d")
stopifnot(identical(result, "42"))
"#,
    )
    .unwrap();
}

#[test]
fn format_c_fixed_point() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- formatC(3.14159, format = "f", digits = 2)
stopifnot(identical(result, "3.14"))
"#,
    )
    .unwrap();
}

#[test]
fn format_c_width_padding() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- formatC(42, width = 6, format = "d")
stopifnot(nchar(result) == 6)

# Left-justified
result2 <- formatC(42, width = 6, format = "d", flag = "-")
stopifnot(nchar(result2) == 6)
"#,
    )
    .unwrap();
}

#[test]
fn format_c_zero_pad() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- formatC(42, width = 6, format = "d", flag = "0")
stopifnot(identical(result, "000042"))
"#,
    )
    .unwrap();
}

#[test]
fn format_c_sign() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- formatC(42, format = "d", flag = "+")
stopifnot(identical(result, "+42"))

result2 <- formatC(-42, format = "d", flag = "+")
stopifnot(identical(result2, "-42"))
"#,
    )
    .unwrap();
}

#[test]
fn format_c_string() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- formatC("hello", width = 10, format = "s")
stopifnot(nchar(result) == 10)
"#,
    )
    .unwrap();
}

#[test]
fn format_c_vectorized() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- formatC(c(1, 22, 333), width = 5, format = "d")
stopifnot(length(result) == 3)
stopifnot(all(nchar(result) == 5))
"#,
    )
    .unwrap();
}

// endregion

// region: encodeString

#[test]
fn encode_string_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- encodeString("hello")
stopifnot(identical(result, "hello"))
"#,
    )
    .unwrap();
}

#[test]
fn encode_string_with_quote() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- encodeString("hello", quote = "\"")
stopifnot(identical(result, "\"hello\""))
"#,
    )
    .unwrap();
}

#[test]
fn encode_string_escapes() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Newlines and tabs should be escaped
result <- encodeString("a\tb\nc")
stopifnot(identical(result, "a\\tb\\nc"))
"#,
    )
    .unwrap();
}

#[test]
fn encode_string_vectorized() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- encodeString(c("hello", "world"))
stopifnot(identical(result, c("hello", "world")))
"#,
    )
    .unwrap();
}

#[test]
fn encode_string_na_encode_true() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- encodeString(c("hello", NA))
stopifnot(identical(result[1], "hello"))
stopifnot(identical(result[2], "NA"))
"#,
    )
    .unwrap();
}

#[test]
fn encode_string_na_encode_false() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- encodeString(c("hello", NA), na.encode = FALSE)
stopifnot(identical(result[1], "hello"))
stopifnot(is.na(result[2]))
"#,
    )
    .unwrap();
}

#[test]
fn encode_string_with_width() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# width=0 means pad to max element width
result <- encodeString(c("a", "bb", "ccc"), width = 0)
stopifnot(all(nchar(result) == 3))

# explicit width
result2 <- encodeString(c("hi", "there"), width = 8)
stopifnot(all(nchar(result2) == 8))
"#,
    )
    .unwrap();
}

#[test]
fn encode_string_justify() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Right justify
result <- encodeString("hi", width = 6, justify = "right")
stopifnot(identical(result, "    hi"))

# Left justify (default when width is set)
result2 <- encodeString("hi", width = 6, justify = "left")
stopifnot(identical(result2, "hi    "))

# Centre justify
result3 <- encodeString("hi", width = 6, justify = "centre")
stopifnot(identical(result3, "  hi  "))
"#,
    )
    .unwrap();
}

// endregion
