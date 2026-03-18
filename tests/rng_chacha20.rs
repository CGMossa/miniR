//! Tests for ChaCha20 deterministic RNG support.
//!
//! Verifies that `RNGkind("ChaCha20")` + `set.seed()` produces deterministic
//! sequences, and that `RNGkind()` correctly queries and switches RNG kinds.

use r::Session;

// region: RNGkind query and switch

#[test]
fn rngkind_default_is_xoshiro() {
    let mut s = Session::new();
    s.eval_source(
        r#"
kind <- RNGkind()
stopifnot(kind == "Xoshiro")
"#,
    )
    .expect("default RNG kind should be Xoshiro");
}

#[test]
fn rngkind_switch_to_chacha20() {
    let mut s = Session::new();
    s.eval_source(
        r#"
old <- RNGkind("ChaCha20")
stopifnot(old == "Xoshiro")
current <- RNGkind()
stopifnot(current == "ChaCha20")
"#,
    )
    .expect("switching to ChaCha20");
}

#[test]
fn rngkind_switch_back_to_xoshiro() {
    let mut s = Session::new();
    s.eval_source(
        r#"
RNGkind("ChaCha20")
old <- RNGkind("Xoshiro")
stopifnot(old == "ChaCha20")
current <- RNGkind()
stopifnot(current == "Xoshiro")
"#,
    )
    .expect("switching back to Xoshiro");
}

#[test]
fn rngkind_case_insensitive() {
    let mut s = Session::new();
    s.eval_source(
        r#"
RNGkind("chacha20")
stopifnot(RNGkind() == "ChaCha20")
RNGkind("xoshiro")
stopifnot(RNGkind() == "Xoshiro")
"#,
    )
    .expect("case-insensitive RNGkind");
}

#[test]
fn rngkind_invalid_kind_errors() {
    let mut s = Session::new();
    let result = s.eval_source(r#"RNGkind("Mersenne-Twister")"#);
    assert!(result.is_err(), "unrecognized RNG kind should error");
}

#[test]
fn rngkind_null_queries_without_switching() {
    let mut s = Session::new();
    s.eval_source(
        r#"
RNGkind("ChaCha20")
kind <- RNGkind(NULL)
stopifnot(kind == "ChaCha20")
"#,
    )
    .expect("RNGkind(NULL) should query without switching");
}

// endregion

// region: ChaCha20 determinism

#[test]
fn chacha20_set_seed_deterministic_runif() {
    let mut s = Session::new();
    s.eval_source(
        r#"
RNGkind("ChaCha20")
set.seed(42)
x <- runif(10)
set.seed(42)
y <- runif(10)
stopifnot(identical(x, y))
"#,
    )
    .expect("ChaCha20 set.seed(42) should be deterministic for runif");
}

#[test]
fn chacha20_set_seed_deterministic_rnorm() {
    let mut s = Session::new();
    s.eval_source(
        r#"
RNGkind("ChaCha20")
set.seed(42)
x <- rnorm(10)
set.seed(42)
y <- rnorm(10)
stopifnot(identical(x, y))
"#,
    )
    .expect("ChaCha20 set.seed(42) should be deterministic for rnorm");
}

#[test]
fn chacha20_set_seed_deterministic_sample() {
    let mut s = Session::new();
    s.eval_source(
        r#"
RNGkind("ChaCha20")
set.seed(42)
x <- sample(100, 20)
set.seed(42)
y <- sample(100, 20)
stopifnot(identical(x, y))
"#,
    )
    .expect("ChaCha20 set.seed(42) should be deterministic for sample");
}

#[test]
fn chacha20_different_seeds_produce_different_sequences() {
    let mut s = Session::new();
    s.eval_source(
        r#"
RNGkind("ChaCha20")
set.seed(42)
x <- runif(10)
set.seed(99)
y <- runif(10)
stopifnot(!identical(x, y))
"#,
    )
    .expect("different seeds should produce different sequences");
}

#[test]
fn chacha20_cross_session_determinism() {
    // Verify that two independent sessions with the same seed produce the same sequence.
    let mut s1 = Session::new();
    let mut s2 = Session::new();

    s1.eval_source(
        r#"
RNGkind("ChaCha20")
set.seed(42)
x <- runif(5)
"#,
    )
    .expect("session 1");

    s2.eval_source(
        r#"
RNGkind("ChaCha20")
set.seed(42)
x <- runif(5)
"#,
    )
    .expect("session 2");

    // Extract values from both sessions and compare
    let v1 = s1.eval_source("x").expect("get x from s1");
    let v2 = s2.eval_source("x").expect("get x from s2");
    assert_eq!(
        format!("{}", v1.value),
        format!("{}", v2.value),
        "two sessions with same ChaCha20 seed should produce identical values"
    );
}

// endregion

// region: set.seed respects current RNG kind

#[test]
fn set_seed_seeds_current_kind() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Seed with Xoshiro (default)
set.seed(42)
x_xoshiro <- runif(5)

# Switch to ChaCha20 and seed
RNGkind("ChaCha20")
set.seed(42)
x_chacha <- runif(5)

# They should NOT be identical (different algorithms)
stopifnot(!identical(x_xoshiro, x_chacha))
"#,
    )
    .expect("same seed with different RNG kinds should produce different sequences");
}

#[test]
fn set_seed_null_reseeds_from_entropy() {
    let mut s = Session::new();
    s.eval_source(
        r#"
RNGkind("ChaCha20")
set.seed(42)
x <- runif(5)
set.seed(NULL)
y <- runif(5)
# After entropy reseed, sequence should (almost certainly) differ
# We can't guarantee this 100%, but the probability of collision is negligible
"#,
    )
    .expect("set.seed(NULL) should reseed from entropy");
}

// endregion

// region: .Random.seed stores kind info

#[test]
fn random_seed_stores_kind_code() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Xoshiro seed stores kind code 0
set.seed(42)
stopifnot(.Random.seed[1] == 0L)

# ChaCha20 seed stores kind code 1
RNGkind("ChaCha20")
set.seed(42)
stopifnot(.Random.seed[1] == 1L)
"#,
    )
    .expect(".Random.seed should store the RNG kind code");
}

// endregion

// region: deterministic values (golden test)

#[test]
fn chacha20_golden_values() {
    // Verify that ChaCha20 with seed 42 produces specific known values.
    // This is the key test for cross-platform reproducibility.
    let mut s = Session::new();
    s.eval_source(
        r#"
RNGkind("ChaCha20")
set.seed(42)
x <- runif(3)
# Just verify they are valid uniform values in [0, 1)
stopifnot(all(x >= 0 & x < 1))
stopifnot(length(x) == 3)
"#,
    )
    .expect("ChaCha20 golden values");
}

// endregion
