//! Random number generation builtins: set.seed, runif, rnorm, rbinom, sample, etc.
//! Uses the per-interpreter RNG state via `BuiltinContext`.

use derive_more::{Display, Error};
use rand::RngExt;
use rand::SeedableRng;
use rand_distr::Distribution;

use crate::interpreter::coerce::{f64_to_i64, f64_to_u64, i64_to_f64};
use crate::interpreter::value::*;
use crate::interpreter::BuiltinContext;
use minir_macros::interpreter_builtin;

// region: RandomError

#[derive(Debug, Display, Error)]
pub enum RandomError {
    #[display("invalid '{}' argument", param)]
    InvalidParam { param: &'static str },

    #[display("invalid argument: '{}' must be non-negative", param)]
    NonNegative { param: &'static str },

    #[display("invalid arguments: 'min' must not be greater than 'max'")]
    MinGreaterThanMax,

    #[display("invalid distribution parameters: {}", reason)]
    InvalidDistribution { reason: String },

    #[display("invalid first argument: must be a positive integer or a vector")]
    InvalidSampleInput,

    #[display(
        "cannot take a sample larger than the population ({} > {}) when 'replace = FALSE'",
        size,
        pop_len
    )]
    SampleTooLarge { size: usize, pop_len: usize },

    #[display("argument '{}' is missing, with no default", param)]
    MissingParam { param: &'static str },

    #[display("NA in probability vector")]
    NaInProb,

    #[display("negative probability")]
    NegativeProb,

    #[display(
        "'prob' must have the same length as the population ({} != {})",
        prob_len,
        pop_len
    )]
    ProbLengthMismatch { prob_len: usize, pop_len: usize },

    #[display("too few positive probabilities")]
    TooFewPositiveProbs,
}

impl RandomError {
    fn invalid_dist(e: impl std::fmt::Display) -> Self {
        RandomError::InvalidDistribution {
            reason: e.to_string(),
        }
    }
}

impl From<RandomError> for RError {
    fn from(e: RandomError) -> Self {
        RError::from_source(RErrorKind::Argument, e)
    }
}

// endregion

/// Extract a positive integer `n` from `args[0]`.
///
/// Dispatch-level `reorder_builtin_args` ensures `args[0]` is always the first
/// formal parameter regardless of how the user passed it (named or positional).
fn extract_n(args: &[RValue]) -> Result<usize, RandomError> {
    let n = args
        .first()
        .and_then(|v| v.as_vector())
        .and_then(|v| v.as_integer_scalar())
        .ok_or(RandomError::InvalidParam { param: "n" })?;
    if n < 0 {
        return Err(RandomError::NonNegative { param: "n" });
    }
    usize::try_from(n).map_err(|_| RandomError::InvalidParam { param: "n" })
}

/// Extract an optional f64 parameter from named args or positional index.
///
/// Checks named args first (handles gap cases where the arg was named but
/// earlier formals were omitted), then falls back to positional index.
fn extract_param(
    args: &[RValue],
    named: &[(String, RValue)],
    name: &str,
    positional_index: usize,
    default: f64,
) -> f64 {
    for (k, v) in named {
        if k == name {
            if let Some(rv) = v.as_vector() {
                if let Some(d) = rv.as_double_scalar() {
                    return d;
                }
            }
        }
    }
    args.get(positional_index)
        .and_then(|v| v.as_vector())
        .and_then(|v| v.as_double_scalar())
        .unwrap_or(default)
}

/// Extract a required f64 parameter from named args or positional index.
fn require_param(
    args: &[RValue],
    named: &[(String, RValue)],
    name: &'static str,
    positional_index: usize,
) -> Result<f64, RandomError> {
    for (k, v) in named {
        if k == name {
            if let Some(rv) = v.as_vector() {
                if let Some(d) = rv.as_double_scalar() {
                    return Ok(d);
                }
            }
        }
    }
    args.get(positional_index)
        .and_then(|v| v.as_vector())
        .and_then(|v| v.as_double_scalar())
        .ok_or(RandomError::MissingParam { param: name })
}

// region: set.seed

