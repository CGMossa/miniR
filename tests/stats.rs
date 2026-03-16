use r::Session;

/// Test cov() — sample covariance
#[test]
fn cov_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Perfect positive correlation
x <- c(1, 2, 3, 4, 5)
y <- c(2, 4, 6, 8, 10)
# cov = sum((x-mean(x))*(y-mean(y))) / (n-1)
# mean(x) = 3, mean(y) = 6
# numerator = (-2)(-4) + (-1)(-2) + 0*0 + 1*2 + 2*4 = 8+2+0+2+8 = 20
# cov = 20 / 4 = 5
stopifnot(cov(x, y) == 5)

# Covariance with itself is variance
stopifnot(abs(cov(x, x) - var(x)) < 1e-10)

# cov of constant is NaN (n=1 case too few)
stopifnot(is.nan(cov(c(1), c(2))))
"#,
    )
    .unwrap();
}

/// Test cor() — Pearson correlation coefficient
#[test]
fn cor_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Perfect positive linear relationship
x <- c(1, 2, 3, 4, 5)
y <- c(2, 4, 6, 8, 10)
stopifnot(abs(cor(x, y) - 1.0) < 1e-10)

# Perfect negative linear relationship
y_neg <- c(10, 8, 6, 4, 2)
stopifnot(abs(cor(x, y_neg) - (-1.0)) < 1e-10)

# Correlation of a variable with itself is 1
stopifnot(abs(cor(x, x) - 1.0) < 1e-10)

# Uncorrelated (or weakly correlated)
a <- c(1, 2, 3, 4, 5)
b <- c(5, 1, 4, 2, 3)
r <- cor(a, b)
stopifnot(abs(r) < 1)  # should be between -1 and 1
"#,
    )
    .unwrap();
}

/// Test cor() only supports method = "pearson"
#[test]
fn cor_method_error() {
    let mut s = Session::new();
    let result = s.eval_source(r#"cor(c(1, 2, 3), c(4, 5, 6), method = "spearman")"#);
    assert!(result.is_err());
}

/// Test weighted.mean()
#[test]
fn weighted_mean_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Simple weighted mean
x <- c(1, 2, 3)
w <- c(1, 1, 1)
# Equal weights => regular mean
stopifnot(abs(weighted.mean(x, w) - 2.0) < 1e-10)

# Weighted toward first element
w2 <- c(3, 1, 1)
# (1*3 + 2*1 + 3*1) / (3+1+1) = 8/5 = 1.6
stopifnot(abs(weighted.mean(x, w2) - 1.6) < 1e-10)

# With NAs and na.rm = TRUE
x_na <- c(1, NA, 3)
w_na <- c(1, 1, 1)
stopifnot(is.na(weighted.mean(x_na, w_na)))
stopifnot(abs(weighted.mean(x_na, w_na, na.rm = TRUE) - 2.0) < 1e-10)
"#,
    )
    .unwrap();
}

/// Test scale()
#[test]
fn scale_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Center and scale
x <- c(2, 4, 6, 8, 10)
result <- scale(x)
# mean = 6, sd = sqrt(10) = 3.1623...
# scaled values should have mean ~0 and sd ~1
m <- mean(result)
stopifnot(abs(m) < 1e-10)

# Center only, no scaling
result2 <- scale(x, center = TRUE, scale = FALSE)
stopifnot(abs(mean(result2)) < 1e-10)
# The values should be x - mean(x) = -4, -2, 0, 2, 4
stopifnot(abs(result2[1] - (-4)) < 1e-10)
stopifnot(abs(result2[3] - 0) < 1e-10)

# No centering, scale only
result3 <- scale(x, center = FALSE, scale = TRUE)
# Should divide by sd computed from raw (uncentered) values
stopifnot(all(!is.na(result3)))
"#,
    )
    .unwrap();
}

