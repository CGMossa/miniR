use r::Session;

// region: match() with ignore.case

#[test]
fn match_ignore_case_basic() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        x <- c("hello", "WORLD", "Foo")
        table <- c("Hello", "world", "foo")
        result <- match(x, table, ignore.case = TRUE)
        stopifnot(identical(result, c(1L, 2L, 3L)))
    "#,
    )
    .unwrap();
}

#[test]
fn match_ignore_case_false_is_default() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        x <- c("hello", "WORLD")
        table <- c("Hello", "world")
        result <- match(x, table)
        stopifnot(identical(result, c(NA_integer_, NA_integer_)))
    "#,
    )
    .unwrap();
}

#[test]
fn match_ignore_case_unicode() {
    let mut r = Session::new();
    // UniCase handles Unicode case folding beyond ASCII
    r.eval_source(
        r#"
        result <- match("RÉSUMÉ", c("résumé"), ignore.case = TRUE)
        stopifnot(identical(result, 1L))
    "#,
    )
    .unwrap();
}

#[test]
fn match_ignore_case_na_propagation() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        result <- match(c("a", NA), c("A", "b"), ignore.case = TRUE)
        stopifnot(identical(result, c(1L, NA_integer_)))
    "#,
    )
    .unwrap();
}

// endregion

// region: nchar with type="graphemes"

#[test]
fn nchar_graphemes_with_combining_chars() {
    let mut r = Session::new();
    // Use intToUtf8 to build "e" + combining acute accent (U+0301)
    // This is 1 grapheme cluster but 2 Unicode code points
    r.eval_source(
        r#"
        s <- intToUtf8(c(101L, 769L))
        stopifnot(nchar(s, type = "chars") == 2L)
        stopifnot(nchar(s, type = "graphemes") == 1L)
    "#,
    )
    .unwrap();
}

#[test]
fn nchar_graphemes_ascii() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        stopifnot(nchar("hello", type = "graphemes") == 5L)
        stopifnot(nchar("", type = "graphemes") == 0L)
    "#,
    )
    .unwrap();
}

#[test]
fn nchar_graphemes_vectorized() {
    let mut r = Session::new();
    // Use intToUtf8 to build string with combining char
    r.eval_source(
        r#"
        s <- intToUtf8(c(101L, 769L))
        result <- nchar(c("ab", s, "x"), type = "graphemes")
        stopifnot(identical(result, c(2L, 1L, 1L)))
    "#,
    )
    .unwrap();
}

// endregion
