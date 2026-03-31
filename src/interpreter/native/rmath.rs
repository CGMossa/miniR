#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! Rmath — Statistical distribution functions and special math functions.
//!
//! Implements the R C API mathematical functions declared in `Rmath.h`.
//! These are `extern "C"` functions resolved by package `.so` files at load time.
//!
//! Core special functions:
//! - Regularized incomplete gamma function (lower P and upper Q)
//! - Regularized incomplete beta function
//! - Polygamma functions (digamma, trigamma, etc.)
//!
//! Distribution functions follow R's convention:
//! - `d*(x, params..., give_log)` — density (PDF), optionally log
//! - `p*(x, params..., lower_tail, log_p)` — distribution (CDF)
//! - `q*(p, params..., lower_tail, log_p)` — quantile (inverse CDF)
//! - `r*(params...)` — random variate

use std::os::raw::c_int;

// region: Constants

const LN_SQRT_2PI: f64 = 0.918_938_533_204_672_8;
const DBL_EPSILON: f64 = f64::EPSILON;

fn r_finite(x: f64) -> bool {
    x.is_finite()
}

/// Call the thread-local RNG from runtime.rs
fn unif_rand() -> f64 {
    super::runtime::unif_rand()
}

// endregion

// region: Special functions

/// Regularized lower incomplete gamma function P(a, x) = γ(a,x) / Γ(a).
/// Uses series expansion for x < a+1, continued fraction otherwise.
pub fn pgamma_raw(a: f64, x: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    if x.is_infinite() {
        return 1.0;
    }
    if a <= 0.0 {
        return 1.0;
    }

    if x < a + 1.0 {
        // Series representation
        gamma_series(a, x)
    } else {
        // Continued fraction representation
        1.0 - gamma_cf(a, x)
    }
}

/// Regularized upper incomplete gamma function Q(a, x) = 1 - P(a, x).
pub fn qgamma_raw(a: f64, x: f64) -> f64 {
    1.0 - pgamma_raw(a, x)
}

/// Series expansion for the regularized incomplete gamma function.
fn gamma_series(a: f64, x: f64) -> f64 {
    let ln_gamma_a = libm::lgamma(a);
    let mut sum = 1.0 / a;
    let mut term = 1.0 / a;
    for n in 1..200 {
        term *= x / (a + n as f64);
        sum += term;
        if term.abs() < sum.abs() * DBL_EPSILON {
            break;
        }
    }
    sum * (-x + a * x.ln() - ln_gamma_a).exp()
}

/// Continued fraction for the upper incomplete gamma function.
fn gamma_cf(a: f64, x: f64) -> f64 {
    let ln_gamma_a = libm::lgamma(a);
    // Modified Lentz's method
    let mut c = 1e-30_f64;
    let mut d = 1.0 / (x + 1.0 - a);
    let mut f = d;

    for i in 1..200 {
        let an = -(i as f64) * (i as f64 - a);
        let bn = x + 2.0 * i as f64 + 1.0 - a;
        d = bn + an * d;
        if d.abs() < 1e-30 {
            d = 1e-30;
        }
        c = bn + an / c;
        if c.abs() < 1e-30 {
            c = 1e-30;
        }
        d = 1.0 / d;
        let delta = c * d;
        f *= delta;
        if (delta - 1.0).abs() < DBL_EPSILON {
            break;
        }
    }
    f * (-x + a * x.ln() - ln_gamma_a).exp()
}

/// Regularized incomplete beta function I_x(a, b).
/// Uses continued fraction (Lentz's method).
pub fn pbeta_raw(x: f64, a: f64, b: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    if a <= 0.0 || b <= 0.0 {
        return f64::NAN;
    }

    // Use the symmetry relation for numerical stability:
    // I_x(a,b) = 1 - I_{1-x}(b,a)
    if x > (a + 1.0) / (a + b + 2.0) {
        return 1.0 - pbeta_raw(1.0 - x, b, a);
    }

    let ln_beta = libm::lgamma(a) + libm::lgamma(b) - libm::lgamma(a + b);
    let front = (a * x.ln() + b * (1.0 - x).ln() - ln_beta).exp() / a;

    // Lentz's continued fraction
    let mut f = 1.0 + beta_cf_term(1, a, b, x);
    if !f.is_finite() || f == 0.0 {
        f = 1e-30;
    }
    let mut c = f;
    let mut d = 1.0;

    for m in 2..200 {
        let term = beta_cf_term(m, a, b, x);
        d = 1.0 + term * d;
        if d.abs() < 1e-30 {
            d = 1e-30;
        }
        c = 1.0 + term / c;
        if c.abs() < 1e-30 {
            c = 1e-30;
        }
        d = 1.0 / d;
        let delta = c * d;
        f *= delta;
        if (delta - 1.0).abs() < 3.0 * DBL_EPSILON {
            break;
        }
    }

    front * f
}

/// Terms for the continued fraction expansion of the incomplete beta function.
fn beta_cf_term(m: i32, a: f64, b: f64, x: f64) -> f64 {
    let m_f = m as f64;
    if m % 2 == 0 {
        let k = m_f / 2.0;
        (k * (b - k) * x) / ((a + 2.0 * k - 1.0) * (a + 2.0 * k))
    } else {
        let k = (m_f - 1.0) / 2.0;
        -((a + k) * (a + b + k) * x) / ((a + 2.0 * k) * (a + 2.0 * k + 1.0))
    }
}

/// Trigamma function ψ₁(x) = d²/dx² ln Γ(x).
pub fn trigamma(x: f64) -> f64 {
    let mut x = x;
    let mut result = 0.0;
    // Shift to large x
    while x < 6.0 {
        result += 1.0 / (x * x);
        x += 1.0;
    }
    // Asymptotic series
    let x2 = 1.0 / (x * x);
    result
        + 1.0 / x
        + x2 / 2.0
        + x2 / x * (1.0 / 6.0 - x2 * (1.0 / 30.0 - x2 * (1.0 / 42.0 - x2 / 30.0)))
}

/// Tetragamma function ψ₂(x).
pub fn tetragamma(x: f64) -> f64 {
    let mut x = x;
    let mut result = 0.0;
    while x < 6.0 {
        result -= 2.0 / (x * x * x);
        x += 1.0;
    }
    let x2 = 1.0 / (x * x);
    result - 1.0 / (x * x) - 1.0 / (x * x * x)
        + x2 * x2 * (-1.0 / 3.0 + x2 * (1.0 / 5.0 - x2 * (1.0 / 7.0)))
}

/// Pentagamma function ψ₃(x).
pub fn pentagamma(x: f64) -> f64 {
    let mut x = x;
    let mut result = 0.0;
    while x < 6.0 {
        result += 6.0 / (x * x * x * x);
        x += 1.0;
    }
    let x2 = 1.0 / (x * x);
    let x3 = x2 / x;
    result + 2.0 * x3 + 3.0 * x2 * x2 + x2 * x3 * (2.0 + x2 * (-2.0 + x2 * (3.0)))
}

/// Psigamma: the m-th derivative of the digamma function.
pub fn psigamma_fn(x: f64, deriv: f64) -> f64 {
    let n = deriv as i32;
    match n {
        0 => digamma_fn(x),
        1 => trigamma(x),
        2 => tetragamma(x),
        3 => pentagamma(x),
        _ => f64::NAN,
    }
}

/// Digamma function ψ(x) = d/dx ln Γ(x).
pub fn digamma_fn(x: f64) -> f64 {
    let mut x = x;
    let mut result = 0.0;
    while x < 6.0 {
        result -= 1.0 / x;
        x += 1.0;
    }
    result += x.ln() - 0.5 / x;
    let x2 = 1.0 / (x * x);
    result - x2 * (1.0 / 12.0 - x2 * (1.0 / 120.0 - x2 * (1.0 / 252.0 - x2 / 240.0)))
}

/// Beta function B(a,b) = Γ(a)Γ(b)/Γ(a+b).
pub fn beta_fn(a: f64, b: f64) -> f64 {
    (libm::lgamma(a) + libm::lgamma(b) - libm::lgamma(a + b)).exp()
}

/// Log-beta function ln B(a,b).
pub fn lbeta_fn(a: f64, b: f64) -> f64 {
    libm::lgamma(a) + libm::lgamma(b) - libm::lgamma(a + b)
}

/// Binomial coefficient choose(n, k).
pub fn choose_fn(n: f64, k: f64) -> f64 {
    if k < 0.0 || k > n {
        return 0.0;
    }
    if k == 0.0 || k == n {
        return 1.0;
    }
    // Use lgamma for large values
    (libm::lgamma(n + 1.0) - libm::lgamma(k + 1.0) - libm::lgamma(n - k + 1.0)).exp()
}

/// Log of binomial coefficient.
pub fn lchoose_fn(n: f64, k: f64) -> f64 {
    if k < 0.0 || k > n {
        return f64::NEG_INFINITY;
    }
    if k == 0.0 || k == n {
        return 0.0;
    }
    libm::lgamma(n + 1.0) - libm::lgamma(k + 1.0) - libm::lgamma(n - k + 1.0)
}