/// Set the random number generator seed for reproducibility.
///
/// Seeds the per-interpreter RNG deterministically so that subsequent random
/// draws produce the same sequence. The RNG algorithm seeded depends on the
/// current `RNGkind()` setting — either Xoshiro (default) or ChaCha20.
///
/// Also stores the seed value in `.Random.seed` in the global environment
/// (as an integer vector whose first element is the seed), matching R's
/// convention of exposing RNG state there.
///
/// @param seed integer seed value (or NULL to re-seed from system entropy)
/// @return NULL, invisibly
#[interpreter_builtin(name = "set.seed", min_args = 1)]
fn interp_set_seed(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    use crate::interpreter::{InterpreterRng, RngKind};

    // set.seed(NULL) re-seeds from system entropy (like a fresh interpreter)
    if matches!(args[0], RValue::Null) {
        context.with_interpreter(|interp| {
            let mut thread_rng = rand::rng();
            let new_rng = match interp.rng_kind.get() {
                RngKind::Xoshiro => {
                    InterpreterRng::Fast(rand::rngs::SmallRng::from_rng(&mut thread_rng))
                }
                RngKind::ChaCha20 => InterpreterRng::Deterministic(Box::new(
                    rand_chacha::ChaCha20Rng::from_rng(&mut thread_rng),
                )),
            };
            *interp.rng().borrow_mut() = new_rng;
            // Remove .Random.seed when re-seeding from entropy
            interp.global_env.remove(".Random.seed");
        });
        return Ok(RValue::Null);
    }

    let seed_f64 = args[0]
        .as_vector()
        .and_then(|v| v.as_double_scalar())
        .ok_or(RandomError::InvalidParam { param: "seed" })?;
    let seed = f64_to_u64(seed_f64)?;
    context.with_interpreter(|interp| {
        let kind = interp.rng_kind.get();
        let new_rng = match kind {
            RngKind::Xoshiro => InterpreterRng::Fast(rand::rngs::SmallRng::seed_from_u64(seed)),
            RngKind::ChaCha20 => InterpreterRng::Deterministic(Box::new(
                rand_chacha::ChaCha20Rng::seed_from_u64(seed),
            )),
        };
        *interp.rng().borrow_mut() = new_rng;
        // Store the seed in .Random.seed in the global env.
        // R's .Random.seed is an integer vector; we store the u64 seed as two
        // i64 values: a "kind" marker (0 = Xoshiro, 1 = ChaCha20) and the seed.
        // This is a simplified version of R's full .Random.seed protocol.
        let kind_code = match kind {
            RngKind::Xoshiro => 0i64,
            RngKind::ChaCha20 => 1i64,
        };
        let seed_i64 = i64::try_from(seed).unwrap_or(i64::MAX);
        interp.global_env.set(
            ".Random.seed".to_string(),
            RValue::vec(Vector::Integer(
                vec![Some(kind_code), Some(seed_i64)].into(),
            )),
        );
    });
    Ok(RValue::Null)
}

// endregion

// region: RNGkind

/// Query or set the RNG algorithm.
///
/// With no arguments, returns the name of the current RNG kind as a character
/// vector. With a `kind` argument, switches to the specified algorithm.
///
/// Supported kinds:
/// - `"Xoshiro"` (default) — fast, non-cryptographic (`SmallRng` / Xoshiro256++)
/// - `"ChaCha20"` — deterministic across platforms and Rust versions
///
/// After switching the RNG kind, call `set.seed()` to seed the new algorithm.
/// The switch itself does NOT re-seed — the new RNG starts from system entropy.
///
/// @param kind character string naming the RNG algorithm (optional)
/// @return character vector with the previous RNG kind (invisibly when setting)
#[interpreter_builtin(name = "RNGkind")]
fn interp_rng_kind(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    use crate::interpreter::{InterpreterRng, RngKind};

    let old_kind = context.with_interpreter(|interp| interp.rng_kind.get());
    let old_kind_str = old_kind.to_string();

    // Extract kind argument (positional or named)
    let kind_arg = named
        .iter()
        .find(|(k, _)| k == "kind")
        .map(|(_, v)| v)
        .or(args.first());

    if let Some(kind_val) = kind_arg {
        // NULL means query-only (same as no argument)
        if matches!(kind_val, RValue::Null) {
            return Ok(RValue::vec(Vector::Character(
                vec![Some(old_kind_str)].into(),
            )));
        }

        let kind_str = kind_val
            .as_vector()
            .and_then(|v| v.as_character_scalar())
            .ok_or_else(|| {
                RError::new(
                    RErrorKind::Argument,
                    "RNGkind() requires a character string argument".to_string(),
                )
            })?;

        let new_kind = match kind_str.as_str() {
            "Xoshiro" | "xoshiro" => RngKind::Xoshiro,
            "ChaCha20" | "chacha20" | "ChaCha" | "chacha" => RngKind::ChaCha20,
            other => {
                return Err(RError::new(
                    RErrorKind::Argument,
                    format!(
                        "RNGkind(\"{other}\") is not a recognized RNG kind.\n  \
                         Valid choices: \"Xoshiro\" (default, fast) or \"ChaCha20\" (deterministic, cross-platform)."
                    ),
                ));
            }
        };

        context.with_interpreter(|interp| {
            interp.rng_kind.set(new_kind);
            // Replace the RNG with a fresh instance of the new kind, seeded from entropy.
            let mut thread_rng = rand::rng();
            let new_rng = match new_kind {
                RngKind::Xoshiro => {
                    InterpreterRng::Fast(rand::rngs::SmallRng::from_rng(&mut thread_rng))
                }
                RngKind::ChaCha20 => InterpreterRng::Deterministic(Box::new(
                    rand_chacha::ChaCha20Rng::from_rng(&mut thread_rng),
                )),
            };
            *interp.rng().borrow_mut() = new_rng;
        });
    }

    Ok(RValue::vec(Vector::Character(
        vec![Some(old_kind_str)].into(),
    )))
}

