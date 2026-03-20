//! Tests that function call argument passing works correctly across the SmallVec
//! inline/heap boundary. SmallVec<[RValue; 4]> stores up to 4 positional args
//! inline; SmallVec<[(String, RValue); 2]> stores up to 2 named args inline.
//! These tests exercise 0, 1, 4 (at capacity), and >4 (spilled to heap) args
//! to verify the optimization doesn't change behavior.

use r::Session;

#[test]
fn zero_args() {
    let mut s = Session::new();
    s.eval_source(
        r#"
f <- function() 42
stopifnot(f() == 42)
"#,
    );
}

#[test]
fn one_positional_arg() {
    let mut s = Session::new();
    s.eval_source(
        r#"
f <- function(x) x + 1
stopifnot(f(10) == 11)
"#,
    );
}

#[test]
fn four_positional_args_at_smallvec_capacity() {
    let mut s = Session::new();
    s.eval_source(
        r#"
f <- function(a, b, c, d) a + b + c + d
stopifnot(f(1, 2, 3, 4) == 10)
"#,
    );
}

#[test]
fn five_positional_args_spills_to_heap() {
    let mut s = Session::new();
    s.eval_source(
        r#"
f <- function(a, b, c, d, e) a + b + c + d + e
stopifnot(f(1, 2, 3, 4, 5) == 15)
"#,
    );
}

#[test]
fn eight_positional_args_well_beyond_capacity() {
    let mut s = Session::new();
    s.eval_source(
        r#"
f <- function(a, b, c, d, e, f, g, h) a + b + c + d + e + f + g + h
stopifnot(f(1, 2, 3, 4, 5, 6, 7, 8) == 36)
"#,
    );
}

#[test]
fn one_named_arg() {
    let mut s = Session::new();
    s.eval_source(
        r#"
f <- function(x, y) x - y
stopifnot(f(y = 3, x = 10) == 7)
"#,
    );
}

#[test]
fn two_named_args_at_smallvec_capacity() {
    let mut s = Session::new();
    s.eval_source(
        r#"
f <- function(alpha, beta) alpha * beta
stopifnot(f(beta = 5, alpha = 6) == 30)
"#,
    );
}

#[test]
fn three_named_args_spills_to_heap() {
    let mut s = Session::new();
    s.eval_source(
        r#"
f <- function(x, y, z) paste(x, y, z)
stopifnot(f(z = "c", y = "b", x = "a") == "a b c")
"#,
    );
}

#[test]
fn mixed_positional_and_named_args() {
    let mut s = Session::new();
    s.eval_source(
        r#"
f <- function(a, b, c, d, e) c(a, b, c, d, e)
result <- f(1, 2, e = 5, c = 3, d = 4)
stopifnot(identical(result, c(1, 2, 3, 4, 5)))
"#,
    );
}

#[test]
fn dots_with_many_args() {
    let mut s = Session::new();
    s.eval_source(
        r#"
f <- function(...) length(list(...))
stopifnot(f(1, 2, 3, 4, 5, 6) == 6)
"#,
    );
}

#[test]
fn dots_forwarding_many_named_args() {
    let mut s = Session::new();
    s.eval_source(
        r#"
inner <- function(a, b, c) a + b + c
outer <- function(...) inner(...)
stopifnot(outer(c = 30, a = 10, b = 20) == 60)
"#,
    );
}

#[test]
fn nested_calls_with_many_args() {
    let mut s = Session::new();
    s.eval_source(
        r#"
add4 <- function(a, b, c, d) a + b + c + d
mul4 <- function(a, b, c, d) a * b * c * d
stopifnot(add4(mul4(1, 2, 3, 4), mul4(1, 1, 1, 1), 0, 0) == 25)
"#,
    );
}

#[test]
fn recursive_calls_preserve_args() {
    let mut s = Session::new();
    s.eval_source(
        r#"
fib <- function(n) if (n <= 1) n else fib(n - 1) + fib(n - 2)
stopifnot(fib(10) == 55)
"#,
    );
}
