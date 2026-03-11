# nalgebra integration plan

> `nalgebra` 0.34 — General-purpose linear algebra library.
> <https://github.com/dimforge/nalgebra>

## What it does

Matrix and vector types with compile-time or runtime dimensions. Decompositions:
QR, LU, SVD, Cholesky, Schur, Hessenberg, eigendecomposition. Solvers for linear
systems. Pure Rust, optional BLAS/LAPACK acceleration.

Key types:

- `DMatrix<f64>` — dynamically-sized matrix (what R uses)
- `DVector<f64>` — dynamically-sized vector
- Decompositions: `QR`, `LU`, `SVD`, `Cholesky`, `Eigen`, `Schur`

## Where it fits in miniR

### 1. Matrix operations

| R function | nalgebra equivalent |
|---|---|
| `%*%` (matrix multiply) | `a * b` or `a.mul_to(&b, &mut c)` |
| `t()` (transpose) | `m.transpose()` |
| `solve(A, b)` | `a.lu().solve(&b)` or `a.qr().solve(&b)` |
| `solve(A)` (inverse) | `a.try_inverse()` |
| `det(A)` | `a.determinant()` |
| `qr(A)` | `a.qr()` → `.q()`, `.r()` |
| `svd(A)` | `a.svd(true, true)` → `.u`, `.v_t`, `.singular_values` |
| `chol(A)` | `a.cholesky()` → `.l()` |
| `eigen(A)` | `a.symmetric_eigen()` or `a.eigenvalues()` |
| `crossprod(A)` | `a.tr_mul(&a)` (= t(A) %*% A) |
| `tcrossprod(A)` | `a.mul_to(&a.transpose(), &mut c)` |
| `norm(A, type)` | `a.norm()`, `a.norm1()`, `a.norm_inf()` |
| `rcond(A)` | via condition number estimation |
| `kappa(A)` | `a.svd().singular_values` → max/min ratio |

### 2. Matrix construction

| R function | nalgebra equivalent |
|---|---|
| `matrix(data, nrow, ncol)` | `DMatrix::from_vec(nrow, ncol, data)` |
| `diag(n)` | `DMatrix::identity(n, n)` |
| `diag(x)` | `DMatrix::from_diagonal(&DVector::from_vec(x))` |
| `rbind(A, B)` | stack rows |
| `cbind(A, B)` | stack columns |

### 3. Statistical operations built on linear algebra

- `lm()` (linear regression) → QR decomposition + solve
- `cor()` / `cov()` → matrix operations
- `prcomp()` / `princomp()` (PCA) → SVD

## Relationship to builtins plan

| Phase | Builtins affected | Impact |
|---|---|---|
| Phase 8 (linalg) | `%*%`, `solve()`, `det()`, `t()` | core matrix operations |
| Phase 8 (linalg) | `qr()`, `svd()`, `chol()`, `eigen()` | decompositions |
| Phase 8 (linalg) | `crossprod()`, `tcrossprod()`, `norm()` | derived operations |
| Statistics | `lm()`, `cor()`, `cov()`, `prcomp()` | stat models built on linalg |

## Recommendation

**Add when implementing Phase 8 (linear algebra).** nalgebra is the most complete
pure-Rust linear algebra library. For BLAS-accelerated performance, can optionally
link to OpenBLAS via `openblas-src`.

**Effort:** 3-4 hours for core matrix ops, additional sessions for decompositions.

**Alternatives:**

- `ndarray` + `ndarray-linalg` — more NumPy-like API but less complete decompositions
- `faer` — newer, very fast, but less mature ecosystem
