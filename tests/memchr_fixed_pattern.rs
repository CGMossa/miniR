/// Tests for AhoCorasick-accelerated fixed-pattern string operations and
/// Levenshtein-based approximate matching (agrep/agrepl).
///
/// Each test verifies that `fixed=TRUE` produces the same result as `fixed=FALSE`
/// (regex mode) for literal patterns, ensuring the AhoCorasick fast path is correct.
use r::Session;

// region: grepl fixed=TRUE

#[test]
fn grepl_fixed_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- c("hello world", "goodbye", "hello there", "world hello")
stopifnot(identical(
    grepl("hello", x, fixed = TRUE),
    c(TRUE, FALSE, TRUE, TRUE)
))
"#,
    )
    .unwrap();
}

#[test]
fn grepl_fixed_matches_regex_mode() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- c("foo.bar", "fooxbar", "foo", "bar", "foo.bar.baz")
# With fixed=TRUE, the dot is literal
stopifnot(identical(
    grepl("foo.bar", x, fixed = TRUE),
    c(TRUE, FALSE, FALSE, FALSE, TRUE)
))
# With fixed=FALSE (default), the dot matches any character
stopifnot(identical(
    grepl("foo.bar", x),
    c(TRUE, TRUE, FALSE, FALSE, TRUE)
))
"#,
    )
    .unwrap();
}

#[test]
fn grepl_fixed_special_regex_chars() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Patterns that are regex special chars should be treated literally with fixed=TRUE
x <- c("price is $100", "no dollars here", "$100 off")
stopifnot(identical(
    grepl("$100", x, fixed = TRUE),
    c(TRUE, FALSE, TRUE)
))
"#,
    )
    .unwrap();
}

#[test]
fn grepl_fixed_na_propagation() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- c("abc", NA, "def")
result <- grepl("abc", x, fixed = TRUE)
stopifnot(identical(result[1], TRUE))
stopifnot(is.na(result[2]))
stopifnot(identical(result[3], FALSE))
"#,
    )
    .unwrap();
}

#[test]
fn grepl_fixed_empty_pattern() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Empty pattern matches everything
x <- c("abc", "def", "")
stopifnot(identical(
    grepl("", x, fixed = TRUE),
    c(TRUE, TRUE, TRUE)
))
"#,
    )
    .unwrap();
}

#[test]
fn grepl_fixed_ignore_case() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- c("Hello World", "HELLO", "hello", "goodbye")
stopifnot(identical(
    grepl("hello", x, fixed = TRUE, ignore.case = TRUE),
    c(TRUE, TRUE, TRUE, FALSE)
))
"#,
    )
    .unwrap();
}

// endregion

// region: grep fixed=TRUE

#[test]
fn grep_fixed_indices() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- c("apple", "banana", "pineapple", "grape")
stopifnot(identical(
    grep("apple", x, fixed = TRUE),
    c(1L, 3L)
))
"#,
    )
    .unwrap();
}

#[test]
fn grep_fixed_value() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- c("apple", "banana", "pineapple", "grape")
stopifnot(identical(
    grep("apple", x, fixed = TRUE, value = TRUE),
    c("apple", "pineapple")
))
"#,
    )
    .unwrap();
}

#[test]
fn grep_fixed_special_chars() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Bracket is a regex metacharacter
x <- c("a[1]", "a1", "b[2]", "b2")
stopifnot(identical(
    grep("[1]", x, fixed = TRUE),
    c(1L)
))
"#,
    )
    .unwrap();
}

#[test]
fn grep_fixed_ignore_case() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- c("Apple", "BANANA", "pineapple", "Grape")
stopifnot(identical(
    grep("apple", x, fixed = TRUE, ignore.case = TRUE),
    c(1L, 3L)
))
"#,
    )
    .unwrap();
}

// endregion

// region: sub fixed=TRUE

#[test]
fn sub_fixed_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- c("hello world hello", "goodbye", "hello")
stopifnot(identical(
    sub("hello", "HI", x, fixed = TRUE),
    c("HI world hello", "goodbye", "HI")
))
"#,
    )
    .unwrap();
}

#[test]
fn sub_fixed_special_chars() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# The dot in the pattern should be literal with fixed=TRUE
x <- c("foo.bar", "fooxbar", "foo.bar.baz")
stopifnot(identical(
    sub("foo.bar", "REPLACED", x, fixed = TRUE),
    c("REPLACED", "fooxbar", "REPLACED.baz")
))
"#,
    )
    .unwrap();
}

#[test]
fn sub_fixed_replacement_not_interpreted() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# In fixed mode, replacement is literal (no backreferences)
x <- "hello world"
stopifnot(identical(
    sub("world", "\\1earth", x, fixed = TRUE),
    "hello \\1earth"
))
"#,
    )
    .unwrap();
}

#[test]
fn sub_fixed_na_propagation() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- c("abc", NA, "abcabc")
result <- sub("abc", "X", x, fixed = TRUE)
stopifnot(identical(result[1], "X"))
stopifnot(is.na(result[2]))
stopifnot(identical(result[3], "Xabc"))
"#,
    )
    .unwrap();
}