/// Test complete.cases()
#[test]
fn complete_cases_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Single vector
x <- c(1, NA, 3, NA, 5)
cc <- complete.cases(x)
stopifnot(identical(cc, c(TRUE, FALSE, TRUE, FALSE, TRUE)))

# Multiple vectors
a <- c(1, 2, NA, 4)
b <- c(NA, 2, 3, 4)
cc2 <- complete.cases(a, b)
# Position 1: a=1, b=NA => FALSE
# Position 2: a=2, b=2 => TRUE
# Position 3: a=NA, b=3 => FALSE
# Position 4: a=4, b=4 => TRUE
stopifnot(identical(cc2, c(FALSE, TRUE, FALSE, TRUE)))

# All complete
y <- c(1, 2, 3)
stopifnot(identical(complete.cases(y), c(TRUE, TRUE, TRUE)))
"#,
    )
    .unwrap();
}

/// Test na.omit()
#[test]
fn na_omit_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Remove NAs from numeric vector
x <- c(1, NA, 3, NA, 5)
result <- na.omit(x)
stopifnot(identical(as.double(result), c(1, 3, 5)))

# No NAs — should return unchanged
y <- c(1, 2, 3)
result2 <- na.omit(y)
stopifnot(identical(as.double(result2), c(1, 2, 3)))

# All NAs
z <- c(NA, NA, NA)
result3 <- na.omit(z)
stopifnot(length(result3) == 0)
"#,
    )
    .unwrap();
}

/// Test dnorm() — normal density
#[test]
fn dnorm_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Standard normal density at 0 = 1/sqrt(2*pi) ~= 0.3989423
d0 <- dnorm(0)
stopifnot(abs(d0 - 0.3989423) < 1e-5)

# Density is symmetric: dnorm(-1) == dnorm(1)
stopifnot(abs(dnorm(-1) - dnorm(1)) < 1e-10)

# With mean and sd
d1 <- dnorm(5, mean = 5, sd = 2)
# Same as dnorm(0, 0, 2) = 1/(2*sqrt(2*pi))
stopifnot(abs(d1 - 1/(2*sqrt(2*3.14159265358979))) < 1e-5)

# Vectorized
result <- dnorm(c(-1, 0, 1))
stopifnot(length(result) == 3)
stopifnot(abs(result[2] - 0.3989423) < 1e-5)
"#,
    )
    .unwrap();
}

/// Test pnorm() — normal CDF
#[test]
fn pnorm_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# pnorm(0) = 0.5
stopifnot(abs(pnorm(0) - 0.5) < 1e-7)

# pnorm(-Inf) = 0
stopifnot(pnorm(-Inf) == 0)

# pnorm(Inf) = 1
stopifnot(pnorm(Inf) == 1)

# pnorm(1.96) ~ 0.975
stopifnot(abs(pnorm(1.96) - 0.975) < 0.001)

# pnorm(-1.96) ~ 0.025
stopifnot(abs(pnorm(-1.96) - 0.025) < 0.001)

# With mean and sd
stopifnot(abs(pnorm(10, mean = 10, sd = 1) - 0.5) < 1e-7)
"#,
    )
    .unwrap();
}

/// Test qnorm() — normal quantile function
#[test]
fn qnorm_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# qnorm(0.5) = 0
stopifnot(abs(qnorm(0.5)) < 1e-7)

# qnorm(0.975) ~ 1.96
stopifnot(abs(qnorm(0.975) - 1.96) < 0.001)

# qnorm(0) = -Inf
stopifnot(qnorm(0) == -Inf)

# qnorm(1) = Inf
stopifnot(qnorm(1) == Inf)

# qnorm out of range gives NaN
stopifnot(is.nan(qnorm(-0.1)))
stopifnot(is.nan(qnorm(1.1)))

# With mean and sd
stopifnot(abs(qnorm(0.5, mean = 5, sd = 2) - 5) < 1e-7)
"#,
    )
    .unwrap();
}

