# nalgebra integration plan

> `nalgebra` 0.34 -- General-purpose linear algebra library.
> <https://github.com/dimforge/nalgebra>

## Current state

miniR already has a working linear algebra stack built on `ndarray` 0.17 plus
hand-written decompositions in `src/interpreter/builtins/math.rs` (~400 lines)
and `src/interpreter/ops.rs` (~70 lines for `%*%`).

### What we have today

| R builtin | Implementation | Location |
|---|---|---|
| `%*%` | ndarray `dot()` | ops.rs:578-640 |
| `t()` | manual column-major transpose | math.rs:3135 |
| `crossprod()` | ndarray `tr_mul` + `dot` | math.rs:2413 |
| `tcrossprod()` | ndarray `dot` + transpose | math.rs:2444 |
| `norm(x, type)` | manual O/I/F/M norms | math.rs:2477 |
| `solve(a)` / `solve(a,b)` | hand-rolled Gaussian elimination w/ partial pivoting | math.rs:2532 |
| `det(x)` | hand-rolled Gaussian elimination w/ partial pivoting | math.rs:2646 |
| `chol(x)` | hand-rolled Cholesky (upper triangular) | math.rs:2696 |
| `qr(x)` | hand-rolled Householder QR (no column pivoting) | math.rs:2734 |
| `svd(x)` | hand-rolled one-sided Jacobi SVD | math.rs:2836 |
| `eigen(x)` | hand-rolled Jacobi (symmetric only) | math.rs:2990 |
| `lm()` | normal equations via Gaussian elimination | math.rs:3540+ |

Shared infrastructure: `rvalue_to_array2()` and `array2_to_rvalue()` convert
between R's column-major `RValue` matrices and `ndarray::Array2<f64>`.

### Known limitations of hand-rolled code

- **QR has no column pivoting** -- R's `qr()` does pivoted QR by default
- **SVD uses one-sided Jacobi** -- O(n^4) convergence, fragile for
  ill-conditioned or non-square matrices; no thin/full distinction
- **eigen() is symmetric-only** -- non-symmetric matrices produce an error
  instead of complex eigenvalues
- **solve() has no condition number check** -- just a hard 1e-15 tolerance
- **No LU decomposition** exposed as a standalone builtin
- **No backsolve/forwardsolve** -- triangular solvers needed for efficient
  post-decomposition work
- **No rcond/kappa** -- condition number estimation
- **No Schur/Hessenberg** -- needed for general (non-symmetric) eigenproblems
- **lm() uses normal equations** -- numerically inferior to QR-based least
  squares

## What nalgebra would replace

The hand-written decompositions (lines 2532-3131 of math.rs, ~600 lines) would
be replaced by calls into nalgebra's battle-tested implementations:

| Current hand-rolled code | nalgebra replacement |
|---|---|
| Gaussian elimination (solve, det) | `DMatrix::lu()` with partial pivoting |
| Cholesky | `DMatrix::cholesky()` |
| Householder QR (no pivoting) | `DMatrix::qr()` (or `col_piv_qr()` for pivoting) |
| One-sided Jacobi SVD | `DMatrix::svd()` -- uses bidiagonalization + implicit QR, thin/full |
| Jacobi symmetric eigen | `DMatrix::symmetric_eigen()` for symmetric, `DMatrix::schur()` + eigenvalue extraction for general |
| Gaussian elimination in lm() | `QR::solve()` for numerically stable least squares |

ndarray would be **kept** for `%*%`, `crossprod`, `tcrossprod`, and as the
internal matrix storage for `rvalue_to_array2()` / `array2_to_rvalue()`. It is
lightweight (~45KB compiled), already wired into matmul and the conversion
helpers, and there is no reason to rip it out. nalgebra is **additive** --
it supplements ndarray for decompositions and solvers.

## What nalgebra would add (not currently possible)

- **LU decomposition with partial pivoting** -- `lu()` exposes P, L, U factors;
  enables `solve()`, `det()`, `try_inverse()` all from one factorization
- **Column-pivoted QR** -- `col_piv_qr()` gives the pivoted QR that R's `qr()`
  actually computes, including rank estimation
- **Bidiagonal SVD** -- O(mn^2) for m >= n, much faster and more stable than
  Jacobi; supports thin decomposition (`svd(compute_u, compute_v)`)