// endregion

// region: Continuous distributions

/// Random uniform deviates.
///
/// Generates n random values from a uniform distribution.
///
/// @param n number of observations
/// @param min lower limit of the distribution (default 0)
/// @param max upper limit of the distribution (default 1)
/// @return numeric vector of length n
#[interpreter_builtin(min_args = 1)]
fn interp_runif(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let n = extract_n(args)?;
    let min = extract_param(args, named, "min", 1, 0.0);
    let max = extract_param(args, named, "max", 2, 1.0);
    if min > max {
        return Err(RandomError::MinGreaterThanMax.into());
    }
    let dist = rand_distr::Uniform::new(min, max).map_err(RandomError::invalid_dist)?;
    let values: Vec<Option<f64>> = context.with_interpreter(|interp| {
        let mut rng = interp.rng().borrow_mut();
        (0..n).map(|_| Some(dist.sample(&mut *rng))).collect()
    });
    Ok(RValue::vec(Vector::Double(values.into())))
}

/// Random normal deviates.
///
/// Generates n random values from a normal distribution.
#[derive(minir_macros::FromArgs)]
#[builtin(name = "rnorm")]
struct RnormArgs {
    /// number of observations
    n: i64,
    /// mean of the distribution
    #[default(0.0)]
    mean: f64,
    /// standard deviation
    #[default(1.0)]
    sd: f64,
}

impl crate::interpreter::value::Builtin for RnormArgs {
    fn call(self, ctx: &BuiltinContext) -> Result<RValue, RError> {
        let n = usize::try_from(self.n).map_err(|_| RandomError::NonNegative { param: "n" })?;
        let dist =
            rand_distr::Normal::new(self.mean, self.sd).map_err(RandomError::invalid_dist)?;
        let values: Vec<Option<f64>> = ctx.with_interpreter(|interp| {
            let mut rng = interp.rng().borrow_mut();
            (0..n).map(|_| Some(dist.sample(&mut *rng))).collect()
        });
        Ok(RValue::vec(Vector::Double(values.into())))
    }
}

/// Random exponential deviates.
///
/// Generates n random values from an exponential distribution.
///
/// @param n number of observations
/// @param rate rate parameter (default 1)
/// @return numeric vector of length n
#[interpreter_builtin(min_args = 1)]
fn interp_rexp(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let n = extract_n(args)?;
    let rate = extract_param(args, named, "rate", 1, 1.0);
    let dist = rand_distr::Exp::new(rate).map_err(RandomError::invalid_dist)?;
    let values: Vec<Option<f64>> = context.with_interpreter(|interp| {
        let mut rng = interp.rng().borrow_mut();
        (0..n).map(|_| Some(dist.sample(&mut *rng))).collect()
    });
    Ok(RValue::vec(Vector::Double(values.into())))
}

/// Random gamma deviates.
///
/// Generates n random values from a gamma distribution.
///
/// @param n number of observations
/// @param shape shape parameter (default 1)
/// @param rate rate parameter (default 1); scale = 1/rate
/// @return numeric vector of length n
#[interpreter_builtin(min_args = 1)]
fn interp_rgamma(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let n = extract_n(args)?;
    let shape = extract_param(args, named, "shape", 1, 1.0);
    let rate = extract_param(args, named, "rate", 2, 1.0);
    // R uses rate, rand_distr::Gamma uses scale = 1/rate
    let dist = rand_distr::Gamma::new(shape, 1.0 / rate).map_err(RandomError::invalid_dist)?;
    let values: Vec<Option<f64>> = context.with_interpreter(|interp| {
        let mut rng = interp.rng().borrow_mut();
        (0..n).map(|_| Some(dist.sample(&mut *rng))).collect()
    });
    Ok(RValue::vec(Vector::Double(values.into())))
}

/// Random beta deviates.
///
/// Generates n random values from a beta distribution.
///
/// @param n number of observations
/// @param shape1 first shape parameter (default 1)
/// @param shape2 second shape parameter (default 1)
/// @return numeric vector of length n
#[interpreter_builtin(min_args = 1)]
fn interp_rbeta(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let n = extract_n(args)?;
    let shape1 = extract_param(args, named, "shape1", 1, 1.0);
    let shape2 = extract_param(args, named, "shape2", 2, 1.0);
    let dist = rand_distr::Beta::new(shape1, shape2).map_err(RandomError::invalid_dist)?;
    let values: Vec<Option<f64>> = context.with_interpreter(|interp| {
        let mut rng = interp.rng().borrow_mut();
        (0..n).map(|_| Some(dist.sample(&mut *rng))).collect()
    });
    Ok(RValue::vec(Vector::Double(values.into())))
}

