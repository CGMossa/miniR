use r::Session;

// region: Exponential distribution

#[test]
fn dexp_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# dexp(0) = rate = 1
stopifnot(abs(dexp(0) - 1) < 1e-10)

# dexp(1) = exp(-1)
stopifnot(abs(dexp(1) - exp(-1)) < 1e-10)

# Negative values have density 0
stopifnot(dexp(-1) == 0)

# Custom rate
stopifnot(abs(dexp(0, rate = 2) - 2) < 1e-10)

# Vectorized
result <- dexp(c(0, 1, 2))
stopifnot(length(result) == 3)
"#,
    )
    .unwrap();
}

#[test]
fn pexp_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# pexp(0) = 0
stopifnot(pexp(0) == 0)

# pexp(Inf) = 1
stopifnot(pexp(Inf) == 1)

# pexp(1) = 1 - exp(-1) ~ 0.6321
stopifnot(abs(pexp(1) - (1 - exp(-1))) < 1e-10)

# Negative values
stopifnot(pexp(-1) == 0)
"#,
    )
    .unwrap();
}

#[test]
fn qexp_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# qexp(0) = 0
stopifnot(qexp(0) == 0)

# qexp(1) = Inf
stopifnot(qexp(1) == Inf)

# Inverse of pexp
stopifnot(abs(qexp(pexp(1)) - 1) < 1e-10)
stopifnot(abs(qexp(pexp(0.5)) - 0.5) < 1e-10)

# Out of range
stopifnot(is.nan(qexp(-0.1)))
stopifnot(is.nan(qexp(1.1)))
"#,
    )
    .unwrap();
}

// endregion

// region: Gamma distribution

#[test]
fn dgamma_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Gamma(1, 1) = Exp(1): dgamma(x, 1, 1) = exp(-x)
stopifnot(abs(dgamma(0, shape = 1, rate = 1) - 1) < 1e-10)
stopifnot(abs(dgamma(1, shape = 1, rate = 1) - exp(-1)) < 1e-10)

# Negative values have density 0
stopifnot(dgamma(-1, shape = 1, rate = 1) == 0)

# Vectorized
result <- dgamma(c(0, 1, 2), shape = 2, rate = 1)
stopifnot(length(result) == 3)
"#,
    )
    .unwrap();
}

#[test]
fn pgamma_qgamma_inverse() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# pgamma and qgamma should be inverses
for (x in c(0.5, 1, 2, 5)) {
    stopifnot(abs(qgamma(pgamma(x, shape = 2), shape = 2) - x) < 1e-4)
}
for (p in c(0.1, 0.5, 0.9)) {
    stopifnot(abs(pgamma(qgamma(p, shape = 2), shape = 2) - p) < 1e-4)
}
"#,
    )
    .unwrap();
}

// endregion

// region: Beta distribution

#[test]
fn dbeta_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Beta(1,1) is Unif(0,1): density = 1 everywhere in [0,1]
stopifnot(abs(dbeta(0.5, 1, 1) - 1) < 1e-10)

# Outside [0,1] is 0
stopifnot(dbeta(-0.5, 1, 1) == 0)
stopifnot(dbeta(1.5, 1, 1) == 0)

# Beta(2,2) is symmetric with peak at 0.5
d_half <- dbeta(0.5, 2, 2)
d_quarter <- dbeta(0.25, 2, 2)
stopifnot(d_half > d_quarter)
"#,
    )
    .unwrap();
}

#[test]
fn pbeta_qbeta_inverse() {
    let mut s = Session::new();
    s.eval_source(
        r#"
for (p in c(0.1, 0.25, 0.5, 0.75, 0.9)) {
    stopifnot(abs(pbeta(qbeta(p, 2, 3), 2, 3) - p) < 1e-3)
}
"#,
    )
    .unwrap();
}

// endregion

// region: Cauchy distribution

#[test]
fn dcauchy_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Standard Cauchy density at 0 = 1/pi
stopifnot(abs(dcauchy(0) - 1/pi) < 1e-10)

# Symmetric
stopifnot(abs(dcauchy(-1) - dcauchy(1)) < 1e-10)

