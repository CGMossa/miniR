# approx integration plan

> `approx` 0.5 — Approximate floating-point comparisons.
> <https://github.com/brendanzab/approx>

## What it does

Provides traits and macros for comparing floating-point values with
configurable tolerance: `AbsDiffEq`, `RelativeEq`, `UlpsEq`.

```rust
use approx::{abs_diff_eq, relative_eq, ulps_eq};

abs_diff_eq!(1.0, 1.0 + 1e-10, epsilon = 1e-8);  // true
relative_eq!(100.0, 100.001, epsilon = 1e-4);       // true
ulps_eq!(1.0, 1.0000000000000002);                   // true (1 ULP apart)
```

## Where it fits in miniR

### `all.equal()` — proper numeric comparison

R's `all.equal(target, current, tolerance)` is the standard way to compare
floating-point values. The current implementation (`builtins.rs`) uses a
hand-rolled element-wise `(a - b).abs() / max(a.abs(), b.abs())` check,
which is a relative-difference approach. Problems with the current impl:

1. Uses `Debug` format comparison as fallback for non-numeric types
2. No `scale` parameter support
3. Doesn't return descriptive messages like R does ("Mean relative difference: 0.001")
4. Doesn't handle `check.attributes`, `check.names`, `check.environment` params

### Implementation using approx

```rust
use approx::AbsDiffEq;

fn all_equal_numeric(target: &[f64], current: &[f64], tolerance: f64) -> Result<bool, String> {
    if target.len() != current.len() {
        return Err(format!(
            "Lengths ({}, {}) differ",
            target.len(), current.len()
        ));
    }
    let mut max_diff = 0.0f64;
    for (a, b) in target.iter().zip(current.iter()) {
        let diff = (a - b).abs();
        let scale = a.abs().max(b.abs()).max(1.0);
        max_diff = max_diff.max(diff / scale);
    }
    if max_diff <= tolerance {
        Ok(true)
    } else {
        Err(format!("Mean relative difference: {}", max_diff))
    }
}
```

The `approx` crate is useful for:
- `identical()` improvement: use `ulps_eq!` for exact-but-for-floating-point comparisons
- `all.equal()` with `scale` parameter: use `abs_diff_eq!` when scale is specified
- Internal test assertions: `assert_relative_eq!` in Rust integration tests

### `identical()` improvement

The current `identical()` uses `format!("{:?}")` comparison which is fragile.
With approx, NaN handling becomes explicit:

```rust
fn values_identical(a: f64, b: f64) -> bool {
    if a.is_nan() && b.is_nan() { return true; }
    a.to_bits() == b.to_bits()  // bitwise comparison (NA vs NaN distinction)
}
```

(approx isn't needed for identical — bitwise is better — but it helps for all.equal)

## Implementation

1. approx is already vendored (transitive dep via nalgebra)
2. Add `use approx::AbsDiffEq` where needed (no new Cargo.toml entry needed)
3. Rewrite `all.equal()` in builtins.rs:
   - Numeric: element-wise relative difference with tolerance (default 1.5e-8)
   - Return TRUE or descriptive character string (not logical FALSE)
   - Support `check.attributes` (default TRUE), `check.names` (default TRUE)
   - Handle length mismatch, type mismatch, NULL comparison
4. Fix `identical()` to use structural comparison instead of Debug format

## Priority

Medium — `all.equal()` is used heavily in test suites and `stopifnot(all.equal(...))`.
The approx crate is already available; the work is in the R-level semantics.