/// Random Cauchy deviates.
///
/// Generates n random values from a Cauchy distribution.
///
/// @param n number of observations
/// @param location location parameter (default 0)
/// @param scale scale parameter (default 1)
/// @return numeric vector of length n
#[interpreter_builtin(min_args = 1)]
fn interp_rcauchy(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let n = extract_n(args)?;
    let location = extract_param(args, named, "location", 1, 0.0);
    let scale = extract_param(args, named, "scale", 2, 1.0);
    let dist = rand_distr::Cauchy::new(location, scale).map_err(RandomError::invalid_dist)?;
    let values: Vec<Option<f64>> = context.with_interpreter(|interp| {
        let mut rng = interp.rng().borrow_mut();
        (0..n).map(|_| Some(dist.sample(&mut *rng))).collect()
    });
    Ok(RValue::vec(Vector::Double(values.into())))
}

/// Random Weibull deviates.
///
/// Generates n random values from a Weibull distribution.
///
/// @param n number of observations
/// @param shape shape parameter (default 1)
/// @param scale scale parameter (default 1)
/// @return numeric vector of length n
#[interpreter_builtin(min_args = 1)]
fn interp_rweibull(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let n = extract_n(args)?;
    let shape = extract_param(args, named, "shape", 1, 1.0);
    let scale = extract_param(args, named, "scale", 2, 1.0);
    let dist = rand_distr::Weibull::new(scale, shape).map_err(RandomError::invalid_dist)?;
    let values: Vec<Option<f64>> = context.with_interpreter(|interp| {
        let mut rng = interp.rng().borrow_mut();
        (0..n).map(|_| Some(dist.sample(&mut *rng))).collect()
    });
    Ok(RValue::vec(Vector::Double(values.into())))
}

/// Random log-normal deviates.
///
/// Generates n random values from a log-normal distribution.
///
/// @param n number of observations
/// @param meanlog mean of the distribution on the log scale (default 0)
/// @param sdlog standard deviation on the log scale (default 1)
/// @return numeric vector of length n
#[interpreter_builtin(min_args = 1)]
fn interp_rlnorm(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let n = extract_n(args)?;
    let meanlog = extract_param(args, named, "meanlog", 1, 0.0);
    let sdlog = extract_param(args, named, "sdlog", 2, 1.0);
    let dist = rand_distr::LogNormal::new(meanlog, sdlog).map_err(RandomError::invalid_dist)?;
    let values: Vec<Option<f64>> = context.with_interpreter(|interp| {
        let mut rng = interp.rng().borrow_mut();
        (0..n).map(|_| Some(dist.sample(&mut *rng))).collect()
    });
    Ok(RValue::vec(Vector::Double(values.into())))
}

// endregion

// region: Discrete distributions

/// Random binomial deviates.
///
/// Generates n random values from a binomial distribution.
///
/// @param n number of observations
/// @param size number of trials (default 1)
/// @param prob probability of success on each trial (default 0.5)
/// @return integer vector of length n
#[interpreter_builtin(min_args = 2)]
fn interp_rbinom(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let n = extract_n(args)?;
    let size = f64_to_u64(extract_param(args, named, "size", 1, 1.0))?;
    let prob = extract_param(args, named, "prob", 2, 0.5);
    let dist = rand_distr::Binomial::new(size, prob).map_err(RandomError::invalid_dist)?;
    let values: Vec<Option<i64>> = context.with_interpreter(|interp| {
        let mut rng = interp.rng().borrow_mut();
        (0..n)
            .map(|_| i64::try_from(dist.sample(&mut *rng)).map(Some))
            .collect::<Result<_, _>>()
    })?;
    Ok(RValue::vec(Vector::Integer(values.into())))
}

/// Random Poisson deviates.
///
/// Generates n random values from a Poisson distribution.
///
/// @param n number of observations
/// @param lambda mean rate parameter (default 1)
/// @return integer vector of length n
#[interpreter_builtin(min_args = 1)]
fn interp_rpois(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let n = extract_n(args)?;
    let lambda = extract_param(args, named, "lambda", 1, 1.0);
    let dist = rand_distr::Poisson::new(lambda).map_err(RandomError::invalid_dist)?;
    let values: Vec<Option<i64>> = context.with_interpreter(|interp| {
        let mut rng = interp.rng().borrow_mut();
        (0..n)
            .map(|_| f64_to_i64(dist.sample(&mut *rng)).map(Some))
            .collect::<Result<_, _>>()
    })?;
    Ok(RValue::vec(Vector::Integer(values.into())))
}

/// Random geometric deviates.
///
/// Generates n random values from a geometric distribution.
///
/// @param n number of observations
/// @param prob probability of success (default 0.5)
/// @return integer vector of length n
#[interpreter_builtin(min_args = 1)]
fn interp_rgeom(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let n = extract_n(args)?;
    let prob = extract_param(args, named, "prob", 1, 0.5);
    let dist = rand_distr::Geometric::new(prob).map_err(RandomError::invalid_dist)?;
    let values: Vec<Option<i64>> = context.with_interpreter(|interp| {
        let mut rng = interp.rng().borrow_mut();
        (0..n)
            .map(|_| i64::try_from(dist.sample(&mut *rng)).map(Some))
            .collect::<Result<_, _>>()
    })?;
    Ok(RValue::vec(Vector::Integer(values.into())))
}

