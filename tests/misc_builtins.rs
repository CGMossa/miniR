use r::Session;

// region: Encoding builtins

#[test]
fn encoding_returns_unknown_for_ascii() {
    let mut r = Session::new();
    r.eval_source(r#"stopifnot(Encoding("hello") == "unknown")"#)
        .expect("ASCII strings should have 'unknown' encoding");
}

#[test]
fn encoding_returns_utf8_for_non_ascii() {
    let mut r = Session::new();
    // Use a literal UTF-8 string (the parser doesn't handle \u escapes yet)
    r.eval_source("stopifnot(Encoding(\"caf\u{00e9}\") == \"UTF-8\")")
        .expect("Non-ASCII strings should have 'UTF-8' encoding");
}

#[test]
fn enc2utf8_is_passthrough() {
    let mut r = Session::new();
    r.eval_source(r#"stopifnot(identical(enc2utf8("hello"), "hello"))"#)
        .expect("enc2utf8 should be a pass-through");
}

#[test]
fn enc2native_is_passthrough() {
    let mut r = Session::new();
    r.eval_source(r#"stopifnot(identical(enc2native("hello"), "hello"))"#)
        .expect("enc2native should be a pass-through");
}

// endregion

// region: strtrim

#[test]
fn strtrim_trims_to_width() {
    let mut r = Session::new();
    r.eval_source(r#"stopifnot(strtrim("Hello, world!", 5) == "Hello")"#)
        .expect("strtrim should trim to 5 characters");
}

#[test]
fn strtrim_no_trim_needed() {
    let mut r = Session::new();
    r.eval_source(r#"stopifnot(strtrim("Hi", 10) == "Hi")"#)
        .expect("strtrim should not trim when string is shorter than width");
}

#[test]
fn strtrim_vectorized() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        result <- strtrim(c("abc", "defgh"), 3)
        stopifnot(identical(result, c("abc", "def")))
    "#,
    )
    .expect("strtrim should work on vectors");
}

// endregion

// region: xor (vectorized)

#[test]
fn xor_scalar() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        stopifnot(xor(TRUE, FALSE) == TRUE)
        stopifnot(xor(TRUE, TRUE) == FALSE)
        stopifnot(xor(FALSE, FALSE) == FALSE)
        stopifnot(xor(FALSE, TRUE) == TRUE)
    "#,
    )
    .expect("xor should work for all scalar combinations");
}

#[test]
fn xor_vectorized() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        result <- xor(c(TRUE, FALSE, TRUE), c(FALSE, FALSE, TRUE))
        stopifnot(identical(result, c(TRUE, FALSE, FALSE)))
    "#,
    )
    .expect("xor should be vectorized");
}

#[test]
fn xor_with_na() {
    let mut r = Session::new();
    r.eval_source("stopifnot(is.na(xor(TRUE, NA)))")
        .expect("xor with NA should return NA");
}

// endregion

// region: is.element (upgraded to handle numeric)

#[test]
fn is_element_character() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        result <- is.element(c("a", "b", "c"), c("b", "c", "d"))
        stopifnot(identical(result, c(FALSE, TRUE, TRUE)))
    "#,
    )
    .expect("is.element should work with character vectors");
}

#[test]
fn is_element_numeric() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        result <- is.element(c(1, 2, 3), c(2, 4, 6))
        stopifnot(identical(result, c(FALSE, TRUE, FALSE)))
    "#,
    )
    .expect("is.element should work with numeric vectors");
}

// endregion

// region: which with arr.ind

#[test]
fn which_basic() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        result <- which(c(FALSE, TRUE, FALSE, TRUE))
        stopifnot(identical(result, c(2L, 4L)))
    "#,
    )
    .expect("which should return 1-based indices of TRUE elements");
}

#[test]
fn which_arr_ind_on_matrix() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        m <- matrix(c(TRUE, FALSE, TRUE, FALSE, TRUE, FALSE), nrow = 2)
        result <- which(m, arr.ind = TRUE)
        # m is:
        #      [,1]  [,2]  [,3]
        # [1,] TRUE  TRUE  TRUE
        # [2,] FALSE FALSE FALSE
        # TRUE at linear positions 1, 3, 5 -> (1,1), (1,2), (1,3)
        stopifnot(is.matrix(result))
        stopifnot(nrow(result) == 3L)
        stopifnot(ncol(result) == 2L)
    "#,
    )
    .expect("which with arr.ind=TRUE should return a matrix of row/col indices");
}

