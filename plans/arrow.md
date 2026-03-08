# arrow integration plan

> `arrow` 58.0 — Apache Arrow columnar format implementation.
> https://github.com/apache/arrow-rs

## What it does

In-memory columnar data format. Zero-copy reads, SIMD-accelerated operations,
interoperability with Parquet, CSV, JSON. Used by DataFusion, Polars, DuckDB.

Key types:
- `Array` trait — typed columnar arrays (Int32Array, Float64Array, StringArray, etc.)
- `RecordBatch` — named columns (like a data frame)
- `Schema` — column names and types
- Arrow IPC — binary serialization format
- CSV/JSON/Parquet readers built on Arrow arrays

## Where it fits in newr

### 1. Data frame backend

R data frames are currently lists of vectors. Arrow's `RecordBatch` is the
natural high-performance replacement:

```rust
// Current: list of Vec<Option<T>>
// Arrow: RecordBatch with typed arrays
let batch = RecordBatch::try_new(schema, vec![
    Arc::new(Float64Array::from(vec![1.0, 2.0, 3.0])),
    Arc::new(StringArray::from(vec!["a", "b", "c"])),
])?;
```

Benefits:
- Columnar layout → cache-friendly aggregations
- Null bitmap instead of `Option<T>` → compact NA representation
- SIMD operations on numeric columns
- Zero-copy sharing between R and external tools

### 2. `read.csv()` / `write.csv()` — Arrow CSV reader

Arrow includes a high-performance CSV reader that returns Arrow arrays directly.
Much faster than line-by-line parsing.

### 3. Parquet support

Arrow-rs includes Parquet reader/writer. R's `arrow::read_parquet()` equivalent.

### 4. Interop with DuckDB, Polars

Arrow IPC enables zero-copy data exchange with other analytics tools.

### Challenges

- Arrow arrays are immutable — R's copy-on-modify requires careful handling
- Type mapping: R's `NA` maps to Arrow nulls, but R has only 5 atomic types
  while Arrow has dozens
- Heavy dependency (~50+ crates in the dependency tree)

## Relationship to builtins plan

| Phase | Builtins affected | Impact |
|---|---|---|
| Phase 11 (I/O) | `read.csv()`, `write.csv()`, `read.table()` | fast CSV I/O |
| Core (data frames) | `data.frame()`, `[.data.frame`, `$`, `subset()` | columnar backend |
| Phase 3 (collections) | `colSums()`, `rowSums()`, `aggregate()` | SIMD aggregation |

## Recommendation

**Add when implementing data frames as a first-class type.** Arrow is the right
long-term choice for columnar data but is a heavy dependency. Start with simple
list-of-vectors data frames, migrate to Arrow when performance matters.

**Effort:** Major — 1-2 weeks for data frame type + Arrow integration.