/// Random chi-squared deviates.
///
/// Generates n random values from a chi-squared distribution.
///
/// @param n number of observations
/// @param df degrees of freedom (default 1)
/// @return numeric vector of length n
#[interpreter_builtin(min_args = 2)]
fn interp_rchisq(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let n = extract_n(args)?;
    let df = extract_param(args, named, "df", 1, 1.0);
    let dist = rand_distr::ChiSquared::new(df).map_err(RandomError::invalid_dist)?;
    let values: Vec<Option<f64>> = context.with_interpreter(|interp| {
        let mut rng = interp.rng().borrow_mut();
        (0..n).map(|_| Some(dist.sample(&mut *rng))).collect()
    });
    Ok(RValue::vec(Vector::Double(values.into())))
}

/// Random Student's t deviates.
///
/// Generates n random values from a Student's t distribution.
///
/// @param n number of observations
/// @param df degrees of freedom (default 1)
/// @return numeric vector of length n
#[interpreter_builtin(min_args = 2)]
fn interp_rt(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let n = extract_n(args)?;
    let df = extract_param(args, named, "df", 1, 1.0);
    let dist = rand_distr::StudentT::new(df).map_err(RandomError::invalid_dist)?;
    let values: Vec<Option<f64>> = context.with_interpreter(|interp| {
        let mut rng = interp.rng().borrow_mut();
        (0..n).map(|_| Some(dist.sample(&mut *rng))).collect()
    });
    Ok(RValue::vec(Vector::Double(values.into())))
}

/// Random F deviates.
///
/// Generates n random values from an F distribution.
///
/// @param n number of observations
/// @param df1 numerator degrees of freedom (default 1)
/// @param df2 denominator degrees of freedom (default 1)
/// @return numeric vector of length n
#[interpreter_builtin(min_args = 2)]
fn interp_rf(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let n = extract_n(args)?;
    let df1 = extract_param(args, named, "df1", 1, 1.0);
    let df2 = extract_param(args, named, "df2", 2, 1.0);
    let dist = rand_distr::FisherF::new(df1, df2).map_err(RandomError::invalid_dist)?;
    let values: Vec<Option<f64>> = context.with_interpreter(|interp| {
        let mut rng = interp.rng().borrow_mut();
        (0..n).map(|_| Some(dist.sample(&mut *rng))).collect()
    });
    Ok(RValue::vec(Vector::Double(values.into())))
}

/// Random hypergeometric deviates.
///
/// Generates nn random values from a hypergeometric distribution.
///
/// @param nn number of observations
/// @param m number of white balls in the urn
/// @param n number of black balls in the urn
/// @param k number of balls drawn from the urn
/// @return integer vector of length nn
#[interpreter_builtin(min_args = 4)]
fn interp_rhyper(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let nn = extract_n(args)?;
    let m = f64_to_u64(extract_param(args, named, "m", 1, 1.0))?; // white balls
    let n = f64_to_u64(extract_param(args, named, "n", 2, 1.0))?; // black balls
    let k = f64_to_u64(extract_param(args, named, "k", 3, 1.0))?; // draws
    let dist = rand_distr::Hypergeometric::new(m + n, m, k).map_err(RandomError::invalid_dist)?;
    let values: Vec<Option<i64>> = context.with_interpreter(|interp| {
        let mut rng = interp.rng().borrow_mut();
        (0..nn)
            .map(|_| i64::try_from(dist.sample(&mut *rng)).map(Some))
            .collect::<Result<_, _>>()
    })?;
    Ok(RValue::vec(Vector::Integer(values.into())))
}

// endregion

// region: sample