- **General (non-symmetric) eigendecomposition** -- via Schur decomposition;
  produces complex eigenvalues for non-symmetric real matrices
- **Schur decomposition** -- `schur()` gives the real Schur form (quasi-upper
  triangular), useful for matrix exponential, matrix logarithm
- **Hessenberg form** -- `hessenberg()` reduces to upper Hessenberg, a
  preprocessing step for eigenvalue algorithms
- **Condition number estimation** -- enables `rcond()` and `kappa()` builtins
- **backsolve/forwardsolve** -- triangular solvers via `solve_lower_triangular`
  and `solve_upper_triangular`
- **Matrix exponential / log / sqrt** -- building blocks for advanced stats

## Architecture: optional dep alongside ndarray

```
ndarray (required)          nalgebra (optional, feature-gated)
  |                           |
  +-- %*%, crossprod,         +-- solve, det, chol (LU-based)
  |   tcrossprod              +-- qr (col-pivoted)
  +-- rvalue_to_array2        +-- svd (bidiagonal)
  +-- array2_to_rvalue        +-- eigen (symmetric + general)
  +-- lm (matmul parts)       +-- lm (QR-based solver)
                              +-- rcond, kappa, backsolve, forwardsolve
                              +-- schur, hessenberg (new builtins)
```

- Add `nalgebra = { version = "0.34", optional = true }` to Cargo.toml
- Add feature `linalg = ["dep:nalgebra"]` and include it in `default`
- Gate decomposition builtins with `#[cfg(feature = "linalg")]`
- **Keep hand-rolled fallbacks** behind `#[cfg(not(feature = "linalg"))]` so
  the interpreter still works (with limitations) without nalgebra
- Conversion: `rvalue_to_dmatrix()` / `dmatrix_to_rvalue()` helpers that go
  directly from R column-major to `DMatrix<f64>` (nalgebra is also column-major
  natively, so this is zero-reorder)

## Build cost assessment

nalgebra 0.34 pulls in:
- `nalgebra` itself (~large crate, ~50k lines)
- `simba` (SIMD-compatible math traits)
- `num-traits`, `num-complex`, `num-rational` (already have `num-complex`)
- `matrixmultiply` (optimized BLAS-like matmul kernel)
- `typenum` (compile-time type-level integers for fixed-size matrices)
- `approx` (approximate floating-point comparison)

Estimated incremental compile time: **20-40 seconds** for a clean build of
nalgebra on a modern machine. Incremental rebuilds after initial compilation
are fast since nalgebra is a dependency, not modified code.

nalgebra is one of the largest pure-Rust crates. However:
- It compiles fully with `cargo build` (no C dependencies, no system BLAS)
- The `DMatrix` path is what we use -- the const-generic paths add code but
  not runtime cost
- Optional feature gating means users who don't need linalg skip the cost

## Recommendation

**Add nalgebra as an optional dependency.** The hand-rolled decompositions work
for basic cases but have real correctness gaps (no pivoted QR, symmetric-only
eigen, O(n^4) SVD). nalgebra closes all of these gaps with production-quality
implementations while keeping the build pure-Rust and cross-platform.

Do not remove ndarray -- it serves the matmul / conversion layer well and is
much lighter. The two libraries coexist naturally: ndarray for storage and
element-wise ops, nalgebra for factorizations and solvers.

## Implementation order

1. Add `nalgebra` as optional dep, wire up `rvalue_to_dmatrix()` / `dmatrix_to_rvalue()` conversion helpers
2. Replace `solve()` and `det()` with LU-based implementations
3. Replace `chol()` with nalgebra Cholesky
4. Replace `qr()` with column-pivoted QR; add `qr.Q()`, `qr.R()`, `qr.coef()`, `qr.solve()` helpers
5. Replace `svd()` with bidiagonal SVD (thin + full support)
6. Replace `eigen()` with symmetric_eigen + Schur-based general eigen (complex eigenvalues)
7. Switch `lm()` from normal equations to QR-based least squares
8. Add new builtins: `backsolve()`, `forwardsolve()`, `rcond()`, `kappa()`
9. Add new builtins: `schur()`, `hessenberg()` (lower priority, needed for matrix functions)