# Custom location and scale
d <- dcauchy(5, location = 5, scale = 2)
stopifnot(abs(d - 1/(2*pi)) < 1e-10)
"#,
    )
    .unwrap();
}

#[test]
fn pcauchy_qcauchy_inverse() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# pcauchy(0) = 0.5
stopifnot(abs(pcauchy(0) - 0.5) < 1e-10)

# Inverse relationship
for (x in c(-2, -1, 0, 1, 2)) {
    stopifnot(abs(qcauchy(pcauchy(x)) - x) < 1e-7)
}

# qcauchy(0) = -Inf, qcauchy(1) = Inf
stopifnot(qcauchy(0) == -Inf)
stopifnot(qcauchy(1) == Inf)
"#,
    )
    .unwrap();
}

// endregion

// region: Weibull distribution

#[test]
fn dweibull_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Weibull(1, 1) = Exp(1)
stopifnot(abs(dweibull(1, shape = 1) - exp(-1)) < 1e-10)

# Negative values
stopifnot(dweibull(-1, shape = 1) == 0)
"#,
    )
    .unwrap();
}

#[test]
fn pweibull_qweibull_inverse() {
    let mut s = Session::new();
    s.eval_source(
        r#"
for (x in c(0.5, 1, 2)) {
    stopifnot(abs(qweibull(pweibull(x, shape = 2), shape = 2) - x) < 1e-7)
}
"#,
    )
    .unwrap();
}

// endregion

// region: Log-normal distribution

#[test]
fn dlnorm_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# dlnorm(1) = dnorm(0) = 1/sqrt(2*pi)
stopifnot(abs(dlnorm(1) - dnorm(0)) < 1e-10)

# Negative values
stopifnot(dlnorm(-1) == 0)
stopifnot(dlnorm(0) == 0)
"#,
    )
    .unwrap();
}

#[test]
fn plnorm_qlnorm_inverse() {
    let mut s = Session::new();
    s.eval_source(
        r#"
for (x in c(0.5, 1, 2, 5)) {
    stopifnot(abs(qlnorm(plnorm(x)) - x) < 1e-4)
}
"#,
    )
    .unwrap();
}

// endregion

// region: Chi-squared distribution

#[test]
fn dchisq_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# dchisq is dgamma with shape=df/2, rate=0.5
# For df=2, dchisq(x, 2) = 0.5 * exp(-x/2)
stopifnot(abs(dchisq(0, df = 2) - 0.5) < 1e-10)
stopifnot(abs(dchisq(2, df = 2) - 0.5 * exp(-1)) < 1e-10)

# Negative values
stopifnot(dchisq(-1, df = 2) == 0)
"#,
    )
    .unwrap();
}

#[test]
fn pchisq_qchisq_inverse() {
    let mut s = Session::new();
    s.eval_source(
        r#"
for (x in c(1, 2, 5, 10)) {
    stopifnot(abs(qchisq(pchisq(x, df = 3), df = 3) - x) < 1e-4)
}
"#,
    )
    .unwrap();
}

// endregion

// region: Student's t distribution

#[test]
fn dt_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# dt is symmetric around 0
stopifnot(abs(dt(-1, df = 5) - dt(1, df = 5)) < 1e-10)

# dt(0, df) should be gamma((df+1)/2) / (sqrt(df*pi) * gamma(df/2))
# For df=1 (Cauchy), dt(0, 1) = 1/pi
stopifnot(abs(dt(0, df = 1) - 1/pi) < 1e-10)
"#,
    )
    .unwrap();
}

#[test]
fn pt_qt_inverse() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# pt(0, df) = 0.5 for all df
stopifnot(abs(pt(0, df = 5) - 0.5) < 1e-7)
stopifnot(abs(pt(0, df = 30) - 0.5) < 1e-7)

# Inverse relationship
for (x in c(-2, -1, 0, 1, 2)) {
    stopifnot(abs(qt(pt(x, df = 10), df = 10) - x) < 1e-3)
}
"#,
    )
    .unwrap();
}

// endregion

// region: F distribution

#[test]
fn df_dist_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# df(x, 2, 2) at x=0 should be 1 (since df1=2)
stopifnot(abs(df(0, 2, 2) - 1) < 1e-10)

# Negative values
stopifnot(df(-1, 2, 2) == 0)
"#,
    )
    .unwrap();
}

