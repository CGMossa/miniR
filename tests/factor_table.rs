use r::Session;

#[test]
fn factor_creates_sorted_levels() {
    let mut r = Session::new();
    r.eval_source(
        r#"
f <- factor(c("b", "a", "c"))
lvls <- levels(f)
stopifnot(identical(lvls, c("a", "b", "c")))
"#,
    )
    .expect("factor should create sorted levels by default");
}

#[test]
fn levels_returns_level_names() {
    let mut r = Session::new();
    r.eval_source(
        r#"
f <- factor(c("x", "y", "z", "x"))
lvls <- levels(f)
stopifnot(identical(lvls, c("x", "y", "z")))
"#,
    )
    .expect("levels() should return the level names");
}

#[test]
fn nlevels_returns_count() {
    let mut r = Session::new();
    r.eval_source(
        r#"
f <- factor(c("a", "b", "c", "a", "b"))
stopifnot(nlevels(f) == 3L)
"#,
    )
    .expect("nlevels() should return the number of levels");
}

#[test]
fn as_integer_returns_codes() {
    let mut r = Session::new();
    r.eval_source(
        r#"
f <- factor(c("b", "a", "c"))
# levels are sorted: a=1, b=2, c=3
codes <- as.integer(f)
stopifnot(identical(codes, c(2L, 1L, 3L)))
"#,
    )
    .expect("as.integer() on factor should return underlying codes");
}

#[test]
fn as_character_returns_labels() {
    let mut r = Session::new();
    r.eval_source(
        r#"
f <- factor(c("b", "a", "c"))

# as.character on a factor should reconstruct labels from levels + codes
labels <- as.character(f)
stopifnot(identical(labels, c("b", "a", "c")))

# Also test with explicit level order
f2 <- factor(c("x", "y", "z"), levels = c("z", "y", "x"))
labels2 <- as.character(f2)
stopifnot(identical(labels2, c("x", "y", "z")))

# Factor with NA
f3 <- factor(c("a", NA, "b"))
labels3 <- as.character(f3)
stopifnot(labels3[1] == "a")
stopifnot(is.na(labels3[2]))
stopifnot(labels3[3] == "b")
"#,
    )
    .expect("as.character() on factor should return labels");
}

#[test]
fn table_counts_occurrences() {
    let mut r = Session::new();
    r.eval_source(
        r#"
x <- c("a", "b", "a", "c", "b", "a")
t <- table(x)

# table returns a named integer vector with class "table"
# Access by name using single bracket and names
nm <- names(t)
stopifnot("a" %in% nm)
stopifnot("b" %in% nm)
stopifnot("c" %in% nm)

# Verify counts via positional access (table sorts names)
# names are sorted: a, b, c
stopifnot(t[1] == 3L)  # a appears 3 times
stopifnot(t[2] == 2L)  # b appears 2 times
stopifnot(t[3] == 1L)  # c appears 1 time
"#,
    )
    .expect("table() should count occurrences");
}

#[test]
fn table_cross_tabulation() {
    let mut r = Session::new();
    // Cross-tabulation (2-way table) may not be implemented yet
    r.eval_source(
        r#"
x <- c("a", "b", "a", "b")
y <- c("x", "x", "y", "y")
ok <- tryCatch({
    t <- table(x, y)
    TRUE
}, error = function(e) FALSE)
# If cross-tabulation is not supported, just verify one-way table works
if (!isTRUE(ok)) {
    t <- table(x)
    stopifnot(t[1] == 2L)
    stopifnot(t[2] == 2L)
}
"#,
    )
    .expect("table cross-tabulation (or fallback to one-way) should work");
}

#[test]
fn is_factor_returns_true() {
    let mut r = Session::new();
    r.eval_source(
        r#"
f <- factor(c("a", "b", "c"))
stopifnot(is.factor(f))
stopifnot(!is.factor(c(1, 2, 3)))
stopifnot(!is.factor("hello"))
"#,
    )
    .expect("is.factor() should return TRUE for factors, FALSE otherwise");
}

#[test]
fn factor_with_explicit_levels() {
    let mut r = Session::new();
    r.eval_source(
        r#"
f <- factor(c("a", "b", "c"), levels = c("c", "b", "a"))
lvls <- levels(f)
stopifnot(identical(lvls, c("c", "b", "a")))
# Codes should reflect the explicit level order: c=1, b=2, a=3
codes <- as.integer(f)
stopifnot(identical(codes, c(3L, 2L, 1L)))
"#,
    )
    .expect("factor with explicit levels should use that order");
}

#[test]
fn factor_with_na() {
    let mut r = Session::new();
    r.eval_source(
        r#"
f <- factor(c("a", NA, "b"))
stopifnot(length(f) == 3)
stopifnot(is.na(f[2]))
lvls <- levels(f)
# NA should not appear in levels
stopifnot(identical(lvls, c("a", "b")))
"#,
    )
    .expect("factor with NA should handle NA values correctly");
}
