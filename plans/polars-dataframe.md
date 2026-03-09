# Polars Data Frame Backend

Replace our hand-rolled data frame (list-with-class-attr) with polars `DataFrame` as the backing store. This gives us columnar storage, zero-copy slicing, lazy evaluation, and vectorized operations for free.

## Dependency

```toml
polars = { version = "0.53", default-features = false, features = [
  # --- core ---
  "fmt",                  # Display/Debug for DataFrame — needed for print.data.frame
  "csv",                  # read.csv / write.csv via polars CsvReader/CsvWriter
  "lazy",                 # LazyFrame — needed for filter/select/group_by/join chains
  "rows",                 # Row iteration — needed for df[i,] row indexing
  "dataframe_arithmetic", # df + df, df * scalar — needed for arithmetic on frames
  "zip_with",             # Conditional column selection — needed for ifelse on columns
  "describe",             # summary() — descriptive statistics per column
  "partition_by",         # split() — split DataFrame by column values
  "diagonal_concat",      # rbind() — vertical concatenation of frames
  "product",              # prod() aggregation
  "dot_product",          # crossprod, %*% between Series
  "concat_str",           # paste/paste0 on columns

  # --- column operations ---
  "abs",                  # abs()
  "round_series",         # round(), signif()
  "cum_agg",              # cumsum, cumprod, cummax, cummin
  "diff",                 # diff()
  "rank",                 # rank()
  "is_in",                # %in% operator on columns
  "unique_counts",        # table() / tabulate()
  "mode",                 # statistical mode
  "sign",                 # sign()
  "log",                  # log, log2, log10 on columns
  "trigonometry",         # sin, cos, tan, asin, acos, atan on columns
  "is_first_distinct",    # duplicated() — first occurrence
  "is_last_distinct",     # duplicated(fromLast=TRUE)
  "is_unique",            # !duplicated()

  # --- joins ---
  "cross_join",           # merge(all=TRUE) / expand.grid
  "semi_anti_join",       # merge anti-join variants, %in% filtering
  "asof_join",            # time-based joins (for future ts support)

  # --- string ops ---
  "strings",              # String column operations (grep/gsub on columns)
  "regex",                # Regex support in polars expressions
  "string_pad",           # formatC, str_pad equivalent
  "string_reverse",       # stri_reverse equivalent
  "string_to_integer",    # as.integer on string columns

  # --- types ---
  "dtype-slim",           # date, datetime, duration types
  "dtype-struct",         # struct columns (for nested data)
  "dtype-categorical",    # factor() support

  # --- I/O ---
  "parquet",              # read/write parquet (common in data science)
  "json",                 # fromJSON / toJSON
  "decompress",           # gzipped CSV etc.

  # --- stats ---
  "cov",                  # cor() / cov()
  "interpolate",          # approx() / na.approx
  "rolling_window",       # rollapply, zoo::rollmean
  "ewma",                 # exponential weighted moving average
  "moment",               # skewness, kurtosis

  # --- reshaping ---
  "pivot",                # reshape(), tidyr::pivot_wider/longer
  "to_dummies",           # model.matrix, one-hot encoding
  "top_k",                # head/tail optimization

  # --- misc ---
  "coalesce",             # coalesce (ifelse with NAs)
  "repeat_by",            # rep() on columns
  "range",                # seq() / range operations
  "ndarray",              # Bridge to ndarray crate (we already use it for matrices)
  "search_sorted",        # findInterval()
  "replace",              # replace() / ifelse value replacement
  "hist",                 # hist() data (bin counts)
  "cutqcut",              # cut() / quantile-based binning
  "pct_change",           # percentage change (diff/lag ratio)
  "index_of",             # match() — first index of value
  "is_between",           # between() / findInterval range check

  # --- serialization ---
  "serde",                # Needed for saveRDS/readRDS (future)
]}
```

## Features NOT included (and why)

| Feature | Reason |
|---------|--------|
| `simd`, `avx512`, `nightly` | Platform-specific optimizations — add later if perf matters |
| `performant` | Slower builds, marginal gains for interpreter use |
| `cloud`, `aws`, `azure`, `gcp`, `http`, `async` | Cloud storage — not an R interpreter concern |
| `object` | Custom object types in polars — we use RValue |
| `bigidx` | 64-bit indices — overkill for now |
| `polars_cloud_*`, `ir_serde` | Cloud computing features |
| `docs`, `test`, `bench` | Build-time only features |
| `ipc`, `ipc_streaming`, `avro`, `scan_lines` | Niche file formats — add on demand |
| `temporal` | Pulls in polars-time; `dtype-slim` gives us date types without the full time machinery |
| `random` | We handle RNG ourselves (rnorm, runif, etc.) |
| `serde-lazy` | Full lazy serde — overkill, basic `serde` suffices |
| `sql` | SQL on DataFrames — not an R primitive |
| `dynamic_group_by` | Time-based grouping — defer |
| `business` | Business day calculations — defer |
| `timezones` | Timezone support — defer to when we implement POSIXct properly |
| `binary_encoding`, `bitwise` | We handle bitwise in R-level builtins |
| `extract_jsonpath` | JSON path — niche |
| `list_*`, `array_*` | List/array column ops — complex, defer |
| `new_streaming` | New streaming engine — experimental |
| `fused`, `chunked_ids` | Internal optimizations |
| `dtype-full` / exotic dtypes | i8, i16, i128, u8, u16, u128, f16, decimal, extension — not needed for R's 4 atomic types + factor |

## Architecture

### New RValue variant

```
RValue::DataFrame(PolarsDataFrame)
```

Where `PolarsDataFrame` wraps `polars::frame::DataFrame` with:
- Cached class/names/row.names attributes for R compatibility
- Lazy conversion to/from `RValue::List` for R code that treats data frames as lists

### Type mapping

| R type | Polars dtype |
|--------|-------------|
| `integer` | `Int64` |
| `double` | `Float64` |
| `character` | `String` (Utf8) |
| `logical` | `Boolean` |
| `factor` | `Categorical` |
| `Date` | `Date` |
| `POSIXct` | `Datetime` |
| `NA` | null in each column |

### Conversion boundaries

- `data.frame(...)` → construct polars DataFrame directly from column vectors
- `as.data.frame(matrix)` → convert ndarray matrix to polars DataFrame
- `as.list(df)` → extract columns as RValue::Vector list (materialization point)
- `df$col` → extract single Series, convert to RValue::Vector
- `df[i, j]` → use polars slicing/filtering, return DataFrame or Vector
- `df[logical, ]` → convert logical to polars boolean Series, filter
- `print(df)` → use polars `fmt` display

### Implementation order

1. Add polars dependency with features above
2. Create `PolarsDataFrame` wrapper type in `value.rs`
3. Rewrite `data.frame()` to construct polars DataFrames
4. Implement `$`, `[[`, `[,]` indexing on the new type
5. Rewrite `read.csv` / `write.csv` to use polars CsvReader/CsvWriter
6. Add `merge()`, `subset()`, `with()` using polars joins/filter/eval
7. Add `aggregate()` / `tapply()` using polars `group_by`
8. Add `summary()` using polars `describe`
9. Bridge `rbind` / `cbind` to polars `concat` / `hstack`
10. Factor support via `Categorical` dtype
11. Parquet/JSON I/O

### Backward compatibility

Keep the old list-based data frame path as a fallback — if polars construction fails or data contains types polars can't represent (e.g., nested lists), fall back to the current list-with-attrs approach. This means `is.data.frame()` must check both `RValue::DataFrame` and list-with-class.