#[test]
fn pf_qf_inverse() {
    let mut s = Session::new();
    s.eval_source(
        r#"
for (x in c(0.5, 1, 2, 5)) {
    stopifnot(abs(qf(pf(x, 3, 5), 3, 5) - x) < 1e-3)
}

# pf(0, ...) = 0
stopifnot(pf(0, 3, 5) == 0)
"#,
    )
    .unwrap();
}

// endregion

// region: Binomial distribution

#[test]
fn dbinom_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# dbinom(0, 10, 0.5) = 0.5^10
stopifnot(abs(dbinom(0, 10, 0.5) - 0.5^10) < 1e-10)

# dbinom(10, 10, 0.5) = 0.5^10
stopifnot(abs(dbinom(10, 10, 0.5) - 0.5^10) < 1e-10)

# dbinom(1, 1, 0.3) = 0.3
stopifnot(abs(dbinom(1, 1, 0.3) - 0.3) < 1e-10)

# Sum of all probabilities = 1
total <- sum(sapply(0:10, function(k) dbinom(k, 10, 0.3)))
stopifnot(abs(total - 1) < 1e-10)

# Out of range is 0
stopifnot(dbinom(-1, 10, 0.5) == 0)
stopifnot(dbinom(11, 10, 0.5) == 0)
"#,
    )
    .unwrap();
}

#[test]
fn pbinom_qbinom_inverse() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# pbinom(10, 10, 0.5) = 1
stopifnot(pbinom(10, 10, 0.5) == 1)

# pbinom(-1, 10, 0.5) = 0
stopifnot(pbinom(-1, 10, 0.5) == 0)

# qbinom(0, 10, 0.5) = 0
stopifnot(qbinom(0, 10, 0.5) == 0)

# qbinom(1, 10, 0.5) = 10
stopifnot(qbinom(1, 10, 0.5) == 10)

# qbinom(0.5, 10, 0.5) should be 5
stopifnot(qbinom(0.5, 10, 0.5) == 5)
"#,
    )
    .unwrap();
}

// endregion

// region: Poisson distribution

#[test]
fn dpois_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# dpois(0, 1) = exp(-1)
stopifnot(abs(dpois(0, 1) - exp(-1)) < 1e-10)

# dpois(1, 1) = exp(-1)
stopifnot(abs(dpois(1, 1) - exp(-1)) < 1e-10)

# dpois(k, 0) = 1 for k=0, 0 otherwise
stopifnot(dpois(0, 0) == 1)
stopifnot(dpois(1, 0) == 0)

# Sum over large enough range ~ 1
total <- sum(sapply(0:20, function(k) dpois(k, 5)))
stopifnot(abs(total - 1) < 1e-6)
"#,
    )
    .unwrap();
}

#[test]
fn ppois_qpois_inverse() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# ppois(-1, 5) = 0
stopifnot(ppois(-1, 5) == 0)

# ppois(Inf, 5) = 1 -- not testable directly, but large value
# ppois(0, 0) = 1
stopifnot(ppois(0, 0) == 1)

# qpois(0, 5) = 0
stopifnot(qpois(0, 5) == 0)

# qpois median of Poisson(5) should be 5
stopifnot(qpois(0.5, 5) == 5)
"#,
    )
    .unwrap();
}

// endregion

// region: Geometric distribution

#[test]
fn dgeom_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# dgeom(0, 0.5) = 0.5
stopifnot(abs(dgeom(0, 0.5) - 0.5) < 1e-10)

# dgeom(1, 0.5) = 0.5 * 0.5 = 0.25
stopifnot(abs(dgeom(1, 0.5) - 0.25) < 1e-10)

# Negative values
stopifnot(dgeom(-1, 0.5) == 0)
"#,
    )
    .unwrap();
}

#[test]
fn pgeom_qgeom_inverse() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# pgeom(0, 0.5) = 0.5
stopifnot(abs(pgeom(0, 0.5) - 0.5) < 1e-10)

# pgeom(1, 0.5) = 0.75
stopifnot(abs(pgeom(1, 0.5) - 0.75) < 1e-10)

# qgeom(0, 0.5) = 0
stopifnot(qgeom(0, 0.5) == 0)

