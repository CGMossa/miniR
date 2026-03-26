# num-traits integration plan

> `num-traits` 0.2 — Numeric trait abstractions.
> <https://github.com/rust-num/num-traits>

## What it does

Traits for generic numeric programming: `Num`, `Float`, `Integer`, `Zero`, `One`,
`NumCast`, `ToPrimitive`, `FromPrimitive`, `Signed`, `Unsigned`, `Pow`, etc.

```rust
use num_traits::{Float, NumCast};

fn generic_sqrt<T: Float>(x: T) -> T {
    x.sqrt()
}

fn safe_cast<T: NumCast>(x: f64) -> Option<T> {
    NumCast::from(x)
}
```

## Where it fits in miniR

### 1. Generic numeric operations

Write builtin math functions that work across numeric types:

```rust
fn builtin_abs<T: Signed>(x: T) -> T { x.abs() }
fn builtin_sqrt<T: Float>(x: T) -> T { x.sqrt() }
```

This avoids duplicating logic for `f64`, `i32`, and `Complex<f64>`.

### 2. Type conversion — `NumCast` / `ToPrimitive`

R's `as.integer()`, `as.double()`, `as.numeric()` do type conversion with range
checking. `ToPrimitive` provides `to_i32()`, `to_f64()`, etc. with `Option` return
for overflow.

### 3. Integer overflow detection

`num_traits::CheckedAdd`, `CheckedMul`, etc. detect overflow in integer arithmetic,
matching R's behavior of producing `NA` on integer overflow.

## Relationship to builtins plan

| Phase | Builtins affected | Impact |
|---|---|---|
| Phase 1 (math) | All math functions | generic implementations |
| Phase 3 (collections) | `as.integer()`, `as.double()` | safe type conversion |
| Core (arithmetic) | `+`, `-`, `*`, `/` on integers | overflow detection |

## Recommendation

**Add when making math builtins generic across numeric types.** Currently most
builtins only handle `f64`. When we add `Complex` support or want integer-specific
math, num-traits provides the generic foundation.

**Effort:** Trivial to add, gradual adoption in math builtins.