/// Random sampling with or without replacement.
///
/// @param x vector to sample from, or a positive integer n (sample from 1:n)
/// @param size number of items to draw (default: length of x)
/// @param replace if TRUE, sample with replacement (default FALSE)
/// @param prob optional vector of probability weights
/// @return vector of sampled elements
#[interpreter_builtin(min_args = 1)]
fn interp_sample(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    // sample(x, size, replace = FALSE, prob = NULL)
    // If x is a single positive integer n, sample from 1:n
    let x_vec = match &args[0] {
        RValue::Vector(rv) => rv.clone(),
        _ => return Err(RandomError::InvalidSampleInput.into()),
    };

    // Check if x is a single integer n — if so, sample from 1:n
    let population: Vec<i64> = if x_vec.len() == 1 {
        if let Some(n) = x_vec.inner.as_integer_scalar() {
            if n >= 1 {
                (1..=n).collect()
            } else {
                return Err(RandomError::InvalidSampleInput.into());
            }
        } else if let Some(d) = x_vec.inner.as_double_scalar() {
            let n = f64_to_i64(d)?;
            if n >= 1 && (d - i64_to_f64(n)).abs() < 1e-10 {
                (1..=n).collect()
            } else {
                return Err(RandomError::InvalidSampleInput.into());
            }
        } else {
            // Single-element character/logical vector — sample from the element itself
            vec![1]
        }
    } else {
        // x is a vector of length > 1 — return indices
        (1..=i64::try_from(x_vec.len())?).collect()
    };

    let pop_len = population.len();

    // size defaults to length of population
    let size = usize::try_from(
        named
            .iter()
            .find(|(k, _)| k == "size")
            .map(|(_, v)| v)
            .or(args.get(1))
            .and_then(|v| v.as_vector())
            .and_then(|v| v.as_integer_scalar())
            .map_or_else(|| i64::try_from(pop_len), Ok)?,
    )?;

    // replace defaults to FALSE
    let replace = named
        .iter()
        .find(|(k, _)| k == "replace")
        .map(|(_, v)| v)
        .or(args.get(2))
        .and_then(|v| v.as_vector())
        .and_then(|v| v.as_logical_scalar())
        .unwrap_or(false);

    // Extract prob weights (positional 3 or named "prob")
    let prob_arg = named
        .iter()
        .find(|(k, _)| k == "prob")
        .map(|(_, v)| v)
        .or(args.get(3));

    let prob_weights = match prob_arg {
        Some(RValue::Null) | None => None,
        Some(v) => {
            let rv = v
                .as_vector()
                .ok_or(RandomError::InvalidParam { param: "prob" })?;
            let doubles = rv.to_doubles();
            if doubles.len() != pop_len {
                return Err(RandomError::ProbLengthMismatch {
                    prob_len: doubles.len(),
                    pop_len,
                }
                .into());
            }
            // Validate: no NA, no negative
            let mut weights = Vec::with_capacity(pop_len);
            for w in &doubles {
                match w {
                    None => return Err(RandomError::NaInProb.into()),
                    Some(p) if *p < 0.0 => return Err(RandomError::NegativeProb.into()),
                    Some(p) => weights.push(*p),
                }
            }
            Some(weights)
        }
    };

    if !replace && size > pop_len {
        return Err(RandomError::SampleTooLarge { size, pop_len }.into());
    }

    // For weighted without replacement, additionally check that enough items have nonzero weight
    if !replace {
        if let Some(ref weights) = prob_weights {
            let nonzero_count = weights.iter().filter(|&&w| w > 0.0).count();
            if nonzero_count < size {
                return Err(RandomError::TooFewPositiveProbs.into());
            }
        }
    }

    let result: Vec<Option<i64>> = context.with_interpreter(|interp| {
        let mut rng = interp.rng().borrow_mut();
        match prob_weights {
            None => {
                // Unweighted sampling
                if replace {
                    (0..size)
                        .map(|_| Some(population[rng.random_range(0..pop_len)]))
                        .collect()
                } else {
                    // Fisher-Yates partial shuffle
                    let mut pool = population;
                    for i in 0..size {
                        let j = rng.random_range(i..pool.len());
                        pool.swap(i, j);
                    }
                    pool.into_iter().take(size).map(Some).collect()
                }
            }
            Some(weights) => {
                if replace {
                    // Weighted sampling with replacement using cumulative probabilities
                    weighted_sample_with_replacement(&population, &weights, size, &mut *rng)
                } else {
                    // Weighted sampling without replacement: sequential draws
                    weighted_sample_without_replacement(&population, &weights, size, &mut *rng)
                }
            }
        }
    });

    // If the input was a vector with >1 elements, index into it (1-based)
    if x_vec.len() > 1 {
        context
            .with_interpreter(|interp| interp.index_by_integer(&x_vec.inner, &result))
            .map_err(RError::from)
    } else {
        Ok(RValue::vec(Vector::Integer(result.into())))
    }
}

/// Weighted sampling with replacement using cumulative probability + binary search.
fn weighted_sample_with_replacement(
    population: &[i64],
    weights: &[f64],
    size: usize,
    rng: &mut impl rand::Rng,
) -> Vec<Option<i64>> {
    // Normalize weights to cumulative probabilities
    let total: f64 = weights.iter().sum();
    if total <= 0.0 {
        // All weights are zero — return empty or repeat first non-zero? R errors here.
        return vec![None; size];
    }

    let mut cumulative = Vec::with_capacity(weights.len());
    let mut acc = 0.0;
    for &w in weights {
        acc += w / total;
        cumulative.push(acc);
    }
    // Fix rounding: ensure last entry is exactly 1.0
    if let Some(last) = cumulative.last_mut() {
        *last = 1.0;
    }

    let dist = rand_distr::Uniform::new(0.0, 1.0).expect("valid uniform range");
    (0..size)
        .map(|_| {
            let u: f64 = dist.sample(rng);
            let idx = cumulative.partition_point(|&c| c < u);
            let idx = idx.min(population.len() - 1);
            Some(population[idx])
        })
        .collect()
}

