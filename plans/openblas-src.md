# openblas-src integration plan

> `openblas-src` — OpenBLAS source distribution (build dependency).
> <https://github.com/blas-lapack-rs/openblas-src>

## What it does

Compiles OpenBLAS from C/Fortran source and links it into the Rust binary.
OpenBLAS is the de facto open-source BLAS/LAPACK implementation — heavily
optimized assembly kernels for every major CPU architecture, mature, battle-tested.

Used as a backend for `nalgebra-lapack`, `ndarray-linalg`, and other Rust
linear algebra crates via the `blas-src` / `lapack-src` trait system.

Features:

- CPU-specific optimized kernels (Haswell, Zen, Apple M-series, etc.)
- Multi-threaded by default
- Complete BLAS Level 1/2/3 + full LAPACK
- 30+ years of optimization and correctness testing

## Where it fits in miniR

### 1. BLAS backend for nalgebra

nalgebra can use OpenBLAS via `nalgebra-lapack`:

```toml
[dependencies]
nalgebra-lapack = { version = "0.25", features = ["openblas"] }
openblas-src = { version = "0.10", features = ["static"] }
```

This accelerates `%*%`, `solve()`, `qr()`, `svd()`, `chol()`, `eigen()` —
everything in Phase 8 (linear algebra).

### 2. Maximum performance baseline

OpenBLAS is what GNU R links to. Using it gives us comparable (or identical)
matrix operation performance to R.

### 3. `crossprod()` / `tcrossprod()` — DSYRK

Optimized symmetric rank-k update, 2x faster than general GEMM for
`t(A) %*% A` patterns.

## Drawbacks

- **Build time** — compiles C/Fortran from source (~5 minutes)
- **Requires C compiler + Fortran** — not available on all platforms
- **Cross-compilation** — difficult, platform-specific build scripts
- **Binary size** — large static library

## Relationship to builtins plan

| Phase | Builtins affected | Impact |
|---|---|---|
| Phase 8 (linalg) | `%*%`, `solve()`, `qr()`, `svd()`, `chol()`, `eigen()` | BLAS-accelerated operations |
| Statistics | `lm()`, `glm()` | model fitting |

## Recommendation

**Consider as optional `--features blas` backend** for users who need maximum
performance and have a C/Fortran toolchain. The default should be a pure-Rust
backend (oxiblas or nalgebra's built-in) for easy builds.

**Effort:** 1 hour to wire up as nalgebra backend behind a feature flag.