// endregion

// region: arrayInd

#[test]
fn array_ind_basic() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        result <- arrayInd(5L, c(3L, 3L))
        # Linear index 5 in 3x3 matrix: row = (5-1)%%3 + 1 = 2, col = (5-1)%/%3 + 1 = 2
        stopifnot(is.matrix(result))
        stopifnot(result[1, 1] == 2L)
        stopifnot(result[1, 2] == 2L)
    "#,
    )
    .expect("arrayInd should convert linear indices to row/col indices");
}

#[test]
fn array_ind_multiple() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        result <- arrayInd(c(1L, 4L, 7L), c(3L, 3L))
        # In 3x3 matrix (3 rows):
        # 1 -> (1,1), 4 -> (1,2), 7 -> (1,3)
        stopifnot(nrow(result) == 3L)
        stopifnot(result[1, 1] == 1L)
        stopifnot(result[1, 2] == 1L)
        stopifnot(result[2, 1] == 1L)
        stopifnot(result[2, 2] == 2L)
        stopifnot(result[3, 1] == 1L)
        stopifnot(result[3, 2] == 3L)
    "#,
    )
    .expect("arrayInd should handle multiple indices");
}

// endregion

// region: do.call with named list args

#[test]
fn do_call_named_list_args() {
    let mut r = Session::new();
    r.eval_source(r#"stopifnot(do.call(paste, list("a", "b", sep = "-")) == "a-b")"#)
        .expect("do.call should pass named list elements as named arguments");
}

#[test]
fn do_call_positional_args_from_list() {
    let mut r = Session::new();
    r.eval_source("stopifnot(do.call(sum, list(1, 2, 3)) == 6)")
        .expect("do.call should pass positional args from list");
}

#[test]
fn do_call_quote_parameter_accepted() {
    let mut r = Session::new();
    r.eval_source("stopifnot(do.call(sum, list(1, 2, 3), quote = FALSE) == 6)")
        .expect("do.call should accept quote parameter without error");
}

// endregion

// region: Recall

#[test]
fn recall_recursive_factorial() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        f <- function(n) if (n <= 1) 1 else n * Recall(n - 1)
        stopifnot(f(5) == 120)
        stopifnot(f(1) == 1)
        stopifnot(f(0) == 1)
    "#,
    )
    .expect("Recall should enable recursive calls");
}

#[test]
fn recall_outside_function_errors() {
    let mut r = Session::new();
    let result = r.eval_source("Recall(1)");
    assert!(result.is_err(), "Recall() outside a function should error");
}

// endregion

// region: parent.env<-

#[test]
fn parent_env_replacement() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        e1 <- new.env(parent = emptyenv())
        e2 <- new.env(parent = emptyenv())
        `parent.env<-`(e1, e2)
        stopifnot(identical(parent.env(e1), e2))
    "#,
    )
    .expect("parent.env<- should change the parent of an environment");
}

// endregion

// region: sprintf with zero-padding (already existed, verify)

#[test]
fn sprintf_zero_padding() {
    let mut r = Session::new();
    r.eval_source(r#"stopifnot(sprintf("%02d", 5L) == "05")"#)
        .expect("sprintf should support %02d zero-padding");
}

#[test]
fn sprintf_zero_padding_wider() {
    let mut r = Session::new();
    r.eval_source(r#"stopifnot(sprintf("%05d", 42L) == "00042")"#)
        .expect("sprintf should support %05d zero-padding");
}

// endregion

// region: chartr (already existed, verify)

#[test]
fn chartr_translates_characters() {
    let mut r = Session::new();
    r.eval_source(r#"stopifnot(chartr("abc", "ABC", "abcdef") == "ABCdef")"#)
        .expect("chartr should translate characters");
}

// endregion

// region: nchar type="bytes" (already existed, verify)

#[test]
fn nchar_type_bytes() {
    let mut r = Session::new();
    r.eval_source(r#"stopifnot(nchar("hello", type = "bytes") == 5L)"#)
        .expect("nchar with type='bytes' should return byte count");
}

// endregion
