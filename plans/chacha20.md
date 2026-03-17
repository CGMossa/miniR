# chacha20 integration plan

> `chacha20` 0.10 -- ChaCha20 stream cipher with rand_core-compatible RNG.
> <https://github.com/RustCrypto/stream-ciphers>

## What it does

ChaCha20 is a cryptographically secure stream cipher. The `rng` feature
provides `ChaCha20Rng`, a seedable CSPRNG that implements `rand_core::RngCore`
and `rand_core::SeedableRng`.

```rust
use chacha20::rng::ChaCha20Rng;
use rand_core::SeedableRng;

let mut rng = ChaCha20Rng::seed_from_u64(42);
```

Variants: ChaCha8Rng (faster, less rounds), ChaCha12Rng, ChaCha20Rng (most secure).

## Where it fits in miniR

### `set.seed()` -- reproducible random number generation

R's `set.seed(42)` must produce the same sequence of random numbers every time.
This requires a seedable, deterministic RNG. The default `rand::rng()` uses
`ThreadRng` which is not seedable.

With chacha20:

```rust
// set.seed(42) in R
let rng = ChaCha20Rng::seed_from_u64(42);
interpreter.set_rng(rng);

// rnorm(3) now produces deterministic output
```

ChaCha20 is the standard choice for seedable RNG in the Rust ecosystem.
It is what `rand_chacha` provides, but `chacha20` is the underlying crate
and is already vendored as a transitive dependency of `rand`.

### Per-interpreter RNG state

The `ChaCha20Rng` struct lives on the `Interpreter` struct, satisfying the
no-global-state rule. Each interpreter gets its own RNG, seeded independently.

```rust
struct Interpreter {
    rng: ChaCha20Rng,
    // ...
}
```

### `.Random.seed` -- R's RNG state save/restore

R can save and restore RNG state via `.Random.seed`. ChaCha20Rng is
serializable (256-bit seed + 64-bit counter), so we can expose this
as an integer vector in R.

## Relationship to rand plan

The rand plan (plans/rand.md) mentions needing `rand_chacha` for `set.seed()`.
`chacha20` with the `rng` feature is the same thing -- it provides
`ChaCha20Rng` directly. Since it is already vendored as a transitive dep
of `rand`, no new dependency is needed.

## Implementation

1. Enable `rng` feature on `chacha20` (may already be enabled transitively)
2. Store `ChaCha20Rng` on `Interpreter` struct
3. Implement `set.seed(n)` to reseed the interpreter's RNG
4. Wire all `r*()` functions (rnorm, runif, etc.) to use `interpreter.rng`
5. Implement `.Random.seed` save/restore

## Priority

High -- this is a prerequisite for reproducible `set.seed()`, which is
expected by virtually all R code that uses random numbers.