/// Weighted sampling without replacement: sequential weighted draws, removing selected items.
fn weighted_sample_without_replacement(
    population: &[i64],
    weights: &[f64],
    size: usize,
    rng: &mut impl rand::Rng,
) -> Vec<Option<i64>> {
    let mut remaining: Vec<(i64, f64)> = population
        .iter()
        .copied()
        .zip(weights.iter().copied())
        .collect();
    let mut result = Vec::with_capacity(size);
    let dist = rand_distr::Uniform::new(0.0, 1.0).expect("valid uniform range");

    for _ in 0..size {
        // Compute total weight of remaining items
        let total: f64 = remaining.iter().map(|(_, w)| w).sum();
        if total <= 0.0 {
            break;
        }

        // Pick a random point in [0, total)
        let u: f64 = dist.sample(rng) * total;
        let mut acc = 0.0;
        let mut chosen_idx = remaining.len() - 1;
        for (i, (_, w)) in remaining.iter().enumerate() {
            acc += w;
            if acc > u {
                chosen_idx = i;
                break;
            }
        }

        let (val, _) = remaining.remove(chosen_idx);
        result.push(Some(val));
    }

    result
}

// endregion

// region: miniR extension distributions
//
// Distributions available via rand_distr that are NOT part of standard R.
// These are miniR extensions, registered in the "collections" namespace.

/// Random Frechet (Type II extreme value) deviates.
///
/// **miniR extension** -- not available in base R.
///
/// The Frechet distribution models the maximum of many random variables.
/// It is parameterised by shape `alpha`, scale `s`, and location `m`.
///
/// @param n number of observations
/// @param alpha shape parameter (positive)
/// @param s scale parameter (positive, default 1)
/// @param m location parameter (default 0)
/// @return numeric vector of length n
#[interpreter_builtin(min_args = 2, namespace = "collections")]
fn interp_rfrechet(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let n = extract_n(args)?;
    let alpha = require_param(args, named, "alpha", 1)?;
    let s = extract_param(args, named, "s", 2, 1.0);
    let m = extract_param(args, named, "m", 3, 0.0);
    // Frechet::new(location, scale, shape)
    let dist = rand_distr::Frechet::new(m, s, alpha).map_err(RandomError::invalid_dist)?;
    let values: Vec<Option<f64>> = context.with_interpreter(|interp| {
        let mut rng = interp.rng().borrow_mut();
        (0..n).map(|_| Some(dist.sample(&mut *rng))).collect()
    });
    Ok(RValue::vec(Vector::Double(values.into())))
}

/// Random Gumbel (Type I extreme value) deviates.
///
/// **miniR extension** -- not available in base R.
///
/// The Gumbel distribution models the maximum (or minimum) of many samples.
/// It is parameterised by location `mu` and scale `beta`.
///
/// @param n number of observations
/// @param mu location parameter (default 0)
/// @param beta scale parameter (positive, default 1)
/// @return numeric vector of length n
#[interpreter_builtin(min_args = 1, namespace = "collections")]
fn interp_rgumbel(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let n = extract_n(args)?;
    let mu = extract_param(args, named, "mu", 1, 0.0);
    let beta = extract_param(args, named, "beta", 2, 1.0);
    let dist = rand_distr::Gumbel::new(mu, beta).map_err(RandomError::invalid_dist)?;
    let values: Vec<Option<f64>> = context.with_interpreter(|interp| {
        let mut rng = interp.rng().borrow_mut();
        (0..n).map(|_| Some(dist.sample(&mut *rng))).collect()
    });
    Ok(RValue::vec(Vector::Double(values.into())))
}

/// Random inverse Gaussian (Wald) deviates.
///
/// **miniR extension** -- not available in base R.
///
/// The inverse Gaussian distribution is a continuous distribution defined for
/// x > 0, parameterised by mean `mu` and shape `lambda`.
///
/// @param n number of observations
/// @param mu mean parameter (positive)
/// @param lambda shape parameter (positive)
/// @return numeric vector of length n
#[interpreter_builtin(min_args = 3, namespace = "collections")]
fn interp_rinvgauss(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let n = extract_n(args)?;
    let mu = require_param(args, named, "mu", 1)?;
    let lambda = require_param(args, named, "lambda", 2)?;
    let dist = rand_distr::InverseGaussian::new(mu, lambda).map_err(RandomError::invalid_dist)?;
    let values: Vec<Option<f64>> = context.with_interpreter(|interp| {
        let mut rng = interp.rng().borrow_mut();
        (0..n).map(|_| Some(dist.sample(&mut *rng))).collect()
    });
    Ok(RValue::vec(Vector::Double(values.into())))
}

