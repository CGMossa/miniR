//! Integration tests for miniR extension distributions (non-standard R).
//! These distributions are powered by rand_distr and registered in the
//! "collections" namespace.

use r::Session;

// region: rfrechet

#[test]
fn rfrechet_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
set.seed(42)
x <- rfrechet(100, alpha = 2)
stopifnot(is.double(x))
stopifnot(length(x) == 100)
# Frechet with location 0 produces values > 0
stopifnot(all(x > 0))
"#,
    )
    .expect("rfrechet basic");
}

#[test]
fn rfrechet_with_scale_and_location() {
    let mut s = Session::new();
    s.eval_source(
        r#"
set.seed(42)
x <- rfrechet(50, alpha = 1, s = 2, m = 5)
stopifnot(length(x) == 50)
# All values should be > location (m = 5)
stopifnot(all(x > 5))
"#,
    )
    .expect("rfrechet with scale and location");
}

#[test]
fn rfrechet_reproducible() {
    let mut s = Session::new();
    s.eval_source(
        r#"
set.seed(123)
x <- rfrechet(10, alpha = 1.5)
set.seed(123)
y <- rfrechet(10, alpha = 1.5)
stopifnot(identical(x, y))
"#,
    )
    .expect("rfrechet reproducible");
}

// endregion

// region: rgumbel

#[test]
fn rgumbel_defaults() {
    let mut s = Session::new();
    s.eval_source(
        r#"
set.seed(42)
x <- rgumbel(100)
stopifnot(is.double(x))
stopifnot(length(x) == 100)
"#,
    )
    .expect("rgumbel defaults");
}

#[test]
fn rgumbel_with_params() {
    let mut s = Session::new();
    s.eval_source(
        r#"
set.seed(42)
x <- rgumbel(200, mu = 5, beta = 2)
stopifnot(length(x) == 200)
# Mean of Gumbel is mu + beta * euler_gamma (~ 0.5772)
# So mean should be roughly 5 + 2*0.5772 ~ 6.15
m <- mean(x)
stopifnot(m > 4 && m < 9)
"#,
    )
    .expect("rgumbel with params");
}

#[test]
fn rgumbel_reproducible() {
    let mut s = Session::new();
    s.eval_source(
        r#"
set.seed(99)
x <- rgumbel(10, mu = 1, beta = 0.5)
set.seed(99)
y <- rgumbel(10, mu = 1, beta = 0.5)
stopifnot(identical(x, y))
"#,
    )
    .expect("rgumbel reproducible");
}

// endregion

// region: rinvgauss

#[test]
fn rinvgauss_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
set.seed(42)
x <- rinvgauss(100, mu = 1, lambda = 1)
stopifnot(is.double(x))
stopifnot(length(x) == 100)
# Inverse Gaussian is defined for x > 0
stopifnot(all(x > 0))
"#,
    )
    .expect("rinvgauss basic");
}

#[test]
fn rinvgauss_reproducible() {
    let mut s = Session::new();
    s.eval_source(
        r#"
set.seed(7)
x <- rinvgauss(20, mu = 2, lambda = 3)
set.seed(7)
y <- rinvgauss(20, mu = 2, lambda = 3)
stopifnot(identical(x, y))
"#,
    )
    .expect("rinvgauss reproducible");
}

// endregion

// region: rpareto

#[test]
fn rpareto_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
set.seed(42)
x <- rpareto(100, scale = 1, shape = 2)
stopifnot(is.double(x))
stopifnot(length(x) == 100)
# Pareto values are >= scale
stopifnot(all(x >= 1))
"#,
    )
    .expect("rpareto basic");
}

#[test]
fn rpareto_reproducible() {
    let mut s = Session::new();
    s.eval_source(
        r#"
set.seed(55)
x <- rpareto(15, scale = 2, shape = 3)
set.seed(55)
y <- rpareto(15, scale = 2, shape = 3)
stopifnot(identical(x, y))
"#,
    )
    .expect("rpareto reproducible");
}

// endregion

// region: rpert

#[test]
fn rpert_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
set.seed(42)
x <- rpert(100, min = 0, max = 10, mode = 5)
stopifnot(is.double(x))
stopifnot(length(x) == 100)
# PERT values should be in [min, max]
stopifnot(all(x >= 0 & x <= 10))
"#,
    )
    .expect("rpert basic");
}

#[test]
fn rpert_mode_at_boundary() {
    let mut s = Session::new();
    s.eval_source(
        r#"
set.seed(42)
x <- rpert(50, min = 1, max = 5, mode = 1)
stopifnot(all(x >= 1 & x <= 5))
y <- rpert(50, min = 1, max = 5, mode = 5)
stopifnot(all(y >= 1 & y <= 5))
"#,
    )
    .expect("rpert mode at boundary");
}

