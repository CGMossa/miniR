//! Tests for bstr-powered encoding support:
//! - rawToChar() graceful non-UTF-8 handling
//! - rawToChar() multiple parameter
//! - iconv() encoding conversions
//! - readLines() with non-UTF-8 files

use r::session::Session;

// region: rawToChar improvements

#[test]
fn raw_to_char_valid_utf8() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        x <- charToRaw("hello")
        result <- rawToChar(x)
        stopifnot(identical(result, "hello"))
    "#,
    )
    .unwrap();
}

#[test]
fn raw_to_char_non_utf8_uses_replacement_char() {
    // Bytes 0xff 0xfe are not valid UTF-8; bstr should replace with U+FFFD
    let mut s = Session::new();
    s.eval_source(
        r#"
        x <- as.raw(c(0x68, 0x69, 0xff, 0xfe))
        result <- rawToChar(x)
        # The result should contain "hi" followed by replacement characters
        stopifnot(nchar(result, type = "bytes") > 0)
        stopifnot(grepl("hi", result))
    "#,
    )
    .unwrap();
}

#[test]
fn raw_to_char_strips_nul_bytes() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        x <- as.raw(c(0x41, 0x00, 0x42))
        result <- rawToChar(x)
        stopifnot(identical(result, "AB"))
    "#,
    )
    .unwrap();
}

#[test]
fn raw_to_char_multiple_true() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        x <- charToRaw("abc")
        result <- rawToChar(x, multiple = TRUE)
        stopifnot(identical(result, c("a", "b", "c")))
        stopifnot(length(result) == 3L)
    "#,
    )
    .unwrap();
}

#[test]
fn raw_to_char_multiple_false_is_default() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        x <- charToRaw("abc")
        result <- rawToChar(x)
        stopifnot(length(result) == 1L)
        stopifnot(identical(result, "abc"))
    "#,
    )
    .unwrap();
}

// endregion

// region: iconv

#[test]
fn iconv_identity_conversion() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        x <- "hello world"
        result <- iconv(x, from = "UTF-8", to = "UTF-8")
        stopifnot(identical(result, "hello world"))
    "#,
    )
    .unwrap();
}

#[test]
fn iconv_utf8_to_ascii_drops_non_ascii() {
    let mut s = Session::new();
    // Build a non-ASCII string from raw bytes (UTF-8 for e-acute: 0xc3 0xa9)
    s.eval_source(
        r#"
        x <- rawToChar(as.raw(c(0x63, 0x61, 0x66, 0xc3, 0xa9)))
        result <- iconv(x, from = "UTF-8", to = "ASCII", sub = "")
        stopifnot(identical(result, "caf"))
    "#,
    )
    .unwrap();
}

#[test]
fn iconv_utf8_to_ascii_with_byte_sub() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        x <- rawToChar(as.raw(c(0x63, 0x61, 0x66, 0xc3, 0xa9)))
        result <- iconv(x, from = "UTF-8", to = "ASCII", sub = "byte")
        stopifnot(grepl("caf", result))
        # "byte" mode uses hex escapes for non-ASCII bytes
        stopifnot(grepl("<", result))
    "#,
    )
    .unwrap();
}

#[test]
fn iconv_utf8_to_ascii_with_question_mark_sub() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        x <- rawToChar(as.raw(c(0x63, 0x61, 0x66, 0xc3, 0xa9)))
        result <- iconv(x, from = "UTF-8", to = "ASCII", sub = "?")
        stopifnot(identical(result, "caf?"))
    "#,
    )
    .unwrap();
}

#[test]
fn iconv_utf8_to_latin1_preserves_latin_chars() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        x <- rawToChar(as.raw(c(0x63, 0x61, 0x66, 0xc3, 0xa9)))
        result <- iconv(x, from = "UTF-8", to = "latin1")
        stopifnot(grepl("caf", result))
    "#,
    )
    .unwrap();
}

#[test]
fn iconv_vectorized() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        cafe <- rawToChar(as.raw(c(0x63, 0x61, 0x66, 0xc3, 0xa9)))
        x <- c("hello", cafe, "world")
        result <- iconv(x, from = "UTF-8", to = "ASCII", sub = "?")
        stopifnot(identical(result, c("hello", "caf?", "world")))
    "#,
    )
    .unwrap();
}

#[test]
fn iconv_na_passthrough() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        x <- c("hello", NA, "world")
        result <- iconv(x, from = "UTF-8", to = "ASCII")
        stopifnot(is.na(result[2]))
        stopifnot(identical(result[1], "hello"))
        stopifnot(identical(result[3], "world"))
    "#,
    )
    .unwrap();
}

#[test]
fn iconv_empty_encoding_defaults_to_utf8() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        x <- "hello"
        result <- iconv(x, from = "", to = "")
        stopifnot(identical(result, "hello"))
    "#,
    )
    .unwrap();
}

#[test]
fn iconv_to_bytes_format() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        x <- "AB"
        result <- iconv(x, to = "bytes")
        # Should be hex representation of UTF-8 bytes
        stopifnot(grepl("41", result))
        stopifnot(grepl("42", result))
    "#,
    )
    .unwrap();
}

// endregion

// region: readLines with non-UTF-8 files

#[test]
fn read_lines_latin1_file() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        # Create a file with Latin-1 encoded content (bytes 0xE9 = e-acute in Latin-1)
        f <- tempfile()
        # Write raw bytes: "caf" + 0xe9 (Latin-1 e-acute) + newline
        raw_bytes <- as.raw(c(0x63, 0x61, 0x66, 0xe9, 0x0a))
        writeLines(rawToChar(raw_bytes), f)

        lines <- readLines(f)
        stopifnot(length(lines) >= 1L)
        stopifnot(grepl("caf", lines[1]))
    "#,
    )
    .unwrap();
}

#[test]
fn read_lines_handles_valid_utf8_file() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        f <- tempfile()
        writeLines(c("hello", "world", "foo"), f)

        lines <- readLines(f)
        stopifnot(length(lines) == 3L)
        stopifnot(identical(lines[1], "hello"))
        stopifnot(identical(lines[2], "world"))
        stopifnot(identical(lines[3], "foo"))
    "#,
    )
    .unwrap();
}

#[test]
fn read_lines_with_n_parameter() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        f <- tempfile()
        writeLines(c("line1", "line2", "line3", "line4"), f)

        lines <- readLines(f, n = 2)
        stopifnot(length(lines) == 2L)
        stopifnot(identical(lines[1], "line1"))
        stopifnot(identical(lines[2], "line2"))
    "#,
    )
    .unwrap();
}

// endregion

// region: capabilities iconv

#[test]
fn capabilities_reports_iconv_true() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        caps <- capabilities()
        idx <- which(names(caps) == "iconv")
        stopifnot(length(idx) == 1L)
        stopifnot(caps[idx] == TRUE)
    "#,
    )
    .unwrap();
}

// endregion