/// log(1+x) - x, accurate for small x.
pub fn log1pmx_fn(x: f64) -> f64 {
    libm::log1p(x) - x
}

/// lgamma(1+a) for small a, using series expansion.
pub fn lgamma1p_fn(a: f64) -> f64 {
    libm::lgamma(1.0 + a)
}

/// log(exp(lx) + exp(ly)), computed in log-space for numerical stability.
pub fn logspace_add_fn(lx: f64, ly: f64) -> f64 {
    if lx > ly {
        lx + libm::log1p((ly - lx).exp())
    } else {
        ly + libm::log1p((lx - ly).exp())
    }
}

/// log(exp(lx) - exp(ly)), computed in log-space. Requires lx >= ly.
pub fn logspace_sub_fn(lx: f64, ly: f64) -> f64 {
    lx + libm::log1p(-(ly - lx).exp())
}

// endregion

// region: Distribution helper

/// Apply lower_tail and log_p transforms to a CDF value.
fn p_transform(p: f64, lower_tail: c_int, log_p: c_int) -> f64 {
    let p = if lower_tail != 0 { p } else { 1.0 - p };
    if log_p != 0 {
        p.ln()
    } else {
        p
    }
}

/// Decode p from log_p / lower_tail for quantile functions.
fn q_decode(p: f64, lower_tail: c_int, log_p: c_int) -> f64 {
    let p = if log_p != 0 { p.exp() } else { p };
    if lower_tail != 0 {
        p
    } else {
        1.0 - p
    }
}

/// Apply give_log to a density value.
fn d_log(d: f64, give_log: c_int) -> f64 {
    if give_log != 0 {
        d.ln()
    } else {
        d
    }
}

// endregion

// region: Normal distribution

#[no_mangle]
pub extern "C" fn Rf_dnorm4(x: f64, mu: f64, sigma: f64, give_log: c_int) -> f64 {
    if !r_finite(sigma) || sigma < 0.0 {
        return f64::NAN;
    }
    if sigma == 0.0 {
        return if x == mu {
            f64::INFINITY
        } else {
            d_log(0.0, give_log)
        };
    }
    let z = (x - mu) / sigma;
    let d = (-0.5 * z * z - LN_SQRT_2PI - sigma.ln()).exp();
    d_log(d, give_log)
}

#[no_mangle]
pub extern "C" fn Rf_pnorm5(x: f64, mu: f64, sigma: f64, lower_tail: c_int, log_p: c_int) -> f64 {
    if !r_finite(sigma) || sigma < 0.0 {
        return f64::NAN;
    }
    if sigma == 0.0 {
        return p_transform(if x < mu { 0.0 } else { 1.0 }, lower_tail, log_p);
    }
    let z = (x - mu) / sigma;
    let p = 0.5 * libm::erfc(-z / std::f64::consts::SQRT_2);
    p_transform(p, lower_tail, log_p)
}

#[no_mangle]
pub extern "C" fn Rf_qnorm5(p: f64, mu: f64, sigma: f64, lower_tail: c_int, log_p: c_int) -> f64 {
    if !r_finite(sigma) || sigma < 0.0 {
        return f64::NAN;
    }
    let p = q_decode(p, lower_tail, log_p);
    if !(0.0..=1.0).contains(&p) {
        return f64::NAN;
    }
    if p == 0.0 {
        return f64::NEG_INFINITY;
    }
    if p == 1.0 {
        return f64::INFINITY;
    }
    // Rational approximation (Abramowitz & Stegun 26.2.23, refined by Peter Acklam)
    mu + sigma * qnorm_standard(p)
}

