use r::session::Session;

// region: Encoding builtins

#[test]
fn encoding_reports_unknown_for_ascii() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        stopifnot(Encoding("hello") == "unknown")
        stopifnot(Encoding("abc123") == "unknown")
    "#,
    )
    .unwrap();
}

#[test]
fn encoding_reports_utf8_for_non_ascii() {
    let mut s = Session::new();
    // Use a raw UTF-8 string with an actual non-ASCII character
    s.eval_source("stopifnot(Encoding(\"caf\u{00e9}\") == \"UTF-8\")")
        .unwrap();
}

#[test]
fn enc2utf8_is_passthrough() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        x <- c("hello", "world")
        y <- enc2utf8(x)
        stopifnot(identical(x, y))
    "#,
    )
    .unwrap();
}

#[test]
fn enc2native_is_passthrough() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        x <- c("hello", "world")
        y <- enc2native(x)
        stopifnot(identical(x, y))
    "#,
    )
    .unwrap();
}

// endregion

// region: strtrim

#[test]
fn strtrim_trims_to_width() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        stopifnot(strtrim("abcdef", 3) == "abc")
        stopifnot(strtrim("ab", 5) == "ab")
        stopifnot(strtrim("", 3) == "")
    "#,
    )
    .unwrap();
}

#[test]
fn strtrim_vectorized() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        result <- strtrim(c("hello", "world"), c(3, 2))
        stopifnot(result[1] == "hel")
        stopifnot(result[2] == "wo")
    "#,
    )
    .unwrap();
}

// endregion

// region: arrayInd

#[test]
fn array_ind_converts_linear_to_subscripts() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        # 3x4 matrix: linear index 5 = row 2, col 2
        result <- arrayInd(5, c(3, 4))
        stopifnot(result[1, 1] == 2)
        stopifnot(result[1, 2] == 2)
    "#,
    )
    .unwrap();
}

#[test]
fn array_ind_multiple_indices() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        result <- arrayInd(c(1, 4, 7), c(3, 4))
        # Index 1: row=1, col=1
        stopifnot(result[1, 1] == 1)
        stopifnot(result[1, 2] == 1)
        # Index 4: row=1, col=2
        stopifnot(result[2, 1] == 1)
        stopifnot(result[2, 2] == 2)
        # Index 7: row=1, col=3
        stopifnot(result[3, 1] == 1)
        stopifnot(result[3, 2] == 3)
    "#,
    )
    .unwrap();
}

// endregion

// region: xor vectorized

#[test]
fn xor_vectorized() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        result <- xor(c(TRUE, FALSE, TRUE, FALSE), c(TRUE, TRUE, FALSE, FALSE))
        stopifnot(identical(result, c(FALSE, TRUE, TRUE, FALSE)))
    "#,
    )
    .unwrap();
}

#[test]
fn xor_na_propagation() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        result <- xor(c(TRUE, NA), c(FALSE, TRUE))
        stopifnot(result[1] == TRUE)
        stopifnot(is.na(result[2]))
    "#,
    )
    .unwrap();
}

#[test]
fn xor_recycling() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        result <- xor(c(TRUE, FALSE, TRUE), FALSE)
        stopifnot(identical(result, c(TRUE, FALSE, TRUE)))
    "#,
    )
    .unwrap();
}

// endregion

// region: is.element numeric-aware

#[test]
fn is_element_numeric() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        stopifnot(is.element(2, 1:5))
        stopifnot(!is.element(6, 1:5))
    "#,
    )
    .unwrap();
}

#[test]
fn is_element_character() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        stopifnot(is.element("b", c("a", "b", "c")))
        stopifnot(!is.element("d", c("a", "b", "c")))
    "#,
    )
    .unwrap();
}

// endregion

// region: which with arr.ind

#[test]
fn which_arr_ind_on_matrix() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        m <- matrix(c(FALSE, TRUE, FALSE, FALSE, FALSE, TRUE), nrow=2, ncol=3)
        result <- which(m, arr.ind=TRUE)
        # TRUE at positions (2,1) and (2,3)
        stopifnot(nrow(result) == 2)
        stopifnot(result[1, 1] == 2)
        stopifnot(result[1, 2] == 1)
        stopifnot(result[2, 1] == 2)
        stopifnot(result[2, 2] == 3)
    "#,
    )
    .unwrap();
}

#[test]
fn which_without_arr_ind_returns_linear() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        x <- c(FALSE, TRUE, FALSE, TRUE)
        result <- which(x)
        stopifnot(identical(result, c(2L, 4L)))
    "#,
    )
    .unwrap();
}

// endregion

// region: parent.env<-

#[test]
fn parent_env_setter() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        e1 <- new.env(parent = emptyenv())
        e2 <- new.env(parent = emptyenv())
        `parent.env<-`(e1, e2)
        stopifnot(identical(parent.env(e1), e2))
    "#,
    )
    .unwrap();
}

// endregion

// region: do.call with named list args

#[test]
fn do_call_named_list_args() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        f <- function(a, b) a + b
        result <- do.call(f, list(a=1, b=2))
        stopifnot(result == 3)
    "#,
    )
    .unwrap();
}

#[test]
fn do_call_mixed_named_positional() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        f <- function(a, b, c) paste(a, b, c)
        result <- do.call(f, list(1, c="three", b="two"))
        stopifnot(result == "1 two three")
    "#,
    )
    .unwrap();
}

// endregion

// region: digest

#[test]
fn digest_sha256() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        result <- digest("hello")
        # SHA-256 of "hello"
        stopifnot(result == "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824")
    "#,
    )
    .unwrap();
}

#[test]
fn digest_sha512() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        result <- digest("hello", algo="sha512")
        # SHA-512 of "hello"
        stopifnot(nchar(result) == 128)
    "#,
    )
    .unwrap();
}

#[test]
fn digest_unsupported_algo_errors() {
    let mut s = Session::new();
    let result = s.eval_source(r#"digest("hello", algo="md5")"#);
    assert!(result.is_err());
}

#[test]
fn md5_errors_with_suggestion() {
    let mut s = Session::new();
    let result = s.eval_source(r#"md5("hello")"#);
    assert!(result.is_err());
    let err = format!("{}", result.unwrap_err());
    assert!(err.contains("sha256") || err.contains("SHA"));
}

// endregion

// region: CRC32

#[test]
fn digest_crc32() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        result <- digest("hello", algo="crc32")
        # CRC32 of "hello" = 3610a686
        stopifnot(result == "3610a686")
        stopifnot(nchar(result) == 8)
    "#,
    )
    .unwrap();
}

#[test]
fn digest_crc32_empty_string() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        result <- digest("", algo="crc32")
        # CRC32 of empty string = 00000000
        stopifnot(result == "00000000")
        stopifnot(nchar(result) == 8)
    "#,
    )
    .unwrap();
}

// endregion
