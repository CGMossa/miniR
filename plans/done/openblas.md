# oxiblas integration plan

> `oxiblas` 0.2 — Pure Rust BLAS/LAPACK implementation.
> <https://lib.rs/crates/oxiblas>

## What it does

Complete BLAS and LAPACK in pure Rust — no C compiler, no system OpenBLAS/MKL
required. SIMD-optimized for x86_64 (AVX) and AArch64 (NEON).

Capabilities:

- **BLAS Level 1** — dot, nrm2, asum, axpy, scal, swap, copy
- **BLAS Level 2** — gemv, ger, trsv, symv (matrix-vector operations)
- **BLAS Level 3** — gemm, trsm, syrk (matrix-matrix operations)
- **LAPACK** — LU, QR, SVD, Cholesky, eigenvalue decompositions
- **Sparse matrices** — 9 formats (CSR, CSC, COO, etc.) with iterative solvers
- **Tensor operations** — Einstein summation, batched operations
- **Extended precision** — f16 and f128 support

Performance: 80-172% of OpenBLAS depending on platform and operation.
On macOS M3, DGEMM matches OpenBLAS at 101% relative performance.

```rust
use oxiblas::blas::dgemm;
// C = alpha * A * B + beta * C
dgemm(m, n, k, alpha, &a, lda, &b, ldb, beta, &mut c, ldc);
```

## Why oxiblas over openblas-src

| | oxiblas | openblas-src |
|---|---|---|
| Language | Pure Rust | C + Fortran |
| Build | `cargo build` just works | Needs C compiler, Fortran, ~5min compile |
| Portability | Cross-compiles trivially | Platform-specific build scripts |
| Safety | Safe Rust API | FFI bindings (unsafe) |
| Performance | 80-172% of OpenBLAS | Reference implementation |
| Dependencies | Zero external | libgfortran, system BLAS |

For miniR, the pure-Rust approach aligns with our build philosophy (vendored deps,
`cargo build` just works).

## Where it fits in miniR

### 1. Matrix multiply (`%*%`) — DGEMM

The hot path for linear algebra. R's `%*%` on large matrices needs BLAS-speed GEMM:

```r
A <- matrix(rnorm(1e6), 1000, 1000)
B <- matrix(rnorm(1e6), 1000, 1000)
C <- A %*% B  # needs DGEMM
```

nalgebra's pure-Rust GEMM is decent but oxiblas's SIMD-optimized DGEMM is faster
for large matrices.

### 2. `solve()` — LU decomposition

```r
solve(A, b)  # solves Ax = b via LU factorization
solve(A)     # matrix inverse via LU
```

oxiblas provides LAPACK's `dgetrf` (LU factorization) + `dgetrs` (solve).

### 3. `qr()`, `svd()`, `chol()`, `eigen()` — decompositions

| R function | LAPACK routine via oxiblas |
|---|---|
| `qr()` | `dgeqrf` (QR factorization) |
| `svd()` | `dgesvd` (singular value decomposition) |
| `chol()` | `dpotrf` (Cholesky factorization) |
| `eigen()` | `dsyev` / `dgeev` (eigendecomposition) |
| `det()` | via LU: `dgetrf` then product of diagonal |
| `norm()` | `dlange` (matrix norms) |
| `rcond()` | `dgecon` (reciprocal condition number) |

### 4. `crossprod()` / `tcrossprod()` — DSYRK

```r
crossprod(A)   # t(A) %*% A — symmetric, use DSYRK
tcrossprod(A)  # A %*% t(A) — symmetric, use DSYRK
```

DSYRK is 2x faster than DGEMM for symmetric products.

### 5. Sparse matrix support

oxiblas includes sparse matrix formats (CSR, CSC, COO) with iterative solvers.
This maps to R's `Matrix` package sparse matrices — useful for large statistical
models.

### Integration with nalgebra

Two approaches:

1. **Use oxiblas directly** for BLAS/LAPACK calls, with our own matrix wrapper
2. **Use nalgebra + oxiblas as backend** if nalgebra adds oxiblas support

Approach 1 is simpler and avoids nalgebra's abstraction overhead for direct
BLAS operations.

## Relationship to builtins plan

| Phase | Builtins affected | Impact |
|---|---|---|
| Phase 8 (linalg) | `%*%`, `crossprod()`, `tcrossprod()` | BLAS-accelerated multiply |
| Phase 8 (linalg) | `solve()`, `det()`, `rcond()` | LU decomposition |
| Phase 8 (linalg) | `qr()`, `svd()`, `chol()`, `eigen()` | all decompositions |
| Phase 8 (linalg) | `norm()`, `kappa()` | matrix norms/condition |
| Statistics | `lm()`, `glm()` | model fitting via QR/Cholesky |

## Recommendation

**Use instead of openblas-src.** Pure Rust, no build hassle, competitive
performance. Add when implementing Phase 8 (linear algebra).

Can start with nalgebra for the API layer and swap in oxiblas BLAS calls for
hot paths, or use oxiblas directly and skip nalgebra's overhead.

**Effort:** 2-3 hours to wire up BLAS/LAPACK calls for core matrix operations.
