use r::Session;

#[test]
fn inverse_trig() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# asin: inverse sine
stopifnot(abs(asin(0) - 0) < 1e-10)
stopifnot(abs(asin(1) - pi/2) < 1e-10)
stopifnot(abs(asin(-1) - (-pi/2)) < 1e-10)
# asin(sin(x)) == x for x in [-pi/2, pi/2]
stopifnot(abs(asin(sin(0.5)) - 0.5) < 1e-10)

# acos: inverse cosine
stopifnot(abs(acos(1) - 0) < 1e-10)
stopifnot(abs(acos(0) - pi/2) < 1e-10)
stopifnot(abs(acos(-1) - pi) < 1e-10)

# atan: inverse tangent
stopifnot(abs(atan(0) - 0) < 1e-10)
stopifnot(abs(atan(1) - pi/4) < 1e-10)
stopifnot(abs(atan(-1) - (-pi/4)) < 1e-10)

# vectorized
stopifnot(length(asin(c(0, 0.5, 1))) == 3)
stopifnot(length(acos(c(0, 0.5, 1))) == 3)
stopifnot(length(atan(c(0, 1, -1))) == 3)
"#,
    )
    .unwrap();
}

#[test]
fn atan2_two_arg() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# atan2(y, x) — quadrant-aware inverse tangent
stopifnot(abs(atan2(0, 1) - 0) < 1e-10)
stopifnot(abs(atan2(1, 0) - pi/2) < 1e-10)
stopifnot(abs(atan2(0, -1) - pi) < 1e-10)
stopifnot(abs(atan2(-1, 0) - (-pi/2)) < 1e-10)
stopifnot(abs(atan2(1, 1) - pi/4) < 1e-10)

# vectorized with recycling
result <- atan2(c(1, -1), c(1, 1))
stopifnot(length(result) == 2)
stopifnot(abs(result[1] - pi/4) < 1e-10)
stopifnot(abs(result[2] - (-pi/4)) < 1e-10)
"#,
    )
    .unwrap();
}

#[test]
fn hyperbolic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# sinh, cosh, tanh
stopifnot(abs(sinh(0) - 0) < 1e-10)
stopifnot(abs(cosh(0) - 1) < 1e-10)
stopifnot(abs(tanh(0) - 0) < 1e-10)

# Identity: cosh^2(x) - sinh^2(x) == 1
x <- 1.5
stopifnot(abs(cosh(x)^2 - sinh(x)^2 - 1) < 1e-10)

# Identity: tanh(x) == sinh(x) / cosh(x)
stopifnot(abs(tanh(x) - sinh(x) / cosh(x)) < 1e-10)

# vectorized
stopifnot(length(sinh(c(0, 1, 2))) == 3)
stopifnot(length(cosh(c(0, 1, 2))) == 3)
stopifnot(length(tanh(c(0, 1, 2))) == 3)
"#,
    )
    .unwrap();
}

#[test]
fn inverse_hyperbolic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# asinh: inverse of sinh
stopifnot(abs(asinh(0) - 0) < 1e-10)
stopifnot(abs(asinh(sinh(1.5)) - 1.5) < 1e-10)

# acosh: inverse of cosh (domain: x >= 1)
stopifnot(abs(acosh(1) - 0) < 1e-10)
stopifnot(abs(acosh(cosh(2.0)) - 2.0) < 1e-10)

# atanh: inverse of tanh (domain: -1 < x < 1)
stopifnot(abs(atanh(0) - 0) < 1e-10)
stopifnot(abs(atanh(tanh(0.5)) - 0.5) < 1e-10)

# vectorized
stopifnot(length(asinh(c(-1, 0, 1))) == 3)
stopifnot(length(acosh(c(1, 2, 3))) == 3)
stopifnot(length(atanh(c(-0.5, 0, 0.5))) == 3)
"#,
    )
    .unwrap();
}

#[test]
fn expm1_log1p() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# expm1: exp(x) - 1, numerically stable near 0
stopifnot(abs(expm1(0) - 0) < 1e-15)
stopifnot(abs(expm1(1) - (exp(1) - 1)) < 1e-10)
# Near zero, expm1 should be more accurate than exp(x)-1
stopifnot(abs(expm1(1e-15) - 1e-15) < 1e-25)

# log1p: log(1 + x), numerically stable near 0
stopifnot(abs(log1p(0) - 0) < 1e-15)
stopifnot(abs(log1p(1) - log(2)) < 1e-10)
# Near zero, log1p should be more accurate than log(1+x)
stopifnot(abs(log1p(1e-15) - 1e-15) < 1e-25)

# Identity: log1p(expm1(x)) == x
stopifnot(abs(log1p(expm1(0.5)) - 0.5) < 1e-10)

# vectorized
stopifnot(length(expm1(c(0, 1, 2))) == 3)
stopifnot(length(log1p(c(0, 1, 2))) == 3)
"#,
    )
    .unwrap();
}

