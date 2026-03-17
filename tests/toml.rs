use r::Session;

// region: toml_parse tests

#[test]
fn toml_parse_string() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
x <- toml_parse('name = "Alice"')
stopifnot(is.list(x))
stopifnot(identical(x$name, "Alice"))
"#,
    );
}

#[test]
fn toml_parse_integer() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
x <- toml_parse("count = 42")
stopifnot(is.list(x))
stopifnot(identical(x$count, 42L))
"#,
    );
}

#[test]
fn toml_parse_float() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
x <- toml_parse("pi = 3.14")
stopifnot(is.list(x))
stopifnot(is.double(x$pi))
stopifnot(abs(x$pi - 3.14) < 1e-10)
"#,
    );
}

#[test]
fn toml_parse_boolean() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
x <- toml_parse("enabled = true\ndisabled = false")
stopifnot(identical(x$enabled, TRUE))
stopifnot(identical(x$disabled, FALSE))
"#,
    );
}

#[test]
fn toml_parse_nested_table() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
toml_text <- "[server]\nhost = \"localhost\"\nport = 8080"
x <- toml_parse(toml_text)
stopifnot(is.list(x))
stopifnot(is.list(x$server))
stopifnot(identical(x$server$host, "localhost"))
stopifnot(identical(x$server$port, 8080L))
"#,
    );
}

#[test]
fn toml_parse_array_of_integers() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
x <- toml_parse("values = [1, 2, 3]")
stopifnot(is.integer(x$values))
stopifnot(identical(x$values, c(1L, 2L, 3L)))
"#,
    );
}

#[test]
fn toml_parse_array_of_strings() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
x <- toml_parse('colors = ["red", "green", "blue"]')
stopifnot(is.character(x$colors))
stopifnot(identical(x$colors, c("red", "green", "blue")))
"#,
    );
}

#[test]
fn toml_parse_array_of_booleans() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
x <- toml_parse("flags = [true, false, true]")
stopifnot(is.logical(x$flags))
stopifnot(identical(x$flags, c(TRUE, FALSE, TRUE)))
"#,
    );
}

#[test]
fn toml_parse_array_of_floats() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
x <- toml_parse("values = [1.1, 2.2, 3.3]")
stopifnot(is.double(x$values))
stopifnot(length(x$values) == 3L)
"#,
    );
}

#[test]
fn toml_parse_empty_table() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
x <- toml_parse("")
stopifnot(is.list(x))
stopifnot(length(x) == 0L)
"#,
    );
}

#[test]
fn toml_parse_invalid_toml() {
    let mut s = Session::new();
    let result = s.eval_source(r#"toml_parse("= invalid")"#);
    assert!(result.is_err(), "should error on invalid TOML");
}

#[test]
fn toml_parse_array_of_inline_tables_to_dataframe() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
toml_text <- 'records = [{name = "Alice", age = 30}, {name = "Bob", age = 25}]'
x <- toml_parse(toml_text)
stopifnot(is.data.frame(x$records))
stopifnot(identical(x$records$name, c("Alice", "Bob")))
stopifnot(identical(x$records$age, c(30L, 25L)))
"#,
    );
}

#[test]
fn toml_parse_datetime_becomes_character() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
x <- toml_parse('dt = 1979-05-27T07:32:00')
stopifnot(is.character(x$dt))
"#,
    );
}

// endregion

// region: toml_serialize tests

#[test]
fn toml_serialize_simple_list() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
x <- list(name = "Alice", age = 30L)
result <- toml_serialize(x)
stopifnot(is.character(result))
stopifnot(length(result) == 1L)
# Parse back to verify roundtrip
y <- toml_parse(result)
stopifnot(identical(y$name, "Alice"))
stopifnot(identical(y$age, 30L))
"#,
    );
}

#[test]
fn toml_serialize_nested_list() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
x <- list(server = list(host = "localhost", port = 8080L))
result <- toml_serialize(x)
stopifnot(is.character(result))
y <- toml_parse(result)
stopifnot(identical(y$server$host, "localhost"))
stopifnot(identical(y$server$port, 8080L))
"#,
    );
}

#[test]
fn toml_serialize_vector_values() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
x <- list(tags = c("a", "b", "c"), counts = c(1L, 2L, 3L))
result <- toml_serialize(x)
y <- toml_parse(result)
stopifnot(identical(y$tags, c("a", "b", "c")))
stopifnot(identical(y$counts, c(1L, 2L, 3L)))
"#,
    );
}

