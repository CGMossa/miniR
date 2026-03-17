use r::Session;

#[test]
fn digamma_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# digamma(1) == -gamma (Euler-Mascheroni constant)
stopifnot(abs(digamma(1) - (-0.5772156649015329)) < 1e-10)

# digamma(2) == 1 - gamma
stopifnot(abs(digamma(2) - (1 - 0.5772156649015329)) < 1e-10)

# Recurrence relation: digamma(x+1) = digamma(x) + 1/x
stopifnot(abs(digamma(3) - (digamma(2) + 1/2)) < 1e-10)
stopifnot(abs(digamma(4) - (digamma(3) + 1/3)) < 1e-10)

# digamma(0.5) == -gamma - 2*log(2)
stopifnot(abs(digamma(0.5) - (-0.5772156649015329 - 2 * log(2))) < 1e-10)

# vectorized
result <- digamma(c(1, 2, 3))
stopifnot(length(result) == 3)

# NA propagation
stopifnot(is.na(digamma(NA)))
"#,
    )
    .unwrap();
}

#[test]
fn trigamma_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# trigamma(1) == pi^2/6
stopifnot(abs(trigamma(1) - pi^2/6) < 1e-10)

# Recurrence relation: trigamma(x) = trigamma(x+1) + 1/x^2
stopifnot(abs(trigamma(1) - (trigamma(2) + 1)) < 1e-10)
stopifnot(abs(trigamma(2) - (trigamma(3) + 1/4)) < 1e-10)

# trigamma(0.5) == pi^2/2
stopifnot(abs(trigamma(0.5) - pi^2/2) < 1e-10)

# Large values: trigamma(x) ~ 1/x for large x
stopifnot(abs(trigamma(100) - 1/100) < 0.001)

# vectorized
result <- trigamma(c(1, 2, 3))
stopifnot(length(result) == 3)

# NA propagation
stopifnot(is.na(trigamma(NA)))
"#,
    )
    .unwrap();
}

#[test]
fn bessel_j_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# J_0(0) == 1
stopifnot(abs(besselJ(0, 0) - 1) < 1e-10)

# J_1(0) == 0
stopifnot(abs(besselJ(0, 1) - 0) < 1e-10)

# J_0 at known values
# J_0(1) ~ 0.7651976866
stopifnot(abs(besselJ(1, 0) - 0.7651976866) < 1e-6)

# J_1(1) ~ 0.4400505857
stopifnot(abs(besselJ(1, 1) - 0.4400505857) < 1e-6)

# vectorized
result <- besselJ(c(0, 1, 2), 0)
stopifnot(length(result) == 3)
stopifnot(abs(result[1] - 1) < 1e-10)

# NA propagation
stopifnot(is.na(besselJ(NA, 0)))
"#,
    )
    .unwrap();
}

#[test]
fn bessel_y_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Y_0(1) ~ 0.0882569642
stopifnot(abs(besselY(1, 0) - 0.0882569642) < 1e-6)

# Y_1(1) ~ -0.7812128213
stopifnot(abs(besselY(1, 1) - (-0.7812128213)) < 1e-6)

# Y_0 at known value: Y_0(2) ~ 0.5103756726
stopifnot(abs(besselY(2, 0) - 0.5103756726) < 1e-6)

# vectorized
result <- besselY(c(1, 2, 3), 0)
stopifnot(length(result) == 3)

# NA propagation
stopifnot(is.na(besselY(NA, 0)))
"#,
    )
    .unwrap();
}

#[test]
fn cbrt_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Cube root of positive numbers
stopifnot(abs(cbrt(8) - 2) < 1e-10)
stopifnot(abs(cbrt(27) - 3) < 1e-10)
stopifnot(abs(cbrt(1) - 1) < 1e-10)

# Cube root of negative numbers (unlike x^(1/3) which gives NaN)
stopifnot(abs(cbrt(-8) - (-2)) < 1e-10)
stopifnot(abs(cbrt(-27) - (-3)) < 1e-10)

# Cube root of zero
stopifnot(cbrt(0) == 0)

# vectorized
result <- cbrt(c(-8, 0, 8, 27))
stopifnot(length(result) == 4)
stopifnot(abs(result[1] - (-2)) < 1e-10)
stopifnot(abs(result[4] - 3) < 1e-10)

# NA propagation
stopifnot(is.na(cbrt(NA)))
"#,
    )
    .unwrap();
}

#[test]
fn hypot_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Classic 3-4-5 triangle
stopifnot(abs(hypot(3, 4) - 5) < 1e-10)

# hypot(0, x) == abs(x)
stopifnot(abs(hypot(0, 5) - 5) < 1e-10)
stopifnot(abs(hypot(3, 0) - 3) < 1e-10)

# hypot(1, 1) == sqrt(2)
stopifnot(abs(hypot(1, 1) - sqrt(2)) < 1e-10)

# Large values without overflow (this is the whole point of hypot)
big <- 1e308
stopifnot(is.finite(hypot(big, big)))

# vectorized with recycling
result <- hypot(c(3, 5, 8), c(4, 12, 15))
stopifnot(length(result) == 3)
stopifnot(abs(result[1] - 5) < 1e-10)
stopifnot(abs(result[2] - 13) < 1e-10)
stopifnot(abs(result[3] - 17) < 1e-10)

# NA propagation
stopifnot(is.na(hypot(NA, 1)))
stopifnot(is.na(hypot(1, NA)))
"#,
    )
    .unwrap();
}

#[test]
fn digamma_trigamma_relation() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Verify that trigamma is numerically close to the finite difference
# approximation of the derivative of digamma
h <- 1e-6
for (x in c(0.5, 1, 2, 5, 10)) {
    fd <- (digamma(x + h) - digamma(x - h)) / (2 * h)
    stopifnot(abs(fd - trigamma(x)) < 1e-4)
}
"#,
    )
    .unwrap();
}
