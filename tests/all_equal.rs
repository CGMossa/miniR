use r::Session;

#[test]
fn all_equal_numeric_within_tolerance() {
    let mut s = Session::new();
    // 1e-10 is well within the default tolerance of 1.5e-8
    s.eval_source(
        r#"
stopifnot(isTRUE(all.equal(1, 1 + 1e-10)))
stopifnot(isTRUE(all.equal(100, 100 + 1e-9)))
stopifnot(isTRUE(all.equal(c(1, 2, 3), c(1, 2, 3))))
"#,
    )
    .unwrap();
}

#[test]
fn all_equal_numeric_exceeds_tolerance() {
    let mut s = Session::new();
    // all.equal(1, 2) should return a character string, not TRUE
    s.eval_source(
        r#"
result <- all.equal(1, 2)
stopifnot(is.character(result))
stopifnot(grepl("Mean relative difference", result))
"#,
    )
    .unwrap();
}

#[test]
fn all_equal_length_mismatch() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- all.equal(1:3, 1:4)
stopifnot(is.character(result))
stopifnot(grepl("Lengths", result))
stopifnot(grepl("3", result))
stopifnot(grepl("4", result))
"#,
    )
    .unwrap();
}

#[test]
fn all_equal_character_mismatch() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- all.equal("a", "b")
stopifnot(is.character(result))
stopifnot(grepl("string mismatch", result))
"#,
    )
    .unwrap();
}

#[test]
fn all_equal_character_match() {
    let mut s = Session::new();
    s.eval_source(
        r#"
stopifnot(isTRUE(all.equal("hello", "hello")))
stopifnot(isTRUE(all.equal(c("a", "b"), c("a", "b"))))
"#,
    )
    .unwrap();
}

#[test]
fn all_equal_null_null() {
    let mut s = Session::new();
    s.eval_source(
        r#"
stopifnot(isTRUE(all.equal(NULL, NULL)))
"#,
    )
    .unwrap();
}

#[test]
fn all_equal_type_mismatch() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- all.equal(NULL, 1)
stopifnot(is.character(result))
stopifnot(grepl("target is NULL", result))
stopifnot(grepl("current is double", result))
"#,
    )
    .unwrap();
}

#[test]
fn all_equal_list_comparison() {
    let mut s = Session::new();
    s.eval_source(
        r#"
stopifnot(isTRUE(all.equal(list(1, 2), list(1, 2))))
"#,
    )
    .unwrap();

    s.eval_source(
        r#"
result <- all.equal(list(1, 2), list(1, 3))
stopifnot(is.character(result))
"#,
    )
    .unwrap();
}

#[test]
fn all_equal_list_length_mismatch() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- all.equal(list(1), list(1, 2))
stopifnot(is.character(result))
stopifnot(grepl("Lengths", result))
"#,
    )
    .unwrap();
}

#[test]
fn all_equal_custom_tolerance() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# With a large tolerance, 1 and 1.1 are "equal"
stopifnot(isTRUE(all.equal(1, 1.1, tolerance = 0.2)))
# With a small tolerance, they are not
result <- all.equal(1, 1.1, tolerance = 0.01)
stopifnot(is.character(result))
"#,
    )
    .unwrap();
}

#[test]
fn all_equal_integer_double_comparison() {
    let mut s = Session::new();
    // Integers and doubles should be comparable numerically
    s.eval_source(
        r#"
stopifnot(isTRUE(all.equal(1L, 1.0)))
stopifnot(isTRUE(all.equal(1:3, c(1.0, 2.0, 3.0))))
"#,
    )
    .unwrap();
}

#[test]
fn all_equal_multiple_string_mismatches() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- all.equal(c("a", "b", "c"), c("x", "y", "z"))
stopifnot(is.character(result))
stopifnot(grepl("3 string mismatches", result))
"#,
    )
    .unwrap();
}

#[test]
fn all_equal_empty_vectors() {
    let mut s = Session::new();
    s.eval_source(
        r#"
stopifnot(isTRUE(all.equal(numeric(0), numeric(0))))
stopifnot(isTRUE(all.equal(character(0), character(0))))
"#,
    )
    .unwrap();
}