#[test]
fn rpert_reproducible() {
    let mut s = Session::new();
    s.eval_source(
        r#"
set.seed(11)
x <- rpert(10, min = -1, max = 1, mode = 0)
set.seed(11)
y <- rpert(10, min = -1, max = 1, mode = 0)
stopifnot(identical(x, y))
"#,
    )
    .expect("rpert reproducible");
}

// endregion

// region: rskewnorm

#[test]
fn rskewnorm_defaults_to_standard_normal() {
    let mut s = Session::new();
    s.eval_source(
        r#"
set.seed(42)
# shape = 0 is a standard normal
x <- rskewnorm(1000)
stopifnot(is.double(x))
stopifnot(length(x) == 1000)
# Mean should be close to 0 for standard normal
m <- mean(x)
stopifnot(abs(m) < 0.2)
"#,
    )
    .expect("rskewnorm defaults to standard normal");
}

#[test]
fn rskewnorm_with_positive_shape() {
    let mut s = Session::new();
    s.eval_source(
        r#"
set.seed(42)
x <- rskewnorm(500, shape = 5)
stopifnot(length(x) == 500)
# Positive shape = right skew, mean should be positive
m <- mean(x)
stopifnot(m > 0)
"#,
    )
    .expect("rskewnorm with positive shape");
}

#[test]
fn rskewnorm_reproducible() {
    let mut s = Session::new();
    s.eval_source(
        r#"
set.seed(33)
x <- rskewnorm(10, location = 2, scale = 3, shape = -1)
set.seed(33)
y <- rskewnorm(10, location = 2, scale = 3, shape = -1)
stopifnot(identical(x, y))
"#,
    )
    .expect("rskewnorm reproducible");
}

// endregion

// region: rtriangular

#[test]
fn rtriangular_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
set.seed(42)
x <- rtriangular(100, min = 0, max = 10, mode = 5)
stopifnot(is.double(x))
stopifnot(length(x) == 100)
# Values should be in [min, max]
stopifnot(all(x >= 0 & x <= 10))
"#,
    )
    .expect("rtriangular basic");
}

#[test]
fn rtriangular_mode_at_min() {
    let mut s = Session::new();
    s.eval_source(
        r#"
set.seed(42)
x <- rtriangular(100, min = 0, max = 1, mode = 0)
stopifnot(all(x >= 0 & x <= 1))
# With mode at min, distribution is right-skewed; mean should be < 0.5
stopifnot(mean(x) < 0.5)
"#,
    )
    .expect("rtriangular mode at min");
}

#[test]
fn rtriangular_reproducible() {
    let mut s = Session::new();
    s.eval_source(
        r#"
set.seed(77)
x <- rtriangular(10, min = -5, max = 5, mode = 0)
set.seed(77)
y <- rtriangular(10, min = -5, max = 5, mode = 0)
stopifnot(identical(x, y))
"#,
    )
    .expect("rtriangular reproducible");
}

// endregion

// region: rzeta

#[test]
fn rzeta_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
set.seed(42)
x <- rzeta(100, s = 2)
stopifnot(is.double(x))
stopifnot(length(x) == 100)
# Zeta values are >= 1
stopifnot(all(x >= 1))
"#,
    )
    .expect("rzeta basic");
}

#[test]
fn rzeta_reproducible() {
    let mut s = Session::new();
    s.eval_source(
        r#"
set.seed(42)
x <- rzeta(20, s = 1.5)
set.seed(42)
y <- rzeta(20, s = 1.5)
stopifnot(identical(x, y))
"#,
    )
    .expect("rzeta reproducible");
}

// endregion

// region: n = 0 returns empty vector

#[test]
fn zero_length_output() {
    let mut s = Session::new();
    s.eval_source(
        r#"
stopifnot(length(rfrechet(0, alpha = 1)) == 0)
stopifnot(length(rgumbel(0)) == 0)
stopifnot(length(rinvgauss(0, mu = 1, lambda = 1)) == 0)
stopifnot(length(rpareto(0, scale = 1, shape = 1)) == 0)
stopifnot(length(rpert(0, min = 0, max = 1, mode = 0.5)) == 0)
stopifnot(length(rskewnorm(0)) == 0)
stopifnot(length(rtriangular(0, min = 0, max = 1, mode = 0.5)) == 0)
stopifnot(length(rzeta(0, s = 2)) == 0)
"#,
    )
    .expect("zero length output");
}

// endregion