#[test]
fn sub_fixed_no_match() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- c("hello", "world")
stopifnot(identical(
    sub("xyz", "REPLACED", x, fixed = TRUE),
    c("hello", "world")
))
"#,
    )
    .unwrap();
}

// endregion

// region: gsub fixed=TRUE

#[test]
fn gsub_fixed_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- c("hello world hello", "goodbye", "hello hello hello")
stopifnot(identical(
    gsub("hello", "HI", x, fixed = TRUE),
    c("HI world HI", "goodbye", "HI HI HI")
))
"#,
    )
    .unwrap();
}

#[test]
fn gsub_fixed_special_chars() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Parentheses are regex metacharacters
x <- c("f(x)", "f(x) + g(x)", "no parens")
stopifnot(identical(
    gsub("(x)", "[y]", x, fixed = TRUE),
    c("f[y]", "f[y] + g[y]", "no parens")
))
"#,
    )
    .unwrap();
}

#[test]
fn gsub_fixed_replacement_with_dollar() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Dollar sign in replacement is literal in fixed mode
x <- "price: 100"
stopifnot(identical(
    gsub("100", "$200", x, fixed = TRUE),
    "price: $200"
))
"#,
    )
    .unwrap();
}

#[test]
fn gsub_fixed_na_propagation() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- c("aaa", NA, "bbb")
result <- gsub("a", "X", x, fixed = TRUE)
stopifnot(identical(result[1], "XXX"))
stopifnot(is.na(result[2]))
stopifnot(identical(result[3], "bbb"))
"#,
    )
    .unwrap();
}

#[test]
fn gsub_fixed_empty_replacement() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Remove all occurrences
x <- "a.b.c.d"
stopifnot(identical(
    gsub(".", "", x, fixed = TRUE),
    "abcd"
))
"#,
    )
    .unwrap();
}

#[test]
fn gsub_fixed_adjacent_matches() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Adjacent non-overlapping matches
x <- "aaa"
stopifnot(identical(
    gsub("aa", "X", x, fixed = TRUE),
    "Xa"
))
"#,
    )
    .unwrap();
}

// endregion

// region: strsplit fixed=TRUE

#[test]
fn strsplit_fixed_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- strsplit("a.b.c", ".", fixed = TRUE)
stopifnot(identical(result[[1]], c("a", "b", "c")))
"#,
    )
    .unwrap();
}

#[test]
fn strsplit_fixed_vs_regex() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# With fixed=TRUE, dot is literal
fixed_result <- strsplit("a.b.c", ".", fixed = TRUE)
stopifnot(identical(fixed_result[[1]], c("a", "b", "c")))

# With fixed=FALSE (default), dot matches any character
# "a.b.c" has 5 chars, so regex . splits into 6 empty strings
regex_result <- strsplit("a.b.c", ".")
stopifnot(identical(regex_result[[1]], c("", "", "", "", "", "")))
"#,
    )
    .unwrap();
}

#[test]
fn strsplit_fixed_multi_char_delimiter() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- strsplit("foo::bar::baz", "::", fixed = TRUE)
stopifnot(identical(result[[1]], c("foo", "bar", "baz")))
"#,
    )
    .unwrap();
}

#[test]
fn strsplit_fixed_no_match() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- strsplit("hello", "xyz", fixed = TRUE)
stopifnot(identical(result[[1]], "hello"))
"#,
    )
    .unwrap();
}

#[test]
fn strsplit_fixed_vectorized() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- strsplit(c("a.b", "c.d.e"), ".", fixed = TRUE)
stopifnot(identical(result[[1]], c("a", "b")))
stopifnot(identical(result[[2]], c("c", "d", "e")))
"#,
    )
    .unwrap();
}

#[test]
fn strsplit_fixed_na_propagation() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- strsplit(c("a.b", NA), ".", fixed = TRUE)
stopifnot(identical(result[[1]], c("a", "b")))
stopifnot(is.na(result[[2]]))
"#,
    )
    .unwrap();
}

#[test]
fn strsplit_fixed_trailing_delimiter() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- strsplit("a.b.", ".", fixed = TRUE)
stopifnot(identical(result[[1]], c("a", "b", "")))
"#,
    )
    .unwrap();
}

#[test]
fn strsplit_fixed_leading_delimiter() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- strsplit(".a.b", ".", fixed = TRUE)
stopifnot(identical(result[[1]], c("", "a", "b")))
"#,
    )
    .unwrap();
}

// endregion

// region: consistency between fixed=TRUE and regex for literal patterns

#[test]
fn grep_fixed_vs_regex_consistency() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# For patterns that contain no regex metacharacters,
# fixed=TRUE and fixed=FALSE should give the same results
x <- c("the quick brown fox", "jumped over", "the lazy dog", "quick fox")
pattern <- "quick"
stopifnot(identical(
    grep(pattern, x, fixed = TRUE),
    grep(pattern, x)
))
stopifnot(identical(
    grep(pattern, x, fixed = TRUE, value = TRUE),
    grep(pattern, x, value = TRUE)
))
"#,
    )
    .unwrap();
}