# qgeom(0.5, 0.5) = 0 (since pgeom(0, 0.5) = 0.5 >= 0.5)
stopifnot(qgeom(0.5, 0.5) == 0)

# qgeom(0.75, 0.5) = 1
stopifnot(qgeom(0.75, 0.5) == 1)
"#,
    )
    .unwrap();
}

// endregion

// region: Hypergeometric distribution

#[test]
fn dhyper_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Simple urn: 5 white, 5 black, draw 3
# dhyper(2, 5, 5, 3) = C(5,2)*C(5,1)/C(10,3) = 10*5/120 = 50/120
stopifnot(abs(dhyper(2, 5, 5, 3) - 50/120) < 1e-10)

# Out of range
stopifnot(dhyper(-1, 5, 5, 3) == 0)
stopifnot(dhyper(4, 5, 5, 3) == 0)  # can't draw 4 white from 5 in 3 draws
"#,
    )
    .unwrap();
}

#[test]
fn phyper_qhyper_basic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Sum of all probabilities
total <- sum(sapply(0:3, function(x) dhyper(x, 5, 5, 3)))
stopifnot(abs(total - 1) < 1e-10)

# phyper at max value = 1
stopifnot(abs(phyper(3, 5, 5, 3) - 1) < 1e-10)

# qhyper(0, ...) should return min possible
stopifnot(qhyper(0, 5, 5, 3) == 0)

# qhyper(1, ...) should return max possible
stopifnot(qhyper(1, 5, 5, 3) == 3)
"#,
    )
    .unwrap();
}

// endregion

// region: Error handling

#[test]
fn distribution_parameter_errors() {
    let mut s = Session::new();

    // dexp with non-positive rate
    assert!(s.eval_source("dexp(1, rate = -1)").is_err());

    // dgamma with non-positive shape
    assert!(s.eval_source("dgamma(1, shape = -1)").is_err());

    // dbeta with non-positive shape
    assert!(s
        .eval_source("dbeta(0.5, shape1 = -1, shape2 = 1)")
        .is_err());

    // dcauchy with non-positive scale
    assert!(s.eval_source("dcauchy(0, scale = -1)").is_err());

    // dweibull with non-positive shape
    assert!(s.eval_source("dweibull(1, shape = -1)").is_err());

    // dlnorm with negative sdlog
    assert!(s.eval_source("dlnorm(1, sdlog = -1)").is_err());

    // dchisq with non-positive df
    assert!(s.eval_source("dchisq(1, df = -1)").is_err());

    // dt with non-positive df
    assert!(s.eval_source("dt(1, df = -1)").is_err());

    // dbinom with invalid prob
    assert!(s.eval_source("dbinom(1, 10, prob = 1.5)").is_err());

    // dpois with negative lambda
    assert!(s.eval_source("dpois(1, lambda = -1)").is_err());

    // dhyper with k > m+n
    assert!(s.eval_source("dhyper(1, 3, 3, 10)").is_err());
}

// endregion

// region: lower.tail, log.p, and log parameters

#[test]
fn pnorm_lower_tail() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# pnorm(0) ~ 0.5 (erfc approx has ~1e-9 precision)
stopifnot(abs(pnorm(0) - 0.5) < 1e-7)

# pnorm(0, lower.tail=FALSE) = 1 - pnorm(0) ~ 0.5
stopifnot(abs(pnorm(0, lower.tail = FALSE) - 0.5) < 1e-7)

# pnorm(1, lower.tail=FALSE) = 1 - pnorm(1)
p1 <- pnorm(1)
stopifnot(abs(pnorm(1, lower.tail = FALSE) - (1 - p1)) < 1e-10)

# pnorm(-Inf, lower.tail=FALSE) = 1
stopifnot(abs(pnorm(-Inf, lower.tail = FALSE) - 1) < 1e-10)
"#,
    )
    .unwrap();
}

#[test]
fn dnorm_log() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# dnorm(0, log=TRUE) should equal log(dnorm(0))
stopifnot(abs(dnorm(0, log = TRUE) - log(dnorm(0))) < 1e-10)

# dnorm(1, log=TRUE) should equal log(dnorm(1))
stopifnot(abs(dnorm(1, log = TRUE) - log(dnorm(1))) < 1e-10)

