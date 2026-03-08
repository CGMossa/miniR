# rayon integration plan

> `rayon` 1.11 — Data parallelism library.
> https://github.com/rayon-rs/rayon

## What it does

Drop-in parallel iterators. Replace `.iter()` with `.par_iter()` to parallelize.

```rust
use rayon::prelude::*;
let sum: f64 = values.par_iter().map(|x| x * x).sum();
```

Also: `par_sort()`, `par_extend()`, parallel `join()` for fork-join.
Thread pool is global, work-stealing scheduler.

## Where it fits in newr

### 1. Vectorized math — parallel element-wise operations

R's vectorized operations on large vectors can be parallelized:

```r
x <- rnorm(1e7)
y <- sqrt(x^2 + 1)  # 10M element-wise operations
```

```rust
Vector::Double(vals) => {
    let result: Vec<Option<f64>> = vals.par_iter()
        .map(|x| x.map(|v| (v * v + 1.0).sqrt()))
        .collect();
    Vector::Double(result.into())
}
```

### 2. `sapply()` / `lapply()` — parallel apply

For pure functions over large vectors, parallel apply is safe:

```r
sapply(1:1e6, function(x) x^2 + 1)
```

### 3. `rowSums()` / `colSums()` — parallel aggregation

Matrix row/column operations over large matrices benefit from parallelism.

### 4. Sorting — `par_sort()`

R's `sort()` on large vectors can use rayon's parallel sort.

### Caveats

- Only safe for pure operations (no environment mutation)
- Overhead makes it slower for small vectors (<10K elements)
- R's single-threaded semantics mean shared mutable state is tricky

## Relationship to builtins plan

| Phase | Builtins affected | Impact |
|---|---|---|
| Phase 1 (math) | `sqrt()`, `abs()`, `exp()`, `log()`, arithmetic | parallel vectorized math |
| Phase 3 (collections) | `sapply()`, `lapply()`, `sort()` | parallel apply/sort |
| Phase 3 (collections) | `rowSums()`, `colSums()`, `rowMeans()` | parallel aggregation |

## Recommendation

**Add when we have benchmarks showing vectorized operations are a bottleneck.**
Don't prematurely parallelize — the overhead of thread coordination is significant
for small inputs. Add a threshold (e.g. >100K elements) before switching to par_iter.

**Effort:** 1-2 hours to add parallel paths with size thresholds.