#[test]
fn toml_serialize_boolean() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
x <- list(flag = TRUE)
result <- toml_serialize(x)
y <- toml_parse(result)
stopifnot(identical(y$flag, TRUE))
"#,
    );
}

#[test]
fn toml_serialize_null_values_omitted() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
x <- list(a = 1L, b = NULL, c = "hello")
result <- toml_serialize(x)
y <- toml_parse(result)
stopifnot(identical(y$a, 1L))
stopifnot(identical(y$c, "hello"))
# NULL values should be omitted, so b should not exist
stopifnot(is.null(y$b))
"#,
    );
}

#[test]
fn toml_serialize_rejects_unnamed_list() {
    let mut s = Session::new();
    let result = s.eval_source(
        r#"
x <- list(1, 2, 3)
toml_serialize(x)
"#,
    );
    assert!(result.is_err(), "should reject unnamed list");
}

#[test]
fn toml_serialize_rejects_non_list() {
    let mut s = Session::new();
    let result = s.eval_source(r#"toml_serialize(42)"#);
    assert!(result.is_err(), "should reject non-list input");
}

#[test]
fn toml_serialize_rejects_na_values() {
    let mut s = Session::new();
    let result = s.eval_source(
        r#"
x <- list(value = NA)
toml_serialize(x)
"#,
    );
    assert!(result.is_err(), "should reject NA values");
}

// endregion

// region: read.toml / write.toml file I/O tests

#[test]
fn write_toml_then_read_toml_roundtrip() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
x <- list(title = "Example", version = 1L, enabled = TRUE)
tmpfile <- tempfile(fileext = ".toml")
write.toml(x, tmpfile)
y <- read.toml(tmpfile)
stopifnot(identical(y$title, "Example"))
stopifnot(identical(y$version, 1L))
stopifnot(identical(y$enabled, TRUE))
"#,
    );
}

#[test]
fn read_toml_nonexistent_file() {
    let mut s = Session::new();
    let result = s.eval_source(r#"read.toml("/nonexistent/path/file.toml")"#);
    assert!(result.is_err(), "should error on nonexistent file");
}

#[test]
fn write_toml_nested_roundtrip() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
x <- list(
    package = list(name = "mypack", version = "0.1.0"),
    deps = list(rlang = "1.0", dplyr = "1.1")
)
tmpfile <- tempfile(fileext = ".toml")
write.toml(x, tmpfile)
y <- read.toml(tmpfile)
stopifnot(identical(y$package$name, "mypack"))
stopifnot(identical(y$package$version, "0.1.0"))
stopifnot(identical(y$deps$rlang, "1.0"))
stopifnot(identical(y$deps$dplyr, "1.1"))
"#,
    );
}

// endregion

// region: array-of-tables tests

#[test]
fn toml_parse_array_of_tables() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
toml_text <- "[[products]]\nname = \"apple\"\nprice = 1.5\n\n[[products]]\nname = \"banana\"\nprice = 0.5"
x <- toml_parse(toml_text)
stopifnot(is.data.frame(x$products))
stopifnot(identical(x$products$name, c("apple", "banana")))
stopifnot(is.double(x$products$price))
"#,
    );
}

#[test]
fn toml_parse_mixed_array_to_list() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
toml_text <- 'mixed = [1, "two", 3]'
x <- toml_parse(toml_text)
stopifnot(is.list(x$mixed))
"#,
    );
}

// endregion

// region: roundtrip tests

#[test]
fn toml_roundtrip_double_values() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
x <- list(pi = 3.14159, e = 2.71828)
result <- toml_serialize(x)
y <- toml_parse(result)
stopifnot(abs(y$pi - 3.14159) < 1e-5)
stopifnot(abs(y$e - 2.71828) < 1e-5)
"#,
    );
}

#[test]
fn toml_roundtrip_logical_vector() {
    let mut s = Session::new();
    let _ = s.eval_source(
        r#"
x <- list(flags = c(TRUE, FALSE, TRUE))
result <- toml_serialize(x)
y <- toml_parse(result)
stopifnot(identical(y$flags, c(TRUE, FALSE, TRUE)))
"#,
    );
}

// endregion
