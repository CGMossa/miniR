//! Tests for sample() weighted sampling (prob parameter).
//!
//! Verifies that sample(x, size, replace, prob) correctly handles probability
//! weights for both with-replacement and without-replacement cases.

use r::Session;

// region: Weighted sampling with replacement

#[test]
fn sample_prob_with_replacement_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
set.seed(42)
# Weight heavily toward 1 — almost all draws should be 1
x <- sample(3, size = 100, replace = TRUE, prob = c(1000, 0, 0))
stopifnot(all(x == 1))
"#,
    )
    .expect("weighted sample with replacement should respect prob weights");
}

#[test]
fn sample_prob_with_replacement_two_nonzero() {
    let mut s = Session::new();
    s.eval_source(
        r#"
set.seed(42)
# 50/50 between 1 and 3, zero weight for 2
x <- sample(3, size = 1000, replace = TRUE, prob = c(1, 0, 1))
stopifnot(all(x %in% c(1, 3)))
stopifnot(!(2 %in% x))
# Roughly equal counts (allow wide tolerance for randomness)
n1 <- sum(x == 1)
n3 <- sum(x == 3)
stopifnot(n1 > 300 && n1 < 700)
stopifnot(n3 > 300 && n3 < 700)
"#,
    )
    .expect("weighted sample should only select items with nonzero weights");
}

#[test]
fn sample_prob_with_replacement_vector_input() {
    let mut s = Session::new();
    s.eval_source(
        r#"
set.seed(42)
x <- sample(c("a", "b", "c"), size = 100, replace = TRUE, prob = c(0, 0, 1))
stopifnot(all(x == "c"))
"#,
    )
    .expect("weighted sample from character vector should work");
}

// endregion

// region: Weighted sampling without replacement

#[test]
fn sample_prob_without_replacement_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
set.seed(42)
# With high weight on element 1, it should always appear first
x <- sample(5, size = 3, replace = FALSE, prob = c(1000, 1, 1, 1, 1))
stopifnot(x[1] == 1)
stopifnot(length(x) == 3)
# All elements should be unique (no replacement)
stopifnot(length(unique(x)) == 3)
"#,
    )
    .expect("weighted sample without replacement should work");
}

#[test]
fn sample_prob_without_replacement_zero_weights_excluded() {
    let mut s = Session::new();
    s.eval_source(
        r#"
set.seed(42)
# Zero weight for element 2 — it should never be selected
x <- sample(4, size = 3, replace = FALSE, prob = c(1, 0, 1, 1))
stopifnot(!(2 %in% x))
stopifnot(length(x) == 3)
stopifnot(length(unique(x)) == 3)
"#,
    )
    .expect("zero-weight elements should be excluded from sample");
}

#[test]
fn sample_prob_without_replacement_full() {
    let mut s = Session::new();
    s.eval_source(
        r#"
set.seed(42)
# Sample all elements — order should be influenced by weights
x <- sample(3, size = 3, replace = FALSE, prob = c(100, 10, 1))
stopifnot(length(x) == 3)
stopifnot(all(sort(x) == 1:3))
"#,
    )
    .expect("weighted sample of all elements should include all");
}

// endregion

// region: Error handling

#[test]
fn sample_prob_length_mismatch_errors() {
    let mut s = Session::new();
    let result = s.eval_source(
        r#"
sample(3, size = 2, prob = c(1, 1))
"#,
    );
    assert!(result.is_err(), "prob length mismatch should error");
    let err = format!("{}", result.unwrap_err());
    assert!(
        err.contains("same length") || err.contains("!="),
        "error should mention length mismatch: {err}"
    );
}

#[test]
fn sample_prob_na_errors() {
    let mut s = Session::new();
    let result = s.eval_source(
        r#"
sample(3, size = 2, replace = TRUE, prob = c(1, NA, 1))
"#,
    );
    assert!(result.is_err(), "NA in prob should error");
    let err = format!("{}", result.unwrap_err());
    assert!(
        err.contains("NA"),
        "error should mention NA: {err}"
    );
}

#[test]
fn sample_prob_negative_errors() {
    let mut s = Session::new();
    let result = s.eval_source(
        r#"
sample(3, size = 2, replace = TRUE, prob = c(1, -1, 1))
"#,
    );
    assert!(result.is_err(), "negative prob should error");
    let err = format!("{}", result.unwrap_err());
    assert!(
        err.contains("negative"),
        "error should mention negative probability: {err}"
    );
}

#[test]
fn sample_prob_too_few_positive_without_replacement() {
    let mut s = Session::new();
    let result = s.eval_source(
        r#"
sample(5, size = 3, replace = FALSE, prob = c(1, 1, 0, 0, 0))
"#,
    );
    assert!(
        result.is_err(),
        "too few positive probabilities should error"
    );
}

// endregion

// region: Unweighted sample still works

#[test]
fn sample_unweighted_still_works() {
    let mut s = Session::new();
    s.eval_source(
        r#"
set.seed(42)
x <- sample(10, size = 5)
stopifnot(length(x) == 5)
stopifnot(length(unique(x)) == 5)
stopifnot(all(x >= 1 & x <= 10))
"#,
    )
    .expect("unweighted sample should still work");
}

#[test]
fn sample_prob_normalizes_weights() {
    let mut s = Session::new();
    s.eval_source(
        r#"
set.seed(42)
# Unnormalized weights — should still work (weights get normalized internally)
x <- sample(2, size = 100, replace = TRUE, prob = c(100, 100))
n1 <- sum(x == 1)
n2 <- sum(x == 2)
# Both should be roughly 50
stopifnot(n1 > 30 && n1 < 70)
stopifnot(n2 > 30 && n2 < 70)
"#,
    )
    .expect("prob weights should be normalized");
}

// endregion