#[test]
fn grepl_fixed_vs_regex_consistency() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- c("hello", "world", "hello world", "hi")
pattern <- "hello"
stopifnot(identical(
    grepl(pattern, x, fixed = TRUE),
    grepl(pattern, x)
))
"#,
    )
    .unwrap();
}

#[test]
fn sub_fixed_vs_regex_consistency() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- c("abc def abc", "xyz", "abc")
pattern <- "abc"
replacement <- "XYZ"
stopifnot(identical(
    sub(pattern, replacement, x, fixed = TRUE),
    sub(pattern, replacement, x)
))
"#,
    )
    .unwrap();
}

#[test]
fn gsub_fixed_vs_regex_consistency() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- c("abc def abc", "xyz", "abc abc abc")
pattern <- "abc"
replacement <- "XYZ"
stopifnot(identical(
    gsub(pattern, replacement, x, fixed = TRUE),
    gsub(pattern, replacement, x)
))
"#,
    )
    .unwrap();
}

// endregion

// region: gsub/sub fixed=TRUE with ignore.case (AhoCorasick)

#[test]
fn gsub_fixed_ignore_case() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- c("Hello World HELLO", "goodbye", "hElLo")
stopifnot(identical(
    gsub("hello", "HI", x, fixed = TRUE, ignore.case = TRUE),
    c("HI World HI", "goodbye", "HI")
))
"#,
    )
    .unwrap();
}

#[test]
fn sub_fixed_ignore_case() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- c("Hello World HELLO", "goodbye")
stopifnot(identical(
    sub("hello", "HI", x, fixed = TRUE, ignore.case = TRUE),
    c("HI World HELLO", "goodbye")
))
"#,
    )
    .unwrap();
}

// endregion

// region: agrep (approximate grep with Levenshtein distance)

#[test]
fn agrep_exact_match() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- c("lasy", "lazy", "black", "lazier")
# "lasy" is distance 1 from "lazy", "lazy" is exact match
# With default max.distance=0.1, for a 4-char pattern that's floor(0.4)=0
# so only exact substring matches
stopifnot(identical(
    agrep("lazy", x),
    c(2L)
))
"#,
    )
    .unwrap();
}

#[test]
fn agrep_with_max_distance() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- c("lasy", "lazy", "black", "laxy")
# max.distance=1 means up to 1 edit
stopifnot(identical(
    agrep("lazy", x, max.distance = 1),
    c(1L, 2L, 4L)
))
"#,
    )
    .unwrap();
}

#[test]
fn agrep_value_mode() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- c("lasy", "lazy", "black")
stopifnot(identical(
    agrep("lazy", x, max.distance = 1, value = TRUE),
    c("lasy", "lazy")
))
"#,
    )
    .unwrap();
}

#[test]
fn agrep_ignore_case() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- c("LAZY", "lasy", "black")
stopifnot(identical(
    agrep("lazy", x, max.distance = 1, ignore.case = TRUE),
    c(1L, 2L)
))
"#,
    )
    .unwrap();
}

#[test]
fn agrep_no_matches() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- c("completely", "different", "words")
result <- agrep("lazy", x, max.distance = 1)
stopifnot(length(result) == 0L)
"#,
    )
    .unwrap();
}

#[test]
fn agrep_fractional_max_distance() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# For pattern "lazy" (4 chars), max.distance=0.5 means floor(0.5*4)=2 edits
x <- c("la", "lazy", "laz", "lxxxy")
stopifnot(identical(
    agrep("lazy", x, max.distance = 0.5),
    c(1L, 2L, 3L)
))
"#,
    )
    .unwrap();
}

// endregion

// region: agrepl (approximate grepl with Levenshtein distance)

#[test]
fn agrepl_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- c("lasy", "lazy", "black")
stopifnot(identical(
    agrepl("lazy", x, max.distance = 1),
    c(TRUE, TRUE, FALSE)
))
"#,
    )
    .unwrap();
}

#[test]
fn agrepl_default_distance() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- c("lazy", "black")
# Default max.distance=0.1, for "lazy" (4 chars) that's floor(0.4)=0
# So only exact substring matches
stopifnot(identical(
    agrepl("lazy", x),
    c(TRUE, FALSE)
))
"#,
    )
    .unwrap();
}

#[test]
fn agrepl_ignore_case() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- c("LAZY", "Lasy", "black")
stopifnot(identical(
    agrepl("lazy", x, max.distance = 1, ignore.case = TRUE),
    c(TRUE, TRUE, FALSE)
))
"#,
    )
    .unwrap();
}

#[test]
fn agrepl_substring_match() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# agrep/agrepl match substrings, not just whole strings
x <- c("the lazy dog", "the quick fox")
stopifnot(identical(
    agrepl("lazy", x),
    c(TRUE, FALSE)
))
"#,
    )
    .unwrap();
}

#[test]
fn agrepl_na_propagation() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- c("lazy", NA, "black")
result <- agrepl("lazy", x, max.distance = 1)
stopifnot(identical(result[1], TRUE))
stopifnot(is.na(result[2]))
stopifnot(identical(result[3], FALSE))
"#,
    )
    .unwrap();
}

// endregion
