//! Random number generation builtins: set.seed, runif, rnorm, rbinom, sample, etc.
//! Uses the per-interpreter RNG state via `with_interpreter()`.

use rand::Rng;
use rand::SeedableRng;
use rand_distr::Distribution;

use crate::interpreter::coerce::{f64_to_i64, f64_to_u64, i64_to_f64};
use crate::interpreter::value::*;
use crate::interpreter::with_interpreter;
use minir_macros::builtin;

/// Helper: extract a positive integer `n` from the first argument.
fn extract_n(args: &[RValue]) -> Result<usize, RError> {
    let n = args
        .first()
        .and_then(|v| v.as_vector())
        .and_then(|v| v.as_integer_scalar())
        .ok_or_else(|| RError::Argument("invalid 'n' argument".to_string()))?;
    if n < 0 {
        return Err(RError::Argument(
            "invalid argument: 'n' must be non-negative".to_string(),
        ));
    }
    usize::try_from(n).map_err(|_| RError::Argument("invalid 'n' argument".to_string()))
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

#[builtin(name = "set.seed", min_args = 1)]
fn builtin_set_seed(args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    let seed = args[0]
        .as_vector()
        .and_then(|v| v.as_double_scalar())
        .ok_or_else(|| RError::Argument("invalid 'seed' argument".to_string()))
        .and_then(f64_to_u64)?;
    with_interpreter(|interp| {
        *interp.rng().borrow_mut() = rand::rngs::StdRng::seed_from_u64(seed);
    });
    Ok(RValue::Null)
}

// endregion

// region: Continuous distributions

#[builtin(min_args = 1)]
fn builtin_runif(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let n = extract_n(args)?;
    let min = extract_param(args, named, "min", 1, 0.0);
    let max = extract_param(args, named, "max", 2, 1.0);
    if min > max {
        return Err(RError::Argument(
            "invalid arguments: 'min' must not be greater than 'max'".to_string(),
        ));
    }
    let dist = rand_distr::Uniform::new(min, max)
        .map_err(|e| RError::Argument(format!("invalid distribution parameters: {e}")))?;
    let values: Vec<Option<f64>> = with_interpreter(|interp| {
        let mut rng = interp.rng().borrow_mut();
        (0..n).map(|_| Some(dist.sample(&mut *rng))).collect()
    });
    Ok(RValue::vec(Vector::Double(values.into())))
}

#[builtin(min_args = 1)]
fn builtin_rnorm(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let n = extract_n(args)?;
    let mean = extract_param(args, named, "mean", 1, 0.0);
    let sd = extract_param(args, named, "sd", 2, 1.0);
    let dist = rand_distr::Normal::new(mean, sd)
        .map_err(|e| RError::Argument(format!("invalid distribution parameters: {e}")))?;
    let values: Vec<Option<f64>> = with_interpreter(|interp| {
        let mut rng = interp.rng().borrow_mut();
        (0..n).map(|_| Some(dist.sample(&mut *rng))).collect()
    });
    Ok(RValue::vec(Vector::Double(values.into())))
}

#[builtin(min_args = 1)]
fn builtin_rexp(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let n = extract_n(args)?;
    let rate = extract_param(args, named, "rate", 1, 1.0);
    let dist = rand_distr::Exp::new(rate)
        .map_err(|e| RError::Argument(format!("invalid distribution parameters: {e}")))?;
    let values: Vec<Option<f64>> = with_interpreter(|interp| {
        let mut rng = interp.rng().borrow_mut();
        (0..n).map(|_| Some(dist.sample(&mut *rng))).collect()
    });
    Ok(RValue::vec(Vector::Double(values.into())))
}

#[builtin(min_args = 1)]
fn builtin_rgamma(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let n = extract_n(args)?;
    let shape = extract_param(args, named, "shape", 1, 1.0);
    let rate = extract_param(args, named, "rate", 2, 1.0);
    // R uses rate, rand_distr::Gamma uses scale = 1/rate
    let dist = rand_distr::Gamma::new(shape, 1.0 / rate)
        .map_err(|e| RError::Argument(format!("invalid distribution parameters: {e}")))?;
    let values: Vec<Option<f64>> = with_interpreter(|interp| {
        let mut rng = interp.rng().borrow_mut();
        (0..n).map(|_| Some(dist.sample(&mut *rng))).collect()
    });
    Ok(RValue::vec(Vector::Double(values.into())))
}

#[builtin(min_args = 1)]
fn builtin_rbeta(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let n = extract_n(args)?;
    let shape1 = extract_param(args, named, "shape1", 1, 1.0);
    let shape2 = extract_param(args, named, "shape2", 2, 1.0);
    let dist = rand_distr::Beta::new(shape1, shape2)
        .map_err(|e| RError::Argument(format!("invalid distribution parameters: {e}")))?;
    let values: Vec<Option<f64>> = with_interpreter(|interp| {
        let mut rng = interp.rng().borrow_mut();
        (0..n).map(|_| Some(dist.sample(&mut *rng))).collect()
    });
    Ok(RValue::vec(Vector::Double(values.into())))
}

#[builtin(min_args = 1)]
fn builtin_rcauchy(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let n = extract_n(args)?;
    let location = extract_param(args, named, "location", 1, 0.0);
    let scale = extract_param(args, named, "scale", 2, 1.0);
    let dist = rand_distr::Cauchy::new(location, scale)
        .map_err(|e| RError::Argument(format!("invalid distribution parameters: {e}")))?;
    let values: Vec<Option<f64>> = with_interpreter(|interp| {
        let mut rng = interp.rng().borrow_mut();
        (0..n).map(|_| Some(dist.sample(&mut *rng))).collect()
    });
    Ok(RValue::vec(Vector::Double(values.into())))
}

#[builtin(min_args = 1)]
fn builtin_rweibull(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let n = extract_n(args)?;
    let shape = extract_param(args, named, "shape", 1, 1.0);
    let scale = extract_param(args, named, "scale", 2, 1.0);
    let dist = rand_distr::Weibull::new(scale, shape)
        .map_err(|e| RError::Argument(format!("invalid distribution parameters: {e}")))?;
    let values: Vec<Option<f64>> = with_interpreter(|interp| {
        let mut rng = interp.rng().borrow_mut();
        (0..n).map(|_| Some(dist.sample(&mut *rng))).collect()
    });
    Ok(RValue::vec(Vector::Double(values.into())))
}

#[builtin(min_args = 1)]
fn builtin_rlnorm(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let n = extract_n(args)?;
    let meanlog = extract_param(args, named, "meanlog", 1, 0.0);
    let sdlog = extract_param(args, named, "sdlog", 2, 1.0);
    let dist = rand_distr::LogNormal::new(meanlog, sdlog)
        .map_err(|e| RError::Argument(format!("invalid distribution parameters: {e}")))?;
    let values: Vec<Option<f64>> = with_interpreter(|interp| {
        let mut rng = interp.rng().borrow_mut();
        (0..n).map(|_| Some(dist.sample(&mut *rng))).collect()
    });
    Ok(RValue::vec(Vector::Double(values.into())))
}

// endregion

// region: Discrete distributions

#[builtin(min_args = 2)]
fn builtin_rbinom(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let n = extract_n(args)?;
    let size = f64_to_u64(extract_param(args, named, "size", 1, 1.0))?;
    let prob = extract_param(args, named, "prob", 2, 0.5);
    let dist = rand_distr::Binomial::new(size, prob)
        .map_err(|e| RError::Argument(format!("invalid distribution parameters: {e}")))?;
    let values: Vec<Option<i64>> = with_interpreter(|interp| {
        let mut rng = interp.rng().borrow_mut();
        (0..n)
            .map(|_| i64::try_from(dist.sample(&mut *rng)).map(Some))
            .collect::<Result<_, _>>()
    })?;
    Ok(RValue::vec(Vector::Integer(values.into())))
}

#[builtin(min_args = 1)]
fn builtin_rpois(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let n = extract_n(args)?;
    let lambda = extract_param(args, named, "lambda", 1, 1.0);
    let dist = rand_distr::Poisson::new(lambda)
        .map_err(|e| RError::Argument(format!("invalid distribution parameters: {e}")))?;
    let values: Vec<Option<i64>> = with_interpreter(|interp| {
        let mut rng = interp.rng().borrow_mut();
        (0..n)
            .map(|_| f64_to_i64(dist.sample(&mut *rng)).map(Some))
            .collect::<Result<_, _>>()
    })?;
    Ok(RValue::vec(Vector::Integer(values.into())))
}

#[builtin(min_args = 1)]
fn builtin_rgeom(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let n = extract_n(args)?;
    let prob = extract_param(args, named, "prob", 1, 0.5);
    let dist = rand_distr::Geometric::new(prob)
        .map_err(|e| RError::Argument(format!("invalid distribution parameters: {e}")))?;
    let values: Vec<Option<i64>> = with_interpreter(|interp| {
        let mut rng = interp.rng().borrow_mut();
        (0..n)
            .map(|_| i64::try_from(dist.sample(&mut *rng)).map(Some))
            .collect::<Result<_, _>>()
    })?;
    Ok(RValue::vec(Vector::Integer(values.into())))
}

#[builtin(min_args = 2)]
fn builtin_rchisq(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let n = extract_n(args)?;
    let df = extract_param(args, named, "df", 1, 1.0);
    let dist = rand_distr::ChiSquared::new(df)
        .map_err(|e| RError::Argument(format!("invalid distribution parameters: {e}")))?;
    let values: Vec<Option<f64>> = with_interpreter(|interp| {
        let mut rng = interp.rng().borrow_mut();
        (0..n).map(|_| Some(dist.sample(&mut *rng))).collect()
    });
    Ok(RValue::vec(Vector::Double(values.into())))
}

#[builtin(min_args = 2)]
fn builtin_rt(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let n = extract_n(args)?;
    let df = extract_param(args, named, "df", 1, 1.0);
    let dist = rand_distr::StudentT::new(df)
        .map_err(|e| RError::Argument(format!("invalid distribution parameters: {e}")))?;
    let values: Vec<Option<f64>> = with_interpreter(|interp| {
        let mut rng = interp.rng().borrow_mut();
        (0..n).map(|_| Some(dist.sample(&mut *rng))).collect()
    });
    Ok(RValue::vec(Vector::Double(values.into())))
}

#[builtin(min_args = 2)]
fn builtin_rf(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let n = extract_n(args)?;
    let df1 = extract_param(args, named, "df1", 1, 1.0);
    let df2 = extract_param(args, named, "df2", 2, 1.0);
    let dist = rand_distr::FisherF::new(df1, df2)
        .map_err(|e| RError::Argument(format!("invalid distribution parameters: {e}")))?;
    let values: Vec<Option<f64>> = with_interpreter(|interp| {
        let mut rng = interp.rng().borrow_mut();
        (0..n).map(|_| Some(dist.sample(&mut *rng))).collect()
    });
    Ok(RValue::vec(Vector::Double(values.into())))
}

#[builtin(min_args = 4)]
fn builtin_rhyper(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let nn = extract_n(args)?;
    let m = f64_to_u64(extract_param(args, named, "m", 1, 1.0))?; // white balls
    let n = f64_to_u64(extract_param(args, named, "n", 2, 1.0))?; // black balls
    let k = f64_to_u64(extract_param(args, named, "k", 3, 1.0))?; // draws
    let dist = rand_distr::Hypergeometric::new(m + n, m, k)
        .map_err(|e| RError::Argument(format!("invalid distribution parameters: {e}")))?;
    let values: Vec<Option<i64>> = with_interpreter(|interp| {
        let mut rng = interp.rng().borrow_mut();
        (0..nn)
            .map(|_| i64::try_from(dist.sample(&mut *rng)).map(Some))
            .collect::<Result<_, _>>()
    })?;
    Ok(RValue::vec(Vector::Integer(values.into())))
}

// endregion

// region: sample

#[builtin(min_args = 1)]
fn builtin_sample(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    // sample(x, size, replace = FALSE, prob = NULL)
    // If x is a single positive integer n, sample from 1:n
    let x_vec = match &args[0] {
        RValue::Vector(rv) => rv.clone(),
        _ => return Err(RError::Argument("invalid first argument".to_string())),
    };

    // Check if x is a single integer n — if so, sample from 1:n
    let population: Vec<i64> = if x_vec.len() == 1 {
        if let Some(n) = x_vec.inner.as_integer_scalar() {
            if n >= 1 {
                (1..=n).collect()
            } else {
                return Err(RError::Argument(
                    "invalid first argument: must be a positive integer or a vector".to_string(),
                ));
            }
        } else if let Some(d) = x_vec.inner.as_double_scalar() {
            let n = f64_to_i64(d)?;
            if n >= 1 && (d - i64_to_f64(n)).abs() < 1e-10 {
                (1..=n).collect()
            } else {
                return Err(RError::Argument(
                    "invalid first argument: must be a positive integer or a vector".to_string(),
                ));
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
        return Err(RError::Argument(format!(
            "cannot take a sample larger than the population ({size} > {pop_len}) when 'replace = FALSE'"
        )));
    }

    let result: Vec<Option<i64>> = with_interpreter(|interp| {
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
        with_interpreter(|interp| interp.index_by_integer(&x_vec.inner, &result))
            .map_err(RError::from)
    } else {
        Ok(RValue::vec(Vector::Integer(result.into())))
    }
}

// endregion
