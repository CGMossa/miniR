use r::Session;

// region: regexpr

#[test]
fn regexpr_basic_match() {
    let mut s = Session::new();
    s.eval_source(
        r#"
m <- regexpr("[aeiou]", "hello world")
stopifnot(identical(m[[1]], 2L))
stopifnot(identical(attr(m, "match.length")[[1]], 1L))
"#,
    )
    .unwrap();
}

#[test]
fn regexpr_no_match() {
    let mut s = Session::new();
    s.eval_source(
        r#"
m <- regexpr("[0-9]", "hello")
stopifnot(identical(m[[1]], -1L))
stopifnot(identical(attr(m, "match.length")[[1]], -1L))
"#,
    )
    .unwrap();
}



#[test]
fn regexpr_vectorized() {
    let mut s = Session::new();
    s.eval_source(
        r#"
m <- regexpr("[0-9]+", c("abc123", "no digits", "42xyz"))
stopifnot(identical(m[[1]], 4L))
stopifnot(identical(attr(m, "match.length")[[1]], 3L))
stopifnot(identical(m[[2]], -1L))
stopifnot(identical(m[[3]], 1L))
stopifnot(identical(attr(m, "match.length")[[3]], 2L))
"#,
    )
    .unwrap();
}



// endregion

// region: gregexpr

#[test]
fn gregexpr_finds_all_matches() {
    let mut s = Session::new();
    s.eval_source(
        r#"
m <- gregexpr("[aeiou]", "hello world")
stopifnot(identical(m[[1]][[1]], 2L))
stopifnot(identical(m[[1]][[2]], 5L))
stopifnot(identical(m[[1]][[3]], 8L))
stopifnot(identical(length(m[[1]]), 3L))
"#,
    )
    .unwrap();
}

#[test]
fn gregexpr_no_match() {
    let mut s = Session::new();
    s.eval_source(
        r#"
m <- gregexpr("[0-9]", "hello")
stopifnot(identical(m[[1]][[1]], -1L))
stopifnot(identical(attr(m[[1]], "match.length")[[1]], -1L))
"#,
    )
    .unwrap();
}





#[test]
fn gregexpr_vectorized() {
    let mut s = Session::new();
    s.eval_source(
        r#"
m <- gregexpr("[aeiou]", c("hello", "xyz"))
stopifnot(identical(length(m[[1]]), 2L))  # e, o
stopifnot(identical(m[[2]][[1]], -1L))    # no match
"#,
    )
    .unwrap();
}



// endregion

// region: regexec

#[test]
fn regexec_with_capture_groups() {
    let mut s = Session::new();
    s.eval_source(
        r#"
m <- regexec("(\\d+)-(\\d+)-(\\d+)", "date is 2024-01-15")
# Full match + 3 capture groups = 4 elements
stopifnot(identical(length(m[[1]]), 4L))
# Full match starts at position 9
stopifnot(identical(m[[1]][[1]], 9L))
stopifnot(identical(attr(m[[1]], "match.length")[[1]], 10L))
# First capture group "2024" at position 9, length 4
stopifnot(identical(m[[1]][[2]], 9L))
stopifnot(identical(attr(m[[1]], "match.length")[[2]], 4L))
# Second capture group "01" at position 14, length 2
stopifnot(identical(m[[1]][[3]], 14L))
stopifnot(identical(attr(m[[1]], "match.length")[[3]], 2L))
# Third capture group "15" at position 17, length 2
stopifnot(identical(m[[1]][[4]], 17L))
stopifnot(identical(attr(m[[1]], "match.length")[[4]], 2L))
"#,
    )
    .unwrap();
}

#[test]
fn regexec_no_match() {
    let mut s = Session::new();
    s.eval_source(
        r#"
m <- regexec("xyz", "hello world")
stopifnot(identical(m[[1]][[1]], -1L))
stopifnot(identical(attr(m[[1]], "match.length")[[1]], -1L))
"#,
    )
    .unwrap();
}



#[test]
fn regexec_vectorized() {
    let mut s = Session::new();
    s.eval_source(
        r#"
m <- regexec("(\\d+)", c("abc123", "no digits", "42"))
stopifnot(identical(m[[1]][[1]], 4L))
stopifnot(identical(m[[2]][[1]], -1L))
stopifnot(identical(m[[3]][[1]], 1L))
"#,
    )
    .unwrap();
}



// endregion

// region: gsub backreferences

#[test]
fn gsub_backreference_swap() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- gsub("(\\w+) (\\w+)", "\\2 \\1", "hello world")
stopifnot(identical(result, "world hello"))
"#,
    )
    .unwrap();
}

#[test]
fn gsub_backreference_full_match() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- gsub("[aeiou]", "[\\0]", "hello")
stopifnot(identical(result, "h[e]ll[o]"))
"#,
    )
    .unwrap();
}

#[test]
fn sub_backreference() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- sub("(\\w+)", "[\\1]", "hello world")
stopifnot(identical(result, "[hello] world"))
"#,
    )
    .unwrap();
}

#[test]
fn gsub_backreference_with_multiple_groups() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- gsub("(\\d{4})-(\\d{2})-(\\d{2})", "\\2/\\3/\\1", "2024-01-15")
stopifnot(identical(result, "01/15/2024"))
"#,
    )
    .unwrap();
}

// endregion

// region: strsplit regex support

