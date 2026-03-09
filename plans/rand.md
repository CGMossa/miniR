# rand + rand_distr integration plan

> `rand` 0.10 — Random number generation.
> <https://github.com/rust-random/rand>
>
> `rand_distr` 0.6 — Probability distributions.
> <https://github.com/rust-random/rand>

## What they do

`rand`: RNG traits, thread-local RNG, `gen_range()`, `shuffle()`, `choose()`.
`rand_distr`: Normal, Uniform, Binomial, Poisson, Gamma, Beta, Cauchy, Exponential,
Chi-squared, Student's t, F, Geometric, Hypergeometric, Weibull, LogNormal, etc.

```rust
use rand::Rng;
use rand_distr::{Normal, Distribution};

let mut rng = rand::rng();
let x: f64 = rng.random();           // uniform [0, 1)
let n = Normal::new(0.0, 1.0)?;
let sample: f64 = n.sample(&mut rng); // standard normal
```

## Where they fit in newr

### 1. Core random functions

| R function | rand/rand_distr equivalent |
|---|---|
| `runif(n, min, max)` | `Uniform::new(min, max).sample()` |
| `rnorm(n, mean, sd)` | `Normal::new(mean, sd).sample()` |
| `rbinom(n, size, prob)` | `Binomial::new(size, prob).sample()` |
| `rpois(n, lambda)` | `Poisson::new(lambda).sample()` |
| `rexp(n, rate)` | `Exp::new(rate).sample()` |
| `rgamma(n, shape, rate)` | `Gamma::new(shape, 1.0/rate).sample()` |
| `rbeta(n, shape1, shape2)` | `Beta::new(shape1, shape2).sample()` |
| `rcauchy(n, location, scale)` | `Cauchy::new(location, scale).sample()` |
| `rchisq(n, df)` | `ChiSquared::new(df).sample()` |
| `rt(n, df)` | `StudentT::new(df).sample()` |
| `rf(n, df1, df2)` | `FisherF::new(df1, df2).sample()` |
| `rgeom(n, prob)` | `Geometric::new(prob).sample()` |
| `rhyper(nn, m, n, k)` | `Hypergeometric::new(m+n, m, k).sample()` |
| `rweibull(n, shape, scale)` | `Weibull::new(shape, scale).sample()` |
| `rlnorm(n, meanlog, sdlog)` | `LogNormal::new(meanlog, sdlog).sample()` |

### 2. `sample()` — random sampling

```r
sample(1:10, 5)              # sample without replacement
sample(1:10, 5, replace=TRUE) # with replacement
sample(letters, 3, prob=p)    # weighted sampling
```

`rand::seq::index::sample()` for unweighted, `rand::distributions::WeightedIndex`
for weighted sampling.

### 3. `set.seed()` — reproducible RNG

R's `set.seed(42)` seeds the RNG for reproducibility. We need a seedable RNG:

```rust
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

let mut rng = ChaCha8Rng::seed_from_u64(42);
```

Requires `rand_chacha` or another seedable RNG crate.

### 4. Density/quantile/CDF functions

`rand_distr` provides sampling but NOT density (`dnorm`), CDF (`pnorm`), or
quantile (`qnorm`) functions. Those need separate implementations or a statistics
crate like `statrs`.

## Relationship to builtins plan

| Phase | Builtins affected | Impact |
|---|---|---|
| Phase 9 (random) | All `r*()` functions, `sample()`, `set.seed()` | core random number generation |
| Phase 9 (random) | `d*()`, `p*()`, `q*()` | partial (sampling only, not density/CDF) |

## Recommendation

**Add when implementing Phase 9 (random number generation).** This is the only
viable approach — `rand` is the standard Rust RNG ecosystem. Need `rand_chacha`
too for `set.seed()` reproducibility.

**Effort:** 2-3 hours for core r*/sample/set.seed, more for d*/p*/q* functions.