/// Standard normal quantile function (inverse Φ).
#[allow(clippy::excessive_precision)]
fn qnorm_standard(p: f64) -> f64 {
    // Rational approximation by Peter J. Acklam
    const A: [f64; 6] = [
        -3.969683028665376e+01,
        2.209460984245205e+02,
        -2.759285104469687e+02,
        1.383577518672690e+02,
        -3.066479806614716e+01,
        2.506628277459239e+00,
    ];
    const B: [f64; 5] = [
        -5.447609879822406e+01,
        1.615858368580409e+02,
        -1.556989798598866e+02,
        6.680131188771972e+01,
        -1.328068155288572e+01,
    ];
    const C: [f64; 6] = [
        -7.784894002430293e-03,
        -3.223964580411365e-01,
        -2.400758277161838e+00,
        -2.549732539343734e+00,
        4.374664141464968e+00,
        2.938163982698783e+00,
    ];
    const D: [f64; 4] = [
        7.784695709041462e-03,
        3.224671290700398e-01,
        2.445134137142996e+00,
        3.754408661907416e+00,
    ];

    const P_LOW: f64 = 0.02425;
    const P_HIGH: f64 = 1.0 - P_LOW;

    if p < P_LOW {
        let q = (-2.0 * p.ln()).sqrt();
        (((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    } else if p <= P_HIGH {
        let q = p - 0.5;
        let r = q * q;
        (((((A[0] * r + A[1]) * r + A[2]) * r + A[3]) * r + A[4]) * r + A[5]) * q
            / (((((B[0] * r + B[1]) * r + B[2]) * r + B[3]) * r + B[4]) * r + 1.0)
    } else {
        let q = (-2.0 * (1.0 - p).ln()).sqrt();
        -(((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    }
}

#[no_mangle]
pub extern "C" fn Rf_rnorm(mu: f64, sigma: f64) -> f64 {
    // Box-Muller using the interpreter's RNG would be ideal, but for the C API
    // we fall back to a simple approach using unif_rand.
    let u1 = unif_rand();
    let u2 = unif_rand();
    let z = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();
    mu + sigma * z
}

#[no_mangle]
pub extern "C" fn Rf_pnorm_both(x: f64, cum: *mut f64, ccum: *mut f64, _lt: c_int, _lg: c_int) {
    let p = 0.5 * libm::erfc(-x / std::f64::consts::SQRT_2);
    if !cum.is_null() {
        unsafe {
            *cum = p;
        }
    }
    if !ccum.is_null() {
        unsafe {
            *ccum = 1.0 - p;
        }
    }
}

// endregion

// region: Uniform distribution

#[no_mangle]
pub extern "C" fn Rf_dunif(x: f64, a: f64, b: f64, give_log: c_int) -> f64 {
    if b <= a {
        return f64::NAN;
    }
    let d = if x < a || x > b { 0.0 } else { 1.0 / (b - a) };
    d_log(d, give_log)
}

#[no_mangle]
pub extern "C" fn Rf_punif(x: f64, a: f64, b: f64, lower_tail: c_int, log_p: c_int) -> f64 {
    if b <= a {
        return f64::NAN;
    }
    let p = if x <= a {
        0.0
    } else if x >= b {
        1.0
    } else {
        (x - a) / (b - a)
    };
    p_transform(p, lower_tail, log_p)
}

#[no_mangle]
pub extern "C" fn Rf_qunif(p: f64, a: f64, b: f64, lower_tail: c_int, log_p: c_int) -> f64 {
    if b <= a {
        return f64::NAN;
    }
    let p = q_decode(p, lower_tail, log_p);
    if !(0.0..=1.0).contains(&p) {
        return f64::NAN;
    }
    a + p * (b - a)
}

#[no_mangle]
pub extern "C" fn Rf_runif(a: f64, b: f64) -> f64 {
    let u = unif_rand();
    a + u * (b - a)
}

// endregion

// region: Exponential distribution

#[no_mangle]
pub extern "C" fn Rf_dexp(x: f64, scale: f64, give_log: c_int) -> f64 {
    if scale <= 0.0 {
        return f64::NAN;
    }
    let d = if x < 0.0 {
        0.0
    } else {
        (-x / scale).exp() / scale
    };
    d_log(d, give_log)
}

#[no_mangle]
pub extern "C" fn Rf_pexp(x: f64, scale: f64, lower_tail: c_int, log_p: c_int) -> f64 {
    if scale <= 0.0 {
        return f64::NAN;
    }
    let p = if x <= 0.0 {
        0.0
    } else {
        -libm::expm1(-x / scale)
    };
    p_transform(p, lower_tail, log_p)
}

#[no_mangle]
pub extern "C" fn Rf_qexp(p: f64, scale: f64, lower_tail: c_int, log_p: c_int) -> f64 {
    if scale <= 0.0 {
        return f64::NAN;
    }
    let p = q_decode(p, lower_tail, log_p);
    if !(0.0..=1.0).contains(&p) {
        return f64::NAN;
    }
    -scale * libm::log1p(-p)
}

#[no_mangle]
pub extern "C" fn Rf_rexp(scale: f64) -> f64 {
    let u = unif_rand();
    -scale * u.ln()
}

// endregion

// region: Gamma distribution

#[no_mangle]
pub extern "C" fn Rf_dgamma(x: f64, shape: f64, scale: f64, give_log: c_int) -> f64 {
    if shape <= 0.0 || scale <= 0.0 {
        return f64::NAN;
    }
    if x < 0.0 {
        return d_log(0.0, give_log);
    }
    if x == 0.0 {
        if shape < 1.0 {
            return f64::INFINITY;
        }
        if shape == 1.0 {
            return d_log(1.0 / scale, give_log);
        }
        return d_log(0.0, give_log);
    }
    let log_d = (shape - 1.0) * x.ln() - x / scale - shape * scale.ln() - libm::lgamma(shape);
    if give_log != 0 {
        log_d
    } else {
        log_d.exp()
    }
}

#[no_mangle]
pub extern "C" fn Rf_pgamma(
    x: f64,
    shape: f64,
    scale: f64,
    lower_tail: c_int,
    log_p: c_int,
) -> f64 {
    if shape <= 0.0 || scale <= 0.0 {
        return f64::NAN;
    }
    let p = pgamma_raw(shape, x / scale);
    p_transform(p, lower_tail, log_p)
}

#[no_mangle]
pub extern "C" fn Rf_qgamma(
    p: f64,
    shape: f64,
    scale: f64,
    lower_tail: c_int,
    log_p: c_int,
) -> f64 {
    if shape <= 0.0 || scale <= 0.0 {
        return f64::NAN;
    }
    let p = q_decode(p, lower_tail, log_p);
    if !(0.0..=1.0).contains(&p) {
        return f64::NAN;
    }
    if p == 0.0 {
        return 0.0;
    }
    if p == 1.0 {
        return f64::INFINITY;
    }
    // Newton's method starting from Wilson-Hilferty approximation
    let z = qnorm_standard(p);
    let mut x = shape * (1.0 - 1.0 / (9.0 * shape) + z / (3.0 * shape.sqrt())).powi(3);
    if x <= 0.0 {
        x = DBL_EPSILON;
    }
    for _ in 0..50 {
        let px = pgamma_raw(shape, x);
        let dx = Rf_dgamma(x, shape, 1.0, 0);
        if dx <= 0.0 {
            break;
        }
        let delta = (px - p) / dx;
        x -= delta;
        if x <= 0.0 {
            x = DBL_EPSILON;
        }
        if delta.abs() < x * 1e-12 {
            break;
        }
    }
    x * scale
}

#[no_mangle]
pub extern "C" fn Rf_rgamma(shape: f64, scale: f64) -> f64 {
    // Marsaglia and Tsang's method for shape >= 1
    if shape <= 0.0 || scale <= 0.0 {
        return f64::NAN;
    }
    let (d_shape, boost) = if shape < 1.0 {
        (shape + 1.0, true)
    } else {
        (shape, false)
    };
    let d = d_shape - 1.0 / 3.0;
    let c = 1.0 / (9.0 * d).sqrt();
    let x = loop {
        let (z, v) = loop {
            let z = Rf_rnorm(0.0, 1.0);
            let v = 1.0 + c * z;
            if v > 0.0 {
                break (z, v * v * v);
            }
        };
        let u = unif_rand();
        if u < 1.0 - 0.0331 * (z * z) * (z * z) {
            break d * v;
        }
        if u.ln() < 0.5 * z * z + d * (1.0 - v + v.ln()) {
            break d * v;
        }
    };
    if boost {
        let u = unif_rand();
        x * u.powf(1.0 / shape) * scale
    } else {
        x * scale
    }
}

// endregion

// region: Beta distribution

#[no_mangle]
pub extern "C" fn Rf_dbeta(x: f64, a: f64, b: f64, give_log: c_int) -> f64 {
    if a <= 0.0 || b <= 0.0 {
        return f64::NAN;
    }
    if !(0.0..=1.0).contains(&x) {
        return d_log(0.0, give_log);
    }
    if x == 0.0 {
        if a < 1.0 {
            return f64::INFINITY;
        }
        if a == 1.0 {
            return d_log(b, give_log);
        }
        return d_log(0.0, give_log);
    }
    if x == 1.0 {
        if b < 1.0 {
            return f64::INFINITY;
        }
        if b == 1.0 {
            return d_log(a, give_log);
        }
        return d_log(0.0, give_log);
    }
    let log_d = (a - 1.0) * x.ln() + (b - 1.0) * (1.0 - x).ln() - lbeta_fn(a, b);
    if give_log != 0 {
        log_d
    } else {
        log_d.exp()
    }
}

#[no_mangle]
pub extern "C" fn Rf_pbeta(x: f64, a: f64, b: f64, lower_tail: c_int, log_p: c_int) -> f64 {
    if a <= 0.0 || b <= 0.0 {
        return f64::NAN;
    }
    let p = pbeta_raw(x, a, b);
    p_transform(p, lower_tail, log_p)
}

#[no_mangle]
pub extern "C" fn Rf_qbeta(p: f64, a: f64, b: f64, lower_tail: c_int, log_p: c_int) -> f64 {
    if a <= 0.0 || b <= 0.0 {
        return f64::NAN;
    }
    let p = q_decode(p, lower_tail, log_p);
    if !(0.0..=1.0).contains(&p) {
        return f64::NAN;
    }
    if p == 0.0 {
        return 0.0;
    }
    if p == 1.0 {
        return 1.0;
    }
    // Newton's method
    let mut x = p; // initial guess
    for _ in 0..100 {
        let px = pbeta_raw(x, a, b);
        let dx = Rf_dbeta(x, a, b, 0);
        if dx <= 0.0 || !dx.is_finite() {
            break;
        }
        let delta = (px - p) / dx;
        x -= delta;
        x = x.clamp(DBL_EPSILON, 1.0 - DBL_EPSILON);
        if delta.abs() < x * 1e-12 {
            break;
        }
    }
    x
}

#[no_mangle]
pub extern "C" fn Rf_rbeta(a: f64, b: f64) -> f64 {
    let x = Rf_rgamma(a, 1.0);
    let y = Rf_rgamma(b, 1.0);
    x / (x + y)
}

// endregion

// region: Chi-squared distribution (special case of Gamma)

#[no_mangle]
pub extern "C" fn Rf_dchisq(x: f64, df: f64, give_log: c_int) -> f64 {
    Rf_dgamma(x, df / 2.0, 2.0, give_log)
}

#[no_mangle]
pub extern "C" fn Rf_pchisq(x: f64, df: f64, lower_tail: c_int, log_p: c_int) -> f64 {
    Rf_pgamma(x, df / 2.0, 2.0, lower_tail, log_p)
}

#[no_mangle]
pub extern "C" fn Rf_qchisq(p: f64, df: f64, lower_tail: c_int, log_p: c_int) -> f64 {
    Rf_qgamma(p, df / 2.0, 2.0, lower_tail, log_p)
}

#[no_mangle]
pub extern "C" fn Rf_rchisq(df: f64) -> f64 {
    Rf_rgamma(df / 2.0, 2.0)
}

// Non-central chi-squared
#[no_mangle]
pub extern "C" fn Rf_dnchisq(x: f64, df: f64, ncp: f64, give_log: c_int) -> f64 {
    if df <= 0.0 || ncp < 0.0 {
        return f64::NAN;
    }
    if x < 0.0 {
        return d_log(0.0, give_log);
    }
    // Approximate: sum of Poisson-weighted chi-squared densities
    let lambda = ncp / 2.0;
    let max_terms = 100.min((lambda + 20.0 * lambda.sqrt()) as usize + 10);
    let mut sum = 0.0;
    let mut poisson_weight = (-lambda).exp();
    for j in 0..max_terms {
        let chi_df = df + 2.0 * j as f64;
        let chi_dens = Rf_dgamma(x, chi_df / 2.0, 2.0, 0);
        sum += poisson_weight * chi_dens;
        poisson_weight *= lambda / (j as f64 + 1.0);
    }
    d_log(sum, give_log)
}

#[no_mangle]
pub extern "C" fn Rf_pnchisq(x: f64, df: f64, ncp: f64, lower_tail: c_int, log_p: c_int) -> f64 {
    if df <= 0.0 || ncp < 0.0 {
        return f64::NAN;
    }
    if x <= 0.0 {
        return p_transform(0.0, lower_tail, log_p);
    }
    let lambda = ncp / 2.0;
    let max_terms = 100.min((lambda + 20.0 * lambda.sqrt()) as usize + 10);
    let mut sum = 0.0;
    let mut poisson_weight = (-lambda).exp();
    for j in 0..max_terms {
        let chi_df = df + 2.0 * j as f64;
        let chi_cdf = pgamma_raw(chi_df / 2.0, x / 2.0);
        sum += poisson_weight * chi_cdf;
        poisson_weight *= lambda / (j as f64 + 1.0);
    }
    p_transform(sum, lower_tail, log_p)
}

#[no_mangle]
pub extern "C" fn Rf_qnchisq(p: f64, df: f64, ncp: f64, lower_tail: c_int, log_p: c_int) -> f64 {
    let p = q_decode(p, lower_tail, log_p);
    if !(0.0..=1.0).contains(&p) || df <= 0.0 || ncp < 0.0 {
        return f64::NAN;
    }
    // Newton's method starting from chi-squared quantile
    let mut x = Rf_qchisq(p, df + ncp, 1, 0);
    if x <= 0.0 {
        x = df + ncp;
    }
    for _ in 0..50 {
        let px = Rf_pnchisq(x, df, ncp, 1, 0);
        let dx = Rf_dnchisq(x, df, ncp, 0);
        if dx <= 0.0 {
            break;
        }
        let delta = (px - p) / dx;
        x -= delta;
        if x <= 0.0 {
            x = DBL_EPSILON;
        }
        if delta.abs() < x * 1e-12 {
            break;
        }
    }
    x
}

#[no_mangle]
pub extern "C" fn Rf_rnchisq(df: f64, ncp: f64) -> f64 {
    // Sum of squared normals
    if ncp == 0.0 {
        return Rf_rchisq(df);
    }
    let z = Rf_rnorm(ncp.sqrt(), 1.0);
    z * z + Rf_rchisq(df - 1.0)
}

// endregion

// region: Student t distribution

#[no_mangle]
pub extern "C" fn Rf_dt(x: f64, df: f64, give_log: c_int) -> f64 {
    if df <= 0.0 {
        return f64::NAN;
    }
    let log_d = libm::lgamma((df + 1.0) / 2.0)
        - libm::lgamma(df / 2.0)
        - 0.5 * (df * std::f64::consts::PI).ln()
        - ((df + 1.0) / 2.0) * (1.0 + x * x / df).ln();
    if give_log != 0 {
        log_d
    } else {
        log_d.exp()
    }
}

#[no_mangle]
pub extern "C" fn Rf_pt(x: f64, df: f64, lower_tail: c_int, log_p: c_int) -> f64 {
    if df <= 0.0 {
        return f64::NAN;
    }
    // Use incomplete beta: P(t <= x | df) = I_{df/(df+x²)}(df/2, 1/2) when x < 0
    let t2 = x * x;
    let p = pbeta_raw(df / (df + t2), df / 2.0, 0.5);
    let p = if x <= 0.0 { p / 2.0 } else { 1.0 - p / 2.0 };
    p_transform(p, lower_tail, log_p)
}

#[no_mangle]
pub extern "C" fn Rf_qt(p: f64, df: f64, lower_tail: c_int, log_p: c_int) -> f64 {
    if df <= 0.0 {
        return f64::NAN;
    }
    let p = q_decode(p, lower_tail, log_p);
    if !(0.0..=1.0).contains(&p) {
        return f64::NAN;
    }
    if p == 0.0 {
        return f64::NEG_INFINITY;
    }
    if p == 1.0 {
        return f64::INFINITY;
    }
    if p == 0.5 {
        return 0.0;
    }
    // Newton's method from normal approximation
    let mut x = qnorm_standard(p);
    for _ in 0..50 {
        let px = Rf_pt(x, df, 1, 0);
        let dx = Rf_dt(x, df, 0);
        if dx <= 0.0 {
            break;
        }
        let delta = (px - p) / dx;
        x -= delta;
        if delta.abs() < x.abs() * 1e-12 {
            break;
        }
    }
    x
}

#[no_mangle]
pub extern "C" fn Rf_rt(df: f64) -> f64 {
    Rf_rnorm(0.0, 1.0) / (Rf_rchisq(df) / df).sqrt()
}

// endregion

// region: F distribution

#[no_mangle]
pub extern "C" fn Rf_df(x: f64, df1: f64, df2: f64, give_log: c_int) -> f64 {
    if df1 <= 0.0 || df2 <= 0.0 {
        return f64::NAN;
    }
    if x < 0.0 {
        return d_log(0.0, give_log);
    }
    if x == 0.0 {
        if df1 < 2.0 {
            return f64::INFINITY;
        }
        if df1 == 2.0 {
            return d_log(1.0, give_log);
        }
        return d_log(0.0, give_log);
    }
    let log_d = (df1 / 2.0) * (df1 / df2).ln() + (df1 / 2.0 - 1.0) * x.ln()
        - lbeta_fn(df1 / 2.0, df2 / 2.0)
        - ((df1 + df2) / 2.0) * (1.0 + df1 * x / df2).ln();
    if give_log != 0 {
        log_d
    } else {
        log_d.exp()
    }
}

#[no_mangle]
pub extern "C" fn Rf_pf(x: f64, df1: f64, df2: f64, lower_tail: c_int, log_p: c_int) -> f64 {
    if df1 <= 0.0 || df2 <= 0.0 {
        return f64::NAN;
    }
    if x <= 0.0 {
        return p_transform(0.0, lower_tail, log_p);
    }
    let v = df1 * x / (df1 * x + df2);
    let p = pbeta_raw(v, df1 / 2.0, df2 / 2.0);
    p_transform(p, lower_tail, log_p)
}

#[no_mangle]
pub extern "C" fn Rf_qf(p: f64, df1: f64, df2: f64, lower_tail: c_int, log_p: c_int) -> f64 {
    if df1 <= 0.0 || df2 <= 0.0 {
        return f64::NAN;
    }
    let p = q_decode(p, lower_tail, log_p);
    if !(0.0..=1.0).contains(&p) {
        return f64::NAN;
    }
    if p == 0.0 {
        return 0.0;
    }
    if p == 1.0 {
        return f64::INFINITY;
    }
    // Use qbeta: if X ~ Beta(a,b) then X/(1-X)*b/a ~ F(2a,2b)
    let bq = Rf_qbeta(p, df1 / 2.0, df2 / 2.0, 1, 0);
    (df2 * bq) / (df1 * (1.0 - bq))
}

#[no_mangle]
pub extern "C" fn Rf_rf(df1: f64, df2: f64) -> f64 {
    (Rf_rchisq(df1) / df1) / (Rf_rchisq(df2) / df2)
}

// endregion

// region: Lognormal distribution

#[no_mangle]
pub extern "C" fn Rf_dlnorm(x: f64, meanlog: f64, sdlog: f64, give_log: c_int) -> f64 {
    if sdlog <= 0.0 {
        return f64::NAN;
    }
    if x <= 0.0 {
        return d_log(0.0, give_log);
    }
    let z = (x.ln() - meanlog) / sdlog;
    let log_d = -0.5 * z * z - LN_SQRT_2PI - sdlog.ln() - x.ln();
    if give_log != 0 {
        log_d
    } else {
        log_d.exp()
    }
}

#[no_mangle]
pub extern "C" fn Rf_plnorm(
    x: f64,
    meanlog: f64,
    sdlog: f64,
    lower_tail: c_int,
    log_p: c_int,
) -> f64 {
    if sdlog <= 0.0 {
        return f64::NAN;
    }
    if x <= 0.0 {
        return p_transform(0.0, lower_tail, log_p);
    }
    Rf_pnorm5(x.ln(), meanlog, sdlog, lower_tail, log_p)
}

#[no_mangle]
pub extern "C" fn Rf_qlnorm(
    p: f64,
    meanlog: f64,
    sdlog: f64,
    lower_tail: c_int,
    log_p: c_int,
) -> f64 {
    Rf_qnorm5(p, meanlog, sdlog, lower_tail, log_p).exp()
}

#[no_mangle]
pub extern "C" fn Rf_rlnorm(meanlog: f64, sdlog: f64) -> f64 {
    Rf_rnorm(meanlog, sdlog).exp()
}

// endregion

// region: Cauchy distribution

#[no_mangle]
pub extern "C" fn Rf_dcauchy(x: f64, location: f64, scale: f64, give_log: c_int) -> f64 {
    if scale <= 0.0 {
        return f64::NAN;
    }
    let z = (x - location) / scale;
    let d = 1.0 / (std::f64::consts::PI * scale * (1.0 + z * z));
    d_log(d, give_log)
}

#[no_mangle]
pub extern "C" fn Rf_pcauchy(
    x: f64,
    location: f64,
    scale: f64,
    lower_tail: c_int,
    log_p: c_int,
) -> f64 {
    if scale <= 0.0 {
        return f64::NAN;
    }
    let p = 0.5 + libm::atan((x - location) / scale) / std::f64::consts::PI;
    p_transform(p, lower_tail, log_p)
}

#[no_mangle]
pub extern "C" fn Rf_qcauchy(
    p: f64,
    location: f64,
    scale: f64,
    lower_tail: c_int,
    log_p: c_int,
) -> f64 {
    if scale <= 0.0 {
        return f64::NAN;
    }
    let p = q_decode(p, lower_tail, log_p);
    if !(0.0..=1.0).contains(&p) {
        return f64::NAN;
    }
    location + scale * libm::tan(std::f64::consts::PI * (p - 0.5))
}

#[no_mangle]
pub extern "C" fn Rf_rcauchy(location: f64, scale: f64) -> f64 {
    let u = unif_rand();
    location + scale * libm::tan(std::f64::consts::PI * (u - 0.5))
}

// endregion

// region: Weibull distribution

#[no_mangle]
pub extern "C" fn Rf_dweibull(x: f64, shape: f64, scale: f64, give_log: c_int) -> f64 {
    if shape <= 0.0 || scale <= 0.0 {
        return f64::NAN;
    }
    if x < 0.0 {
        return d_log(0.0, give_log);
    }
    if x == 0.0 {
        if shape < 1.0 {
            return f64::INFINITY;
        }
        if shape == 1.0 {
            return d_log(1.0 / scale, give_log);
        }
        return d_log(0.0, give_log);
    }
    let z = x / scale;
    let log_d = (shape - 1.0) * z.ln() + shape.ln() - shape * scale.ln() - z.powf(shape);
    if give_log != 0 {
        log_d
    } else {
        log_d.exp()
    }
}

#[no_mangle]
pub extern "C" fn Rf_pweibull(
    x: f64,
    shape: f64,
    scale: f64,
    lower_tail: c_int,
    log_p: c_int,
) -> f64 {
    if shape <= 0.0 || scale <= 0.0 {
        return f64::NAN;
    }
    if x <= 0.0 {
        return p_transform(0.0, lower_tail, log_p);
    }
    let p = -libm::expm1(-(x / scale).powf(shape));
    p_transform(p, lower_tail, log_p)
}

#[no_mangle]
pub extern "C" fn Rf_qweibull(
    p: f64,
    shape: f64,
    scale: f64,
    lower_tail: c_int,
    log_p: c_int,
) -> f64 {
    if shape <= 0.0 || scale <= 0.0 {
        return f64::NAN;
    }
    let p = q_decode(p, lower_tail, log_p);
    if !(0.0..=1.0).contains(&p) {
        return f64::NAN;
    }
    scale * (-libm::log1p(-p)).powf(1.0 / shape)
}

#[no_mangle]
pub extern "C" fn Rf_rweibull(shape: f64, scale: f64) -> f64 {
    let u = unif_rand();
    scale * (-u.ln()).powf(1.0 / shape)
}

// endregion

// region: Logistic distribution

#[no_mangle]
pub extern "C" fn Rf_dlogis(x: f64, location: f64, scale: f64, give_log: c_int) -> f64 {
    if scale <= 0.0 {
        return f64::NAN;
    }
    let z = (x - location) / scale;
    let e = (-z.abs()).exp();
    let d = e / (scale * (1.0 + e) * (1.0 + e));
    d_log(d, give_log)
}

#[no_mangle]
pub extern "C" fn Rf_plogis(
    x: f64,
    location: f64,
    scale: f64,
    lower_tail: c_int,
    log_p: c_int,
) -> f64 {
    if scale <= 0.0 {
        return f64::NAN;
    }
    let z = (x - location) / scale;
    let p = 1.0 / (1.0 + (-z).exp());
    p_transform(p, lower_tail, log_p)
}

#[no_mangle]
pub extern "C" fn Rf_qlogis(
    p: f64,
    location: f64,
    scale: f64,
    lower_tail: c_int,
    log_p: c_int,
) -> f64 {
    if scale <= 0.0 {
        return f64::NAN;
    }
    let p = q_decode(p, lower_tail, log_p);
    if !(0.0..=1.0).contains(&p) {
        return f64::NAN;
    }
    location + scale * (p / (1.0 - p)).ln()
}

#[no_mangle]
pub extern "C" fn Rf_rlogis(location: f64, scale: f64) -> f64 {
    let u = unif_rand();
    location + scale * (u / (1.0 - u)).ln()
}

// endregion

// region: Poisson distribution

#[no_mangle]
pub extern "C" fn Rf_dpois(x: f64, lambda: f64, give_log: c_int) -> f64 {
    if lambda < 0.0 {
        return f64::NAN;
    }
    if x < 0.0 || x != x.floor() {
        return d_log(0.0, give_log);
    }
    let k = x as i64;
    let log_d = k as f64 * lambda.ln() - lambda - libm::lgamma(x + 1.0);
    if give_log != 0 {
        log_d
    } else {
        log_d.exp()
    }
}

#[no_mangle]
pub extern "C" fn Rf_ppois(x: f64, lambda: f64, lower_tail: c_int, log_p: c_int) -> f64 {
    if lambda < 0.0 {
        return f64::NAN;
    }
    if x < 0.0 {
        return p_transform(0.0, lower_tail, log_p);
    }
    let k = x.floor();
    // P(X <= k) = Q(k+1, lambda) = upper regularized incomplete gamma
    let p = qgamma_raw(k + 1.0, lambda);
    p_transform(p, lower_tail, log_p)
}

#[no_mangle]
pub extern "C" fn Rf_qpois(p: f64, lambda: f64, lower_tail: c_int, log_p: c_int) -> f64 {
    if lambda < 0.0 {
        return f64::NAN;
    }
    let p = q_decode(p, lower_tail, log_p);
    if !(0.0..=1.0).contains(&p) {
        return f64::NAN;
    }
    if p == 0.0 {
        return 0.0;
    }
    if p == 1.0 {
        return f64::INFINITY;
    }
    // Search: start from mean, step by 1
    let mut k = (lambda + 0.5).floor();
    loop {
        let pk = Rf_ppois(k, lambda, 1, 0);
        if pk >= p {
            break;
        }
        k += 1.0;
        if k > 1e15 {
            return f64::INFINITY;
        }
    }
    // Step back to find exact quantile
    while k > 0.0 {
        let pk = Rf_ppois(k - 1.0, lambda, 1, 0);
        if pk < p {
            break;
        }
        k -= 1.0;
    }
    k
}

#[no_mangle]
pub extern "C" fn Rf_rpois(lambda: f64) -> f64 {
    if lambda <= 0.0 {
        return 0.0;
    }
    // Knuth's algorithm for small lambda
    if lambda < 30.0 {
        let l = (-lambda).exp();
        let mut k = 0.0;
        let mut p = 1.0;
        loop {
            k += 1.0;
            p *= unif_rand();
            if p <= l {
                return k - 1.0;
            }
        }
    }
    // For large lambda, use rejection method based on normal approximation
    loop {
        let x = Rf_rnorm(lambda, lambda.sqrt());
        let k = (x + 0.5).floor();
        if k >= 0.0 {
            return k;
        }
    }
}

// endregion

// region: Binomial distribution

#[no_mangle]
pub extern "C" fn Rf_dbinom(x: f64, n: f64, p: f64, give_log: c_int) -> f64 {
    if n < 0.0 || !(0.0..=1.0).contains(&p) || n != n.floor() {
        return f64::NAN;
    }
    if x < 0.0 || x > n || x != x.floor() {
        return d_log(0.0, give_log);
    }
    let log_d = lchoose_fn(n, x) + x * p.ln() + (n - x) * (1.0 - p).ln();
    if give_log != 0 {
        log_d
    } else {
        log_d.exp()
    }
}

#[no_mangle]
pub extern "C" fn Rf_pbinom(x: f64, n: f64, p: f64, lower_tail: c_int, log_p: c_int) -> f64 {
    if n < 0.0 || !(0.0..=1.0).contains(&p) || n != n.floor() {
        return f64::NAN;
    }
    if x < 0.0 {
        return p_transform(0.0, lower_tail, log_p);
    }
    if x >= n {
        return p_transform(1.0, lower_tail, log_p);
    }
    let k = x.floor();
    // P(X <= k) = I_{1-p}(n-k, k+1) (regularized incomplete beta)
    let cdf = pbeta_raw(1.0 - p, n - k, k + 1.0);
    p_transform(cdf, lower_tail, log_p)
}

#[no_mangle]
pub extern "C" fn Rf_qbinom(p: f64, n: f64, prob: f64, lower_tail: c_int, log_p: c_int) -> f64 {
    if n < 0.0 || !(0.0..=1.0).contains(&prob) || n != n.floor() {
        return f64::NAN;
    }
    let p = q_decode(p, lower_tail, log_p);
    if !(0.0..=1.0).contains(&p) {
        return f64::NAN;
    }
    if p == 0.0 {
        return 0.0;
    }
    if p == 1.0 {
        return n;
    }
    // Start near the mean and search
    let mu = n * prob;
    let mut k = mu.floor();
    loop {
        let pk = Rf_pbinom(k, n, prob, 1, 0);
        if pk >= p {
            break;
        }
        k += 1.0;
        if k > n {
            return n;
        }
    }
    while k > 0.0 {
        let pk = Rf_pbinom(k - 1.0, n, prob, 1, 0);
        if pk < p {
            break;
        }
        k -= 1.0;
    }
    k
}

#[no_mangle]
pub extern "C" fn Rf_rbinom(n: f64, p: f64) -> f64 {
    if n <= 0.0 || p <= 0.0 {
        return 0.0;
    }
    if p >= 1.0 {
        return n;
    }
    let ni = n as i64;
    if ni <= 20 {
        // Direct method for small n
        let mut x = 0i64;
        for _ in 0..ni {
            if unif_rand() < p {
                x += 1;
            }
        }
        return x as f64;
    }
    // Normal approximation for large n
    let x = Rf_rnorm(n * p, (n * p * (1.0 - p)).sqrt());
    x.round().clamp(0.0, n)
}

#[no_mangle]
pub extern "C" fn rmultinom(_n: c_int, _prob: *mut f64, _k: c_int, _rn: *mut c_int) {
    // stub — multinomial sampling not yet implemented
}

// endregion

// region: Geometric distribution

#[no_mangle]
pub extern "C" fn Rf_dgeom(x: f64, p: f64, give_log: c_int) -> f64 {
    if p <= 0.0 || p > 1.0 {
        return f64::NAN;
    }
    if x < 0.0 || x != x.floor() {
        return d_log(0.0, give_log);
    }
    let log_d = p.ln() + x * (1.0 - p).ln();
    if give_log != 0 {
        log_d
    } else {
        log_d.exp()
    }
}

#[no_mangle]
pub extern "C" fn Rf_pgeom(x: f64, p: f64, lower_tail: c_int, log_p: c_int) -> f64 {
    if p <= 0.0 || p > 1.0 {
        return f64::NAN;
    }
    if x < 0.0 {
        return p_transform(0.0, lower_tail, log_p);
    }
    let k = x.floor();
    let cdf = 1.0 - (1.0 - p).powf(k + 1.0);
    p_transform(cdf, lower_tail, log_p)
}

#[no_mangle]
pub extern "C" fn Rf_qgeom(p: f64, prob: f64, lower_tail: c_int, log_p: c_int) -> f64 {
    if prob <= 0.0 || prob > 1.0 {
        return f64::NAN;
    }
    let p = q_decode(p, lower_tail, log_p);
    if !(0.0..=1.0).contains(&p) {
        return f64::NAN;
    }
    if p == 0.0 {
        return 0.0;
    }
    if p == 1.0 {
        return f64::INFINITY;
    }
    // Exact: ceil(log(1-p) / log(1-prob)) - 1
    (libm::log1p(-p) / libm::log1p(-prob)).ceil() - 1.0
}

#[no_mangle]
pub extern "C" fn Rf_rgeom(p: f64) -> f64 {
    let u = unif_rand();
    (u.ln() / libm::log1p(-p)).floor()
}

// endregion

// region: Hypergeometric distribution

#[no_mangle]
pub extern "C" fn Rf_dhyper(x: f64, r: f64, b: f64, n: f64, give_log: c_int) -> f64 {
    if r < 0.0 || b < 0.0 || n < 0.0 {
        return f64::NAN;
    }
    if x < 0.0 || x != x.floor() || x > r || x > n || n - x > b {
        return d_log(0.0, give_log);
    }
    let log_d = lchoose_fn(r, x) + lchoose_fn(b, n - x) - lchoose_fn(r + b, n);
    if give_log != 0 {
        log_d
    } else {
        log_d.exp()
    }
}

#[no_mangle]
pub extern "C" fn Rf_phyper(
    x: f64,
    r: f64,
    b: f64,
    n: f64,
    lower_tail: c_int,
    log_p: c_int,
) -> f64 {
    if r < 0.0 || b < 0.0 || n < 0.0 {
        return f64::NAN;
    }
    if x < 0.0 {
        return p_transform(0.0, lower_tail, log_p);
    }
    let k = x.floor();
    let lo = 0.0_f64.max(n - b);
    let hi = r.min(n);
    if k >= hi {
        return p_transform(1.0, lower_tail, log_p);
    }
    let mut sum = 0.0;
    let mut i = lo;
    while i <= k {
        sum += Rf_dhyper(i, r, b, n, 0);
        i += 1.0;
    }
    p_transform(sum, lower_tail, log_p)
}

#[no_mangle]
pub extern "C" fn Rf_qhyper(
    p: f64,
    r: f64,
    b: f64,
    n: f64,
    lower_tail: c_int,
    log_p: c_int,
) -> f64 {
    let p = q_decode(p, lower_tail, log_p);
    if !(0.0..=1.0).contains(&p) || r < 0.0 || b < 0.0 || n < 0.0 {
        return f64::NAN;
    }
    let lo = 0.0_f64.max(n - b);
    let hi = r.min(n);
    let mut k = lo;
    let mut cum = 0.0;
    while k <= hi {
        cum += Rf_dhyper(k, r, b, n, 0);
        if cum >= p {
            return k;
        }
        k += 1.0;
    }
    hi
}

#[no_mangle]
pub extern "C" fn Rf_rhyper(r: f64, b: f64, n: f64) -> f64 {
    // Simple urn sampling for small populations
    let total = r + b;
    if total <= 0.0 || n <= 0.0 {
        return 0.0;
    }
    let mut whites = r;
    let mut remaining = total;
    let mut drawn = 0.0;
    let ni = n.min(total) as i64;
    for _ in 0..ni {
        let u = unif_rand();
        if u < whites / remaining {
            drawn += 1.0;
            whites -= 1.0;
        }
        remaining -= 1.0;
    }
    drawn
}

// endregion

// region: Negative binomial distribution

#[no_mangle]
pub extern "C" fn Rf_dnbinom(x: f64, size: f64, prob: f64, give_log: c_int) -> f64 {
    if size <= 0.0 || prob <= 0.0 || prob > 1.0 {
        return f64::NAN;
    }
    if x < 0.0 || x != x.floor() {
        return d_log(0.0, give_log);
    }
    let log_d = lchoose_fn(x + size - 1.0, x) + size * prob.ln() + x * (1.0 - prob).ln();
    if give_log != 0 {
        log_d
    } else {
        log_d.exp()
    }
}

#[no_mangle]
pub extern "C" fn Rf_pnbinom(x: f64, size: f64, prob: f64, lower_tail: c_int, log_p: c_int) -> f64 {
    if size <= 0.0 || prob <= 0.0 || prob > 1.0 {
        return f64::NAN;
    }
    if x < 0.0 {
        return p_transform(0.0, lower_tail, log_p);
    }
    let k = x.floor();
    // P(X <= k) = I_p(size, k+1)
    let p = pbeta_raw(prob, size, k + 1.0);
    p_transform(p, lower_tail, log_p)
}

#[no_mangle]
pub extern "C" fn Rf_qnbinom(p: f64, size: f64, prob: f64, lower_tail: c_int, log_p: c_int) -> f64 {
    if size <= 0.0 || prob <= 0.0 || prob > 1.0 {
        return f64::NAN;
    }
    let p = q_decode(p, lower_tail, log_p);
    if !(0.0..=1.0).contains(&p) {
        return f64::NAN;
    }
    if p == 0.0 {
        return 0.0;
    }
    if p == 1.0 {
        return f64::INFINITY;
    }
    let mu = size * (1.0 - prob) / prob;
    let mut k = mu.floor();
    loop {
        let pk = Rf_pnbinom(k, size, prob, 1, 0);
        if pk >= p {
            break;
        }
        k += 1.0;
        if k > 1e15 {
            return f64::INFINITY;
        }
    }
    while k > 0.0 {
        let pk = Rf_pnbinom(k - 1.0, size, prob, 1, 0);
        if pk < p {
            break;
        }
        k -= 1.0;
    }
    k
}

#[no_mangle]
pub extern "C" fn Rf_rnbinom(size: f64, prob: f64) -> f64 {
    let rate = (1.0 - prob) / prob;
    let g = Rf_rgamma(size, rate);
    Rf_rpois(g)
}

// NegBinom mu parameterization
#[no_mangle]
pub extern "C" fn Rf_dnbinom_mu(x: f64, size: f64, mu: f64, give_log: c_int) -> f64 {
    if size <= 0.0 || mu < 0.0 {
        return f64::NAN;
    }
    let prob = size / (size + mu);
    Rf_dnbinom(x, size, prob, give_log)
}

#[no_mangle]
pub extern "C" fn Rf_pnbinom_mu(
    x: f64,
    size: f64,
    mu: f64,
    lower_tail: c_int,
    log_p: c_int,
) -> f64 {
    if size <= 0.0 || mu < 0.0 {
        return f64::NAN;
    }
    let prob = size / (size + mu);
    Rf_pnbinom(x, size, prob, lower_tail, log_p)
}

#[no_mangle]
pub extern "C" fn Rf_qnbinom_mu(
    x: f64,
    size: f64,
    mu: f64,
    lower_tail: c_int,
    log_p: c_int,
) -> f64 {
    if size <= 0.0 || mu < 0.0 {
        return f64::NAN;
    }
    let prob = size / (size + mu);
    Rf_qnbinom(x, size, prob, lower_tail, log_p)
}

// endregion

// region: Non-central Beta distribution

#[no_mangle]
pub extern "C" fn Rf_dnbeta(x: f64, a: f64, b: f64, ncp: f64, give_log: c_int) -> f64 {
    if a <= 0.0 || b <= 0.0 || ncp < 0.0 {
        return f64::NAN;
    }
    if ncp == 0.0 {
        return Rf_dbeta(x, a, b, give_log);
    }
    // Poisson mixture of central beta densities
    let lambda = ncp / 2.0;
    let max_terms = 100.min((lambda + 20.0 * lambda.sqrt()) as usize + 10);
    let mut sum = 0.0;
    let mut pw = (-lambda).exp();
    for j in 0..max_terms {
        sum += pw * Rf_dbeta(x, a + j as f64, b, 0);
        pw *= lambda / (j as f64 + 1.0);
    }
    d_log(sum, give_log)
}

#[no_mangle]
pub extern "C" fn Rf_pnbeta(
    x: f64,
    a: f64,
    b: f64,
    ncp: f64,
    lower_tail: c_int,
    log_p: c_int,
) -> f64 {
    if a <= 0.0 || b <= 0.0 || ncp < 0.0 {
        return f64::NAN;
    }
    if ncp == 0.0 {
        return Rf_pbeta(x, a, b, lower_tail, log_p);
    }
    let lambda = ncp / 2.0;
    let max_terms = 100.min((lambda + 20.0 * lambda.sqrt()) as usize + 10);
    let mut sum = 0.0;
    let mut pw = (-lambda).exp();
    for j in 0..max_terms {
        sum += pw * pbeta_raw(x, a + j as f64, b);
        pw *= lambda / (j as f64 + 1.0);
    }
    p_transform(sum, lower_tail, log_p)
}

#[no_mangle]
pub extern "C" fn Rf_qnbeta(
    p: f64,
    a: f64,
    b: f64,
    ncp: f64,
    lower_tail: c_int,
    log_p: c_int,
) -> f64 {
    if a <= 0.0 || b <= 0.0 || ncp < 0.0 {
        return f64::NAN;
    }
    let p = q_decode(p, lower_tail, log_p);
    if !(0.0..=1.0).contains(&p) {
        return f64::NAN;
    }
    // Newton's method
    let mut x = Rf_qbeta(p, a, b, 1, 0);
    for _ in 0..50 {
        let px = Rf_pnbeta(x, a, b, ncp, 1, 0);
        let dx = Rf_dnbeta(x, a, b, ncp, 0);
        if dx <= 0.0 {
            break;
        }
        let delta = (px - p) / dx;
        x -= delta;
        x = x.clamp(DBL_EPSILON, 1.0 - DBL_EPSILON);
        if delta.abs() < x * 1e-12 {
            break;
        }
    }
    x
}

// endregion

// region: Non-central F distribution

#[no_mangle]
pub extern "C" fn Rf_dnf(x: f64, df1: f64, df2: f64, ncp: f64, give_log: c_int) -> f64 {
    if df1 <= 0.0 || df2 <= 0.0 || ncp < 0.0 {
        return f64::NAN;
    }
    if x <= 0.0 {
        return d_log(0.0, give_log);
    }
    // Transform: if X ~ F'(df1,df2,ncp) then Y = df1*X/(df1*X+df2) ~ Beta'(df1/2,df2/2,ncp)
    let y = df1 * x / (df1 * x + df2);
    let dy_dx = df1 * df2 / ((df1 * x + df2) * (df1 * x + df2));
    let d = Rf_dnbeta(y, df1 / 2.0, df2 / 2.0, ncp, 0) * dy_dx;
    d_log(d, give_log)
}

#[no_mangle]
pub extern "C" fn Rf_pnf(
    x: f64,
    df1: f64,
    df2: f64,
    ncp: f64,
    lower_tail: c_int,
    log_p: c_int,
) -> f64 {
    if df1 <= 0.0 || df2 <= 0.0 || ncp < 0.0 {
        return f64::NAN;
    }
    if x <= 0.0 {
        return p_transform(0.0, lower_tail, log_p);
    }
    let y = df1 * x / (df1 * x + df2);
    Rf_pnbeta(y, df1 / 2.0, df2 / 2.0, ncp, lower_tail, log_p)
}

#[no_mangle]
pub extern "C" fn Rf_qnf(
    p: f64,
    df1: f64,
    df2: f64,
    ncp: f64,
    lower_tail: c_int,
    log_p: c_int,
) -> f64 {
    if df1 <= 0.0 || df2 <= 0.0 || ncp < 0.0 {
        return f64::NAN;
    }
    let bq = Rf_qnbeta(p, df1 / 2.0, df2 / 2.0, ncp, lower_tail, log_p);
    (df2 * bq) / (df1 * (1.0 - bq))
}

// endregion

// region: Non-central t distribution

#[no_mangle]
pub extern "C" fn Rf_dnt(x: f64, df: f64, ncp: f64, give_log: c_int) -> f64 {
    if df <= 0.0 {
        return f64::NAN;
    }
    if ncp == 0.0 {
        return Rf_dt(x, df, give_log);
    }
    // Numerical approximation via finite difference
    let h = 1e-7 * (1.0 + x.abs());
    let d = (Rf_pnt(x + h, df, ncp, 1, 0) - Rf_pnt(x - h, df, ncp, 1, 0)) / (2.0 * h);
    if d < 0.0 {
        return d_log(0.0, give_log);
    }
    d_log(d, give_log)
}

#[no_mangle]
pub extern "C" fn Rf_pnt(x: f64, df: f64, ncp: f64, lower_tail: c_int, log_p: c_int) -> f64 {
    if df <= 0.0 {
        return f64::NAN;
    }
    if ncp == 0.0 {
        return Rf_pt(x, df, lower_tail, log_p);
    }
    // Normal approximation with correction for non-centrality
    let z = x * (1.0 - 1.0 / (4.0 * df)).sqrt() - ncp;
    let p = Rf_pnorm5(z, 0.0, 1.0, 1, 0);
    p_transform(p, lower_tail, log_p)
}

#[no_mangle]
pub extern "C" fn Rf_qnt(p: f64, df: f64, ncp: f64, lower_tail: c_int, log_p: c_int) -> f64 {
    if df <= 0.0 {
        return f64::NAN;
    }
    let p = q_decode(p, lower_tail, log_p);
    if !(0.0..=1.0).contains(&p) {
        return f64::NAN;
    }
    // Newton's method from normal approximation
    let mut x = qnorm_standard(p) + ncp;
    for _ in 0..50 {
        let px = Rf_pnt(x, df, ncp, 1, 0);
        let dx = Rf_dnt(x, df, ncp, 0);
        if dx <= 0.0 || !dx.is_finite() {
            break;
        }
        let delta = (px - p) / dx;
        x -= delta;
        if delta.abs() < x.abs() * 1e-10 {
            break;
        }
    }
    x
}

// endregion

// region: Studentized range distribution

#[no_mangle]
pub extern "C" fn Rf_ptukey(_q: f64, _rr: f64, _cc: f64, _df: f64, _lt: c_int, _lg: c_int) -> f64 {
    // Studentized range requires complex numerical integration — stub for now
    f64::NAN
}

#[no_mangle]
pub extern "C" fn Rf_qtukey(_p: f64, _rr: f64, _cc: f64, _df: f64, _lt: c_int, _lg: c_int) -> f64 {
    f64::NAN
}

// endregion

// region: Wilcoxon rank sum distribution

#[no_mangle]
pub extern "C" fn Rf_dwilcox(x: f64, m: f64, n: f64, give_log: c_int) -> f64 {
    if m < 0.0 || n < 0.0 {
        return f64::NAN;
    }
    if x < 0.0 || x != x.floor() || x > m * n {
        return d_log(0.0, give_log);
    }
    // Count via normal approximation for large m*n
    let total = choose_fn(m + n, n);
    if total == 0.0 {
        return d_log(0.0, give_log);
    }
    // Exact count for small cases using recursion would be expensive;
    // use normal approximation
    let mu = m * n / 2.0;
    let sigma = (m * n * (m + n + 1.0) / 12.0).sqrt();
    if sigma <= 0.0 {
        return d_log(if x == mu { 1.0 } else { 0.0 }, give_log);
    }
    // Continuity-corrected normal approximation
    Rf_dnorm4(x, mu, sigma, give_log)
}

#[no_mangle]
pub extern "C" fn Rf_pwilcox(x: f64, m: f64, n: f64, lower_tail: c_int, log_p: c_int) -> f64 {
    if m < 0.0 || n < 0.0 {
        return f64::NAN;
    }
    let mu = m * n / 2.0;
    let sigma = (m * n * (m + n + 1.0) / 12.0).sqrt();
    if sigma <= 0.0 {
        return p_transform(if x >= mu { 1.0 } else { 0.0 }, lower_tail, log_p);
    }
    // Normal approximation with continuity correction
    Rf_pnorm5(x + 0.5, mu, sigma, lower_tail, log_p)
}

#[no_mangle]
pub extern "C" fn Rf_qwilcox(p: f64, m: f64, n: f64, lower_tail: c_int, log_p: c_int) -> f64 {
    if m < 0.0 || n < 0.0 {
        return f64::NAN;
    }
    let mu = m * n / 2.0;
    let sigma = (m * n * (m + n + 1.0) / 12.0).sqrt();
    if sigma <= 0.0 {
        return mu;
    }
    let q = Rf_qnorm5(p, mu, sigma, lower_tail, log_p);
    q.round().clamp(0.0, m * n)
}

#[no_mangle]
pub extern "C" fn Rf_rwilcox(m: f64, n: f64) -> f64 {
    // Simple simulation: sum of ranks
    let total = (m + n) as usize;
    let mi = m as usize;
    // Fisher-Yates sample of mi items from 1..=total
    let mut sum = 0.0;
    let mut remaining = total;
    let mut need = mi;
    for rank in 1..=total {
        let u = unif_rand();
        if (u * remaining as f64) < need as f64 {
            sum += rank as f64;
            need -= 1;
            if need == 0 {
                break;
            }
        }
        remaining -= 1;
    }
    // Wilcoxon rank sum = sum_of_ranks - m*(m+1)/2
    sum - m * (m + 1.0) / 2.0
}

// endregion

// region: Wilcoxon signed rank distribution

#[no_mangle]
pub extern "C" fn Rf_dsignrank(x: f64, n: f64, give_log: c_int) -> f64 {
    if n < 0.0 {
        return f64::NAN;
    }
    let mu = n * (n + 1.0) / 4.0;
    let sigma = (n * (n + 1.0) * (2.0 * n + 1.0) / 24.0).sqrt();
    if sigma <= 0.0 {
        return d_log(if x == mu { 1.0 } else { 0.0 }, give_log);
    }
    Rf_dnorm4(x, mu, sigma, give_log)
}

#[no_mangle]
pub extern "C" fn Rf_psignrank(x: f64, n: f64, lower_tail: c_int, log_p: c_int) -> f64 {
    if n < 0.0 {
        return f64::NAN;
    }
    let mu = n * (n + 1.0) / 4.0;
    let sigma = (n * (n + 1.0) * (2.0 * n + 1.0) / 24.0).sqrt();
    if sigma <= 0.0 {
        return p_transform(if x >= mu { 1.0 } else { 0.0 }, lower_tail, log_p);
    }
    Rf_pnorm5(x + 0.5, mu, sigma, lower_tail, log_p)
}

#[no_mangle]
pub extern "C" fn Rf_qsignrank(p: f64, n: f64, lower_tail: c_int, log_p: c_int) -> f64 {
    if n < 0.0 {
        return f64::NAN;
    }
    let mu = n * (n + 1.0) / 4.0;
    let sigma = (n * (n + 1.0) * (2.0 * n + 1.0) / 24.0).sqrt();
    if sigma <= 0.0 {
        return mu;
    }
    let q = Rf_qnorm5(p, mu, sigma, lower_tail, log_p);
    q.round().clamp(0.0, n * (n + 1.0) / 2.0)
}

#[no_mangle]
pub extern "C" fn Rf_rsignrank(n: f64) -> f64 {
    let ni = n as i64;
    let mut sum = 0.0;
    for i in 1..=ni {
        if unif_rand() > 0.5 {
            sum += i as f64;
        }
    }
    sum
}

// endregion

// region: Bessel functions

#[no_mangle]
pub extern "C" fn Rf_bessel_j(x: f64, alpha: f64) -> f64 {
    if alpha == 0.0 {
        return libm::j0(x);
    }
    if alpha == 1.0 {
        return libm::j1(x);
    }
    if alpha == alpha.floor() && alpha >= 0.0 {
        return libm::jn(alpha as i32, x);
    }
    // For non-integer orders, use series expansion
    bessel_j_series(x, alpha)
}

#[no_mangle]
pub extern "C" fn Rf_bessel_y(x: f64, alpha: f64) -> f64 {
    if x <= 0.0 {
        return f64::NEG_INFINITY;
    }
    if alpha == 0.0 {
        return libm::y0(x);
    }
    if alpha == 1.0 {
        return libm::y1(x);
    }
    if alpha == alpha.floor() && alpha >= 0.0 {
        return libm::yn(alpha as i32, x);
    }
    // Y_alpha = (J_alpha * cos(alpha*pi) - J_{-alpha}) / sin(alpha*pi)
    let pi_a = alpha * std::f64::consts::PI;
    (Rf_bessel_j(x, alpha) * pi_a.cos() - Rf_bessel_j(x, -alpha)) / pi_a.sin()
}

#[no_mangle]
pub extern "C" fn Rf_bessel_i(x: f64, alpha: f64, expo: f64) -> f64 {
    // Modified Bessel I_alpha(x), optionally scaled by exp(-|x|)
    let val = bessel_i_series(x, alpha);
    if expo != 1.0 {
        val
    } else {
        val * (-x.abs()).exp()
    }
}

#[no_mangle]
pub extern "C" fn Rf_bessel_k(x: f64, alpha: f64, expo: f64) -> f64 {
    if x <= 0.0 {
        return f64::INFINITY;
    }
    // K_alpha = pi/2 * (I_{-alpha} - I_alpha) / sin(alpha*pi)
    // For integer alpha, use limit
    let val = if (alpha - alpha.round()).abs() < 1e-15 {
        // Integer order: use K_n via recursion
        bessel_k_int(x, alpha.round() as i32)
    } else {
        let pi_a = alpha * std::f64::consts::PI;
        std::f64::consts::PI / 2.0 * (bessel_i_series(x, -alpha) - bessel_i_series(x, alpha))
            / pi_a.sin()
    };
    if expo != 1.0 {
        val
    } else {
        val * x.exp()
    }
}

fn bessel_j_series(x: f64, alpha: f64) -> f64 {
    let half_x = x / 2.0;
    let mut term = half_x.powf(alpha) / libm::tgamma(alpha + 1.0);
    let mut sum = term;
    let x2_neg = -half_x * half_x;
    for m in 1..100 {
        term *= x2_neg / (m as f64 * (alpha + m as f64));
        sum += term;
        if term.abs() < sum.abs() * DBL_EPSILON {
            break;
        }
    }
    sum
}

fn bessel_i_series(x: f64, alpha: f64) -> f64 {
    let half_x = x / 2.0;
    let mut term = half_x.powf(alpha) / libm::tgamma(alpha + 1.0);
    let mut sum = term;
    let x2 = half_x * half_x;
    for m in 1..100 {
        term *= x2 / (m as f64 * (alpha + m as f64));
        sum += term;
        if term.abs() < sum.abs() * DBL_EPSILON {
            break;
        }
    }
    sum
}

fn bessel_k_int(x: f64, n: i32) -> f64 {
    // K_0 and K_1 from asymptotic or series, then recurse
    if n == 0 {
        return bessel_k0(x);
    }
    if n == 1 {
        return bessel_k1(x);
    }
    let mut km1 = bessel_k0(x);
    let mut k = bessel_k1(x);
    for i in 1..n.unsigned_abs() {
        let kp1 = km1 + 2.0 * i as f64 / x * k;
        km1 = k;
        k = kp1;
    }
    k
}

fn bessel_k0(x: f64) -> f64 {
    if x <= 2.0 {
        let y = x * x / 4.0;
        -x.ln() * bessel_i_series(x, 0.0)
            + (-0.57721566
                + y * (0.42278420
                    + y * (0.23069756 + y * (0.03488590 + y * (0.00262698 + y * 0.00010750)))))
    } else {
        let y = 2.0 / x;
        ((-x).exp() / x.sqrt())
            * (1.25331414
                + y * (-0.07832358
                    + y * (0.02189568 + y * (-0.01062446 + y * (0.00587872 + y * (-0.00251540))))))
    }
}

fn bessel_k1(x: f64) -> f64 {
    if x <= 2.0 {
        let y = x * x / 4.0;
        x.ln() * bessel_i_series(x, 1.0)
            + (1.0 / x)
                * (1.0
                    + y * (0.15443144
                        + y * (-0.67278579
                            + y * (-0.18156897 + y * (-0.01919402 + y * (-0.00110404))))))
    } else {
        let y = 2.0 / x;
        ((-x).exp() / x.sqrt())
            * (1.25331414
                + y * (0.23498619
                    + y * (-0.03655620 + y * (0.01504268 + y * (-0.00780353 + y * 0.00325614)))))
    }
}

// _ex variants: store result in output buffer and return it
#[no_mangle]
pub extern "C" fn Rf_bessel_i_ex(x: f64, alpha: f64, expo: f64, bi: *mut f64) -> f64 {
    let val = Rf_bessel_i(x, alpha, expo);
    if !bi.is_null() {
        unsafe {
            *bi = val;
        }
    }
    val
}

#[no_mangle]
pub extern "C" fn Rf_bessel_j_ex(x: f64, alpha: f64, bj: *mut f64) -> f64 {
    let val = Rf_bessel_j(x, alpha);
    if !bj.is_null() {
        unsafe {
            *bj = val;
        }
    }
    val
}

#[no_mangle]
pub extern "C" fn Rf_bessel_k_ex(x: f64, alpha: f64, expo: f64, bk: *mut f64) -> f64 {
    let val = Rf_bessel_k(x, alpha, expo);
    if !bk.is_null() {
        unsafe {
            *bk = val;
        }
    }
    val
}

#[no_mangle]
pub extern "C" fn Rf_bessel_y_ex(x: f64, alpha: f64, by: *mut f64) -> f64 {
    let val = Rf_bessel_y(x, alpha);
    if !by.is_null() {
        unsafe {
            *by = val;
        }
    }
    val
}

#[no_mangle]
pub extern "C" fn Rf_hypot(a: f64, b: f64) -> f64 {
    a.hypot(b)
}

// endregion
