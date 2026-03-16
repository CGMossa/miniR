use r::Session;

#[test]
fn as_integer_truncates_double() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        result <- as.integer(2.7)
        stopifnot(identical(result, 2L))
    "#,
    )
    .expect("as.integer(2.7) should truncate to 2L");
}

#[test]
fn as_double_from_integer() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        result <- as.double(3L)
        stopifnot(identical(result, 3.0))
    "#,
    )
    .expect("as.double(3L) should give 3.0");
}

#[test]
fn as_character_from_numeric() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        result <- as.character(42)
        stopifnot(identical(result, "42"))
    "#,
    )
    .expect("as.character(42) should give \"42\"");
}

#[test]
fn as_logical_from_numeric() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        stopifnot(identical(as.logical(0), FALSE))
        stopifnot(identical(as.logical(1), TRUE))
    "#,
    )
    .expect("as.logical(0) should be FALSE, as.logical(1) should be TRUE");
}

#[test]
fn as_logical_from_string() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        stopifnot(identical(as.logical("TRUE"), TRUE))
        stopifnot(identical(as.logical("FALSE"), FALSE))
        stopifnot(identical(as.logical("T"), TRUE))
        stopifnot(identical(as.logical("F"), FALSE))
        stopifnot(is.na(as.logical("yes")))
        stopifnot(is.na(as.logical("no")))
        stopifnot(is.na(as.logical("foo")))
    "#,
    )
    .expect("as.logical from string should parse TRUE/FALSE/T/F");
}

#[test]
fn integer_plus_integer_stays_integer() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        result <- 1L + 1L
        stopifnot(typeof(result) == "integer")
        stopifnot(result == 2L)
    "#,
    )
    .expect("1L + 1L should stay integer");
}

#[test]
fn integer_plus_double_becomes_double() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        result <- 1L + 1.0
        stopifnot(typeof(result) == "double")
        stopifnot(result == 2.0)
    "#,
    )
    .expect("1L + 1.0 should become double");
}

#[test]
fn typeof_colon_sequence_is_integer() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        stopifnot(typeof(1:5) == "integer")
    "#,
    )
    .expect("typeof(1:5) should be \"integer\"");
}

#[test]
fn typeof_c_doubles_is_double() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        stopifnot(typeof(c(1.0, 2.0)) == "double")
    "#,
    )
    .expect("typeof(c(1.0, 2.0)) should be \"double\"");
}

#[test]
fn identical_different_types_is_false() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        stopifnot(identical(TRUE, 1) == FALSE)
    "#,
    )
    .expect("identical(TRUE, 1) should be FALSE (different types)");
}