#[test]
fn strsplit_regex_character_class() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- strsplit("hello123world456", "[0-9]+")
stopifnot(identical(result[[1]][[1]], "hello"))
stopifnot(identical(result[[1]][[2]], "world"))
"#,
    )
    .unwrap();
}

#[test]
fn strsplit_regex_dot() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Without fixed=TRUE, "." is regex metachar matching any char
result <- strsplit("a.b.c", ".")
# Each char is a match, so everything splits
stopifnot(identical(length(result[[1]]), 6L))
"#,
    )
    .unwrap();
}

#[test]
fn strsplit_regex_escaped_dot() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- strsplit("a.b.c", "\\.")
stopifnot(identical(result[[1]], c("a", "b", "c")))
"#,
    )
    .unwrap();
}

#[test]
fn strsplit_regex_alternation() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- strsplit("a-b_c-d", "[-_]")
stopifnot(identical(result[[1]], c("a", "b", "c", "d")))
"#,
    )
    .unwrap();
}

#[test]
fn strsplit_regex_whitespace() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- strsplit("hello  world\tfoo", "\\s+")
stopifnot(identical(result[[1]], c("hello", "world", "foo")))
"#,
    )
    .unwrap();
}

// endregion

// region: tolower / toupper

#[test]
fn tolower_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
stopifnot(identical(tolower("HELLO"), "hello"))
stopifnot(identical(tolower(c("ABC", "DEF")), c("abc", "def")))
"#,
    )
    .unwrap();
}

#[test]
fn toupper_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
stopifnot(identical(toupper("hello"), "HELLO"))
stopifnot(identical(toupper(c("abc", "def")), c("ABC", "DEF")))
"#,
    )
    .unwrap();
}

#[test]
fn tolower_na_preserved() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- tolower(c("ABC", NA, "DEF"))
stopifnot(identical(result[1], "abc"))
stopifnot(is.na(result[2]))
stopifnot(identical(result[3], "def"))
"#,
    )
    .unwrap();
}

#[test]
fn toupper_na_preserved() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- toupper(c("abc", NA, "def"))
stopifnot(identical(result[1], "ABC"))
stopifnot(is.na(result[2]))
stopifnot(identical(result[3], "DEF"))
"#,
    )
    .unwrap();
}





// endregion

// region: grep/grepl coercion







// endregion

// region: regmatches with regexpr/gregexpr

#[test]
fn regmatches_with_regexpr() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- c("hello123", "world456")
m <- regexpr("[0-9]+", x)
result <- regmatches(x, m)
stopifnot(identical(result, c("123", "456")))
"#,
    )
    .unwrap();
}

#[test]
fn regmatches_with_gregexpr() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- c("a1b2c3", "x9y")
m <- gregexpr("[0-9]", x)
result <- regmatches(x, m)
stopifnot(identical(result[[1]], c("1", "2", "3")))
stopifnot(identical(result[[2]], "9"))
"#,
    )
    .unwrap();
}

// endregion

// region: regex features

#[test]
fn regex_alternation() {
    let mut s = Session::new();
    s.eval_source(
        r#"
stopifnot(identical(grepl("cat|dog", "I have a dog"), TRUE))
stopifnot(identical(grepl("cat|dog", "I have a bird"), FALSE))
"#,
    )
    .unwrap();
}

#[test]
fn regex_quantifiers() {
    let mut s = Session::new();
    s.eval_source(
        r#"
stopifnot(identical(grepl("^a{3}$", "aaa"), TRUE))
stopifnot(identical(grepl("^a{3}$", "aa"), FALSE))
stopifnot(identical(grepl("^a+$", "aaa"), TRUE))
stopifnot(identical(grepl("^a?b$", "b"), TRUE))
stopifnot(identical(grepl("^a?b$", "ab"), TRUE))
stopifnot(identical(grepl("^a?b$", "aab"), FALSE))
"#,
    )
    .unwrap();
}

#[test]
fn regex_character_classes() {
    let mut s = Session::new();
    s.eval_source(
        r#"
stopifnot(identical(grepl("\\d+", "abc123"), TRUE))
stopifnot(identical(grepl("\\w+", "hello"), TRUE))
stopifnot(identical(grepl("\\s", "no spaces"), TRUE))
stopifnot(identical(grepl("[[:alpha:]]+", "abc123"), TRUE))
stopifnot(identical(grepl("[[:digit:]]+", "abc123"), TRUE))
"#,
    )
    .unwrap();
}

#[test]
fn regex_anchors() {
    let mut s = Session::new();
    s.eval_source(
        r#"
stopifnot(identical(grepl("^hello", "hello world"), TRUE))
stopifnot(identical(grepl("^hello", "say hello"), FALSE))
stopifnot(identical(grepl("world$", "hello world"), TRUE))
stopifnot(identical(grepl("world$", "world says"), FALSE))
"#,
    )
    .unwrap();
}

#[test]
fn regex_ignore_case() {
    let mut s = Session::new();
    s.eval_source(
        r#"
stopifnot(identical(grepl("hello", "HELLO WORLD", ignore.case = TRUE), TRUE))
stopifnot(identical(grepl("hello", "HELLO WORLD", ignore.case = FALSE), FALSE))
stopifnot(identical(regexpr("abc", "ABC", ignore.case = TRUE)[[1]], 1L))
"#,
    )
    .unwrap();
}

// endregion
