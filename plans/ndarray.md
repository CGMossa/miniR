# ndarray integration plan

> `ndarray` 0.17 — N-dimensional arrays for Rust.
> <https://github.com/rust-ndarray/ndarray>

## What it does

NumPy-like N-dimensional arrays. `Array1<T>`, `Array2<T>`, `ArrayD<T>` (dynamic
dimensions). Slicing, broadcasting, element-wise operations, matrix multiply.

```rust
use ndarray::{array, Array2};
let a = array![[1., 2.], [3., 4.]];
let b = a.dot(&a);  // matrix multiply
let c = &a + 1.0;   // broadcasting
```

## Where it fits in newr

### 1. R's array/matrix type

R arrays are vectors with a `dim` attribute. ndarray provides the N-dimensional
array operations:

```r
a <- array(1:24, dim = c(2, 3, 4))
a[1, 2, 3]     # indexing
apply(a, 2, sum) # margin operations
```

### 2. Overlap with nalgebra

ndarray focuses on N-dimensional arrays and element-wise operations.
nalgebra focuses on linear algebra decompositions.

For R:

- **ndarray** → `array()`, `dim()`, `apply()`, element-wise ops, broadcasting
- **nalgebra** → `solve()`, `qr()`, `svd()`, `%*%`

Some projects use both, or use `ndarray-linalg` to bridge them.

### 3. Broadcasting

R recycles vectors (broadcasting): `c(1,2,3) + c(10,20)` → `c(11,22,13)`.
ndarray has NumPy-style broadcasting which is similar but not identical to R's
recycling rules.

## Relationship to builtins plan

| Phase | Builtins affected | Impact |
|---|---|---|
| Phase 3 (collections) | `array()`, `dim()`, `apply()` | N-dimensional arrays |
| Phase 8 (linalg) | matrix operations | overlaps with nalgebra |

## Recommendation

**Consider as alternative or complement to nalgebra.** If we need N-dimensional
arrays (3D+), ndarray is the right choice. For just matrices (2D), nalgebra is
more complete for linear algebra.

**Decision point:** When implementing `array()` with `dim` attribute, decide
whether to use ndarray as the backing store or keep the current flat-vector-with-dim
approach.

**Effort:** Medium — depends on how deeply we integrate array semantics.