/// Random Pareto deviates.
///
/// **miniR extension** -- not available in base R.
///
/// The Pareto distribution is a power-law distribution parameterised by
/// scale (minimum value) and shape (tail index).
///
/// @param n number of observations
/// @param scale scale parameter (positive, minimum value of the distribution)
/// @param shape shape parameter (positive, controls tail heaviness)
/// @return numeric vector of length n
#[interpreter_builtin(min_args = 3, namespace = "collections")]
fn interp_rpareto(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let n = extract_n(args)?;
    let scale = require_param(args, named, "scale", 1)?;
    let shape = require_param(args, named, "shape", 2)?;
    let dist = rand_distr::Pareto::new(scale, shape).map_err(RandomError::invalid_dist)?;
    let values: Vec<Option<f64>> = context.with_interpreter(|interp| {
        let mut rng = interp.rng().borrow_mut();
        (0..n).map(|_| Some(dist.sample(&mut *rng))).collect()
    });
    Ok(RValue::vec(Vector::Double(values.into())))
}

/// Random PERT deviates.
///
/// **miniR extension** -- not available in base R.
///
/// The PERT distribution is similar to the triangular distribution but with
/// a smooth (beta-shaped) PDF. It is parameterised by min, max, and mode.
///
/// @param n number of observations
/// @param min minimum value
/// @param max maximum value
/// @param mode most likely value (must be in [min, max])
/// @return numeric vector of length n
#[interpreter_builtin(min_args = 4, namespace = "collections")]
fn interp_rpert(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let n = extract_n(args)?;
    let min = require_param(args, named, "min", 1)?;
    let max = require_param(args, named, "max", 2)?;
    let mode = require_param(args, named, "mode", 3)?;
    let dist = rand_distr::Pert::new(min, max)
        .with_mode(mode)
        .map_err(RandomError::invalid_dist)?;
    let values: Vec<Option<f64>> = context.with_interpreter(|interp| {
        let mut rng = interp.rng().borrow_mut();
        (0..n).map(|_| Some(dist.sample(&mut *rng))).collect()
    });
    Ok(RValue::vec(Vector::Double(values.into())))
}

/// Random skew-normal deviates.
///
/// **miniR extension** -- not available in base R.
///
/// The skew-normal distribution generalises the normal distribution to allow
/// non-zero skewness. When shape = 0 it reduces to the normal distribution.
///
/// @param n number of observations
/// @param location location parameter (default 0)
/// @param scale scale parameter (positive, default 1)
/// @param shape skewness parameter (default 0; 0 = normal)
/// @return numeric vector of length n
#[interpreter_builtin(min_args = 1, namespace = "collections")]
fn interp_rskewnorm(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let n = extract_n(args)?;
    let location = extract_param(args, named, "location", 1, 0.0);
    let scale = extract_param(args, named, "scale", 2, 1.0);
    let shape = extract_param(args, named, "shape", 3, 0.0);
    let dist =
        rand_distr::SkewNormal::new(location, scale, shape).map_err(RandomError::invalid_dist)?;
    let values: Vec<Option<f64>> = context.with_interpreter(|interp| {
        let mut rng = interp.rng().borrow_mut();
        (0..n).map(|_| Some(dist.sample(&mut *rng))).collect()
    });
    Ok(RValue::vec(Vector::Double(values.into())))
}

/// Random triangular deviates.
///
/// **miniR extension** -- not available in base R.
///
/// The triangular distribution has a piecewise linear PDF defined by min, max,
/// and mode. For a smooth alternative, see `rpert()`.
///
/// @param n number of observations
/// @param min minimum value
/// @param max maximum value
/// @param mode most likely value (must be in [min, max])
/// @return numeric vector of length n
#[interpreter_builtin(min_args = 4, namespace = "collections")]
fn interp_rtriangular(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let n = extract_n(args)?;
    let min = require_param(args, named, "min", 1)?;
    let max = require_param(args, named, "max", 2)?;
    let mode = require_param(args, named, "mode", 3)?;
    let dist = rand_distr::Triangular::new(min, max, mode).map_err(RandomError::invalid_dist)?;
    let values: Vec<Option<f64>> = context.with_interpreter(|interp| {
        let mut rng = interp.rng().borrow_mut();
        (0..n).map(|_| Some(dist.sample(&mut *rng))).collect()
    });
    Ok(RValue::vec(Vector::Double(values.into())))
}

/// Random Zeta deviates.
///
/// **miniR extension** -- not available in base R.
///
/// The Zeta distribution is a discrete power-law distribution on positive
/// integers. It is the limit of the Zipf distribution as n -> infinity.
/// The parameter `s` must be strictly greater than 1.
///
/// @param n number of observations
/// @param s shape parameter (must be > 1)
/// @return numeric vector of length n (values are positive integers stored as doubles)
#[interpreter_builtin(min_args = 2, namespace = "collections")]
fn interp_rzeta(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let n = extract_n(args)?;
    let s = require_param(args, named, "s", 1)?;
    let dist = rand_distr::Zeta::new(s).map_err(RandomError::invalid_dist)?;
    let values: Vec<Option<f64>> = context.with_interpreter(|interp| {
        let mut rng = interp.rng().borrow_mut();
        (0..n).map(|_| Some(dist.sample(&mut *rng))).collect()
    });
    Ok(RValue::vec(Vector::Double(values.into())))
}

// endregion
