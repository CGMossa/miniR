use r::Session;

#[test]
fn options_get_set_roundtrip() {
    let mut r = Session::new();
    r.eval_source(r#"options(my_test_opt = 42)"#).unwrap();
    let val = r.eval_source(r#"getOption("my_test_opt")"#).unwrap().value;
    assert_eq!(
        val.as_vector().and_then(|v| v.as_double_scalar()),
        Some(42.0)
    );
}

#[test]
fn options_returns_previous_value() {
    let mut r = Session::new();
    r.eval_source(r#"options(digits = 7)"#).unwrap();
    r.eval_source(r#"old <- options(digits = 3)"#).unwrap();
    let old_val = r.eval_source(r#"old$digits"#).unwrap().value;
    assert_eq!(
        old_val.as_vector().and_then(|v| v.as_double_scalar()),
        Some(7.0)
    );
}

#[test]
fn get_option_with_default() {
    let mut r = Session::new();
    // getOption uses positional arg for default, not named
    let val = r
        .eval_source(r#"getOption("nonexistent", 99)"#)
        .unwrap()
        .value;
    assert_eq!(
        val.as_vector().and_then(|v| v.as_double_scalar()),
        Some(99.0)
    );
}

#[test]
fn machine_constants() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        stopifnot(.Machine$integer.max == 2147483647)
        stopifnot(.Machine$double.eps > 0)
        stopifnot(.Machine$double.eps < 1e-14)
        stopifnot(is.numeric(.Machine$sizeof.pointer))
    "#,
    )
    .unwrap();
}

#[test]
fn gc_stub_does_not_error() {
    let mut r = Session::new();
    r.eval_source("gc()").unwrap();
    r.eval_source("gcinfo(FALSE)").unwrap();
}

#[test]
fn debug_stubs_do_not_error() {
    let mut r = Session::new();
    r.eval_source("f <- function() 1; debug(f); undebug(f)")
        .unwrap();
    let val = r.eval_source("isdebugged(f)").unwrap().value;
    assert_eq!(
        val.as_vector().and_then(|v| v.as_logical_scalar()),
        Some(false)
    );
}

#[test]
fn sys_localeconv_returns_named_vector() {
    let mut r = Session::new();
    let val = r.eval_source("Sys.localeconv()").unwrap().value;
    assert!(val.as_vector().is_some());
}

// --- stdlib batch 1 ---

#[test]
fn strrep_repeats_strings() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        stopifnot(strrep("ab", 3) == "ababab")
        stopifnot(strrep("x", 0) == "")
    "#,
    )
    .unwrap();
}

#[test]
fn pmatch_partial_matching() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        # "me" matches both "mean" and "median" — ambiguous, returns NA
        stopifnot(is.na(pmatch("me", c("mean", "median"))))
        # "mea" uniquely matches "mean"
        stopifnot(pmatch("mea", c("mean", "median")) == 1)
    "#,
    )
    .unwrap();
}

#[test]
fn charmatch_character_matching() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        stopifnot(charmatch("m", c("mean", "median")) == 0)
        stopifnot(charmatch("med", c("mean", "median")) == 2)
    "#,
    )
    .unwrap();
}

#[test]
fn cat_writes_to_file() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        tf <- tempfile()
        cat("hello world\n", file = tf)
        result <- readLines(tf)
        stopifnot(result == "hello world")
    "#,
    )
    .unwrap();
}

#[test]
fn sys_source_evaluates_in_environment() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        tf <- tempfile()
        writeLines("test_var <- 42", tf)
        e <- new.env(parent = emptyenv())
        sys.source(tf, envir = e)
        stopifnot(get("test_var", envir = e) == 42)
    "#,
    )
    .unwrap();
}
