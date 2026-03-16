use r::Session;

// region: Set operations (enhanced)

#[test]
fn setdiff_numeric_vectors() {
    let mut r = Session::new();
    r.eval_source("x <- setdiff(1:5, 3:7)").unwrap();
    let result = r.eval_source("x").unwrap().value;
    let v = result.as_vector().unwrap();
    assert_eq!(v.to_integers(), vec![Some(1), Some(2)]);
}

#[test]
fn intersect_numeric_vectors() {
    let mut r = Session::new();
    r.eval_source("x <- intersect(1:5, 3:7)").unwrap();
    let result = r.eval_source("x").unwrap().value;
    let v = result.as_vector().unwrap();
    assert_eq!(v.to_integers(), vec![Some(3), Some(4), Some(5)]);
}

#[test]
fn union_numeric_vectors() {
    let mut r = Session::new();
    r.eval_source("x <- union(1:3, 2:4)").unwrap();
    let result = r.eval_source("x").unwrap().value;
    let v = result.as_vector().unwrap();
    assert_eq!(v.to_integers(), vec![Some(1), Some(2), Some(3), Some(4)]);
}

#[test]
fn setdiff_character_vectors() {
    let mut r = Session::new();
    r.eval_source(r#"x <- setdiff(c("a","b","c"), c("b","d"))"#)
        .unwrap();
    let result = r.eval_source("x").unwrap().value;
    let v = result.as_vector().unwrap();
    assert_eq!(
        v.to_characters(),
        vec![Some("a".to_string()), Some("c".to_string())]
    );
}

// endregion

// region: cut

#[test]
fn cut_bins_numeric_values() {
    let mut r = Session::new();
    // cut with right=TRUE (default): intervals (0,5], (5,10]
    r.eval_source("x <- cut(c(1, 5, 7, 10), breaks = c(0, 5, 10))")
        .unwrap();
    // Check that it returns a factor
    let cls = r.eval_source("class(x)").unwrap().value;
    assert_eq!(
        cls.as_vector().unwrap().as_character_scalar().unwrap(),
        "factor"
    );
    // Check levels
    let levels = r.eval_source("levels(x)").unwrap().value;
    let lvls = levels.as_vector().unwrap().to_characters();
    assert_eq!(
        lvls,
        vec![Some("(0,5]".to_string()), Some("(5,10]".to_string())]
    );
}

#[test]
fn cut_out_of_range_returns_na() {
    let mut r = Session::new();
    // Value 0 is outside (0,5] with right=TRUE
    r.eval_source("x <- cut(c(0, 3), breaks = c(0, 5))")
        .unwrap();
    let result = r.eval_source("is.na(x[1])").unwrap().value;
    assert!(result.as_vector().unwrap().as_logical_scalar().unwrap());
}

// endregion

// region: findInterval

#[test]
fn find_interval_basic() {
    let mut r = Session::new();
    // findInterval(3.5, c(1, 2, 3, 4, 5)) should return 3
    r.eval_source("x <- findInterval(3.5, c(1, 2, 3, 4, 5))")
        .unwrap();
    let result = r.eval_source("x").unwrap().value;
    assert_eq!(result.as_vector().unwrap().as_integer_scalar(), Some(3));
}

#[test]
fn find_interval_vector() {
    let mut r = Session::new();
    r.eval_source("x <- findInterval(c(0.5, 2.5, 4.5), c(1, 2, 3, 4))")
        .unwrap();
    let result = r.eval_source("x").unwrap().value;
    let v = result.as_vector().unwrap();
    assert_eq!(v.to_integers(), vec![Some(0), Some(2), Some(4)]);
}

// endregion

// region: Find, Position, Negate

#[test]
fn find_returns_first_match() {
    let mut r = Session::new();
    r.eval_source("x <- Find(function(x) x > 3, c(1, 2, 4, 5))")
        .unwrap();
    let result = r.eval_source("x").unwrap().value;
    assert_eq!(result.as_vector().unwrap().as_double_scalar(), Some(4.0));
}

#[test]
fn find_returns_null_when_no_match() {
    let mut r = Session::new();
    let result = r
        .eval_source("Find(function(x) x > 10, 1:5)")
        .unwrap()
        .value;
    assert!(result.is_null());
}

#[test]
fn position_returns_1based_index() {
    let mut r = Session::new();
    r.eval_source("x <- Position(function(x) x > 3, c(1, 2, 4, 5))")
        .unwrap();
    let result = r.eval_source("x").unwrap().value;
    assert_eq!(result.as_vector().unwrap().as_integer_scalar(), Some(3));
}

#[test]
fn position_returns_null_when_no_match() {
    let mut r = Session::new();
    let result = r
        .eval_source("Position(function(x) x > 10, 1:5)")
        .unwrap()
        .value;
    assert!(result.is_null());
}

#[test]
fn negate_reverses_predicate() {
    let mut r = Session::new();
    r.eval_source("not_even <- Negate(function(x) x %% 2 == 0)")
        .unwrap();
    let result = r.eval_source("not_even(4)").unwrap().value;
    assert!(!result.as_vector().unwrap().as_logical_scalar().unwrap());
    let result = r.eval_source("not_even(3)").unwrap().value;
    assert!(result.as_vector().unwrap().as_logical_scalar().unwrap());
}

// endregion

// region: rapply

#[test]
fn rapply_unlist_doubles_all_leaves() {
    let mut r = Session::new();
    r.eval_source("x <- rapply(list(1, list(2, 3)), function(x) x * 2)")
        .unwrap();
    let result = r.eval_source("x").unwrap().value;
    let v = result.as_vector().unwrap();
    assert_eq!(v.to_doubles(), vec![Some(2.0), Some(4.0), Some(6.0)]);
}

#[test]
fn rapply_replace_preserves_structure() {
    let mut r = Session::new();
    r.eval_source(r#"x <- rapply(list(1, list(2, 3)), function(x) x * 10, how = "replace")"#)
        .unwrap();
    // x[[2]][[1]] should be 20
    let result = r.eval_source("x[[2]][[1]]").unwrap().value;
    assert_eq!(result.as_vector().unwrap().as_double_scalar(), Some(20.0));
}

// endregion

// region: match.call

#[test]
fn match_call_returns_language_object() {
    let mut r = Session::new();
    r.eval_source(
        r#"
f <- function(x, y, z = 1) match.call()
result <- f(10, y = 20)
"#,
    )
    .unwrap();
    // result should be a language object; deparse it
    let result = r.eval_source("deparse(result)").unwrap().value;
    let s = result.as_vector().unwrap().as_character_scalar().unwrap();
    // Should contain named arguments
    assert!(s.contains("x = "), "Expected 'x = ' in: {}", s);
    assert!(s.contains("y = "), "Expected 'y = ' in: {}", s);
}

// endregion

// region: on.exit after parameter

#[test]
fn on_exit_after_false_prepends() {
    let mut r = Session::new();
    r.eval_source(
        r#"
result <- character(0)
f <- function() {
    on.exit(result <<- c(result, "first"))
    on.exit(result <<- c(result, "second"), add = TRUE, after = FALSE)
}
f()
"#,
    )
    .unwrap();
    let result = r.eval_source("result").unwrap().value;
    let v = result.as_vector().unwrap().to_characters();
    // "second" should run before "first" because after=FALSE prepends
    assert_eq!(
        v,
        vec![Some("second".to_string()), Some("first".to_string())]
    );
}

// endregion

// region: system.file, Sys.getpid

#[test]
fn system_file_returns_empty_string() {
    let mut r = Session::new();
    let result = r
        .eval_source(r#"system.file("DESCRIPTION", package = "base")"#)
        .unwrap()
        .value;
    assert_eq!(
        result.as_vector().unwrap().as_character_scalar().unwrap(),
        ""
    );
}

#[test]
fn sys_getpid_returns_integer() {
    let mut r = Session::new();
    let result = r.eval_source("Sys.getpid()").unwrap().value;
    let pid = result.as_vector().unwrap().as_integer_scalar().unwrap();
    assert!(pid > 0, "PID should be positive, got {}", pid);
}

// endregion

// region: normalizePath with mustWork

#[test]
fn normalize_path_must_work_errors_on_missing() {
    let mut r = Session::new();
    let result = r.eval_source(r#"normalizePath("/nonexistent/path/xyz", mustWork = TRUE)"#);
    assert!(
        result.is_err(),
        "Expected error for missing path with mustWork=TRUE"
    );
}

#[test]
fn normalize_path_default_returns_original_on_missing() {
    let mut r = Session::new();
    let result = r
        .eval_source(r#"normalizePath("/nonexistent/path/xyz")"#)
        .unwrap()
        .value;
    assert_eq!(
        result.as_vector().unwrap().as_character_scalar().unwrap(),
        "/nonexistent/path/xyz"
    );
}

// endregion
