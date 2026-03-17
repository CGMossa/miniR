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

/// Helper: extract a positive integer `n` from the first argument.
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

/// Helper: extract a named f64 parameter from named args, falling back to positional.
fn extract_param(
    args: &[RValue],
    named: &[(String, RValue)],
    name: &str,
    positional_index: usize,
    default: f64,
) -> f64 {
    // Check named args first
    for (k, v) in named {
        if k == name {
            if let Some(rv) = v.as_vector() {
                if let Some(d) = rv.as_double_scalar() {
                    return d;
                }
            }
        }
    }
    // Fall back to positional
    args.get(positional_index)
        .and_then(|v| v.as_vector())
        .and_then(|v| v.as_double_scalar())
        .unwrap_or(default)
}

// region: set.seed

/// Set the random number generator seed for reproducibility.
///
/// @param seed integer seed value
/// @return NULL, invisibly
#[interpreter_builtin(name = "set.seed", min_args = 1)]
fn interp_set_seed(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let seed_f64 = args[0]
        .as_vector()
        .and_then(|v| v.as_double_scalar())
        .ok_or(RandomError::InvalidParam { param: "seed" })?;
    let seed = f64_to_u64(seed_f64)?;
    context.with_interpreter(|interp| {
        *interp.rng().borrow_mut() = rand::rngs::StdRng::seed_from_u64(seed);
    });
    Ok(RValue::Null)
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

    if !replace && size > pop_len {
        return Err(RandomError::SampleTooLarge { size, pop_len }.into());
    }

    let result: Vec<Option<i64>> = context.with_interpreter(|interp| {
        let mut rng = interp.rng().borrow_mut();
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

// endregion