# dnorm(-2, log=TRUE) should equal log(dnorm(-2))
stopifnot(abs(dnorm(-2, log = TRUE) - log(dnorm(-2))) < 1e-10)
"#,
    )
    .unwrap();
}

#[test]
fn qnorm_log_p() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# qnorm(log(0.5), log.p=TRUE) should equal qnorm(0.5) = 0
stopifnot(abs(qnorm(log(0.5), log.p = TRUE) - 0) < 1e-7)

# qnorm(log(0.975), log.p=TRUE) should equal qnorm(0.975)
stopifnot(abs(qnorm(log(0.975), log.p = TRUE) - qnorm(0.975)) < 1e-7)
"#,
    )
    .unwrap();
}

#[test]
fn qnorm_lower_tail_false() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# qnorm(0.5, lower.tail=FALSE) should equal qnorm(0.5) = 0
# because 1-0.5 = 0.5, and qnorm(0.5) = 0
stopifnot(abs(qnorm(0.5, lower.tail = FALSE) - 0) < 1e-7)

# qnorm(0.025, lower.tail=FALSE) should equal qnorm(0.975)
stopifnot(abs(qnorm(0.025, lower.tail = FALSE) - qnorm(0.975)) < 1e-5)
"#,
    )
    .unwrap();
}

#[test]
fn pnorm_log_p() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# pnorm(0, log.p=TRUE) should equal log(0.5) (erfc approx precision ~1e-9)
stopifnot(abs(pnorm(0, log.p = TRUE) - log(0.5)) < 1e-7)

# pnorm(0, lower.tail=FALSE, log.p=TRUE) should also equal log(0.5)
stopifnot(abs(pnorm(0, lower.tail = FALSE, log.p = TRUE) - log(0.5)) < 1e-7)
"#,
    )
    .unwrap();
}

#[test]
fn pexp_lower_tail_and_log_p() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# pexp(1, lower.tail=FALSE) = exp(-1)
stopifnot(abs(pexp(1, lower.tail = FALSE) - exp(-1)) < 1e-10)

# pexp(1, log.p=TRUE) = log(1-exp(-1))
stopifnot(abs(pexp(1, log.p = TRUE) - log(1 - exp(-1))) < 1e-10)
"#,
    )
    .unwrap();
}

#[test]
fn dexp_log() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# dexp(0, log=TRUE) = log(1) = 0
stopifnot(abs(dexp(0, log = TRUE) - 0) < 1e-10)

# dexp(1, log=TRUE) = log(exp(-1)) = -1
stopifnot(abs(dexp(1, log = TRUE) - (-1)) < 1e-10)
"#,
    )
    .unwrap();
}

#[test]
fn qexp_lower_tail_and_log_p() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# qexp(0.5, lower.tail=FALSE) = qexp(0.5)
# because 1-0.5=0.5
stopifnot(abs(qexp(0.5, lower.tail = FALSE) - qexp(0.5)) < 1e-10)

# qexp(log(0.5), log.p=TRUE) = qexp(0.5)
stopifnot(abs(qexp(log(0.5), log.p = TRUE) - qexp(0.5)) < 1e-7)
"#,
    )
    .unwrap();
}

#[test]
fn dunif_log() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# dunif(0.5, log=TRUE) = log(1) = 0
stopifnot(abs(dunif(0.5, log = TRUE) - 0) < 1e-10)

# dunif(2, log=TRUE) = log(0) = -Inf
stopifnot(dunif(2, log = TRUE) == -Inf)
"#,
    )
    .unwrap();
}

#[test]
fn punif_lower_tail() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# punif(0.5, lower.tail=FALSE) = 1 - 0.5 = 0.5
stopifnot(abs(punif(0.5, lower.tail = FALSE) - 0.5) < 1e-10)

# punif(0.25, lower.tail=FALSE) = 0.75
stopifnot(abs(punif(0.25, lower.tail = FALSE) - 0.75) < 1e-10)
"#,
    )
    .unwrap();
}

#[test]
fn dbinom_log() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# dbinom(5, 10, 0.5, log=TRUE) should equal log(dbinom(5, 10, 0.5))
stopifnot(abs(dbinom(5, 10, 0.5, log = TRUE) - log(dbinom(5, 10, 0.5))) < 1e-10)
"#,
    )
    .unwrap();
}