#[test]
fn gamma_lgamma() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# gamma: gamma(n) == (n-1)! for positive integers
stopifnot(abs(gamma(1) - 1) < 1e-10)
stopifnot(abs(gamma(2) - 1) < 1e-10)
stopifnot(abs(gamma(5) - 24) < 1e-10)   # 4! = 24
stopifnot(abs(gamma(6) - 120) < 1e-10)  # 5! = 120

# gamma(0.5) == sqrt(pi)
stopifnot(abs(gamma(0.5) - sqrt(pi)) < 1e-10)

# lgamma: log of gamma function
stopifnot(abs(lgamma(1) - 0) < 1e-10)
stopifnot(abs(lgamma(5) - log(24)) < 1e-10)
stopifnot(abs(lgamma(6) - log(120)) < 1e-10)

# lgamma == log(gamma) for small positive values
stopifnot(abs(lgamma(3) - log(gamma(3))) < 1e-10)

# vectorized
stopifnot(length(gamma(c(1, 2, 3))) == 3)
stopifnot(length(lgamma(c(1, 2, 3))) == 3)
"#,
    )
    .unwrap();
}

#[test]
fn beta_lbeta() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# beta(a, b) == gamma(a) * gamma(b) / gamma(a + b)
stopifnot(abs(beta(1, 1) - 1) < 1e-10)
stopifnot(abs(beta(2, 3) - gamma(2) * gamma(3) / gamma(5)) < 1e-10)
stopifnot(abs(beta(0.5, 0.5) - pi) < 1e-10)  # B(1/2, 1/2) = pi

# lbeta == log(beta) for reasonable values
stopifnot(abs(lbeta(2, 3) - log(beta(2, 3))) < 1e-10)
stopifnot(abs(lbeta(1, 1) - 0) < 1e-10)

# vectorized
stopifnot(length(beta(c(1, 2, 3), c(1, 2, 3))) == 3)
stopifnot(length(lbeta(c(1, 2, 3), c(1, 2, 3))) == 3)
"#,
    )
    .unwrap();
}

#[test]
fn factorial_fn() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# factorial(n) == n!
stopifnot(abs(factorial(0) - 1) < 1e-10)
stopifnot(abs(factorial(1) - 1) < 1e-10)
stopifnot(abs(factorial(5) - 120) < 1e-10)
stopifnot(abs(factorial(10) - 3628800) < 1e-5)

# vectorized
result <- factorial(c(0, 1, 2, 3, 4, 5))
stopifnot(abs(result[1] - 1) < 1e-10)
stopifnot(abs(result[4] - 6) < 1e-10)
stopifnot(abs(result[6] - 120) < 1e-10)
"#,
    )
    .unwrap();
}

#[test]
fn choose_fn() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# choose(n, k) == n! / (k! * (n-k)!)
stopifnot(choose(5, 0) == 1)
stopifnot(choose(5, 1) == 5)
stopifnot(choose(5, 2) == 10)
stopifnot(choose(5, 5) == 1)
stopifnot(choose(10, 3) == 120)

# Edge cases
stopifnot(choose(0, 0) == 1)
stopifnot(choose(5, -1) == 0)  # k < 0 => 0
stopifnot(choose(5, 6) == 0)   # k > n => 0

# vectorized
result <- choose(c(5, 5, 5), c(0, 2, 5))
stopifnot(result[1] == 1)
stopifnot(result[2] == 10)
stopifnot(result[3] == 1)
"#,
    )
    .unwrap();
}

#[test]
fn combn_fn() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# combn(n, m) — all combinations of 1:n taken m at a time
# combn(4, 2) should be a 2x6 matrix (choose(4,2) = 6 combinations)
result <- combn(4, 2)
stopifnot(is.matrix(result))
stopifnot(nrow(result) == 2)
stopifnot(ncol(result) == 6)

# combn(5, 1) — trivial: 5 columns of 1 element each
result2 <- combn(5, 1)
stopifnot(nrow(result2) == 1)
stopifnot(ncol(result2) == 5)

# combn(3, 3) — only one combination
result3 <- combn(3, 3)
stopifnot(nrow(result3) == 3)
stopifnot(ncol(result3) == 1)
"#,
    )
    .unwrap();
}

#[test]
fn na_handling() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# NA propagation through unary math functions
stopifnot(is.na(asin(NA)))
stopifnot(is.na(acos(NA)))
stopifnot(is.na(atan(NA)))
stopifnot(is.na(sinh(NA)))
stopifnot(is.na(cosh(NA)))
stopifnot(is.na(tanh(NA)))
stopifnot(is.na(asinh(NA)))
stopifnot(is.na(acosh(NA)))
stopifnot(is.na(atanh(NA)))
stopifnot(is.na(expm1(NA)))
stopifnot(is.na(log1p(NA)))
stopifnot(is.na(gamma(NA)))
stopifnot(is.na(lgamma(NA)))
stopifnot(is.na(factorial(NA)))

# NA propagation through binary math functions
stopifnot(is.na(atan2(NA, 1)))
stopifnot(is.na(atan2(1, NA)))
stopifnot(is.na(beta(NA, 1)))
stopifnot(is.na(lbeta(1, NA)))
stopifnot(is.na(choose(NA, 2)))
stopifnot(is.na(choose(5, NA)))
"#,
    )
    .unwrap();
}
