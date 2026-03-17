# libm integration plan

> `libm` 0.2 -- Pure Rust implementation of C math library functions.
> <https://github.com/rust-lang/libm>

## What it does

Provides pure Rust implementations of all C99 math functions: `sin`, `cos`,
`tan`, `exp`, `log`, `pow`, `sqrt`, `ceil`, `floor`, `round`, `fma`,
`erf`, `erfc`, `tgamma`, `lgamma`, `j0`/`j1`/`jn` (Bessel functions),
`y0`/`y1`/`yn` (Bessel Y), and more.

```rust
use libm::{lgamma_r, erf, erfc, j0, y0, tgamma};

let (lg, sign) = lgamma_r(5.0);  // lgamma with sign
let e = erf(1.0);                // error function
let b = j0(2.5);                 // Bessel J0
```

Key functions not available in Rust's `f64` methods:
- `erf` / `erfc` -- error function and complement
- `tgamma` / `lgamma` -- gamma and log-gamma
- `j0`, `j1`, `jn` -- Bessel functions of first kind
- `y0`, `y1`, `yn` -- Bessel functions of second kind

## Where it fits in miniR

### Special math functions

R provides many special math functions that are not in Rust's `f64`:

| R function | libm function |
|---|---|
| `gamma(x)` | `tgamma(x)` |
| `lgamma(x)` | `lgamma(x)` |
| `digamma(x)` | not in libm (need custom impl) |
| `trigamma(x)` | not in libm (need custom impl) |
| `beta(a, b)` | `exp(lgamma(a) + lgamma(b) - lgamma(a+b))` |
| `lbeta(a, b)` | `lgamma(a) + lgamma(b) - lgamma(a+b)` |
| `choose(n, k)` | via lgamma: `exp(lgamma(n+1) - lgamma(k+1) - lgamma(n-k+1))` |
| `factorial(n)` | `tgamma(n + 1)` |
| `lfactorial(n)` | `lgamma(n + 1)` |
| `besselJ(x, nu)` | `jn(nu as i32, x)` (integer orders only) |
| `besselY(x, nu)` | `yn(nu as i32, x)` (integer orders only) |

### Error function (statistics)

`erf` and `erfc` are needed for:
- `pnorm()` / `qnorm()` -- normal distribution CDF/quantile
- `dnorm()` -- normal density (uses `exp`, already available)

```r
pnorm(x) = 0.5 * (1 + erf(x / sqrt(2)))
```

### Vectorized math operations

All R math functions are vectorized. With libm providing the scalar
implementations, we wrap them in vectorized operations:

```rust
fn builtin_gamma(args: &CallArgs, ctx: &mut BuiltinContext) -> Result<RValue> {
    let x = args.required_double_vector(0)?;
    let result: Vec<Option<f64>> = x.iter()
        .map(|v| v.map(libm::tgamma))
        .collect();
    Ok(Vector::Double(result.into()).into())
}
```

## Status

Already a direct dependency in Cargo.toml (`libm = "0.2"`). May already
be used in some math builtins.

## Implementation

1. Wire `gamma()`, `lgamma()`, `beta()`, `lbeta()` builtins to libm
2. Wire `factorial()`, `lfactorial()`, `choose()` to lgamma-based formulas
3. Wire `besselJ()`, `besselY()` for integer orders
4. Implement `pnorm()` / `qnorm()` using `erf` / `erfc`
5. All functions should be vectorized over their inputs

## Priority

High -- `gamma`, `lgamma`, `choose`, `factorial` are commonly used in
statistical R code. libm is already a direct dependency.
