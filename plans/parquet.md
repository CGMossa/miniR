# Parquet integration plan

> `parquet` 58.0 — Apache Parquet columnar storage format.
> https://lib.rs/crates/parquet
> Apache-2.0. Part of the arrow-rs project.

## What it does

Reads and writes Apache Parquet files — the standard columnar file format for analytics. Parquet is to data frames what CSV is to text: the default interchange format for structured data in Python (pandas/polars), R (arrow), Spark, DuckDB, etc.

Key capabilities:
- Column-oriented storage with per-column compression
- Schema-embedded (self-describing files)
- Predicate pushdown / column pruning (read only what you need)
- Multiple compression codecs: zstd, snappy, lz4, gzip, brotli

## Feature flags

```toml
# ALL features (with defaults marked)
parquet = { version = "58.0", default-features = false, features = [
    # "arrow",       # (default) Arrow array integration — read/write Arrow RecordBatch
    # "brotli",      # (default) Brotli compression codec
    # "flate2",      # (default) Gzip/deflate compression codec
    # "lz4",         # (default) LZ4 compression codec
    # "zstd",        # (default) Zstandard compression codec
    # "snap",        # (default) Snappy compression codec
    # "simdutf8",    # (default) SIMD-accelerated UTF-8 validation
    # "async",       # Async/tokio-based I/O
    # "json",        # JSON serialization of Parquet schema
    # "cli",         # CLI tools (parquet-read, parquet-schema, etc.)
    # "crc",         # CRC checksums for page verification
    # "experimental",# Experimental features
    # "encryption",  # Parquet modular encryption
    # "variant_experimental", # Variant type support
    # "geospatial",  # GeoParquet support
] }
```

### Recommended configuration for newr

```toml
parquet = { version = "58.0", default-features = false, features = [
    "arrow",    # required — we read into Arrow arrays, then convert to RValue
    "zstd",     # most common codec in modern parquet files
    "snap",     # snappy — second most common, fast
    "flate2",   # gzip — needed for older files
] }
```

**Why not defaults?** Dropping `brotli` (rare in parquet files, heavy build), `lz4` (less common than zstd/snap), and `simdutf8` (marginal benefit for our use case) trims compile time and vendor size. Can add back if needed.

**Why `arrow` feature?** Parquet without Arrow gives raw column chunks. With Arrow, you get `RecordBatch` → trivial conversion to R data frames. This is how R's `arrow::read_parquet()` works internally.

## Where it fits in newr

### 1. `read.parquet()` / `write.parquet()`

Not standard R builtins (they come from the `arrow` package), but essential for modern R workflows:

```rust
#[builtin(name = "read.parquet", min_args = 1)]
fn builtin_read_parquet(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let path = args[0].as_character_scalar()?;
    let file = File::open(&path)?;
    let reader = ParquetRecordBatchReader::try_new(file, /* batch_size */ 1024)?;

    // Collect all batches, convert columns to R vectors
    for batch in reader {
        let batch = batch?;
        for (i, col) in batch.columns().iter().enumerate() {
            // Arrow array → R vector conversion
            // Int32Array → Vector::Integer
            // Float64Array → Vector::Double
            // StringArray → Vector::Character
            // BooleanArray → Vector::Logical
            // null → NA
        }
    }
    // Assemble into data frame (RList with class "data.frame", row.names attr)
}
```

### 2. `write.parquet(df, path)`

```rust
#[builtin(name = "write.parquet", min_args = 2)]
fn builtin_write_parquet(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let df = &args[0];  // must be a data frame (list with "data.frame" class)
    let path = args[1].as_character_scalar()?;

    // Convert R columns → Arrow arrays
    // Build RecordBatch → write with ArrowWriter
    let schema = /* build from column names and types */;
    let batch = /* convert R vectors to Arrow arrays */;
    let file = File::create(&path)?;
    let mut writer = ArrowWriter::try_new(file, schema, None)?;
    writer.write(&batch)?;
    writer.close()?;
    Ok(RValue::Null)
}
```

### 3. Column selection (predicate pushdown)

```r
# Read only specific columns — parquet excels at this
read.parquet("data.parquet", columns = c("name", "age"))
```

Parquet's columnar format means reading 2 columns from a 100-column file is nearly free. This is a major advantage over CSV.

## Type mapping

| Parquet type | Arrow type | R type |
|-------------|-----------|--------|
| INT32 | Int32Array | Vector::Integer |
| INT64 | Int64Array | Vector::Double (R has no 64-bit int) |
| FLOAT | Float32Array | Vector::Double |
| DOUBLE | Float64Array | Vector::Double |
| BOOLEAN | BooleanArray | Vector::Logical |
| BYTE_ARRAY (UTF8) | StringArray | Vector::Character |
| null | null bitmap | NA values |
| DATE32 | Date32Array | (future: Date class) |
| TIMESTAMP | TimestampArray | (future: POSIXct class) |

## Relationship to other plans

### arrow.md

Parquet depends on Arrow (the `arrow` feature). Adding parquet effectively adds Arrow too. This means:
- `read.csv()` can also use Arrow's CSV reader (faster than our csv crate for large files)
- Data frame internals could optionally use Arrow RecordBatch
- **Decision:** Add parquet+arrow together; they share the dependency tree

### polars-dataframe.md

Polars includes its own parquet support via the `"parquet"` feature flag. Two paths:

| Approach | Pros | Cons |
|----------|------|------|
| **parquet crate directly** | Lighter, we control conversion | Must write Arrow↔RValue conversion ourselves |
| **Polars with parquet feature** | Free if we use Polars for data frames | Heavy dep, couples I/O to data frame backend |

**Recommendation:** Use the `parquet` crate directly. It's lighter than pulling all of Polars, and we need Arrow↔RValue conversion anyway for data frames. If we later adopt Polars as the data frame backend, we can swap the internals.

### serde.md — RDS serialization

R's `saveRDS()` / `readRDS()` uses R's internal XDR binary format — completely unrelated to Parquet. Two separate systems:

| Format | Use case | Crate |
|--------|----------|-------|
| Parquet | Columnar analytics data (data frames) | `parquet` |
| RDS | Arbitrary R objects (any RValue) | Custom format + `serde` |
| JSON | Interchange, config | `serde_json` |
| CSV | Text tabular data | `csv` (already vendored) |

For RDS, we need to either:
1. Implement R's actual RDS binary format (for compatibility with GNU R)
2. Define our own binary format using serde (simpler, not GNU R compatible)

**Recommendation:** Start with our own serde-based format (`.mrds`?) for save/load. Add GNU R RDS compatibility later if needed for package testing.

## Implementation order

1. Add `parquet = { version = "58.0", default-features = false, features = ["arrow", "zstd", "snap", "flate2"] }` to Cargo.toml
2. Vendor (`just vendor`)
3. Implement Arrow array → R vector conversion helpers (shared with future data frame work)
4. Implement `read.parquet(file, columns)` builtin
5. Implement `write.parquet(df, file)` builtin
6. Add to TODO.md / DONE.md

## Dependency weight

The `parquet` crate with `arrow` feature pulls in ~30-40 transitive crates (arrow-array, arrow-buffer, arrow-schema, arrow-data, arrow-select, arrow-ipc, plus compression libs). This is significant but justified — Parquet is the standard data exchange format and every serious R workflow touches it.

## Priority

Medium — Parquet support is a "wow factor" feature that immediately makes newr useful for real data science workflows. But it depends on having at least basic data frame support first (a list with class "data.frame" and column vectors). Implement after `data.frame()` constructor works.