#[test]
fn pbinom_lower_tail() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# pbinom(5, 10, 0.5, lower.tail=FALSE) = 1 - pbinom(5, 10, 0.5)
stopifnot(abs(pbinom(5, 10, 0.5, lower.tail = FALSE) - (1 - pbinom(5, 10, 0.5))) < 1e-10)
"#,
    )
    .unwrap();
}

#[test]
fn dpois_log() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# dpois(3, 5, log=TRUE) should equal log(dpois(3, 5))
stopifnot(abs(dpois(3, 5, log = TRUE) - log(dpois(3, 5))) < 1e-10)
"#,
    )
    .unwrap();
}

#[test]
fn pt_lower_tail() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# pt(0, df=5, lower.tail=FALSE) = 0.5
stopifnot(abs(pt(0, df = 5, lower.tail = FALSE) - 0.5) < 1e-7)
"#,
    )
    .unwrap();
}

#[test]
fn pchisq_lower_tail_log_p() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# pchisq(5, df=3, lower.tail=FALSE) = 1 - pchisq(5, df=3)
p <- pchisq(5, df = 3)
stopifnot(abs(pchisq(5, df = 3, lower.tail = FALSE) - (1 - p)) < 1e-10)

# pchisq(5, df=3, log.p=TRUE) = log(pchisq(5, df=3))
stopifnot(abs(pchisq(5, df = 3, log.p = TRUE) - log(p)) < 1e-10)
"#,
    )
    .unwrap();
}

#[test]
fn all_distributions_log_flag_for_density() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Test log flag for all d* functions
check_log <- function(name, d, d_log) {
    expected <- log(d)
    tol <- 1e-8
    if (is.finite(expected)) {
        stopifnot(abs(d_log - expected) < tol)
    }
}

check_log("dgamma", dgamma(2, shape = 2), dgamma(2, shape = 2, log = TRUE))
check_log("dbeta", dbeta(0.5, 2, 3), dbeta(0.5, 2, 3, log = TRUE))
check_log("dcauchy", dcauchy(1), dcauchy(1, log = TRUE))
check_log("dweibull", dweibull(1, shape = 2), dweibull(1, shape = 2, log = TRUE))
check_log("dlnorm", dlnorm(1), dlnorm(1, log = TRUE))
check_log("dchisq", dchisq(2, df = 3), dchisq(2, df = 3, log = TRUE))
check_log("dt", dt(1, df = 5), dt(1, df = 5, log = TRUE))
check_log("df", df(1, 3, 5), df(1, 3, 5, log = TRUE))
check_log("dgeom", dgeom(2, 0.3), dgeom(2, 0.3, log = TRUE))
check_log("dhyper", dhyper(2, 5, 5, 3), dhyper(2, 5, 5, 3, log = TRUE))
"#,
    )
    .unwrap();
}

#[test]
fn all_distributions_lower_tail_for_cdf() {
    let mut s = Session::new();
    s.eval_source(
        r#"
check_lt <- function(name, p, p_ut) {
    tol <- 1e-8
    stopifnot(abs(p_ut - (1 - p)) < tol)
}

check_lt("pgamma", pgamma(2, shape = 2), pgamma(2, shape = 2, lower.tail = FALSE))
check_lt("pbeta", pbeta(0.5, 2, 3), pbeta(0.5, 2, 3, lower.tail = FALSE))
check_lt("pcauchy", pcauchy(1), pcauchy(1, lower.tail = FALSE))
check_lt("pweibull", pweibull(1, shape = 2), pweibull(1, shape = 2, lower.tail = FALSE))
check_lt("plnorm", plnorm(1), plnorm(1, lower.tail = FALSE))
check_lt("pf", pf(1, 3, 5), pf(1, 3, 5, lower.tail = FALSE))
check_lt("pgeom", pgeom(2, 0.3), pgeom(2, 0.3, lower.tail = FALSE))
check_lt("phyper", phyper(2, 5, 5, 3), phyper(2, 5, 5, 3, lower.tail = FALSE))
"#,
    )
    .unwrap();
}

// endregion