/// Test pnorm and qnorm are inverses
#[test]
fn pnorm_qnorm_inverse() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# qnorm(pnorm(x)) should be ~ x (relaxed tolerance for tail values)
for (x in c(-2, -1, 0, 1, 2)) {
    stopifnot(abs(qnorm(pnorm(x)) - x) < 1e-5)
}

# pnorm(qnorm(p)) should be ~ p
for (p in c(0.05, 0.1, 0.25, 0.5, 0.75, 0.9, 0.95)) {
    stopifnot(abs(pnorm(qnorm(p)) - p) < 1e-5)
}
"#,
    )
    .unwrap();
}

/// Test dunif() — uniform density
#[test]
fn dunif_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# dunif(0.5) = 1 (default [0,1])
stopifnot(dunif(0.5) == 1)

# Outside range is 0
stopifnot(dunif(-1) == 0)
stopifnot(dunif(2) == 0)

# Custom range [0, 10]
stopifnot(abs(dunif(5, min = 0, max = 10) - 0.1) < 1e-10)
stopifnot(dunif(11, min = 0, max = 10) == 0)
"#,
    )
    .unwrap();
}

/// Test punif() — uniform CDF
#[test]
fn punif_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# punif(0.5) = 0.5
stopifnot(abs(punif(0.5) - 0.5) < 1e-10)

# punif(0) = 0
stopifnot(punif(0) == 0)

# punif(1) = 1
stopifnot(punif(1) == 1)

# Below range
stopifnot(punif(-1) == 0)

# Above range
stopifnot(punif(2) == 1)

# Custom range
stopifnot(abs(punif(5, min = 0, max = 10) - 0.5) < 1e-10)
"#,
    )
    .unwrap();
}

/// Test qunif() — uniform quantile function
#[test]
fn qunif_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# qunif(0.5) = 0.5 (default [0,1])
stopifnot(abs(qunif(0.5) - 0.5) < 1e-10)

# qunif(0) = 0
stopifnot(qunif(0) == 0)

# qunif(1) = 1
stopifnot(qunif(1) == 1)

# Custom range
stopifnot(abs(qunif(0.5, min = 2, max = 8) - 5) < 1e-10)
stopifnot(abs(qunif(0, min = 2, max = 8) - 2) < 1e-10)
stopifnot(abs(qunif(1, min = 2, max = 8) - 8) < 1e-10)

# Out of range gives NaN
stopifnot(is.nan(qunif(-0.1)))
stopifnot(is.nan(qunif(1.1)))
"#,
    )
    .unwrap();
}

/// Test na.rm already works for mean, sum, min, max (verification test)
#[test]
fn na_rm_already_supported() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- c(1, NA, 3, NA, 5)

# Without na.rm, should return NA
stopifnot(is.na(mean(x)))
stopifnot(is.na(sum(x)))

# With na.rm = TRUE, should work
stopifnot(abs(mean(x, na.rm = TRUE) - 3) < 1e-10)
stopifnot(abs(sum(x, na.rm = TRUE) - 9) < 1e-10)
stopifnot(min(x, na.rm = TRUE) == 1)
stopifnot(max(x, na.rm = TRUE) == 5)
"#,
    )
    .unwrap();
}

/// Test error messages are helpful
#[test]
fn stats_error_messages() {
    let mut s = Session::new();

    // cor with different length vectors
    let r = s.eval_source("cor(c(1, 2), c(1, 2, 3))");
    assert!(r.is_err());
    let err = format!("{}", r.unwrap_err());
    assert!(
        err.contains("equal length"),
        "error should mention equal length: {err}"
    );

    // cov with different length vectors
    let r2 = s.eval_source("cov(c(1, 2), c(1, 2, 3))");
    assert!(r2.is_err());

    // dnorm with negative sd
    let r3 = s.eval_source("dnorm(0, sd = -1)");
    assert!(r3.is_err());
    let err3 = format!("{}", r3.unwrap_err());
    assert!(
        err3.contains("non-negative"),
        "error should mention non-negative: {err3}"
    );
}
