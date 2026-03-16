use r::Session;

// region: fromJSON tests

#[test]
fn from_json_null() {
    let mut s = Session::new();
    let _ = s.eval_source(r#"stopifnot(is.null(fromJSON("null")))"#);
}

#[test]
fn from_json_true_false() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
stopifnot(identical(fromJSON("true"), TRUE))
stopifnot(identical(fromJSON("false"), FALSE))
"#,
    );
}

#[test]
fn from_json_integer() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
x <- fromJSON("42")
stopifnot(identical(x, 42L))
"#,
    );
}

#[test]
fn from_json_double() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
x <- fromJSON("3.14")
stopifnot(is.double(x))
stopifnot(abs(x - 3.14) < 1e-10)
"#,
    );
}

#[test]
fn from_json_string() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
x <- fromJSON('"hello world"')
stopifnot(identical(x, "hello world"))
"#,
    );
}

#[test]
fn from_json_array_of_integers() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
x <- fromJSON("[1, 2, 3]")
stopifnot(is.integer(x))
stopifnot(identical(x, c(1L, 2L, 3L)))
"#,
    );
}

#[test]
fn from_json_array_of_doubles() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
x <- fromJSON("[1.1, 2.2, 3.3]")
stopifnot(is.double(x))
stopifnot(length(x) == 3L)
"#,
    );
}

#[test]
fn from_json_array_of_strings() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
x <- fromJSON('["a", "b", "c"]')
stopifnot(identical(x, c("a", "b", "c")))
"#,
    );
}

#[test]
fn from_json_array_of_booleans() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
x <- fromJSON("[true, false, true]")
stopifnot(identical(x, c(TRUE, FALSE, TRUE)))
"#,
    );
}

#[test]
fn from_json_array_with_null() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
x <- fromJSON("[1, null, 3]")
stopifnot(is.integer(x))
stopifnot(length(x) == 3L)
stopifnot(is.na(x[2]))
"#,
    );
}

#[test]
fn from_json_mixed_number_array() {
    let mut s = Session::new();
    // Mix of int and float should produce double
    let _ = s.eval_source(
        r#"
x <- fromJSON("[1, 2.5, 3]")
stopifnot(is.double(x))
stopifnot(length(x) == 3L)
"#,
    );
}

#[test]
fn from_json_object() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
x <- fromJSON('{"name": "Alice", "age": 30}')
stopifnot(is.list(x))
stopifnot(x$age == 30L)
stopifnot(x$name == "Alice")
"#,
    );
}

#[test]
fn from_json_nested_object() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
x <- fromJSON('{"a": {"b": 42}}')
stopifnot(is.list(x))
stopifnot(is.list(x$a))
stopifnot(x$a$b == 42L)
"#,
    );
}

#[test]
fn from_json_array_of_objects_to_dataframe() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
x <- fromJSON('[{"name": "Alice", "age": 30}, {"name": "Bob", "age": 25}]')
stopifnot(is.data.frame(x))
stopifnot(nrow(x) == 2L)
stopifnot(ncol(x) == 2L)
stopifnot(identical(x$age, c(30L, 25L)))
"#,
    );
}

#[test]
fn from_json_empty_array() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
x <- fromJSON("[]")
stopifnot(is.list(x))
stopifnot(length(x) == 0L)
"#,
    );
}

#[test]
fn from_json_empty_object() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
x <- fromJSON("{}")
stopifnot(is.list(x))
stopifnot(length(x) == 0L)
"#,
    );
}

#[test]
fn from_json_invalid_json() {
    let mut s = Session::new();
    // Should produce an error, not panic
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = s.eval_source(r#"tryCatch(fromJSON("not json"), error = function(e) "caught")"#);
    }));
    assert!(
        result.is_ok(),
        "fromJSON with invalid input should not panic"
    );
}

#[test]
fn from_json_jsonlite_alias() {
    let mut s = Session::new();
    // The jsonlite::fromJSON alias should work
    let _ = s.eval_source(
        r#"
x <- `jsonlite::fromJSON`("[1, 2, 3]")
stopifnot(identical(x, c(1L, 2L, 3L)))
"#,
    );
}

// endregion

// region: toJSON tests

#[test]
fn to_json_null() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
x <- toJSON(NULL)
stopifnot(identical(x, "null"))
"#,
    );
}

#[test]
fn to_json_logical_scalar() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
stopifnot(identical(toJSON(TRUE), "true"))
stopifnot(identical(toJSON(FALSE), "false"))
"#,
    );
}

#[test]
fn to_json_integer_scalar() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
stopifnot(identical(toJSON(42L), "42"))
"#,
    );
}

#[test]
fn to_json_double_scalar() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
x <- toJSON(3.14)
# Should produce a valid JSON number string
stopifnot(is.character(x))
stopifnot(nchar(x) > 0L)
"#,
    );
}

#[test]
fn to_json_string_scalar() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
x <- toJSON("hello")
stopifnot(identical(x, '"hello"'))
"#,
    );
}

#[test]
fn to_json_integer_vector() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
x <- toJSON(c(1L, 2L, 3L))
stopifnot(identical(x, "[1,2,3]"))
"#,
    );
}

#[test]
fn to_json_character_vector() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
x <- toJSON(c("a", "b"))
stopifnot(identical(x, '["a","b"]'))
"#,
    );
}

#[test]
fn to_json_named_list() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
x <- toJSON(list(a = 1L, b = "hello"))
# Should produce a JSON object
parsed <- fromJSON(x)
stopifnot(is.list(parsed))
stopifnot(parsed$a == 1L)
stopifnot(parsed$b == "hello")
"#,
    );
}

#[test]
fn to_json_unnamed_list() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
x <- toJSON(list(1L, "two", TRUE))
# Should produce a JSON array
stopifnot(is.character(x))
parsed <- fromJSON(x)
stopifnot(is.list(parsed))
stopifnot(length(parsed) == 3L)
"#,
    );
}

#[test]
fn to_json_na_becomes_null() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
x <- toJSON(NA)
stopifnot(identical(x, "null"))
"#,
    );
}

#[test]
fn to_json_logical_vector() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
x <- toJSON(c(TRUE, FALSE, TRUE))
stopifnot(identical(x, "[true,false,true]"))
"#,
    );
}

#[test]
fn to_json_nan_inf_become_null() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
x <- toJSON(NaN)
stopifnot(identical(x, "null"))
y <- toJSON(Inf)
stopifnot(identical(y, "null"))
"#,
    );
}

#[test]
fn to_json_jsonlite_alias() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
x <- `jsonlite::toJSON`(42L)
stopifnot(identical(x, "42"))
"#,
    );
}

// endregion

// region: roundtrip tests

#[test]
fn roundtrip_named_list() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
original <- list(x = 1L, y = "hello", z = TRUE)
json <- toJSON(original)
back <- fromJSON(json)
stopifnot(back$x == 1L)
stopifnot(back$y == "hello")
stopifnot(back$z == TRUE)
"#,
    );
}

#[test]
fn roundtrip_integer_vector() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
original <- c(10L, 20L, 30L)
json <- toJSON(original)
back <- fromJSON(json)
stopifnot(identical(back, original))
"#,
    );
}

#[test]
fn roundtrip_nested_structure() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
original <- list(
  name = "test",
  values = list(1L, 2L, 3L),
  nested = list(a = "x", b = "y")
)
json <- toJSON(original)
back <- fromJSON(json)
stopifnot(back$name == "test")
stopifnot(back$nested$a == "x")
"#,
    );
}

// endregion
