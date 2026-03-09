# RNG State Plan

> Per-interpreter random number generator state for `set.seed()`, `runif()`, `rnorm()`, and friends.

## Current state

- `set.seed()` is a noop (math.rs:1149) — accepts args, returns NULL
- No random number builtins exist yet (runif, rnorm, rbinom are noop stubs in stubs.rs)
- `rand.md` covers the crate choice (rand + rand_distr) but not state management

## Design

### RNG on the Interpreter

```rust
use rand::rngs::StdRng;
use rand::SeedableRng;

pub struct Interpreter {
    pub global_env: Environment,
    s3_dispatch_stack: RefCell<Vec<S3DispatchContext>>,
    rng: RefCell<StdRng>,  // NEW — seeded RNG
}

impl Interpreter {
    pub fn new() -> Self {
        Interpreter {
            global_env: /* ... */,
            s3_dispatch_stack: RefCell::new(Vec::new()),
            rng: RefCell::new(StdRng::from_os_rng()),  // random seed by default
        }
    }
}
```

**Why `StdRng`:**
- Deterministic (implements `SeedableRng`) — needed for `set.seed()` reproducibility
- Cryptographically strong default, but we only care about determinism-when-seeded
- `ChaCha12Rng` (what `StdRng` currently aliases) is fast enough

**Why on Interpreter, not thread-local:**
- Follows the project's "new state goes on Interpreter struct" rule
- Multiple interpreters in the same thread get independent RNG streams
- `RefCell` for interior mutability through `&self` (same pattern as dispatch stack)

### set.seed

```rust
#[interpreter_builtin(name = "set.seed", min_args = 1)]
fn interp_set_seed(args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    let seed = args[0].as_double_scalar()? as u64;
    with_interpreter(|interp| {
        *interp.rng.borrow_mut() = StdRng::seed_from_u64(seed);
    });
    Ok(RValue::Null)
}
```

After `set.seed(42)`, all subsequent random draws are deterministic and reproducible.

### Random number builtins

All random builtins follow the same pattern — borrow the RNG, draw from a distribution:

```rust
use rand::Rng;
use rand_distr::{Uniform, Normal, Binomial, /* etc */};

#[interpreter_builtin(name = "runif")]
fn interp_runif(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let n = /* extract n, default 1 */;
    let min = /* extract min, default 0.0 */;
    let max = /* extract max, default 1.0 */;

    with_interpreter(|interp| {
        let mut rng = interp.rng.borrow_mut();
        let dist = Uniform::new(min, max);
        let values: Vec<Option<f64>> = (0..n)
            .map(|_| Some(rng.sample(dist)))
            .collect();
        Ok(RValue::vec(Vector::Double(values.into())))
    })
}
```

### Distribution mapping

| R function | rand_distr type | Parameters |
|-----------|----------------|------------|
| `runif(n, min, max)` | `Uniform::new(min, max)` | min=0, max=1 |
| `rnorm(n, mean, sd)` | `Normal::new(mean, sd)` | mean=0, sd=1 |
| `rbinom(n, size, prob)` | `Binomial::new(size, prob)` | — |
| `rpois(n, lambda)` | `Poisson::new(lambda)` | — |
| `rexp(n, rate)` | `Exp::new(rate)` | rate=1 |
| `rgamma(n, shape, rate)` | `Gamma::new(shape, 1/rate)` | — |
| `sample(x, size, replace)` | `rng.gen_range()` / shuffle | — |

### sample()

`sample()` is the most complex — it handles both sampling from a vector and generating random permutations:

```rust
// sample(x, size, replace = FALSE, prob = NULL)
// If x is a single integer n, sample from 1:n
// If replace = FALSE, use Fisher-Yates partial shuffle
// If replace = TRUE, use weighted/unweighted sampling with replacement
```

This needs careful implementation but no special RNG state beyond what we already have.

## Dependencies

Already vendored (transitive through other crates):
- `rand` — check if already in vendor/, otherwise add `rand = "0.9"`
- `rand_distr` — `rand_distr = "0.5"` for distribution types

Check `plans/rand.md` for exact version pins.

## Implementation order

1. Add `rand` and `rand_distr` to Cargo.toml, vendor
2. Add `rng: RefCell<StdRng>` to `Interpreter`
3. Rewrite `set.seed()` as interpreter_builtin (needs `with_interpreter`)
4. Implement `runif()` as interpreter_builtin
5. Implement `rnorm()`, `sample()`
6. Implement `rbinom()`, `rpois()`, `rexp()`, `rgamma()`
7. Remove noop stubs

## Priority

Medium-high — `sample()` and `runif()` are used very frequently. `set.seed()` is essential for reproducible scripts. The implementation is straightforward once the RNG field exists.
