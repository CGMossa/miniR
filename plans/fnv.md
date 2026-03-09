# fnv integration plan

> `fnv` 1.0 — Fowler-Noll-Vo hash function.
> Already vendored as a transitive dependency of reedline.

## What it does

A non-cryptographic hash function optimized for small keys. FNV-1a is faster than the default `SipHash` (Rust's default `HashMap` hasher) for keys shorter than ~20 bytes.

## Where it fits in newr

### The opportunity

R symbol names (variable names, function names) are typically short strings: `x`, `df`, `mean`, `data.frame`, `stringsAsFactors`. Our `Environment` uses `HashMap<String, RValue>` with the default SipHash hasher.

Since environment lookups happen on every variable reference and function call, switching to FNV could measurably speed up interpretation.

### Integration

```rust
use fnv::FnvHashMap;

struct EnvInner {
    bindings: FnvHashMap<String, RValue>,  // was HashMap<String, RValue>
    parent: Option<Environment>,
    name: Option<String>,
}
```

That's it — a one-line type change. `FnvHashMap` has the same API as `HashMap`.

### Tradeoffs

- **Pro**: Faster lookups for short keys (most R symbols)
- **Pro**: Zero API change — drop-in replacement
- **Con**: FNV is not HashDoS-resistant (SipHash is). Not a concern for an interpreter where the user controls all input.
- **Con**: Slightly slower for keys > ~20 bytes (rare for R symbols)

### Benchmark first

Before committing to this, benchmark with a real R script:

```rust
// In environment.rs, behind a feature flag or just test both
#[cfg(feature = "fnv")]
type BindingMap = FnvHashMap<String, RValue>;
#[cfg(not(feature = "fnv"))]
type BindingMap = HashMap<String, RValue>;
```

## Priority

Low — micro-optimization. Profile first to confirm environment lookup is a bottleneck. Already vendored, trivial to integrate.
