# Polars Data Frame Backend

Replace our hand-rolled data frame (list-with-class-attr) with polars `DataFrame` as the backing store. This gives us columnar storage, zero-copy slicing, lazy evaluation, and vectorized operations for free.

## Motivation

Our current data frame is an `RList` with `class = "data.frame"`, `names`, and `row.names` attributes. Every operation — filtering, column selection, aggregation — is implemented by hand in Rust, iterating element-by-element. This is:

- **Slow**: No SIMD, no columnar memory layout, no query optimization
- **Incomplete**: We're missing `merge()`, `aggregate()`, `reshape()`, `subset()` with complex predicates, `with()`, grouped operations, etc.
- **Fragile**: Every new operation is a bespoke implementation that needs to handle NA propagation, type coercion, name propagation, and row.names bookkeeping

Polars gives us all of this for free — it's a production-grade columnar DataFrame engine written in Rust, with lazy evaluation, predicate pushdown, and parallel execution.

## Dependency

```toml
polars = { version = "0.53", default-features = false, features = [
  # --- core ---
  "fmt",                  # Display/Debug for DataFrame — needed for print.data.frame
  "csv",                  # read.csv / write.csv via polars CsvReader/CsvWriter
  "lazy",                 # LazyFrame — needed for filter/select/group_by/join chains
  "rows",                 # Row iteration — needed for df[i,] row indexing and get_row()
  "dataframe_arithmetic", # df + df, df * scalar — needed for arithmetic on frames
  "zip_with",             # Conditional column selection — needed for ifelse on columns
  "partition_by",         # split() — split DataFrame by column values
  "diagonal_concat",      # rbind() — vertical concatenation of frames with different schemas
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
  "describe",             # summary() — descriptive statistics per column

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

```rust
pub enum RValue {
    Null,
    Vector(RVector),
    List(RList),
    Function(RFunction),
    Environment(Environment),
    Language(Box<Expr>),
    DataFrame(PolarsDataFrame),  // NEW
}
```

### PolarsDataFrame wrapper

```rust
use polars::prelude::DataFrame;

#[derive(Debug, Clone)]
pub struct PolarsDataFrame {
    pub inner: DataFrame,
    /// Cached row.names (polars has no row names concept)
    pub row_names: Option<Vec<Option<String>>>,
}

impl PolarsDataFrame {
    pub fn new(df: DataFrame) -> Self {
        PolarsDataFrame {
            inner: df,
            row_names: None,
        }
    }

    pub fn nrow(&self) -> usize {
        self.inner.height()
    }

    pub fn ncol(&self) -> usize {
        self.inner.width()
    }

    pub fn col_names(&self) -> Vec<String> {
        self.inner.get_column_names()
            .iter()
            .map(|s| s.to_string())
            .collect()
    }

    pub fn shape(&self) -> (usize, usize) {
        self.inner.shape()
    }
}
```

### Type mapping

| R type | Polars dtype | Conversion notes |
|--------|-------------|------------------|
| `integer` | `Int64` | R's integer is 32-bit, but polars' default int is i64; keeps headroom |
| `double` | `Float64` | Direct mapping |
| `character` | `String` (Utf8) | Direct mapping |
| `logical` | `Boolean` | Direct mapping |
| `factor` | `Categorical` | `dtype-categorical` feature; levels stored in the categorical metadata |
| `Date` | `Date` | Days since epoch; `dtype-slim` |
| `POSIXct` | `Datetime(Microseconds, None)` | `dtype-slim`; timezone deferred |
| `NA` | `null` per column | Each polars dtype has its own null representation |
| `complex` | NOT SUPPORTED | Fall back to list-based data frame |
| `raw` | NOT SUPPORTED | Fall back to list-based data frame |

### Polars 0.53 API changes to be aware of

1. **`Column` vs `Series`**: DataFrames in 0.53 store `Column` (an enum: `Series(SeriesColumn)` | `Scalar(ScalarColumn)`), not raw `Series`. Use `col.as_materialized_series()` to get `&Series`.

2. **`DataFrame::new(height, columns)`**: Now requires an explicit height. Use `DataFrame::new_infer_height(columns)` for auto-inference.

3. **`PlSmallStr`**: Column names are `PlSmallStr` (small-string-optimized), not `&str`. Use `.into()` to convert from string literals.

4. **`describe()` is Python-only**: We must compute summary statistics manually via lazy expressions (see `summary()` section below).

## R-to-Polars Operation Mapping

### Construction

| R | Polars Rust API |
|---|----------------|
| `data.frame(a = 1:3, b = c("x","y","z"))` | `DataFrame::new_infer_height(vec![Column::new("a".into(), &[1,2,3]), Column::new("b".into(), &["x","y","z"])])` |
| `data.frame(x = 1:3, stringsAsFactors = TRUE)` | Construct then cast string columns to `Categorical` |
| `as.data.frame(matrix)` | Extract ndarray rows/cols → polars columns |
| `as.data.frame(list)` | Convert each list element to a `Column` |

Recycling: R recycles shorter columns to match the longest. We must implement this before passing to polars (polars requires all columns same height).

```rust
fn build_data_frame(columns: Vec<(String, RValue)>) -> Result<PolarsDataFrame, RError> {
    // 1. Find max length
    let max_len = columns.iter().map(|(_, v)| v.length()).max().unwrap_or(0);

    // 2. Recycle each column to max_len
    // 3. Convert each RValue column to polars Column
    let polars_cols: Vec<Column> = columns
        .iter()
        .map(|(name, val)| rvalue_to_column(name, val, max_len))
        .collect::<Result<_, _>>()?;

    // 4. Construct DataFrame
    let df = DataFrame::new_infer_height(polars_cols)
        .map_err(|e| RError::Other(format!("data.frame error: {}", e)))?;

    Ok(PolarsDataFrame::new(df))
}
```

### Column access

| R | Polars Rust API |
|---|----------------|
| `df$col` | `df.inner.column("col")?.as_materialized_series()` → convert to `RValue::Vector` |
| `df[["col"]]` | Same as `$` |
| `df[, "col"]` (drop=TRUE) | Same — returns vector |
| `df[, c("a","b")]` | `df.inner.select(["a", "b"])` → new DataFrame |
| `df[, 2]` | `df.inner.select_at_idx(1)` (0-based) |
| `names(df)` | `df.inner.get_column_names()` |

### Row operations

| R | Polars Rust API |
|---|----------------|
| `df[3, ]` | `df.inner.slice(2, 1)` (0-based offset, length 1) |
| `df[1:5, ]` | `df.inner.slice(0, 5)` |
| `df[c(1,3,5), ]` | Construct boolean mask or use `df.inner.take()` with index array |
| `df[logical_vec, ]` | `let mask = Series::new("".into(), &bool_vec); df.inner.filter(&mask)?` |
| `head(df, n)` | `df.inner.head(Some(n))` |
| `tail(df, n)` | `df.inner.tail(Some(n))` |
| `nrow(df)` | `df.inner.height()` |
| `ncol(df)` | `df.inner.width()` |
| `dim(df)` | `df.inner.shape()` → `c(nrow, ncol)` |

### 2D indexing (`df[rows, cols]`)

The most complex operation. Dispatch logic:

```rust
fn index_dataframe_2d(
    df: &PolarsDataFrame,
    row_idx: Option<RValue>,   // None = all rows (empty arg)
    col_idx: Option<RValue>,   // None = all columns (empty arg)
    drop: bool,                // drop=TRUE: single column → vector
) -> Result<RValue, RError> {
    // 1. Resolve columns
    let selected = match col_idx {
        None => df.inner.clone(),
        Some(ref idx) => select_columns(&df.inner, idx)?,
    };

    // 2. Resolve rows
    let filtered = match row_idx {
        None => selected,
        Some(ref idx) => filter_rows(&selected, idx)?,
    };

    // 3. Drop dimension if single column + drop=TRUE
    if drop && filtered.width() == 1 {
        let series = filtered.select_at_idx(0).unwrap()
            .as_materialized_series();
        return Ok(series_to_rvalue(series));
    }

    Ok(RValue::DataFrame(PolarsDataFrame::new(filtered)))
}
```

### Filtering and subset

| R | Polars approach |
|---|----------------|
| `subset(df, age > 30)` | `df.lazy().filter(col("age").gt(lit(30))).collect()` |
| `subset(df, select = c("a","b"))` | `df.lazy().select([col("a"), col("b")]).collect()` |
| `subset(df, age > 30, select = c("name","age"))` | Chain `.filter()` then `.select()` |
| `with(df, expr)` | Create child environment with columns as bindings, eval expr |
| `within(df, { new_col <- a + b })` | Same as `with` but capture assignments back into new columns |

### Joins and merges

| R | Polars Rust API |
|---|----------------|
| `merge(x, y, by = "key")` | `x.join(&y, ["key"], ["key"], JoinArgs::new(JoinType::Inner))` |
| `merge(x, y, by.x = "a", by.y = "b")` | `x.join(&y, ["a"], ["b"], JoinArgs::new(JoinType::Inner))` |
| `merge(x, y, all = TRUE)` | `JoinType::Full` |
| `merge(x, y, all.x = TRUE)` | `JoinType::Left` |
| `merge(x, y, all.y = TRUE)` | `JoinType::Right` |
| `merge(x, y, by = NULL)` (cross join) | `JoinType::Cross` |

Suffix handling: R appends `.x` / `.y` to conflicting names. Polars uses a configurable suffix (`JoinArgs.suffix`).

```rust
fn r_merge(
    x: &DataFrame, y: &DataFrame,
    by_x: &[&str], by_y: &[&str],
    all_x: bool, all_y: bool,
) -> Result<DataFrame, RError> {
    let join_type = match (all_x, all_y) {
        (false, false) => JoinType::Inner,
        (true, false)  => JoinType::Left,
        (false, true)  => JoinType::Right,
        (true, true)   => JoinType::Full,
    };
    let mut args = JoinArgs::new(join_type);
    args.suffix = Some(".y".into());
    x.join(y, by_x, by_y, args)
        .map_err(|e| RError::Other(format!("merge error: {}", e)))
}
```

### Aggregation and group-by

| R | Polars Rust API |
|---|----------------|
| `aggregate(value ~ group, df, FUN = mean)` | `df.lazy().group_by([col("group")]).agg([col("value").mean()]).collect()` |
| `tapply(df$val, df$grp, sum)` | `df.lazy().group_by([col("grp")]).agg([col("val").sum()]).collect()` |
| `table(df$x)` | `df.lazy().group_by([col("x")]).agg([col("x").count()]).collect()` |
| `by(df, df$grp, summary)` | `df.inner.partition_by(["grp"], ...)` → apply summary to each |
| `split(df, df$grp)` | `df.inner.partition_by(["grp"], ...)` → list of DataFrames |

The lazy API is essential here because it lets us build complex aggregation expressions:

```rust
fn r_aggregate(
    df: &DataFrame,
    group_cols: &[&str],
    value_col: &str,
    fun_name: &str,
) -> Result<DataFrame, RError> {
    let agg_expr = match fun_name {
        "mean"   => col(value_col).mean(),
        "sum"    => col(value_col).sum(),
        "min"    => col(value_col).min(),
        "max"    => col(value_col).max(),
        "length" => col(value_col).count(),
        "sd"     => col(value_col).std(1),  // ddof=1 for R's sd()
        "var"    => col(value_col).var(1),
        "median" => col(value_col).median(),
        _ => return Err(RError::Other(format!("unsupported FUN: {}", fun_name))),
    };

    let group_exprs: Vec<polars::prelude::Expr> =
        group_cols.iter().map(|c| col(c)).collect();

    df.clone().lazy()
        .group_by(group_exprs)
        .agg([agg_expr])
        .collect()
        .map_err(|e| RError::Other(format!("aggregate error: {}", e)))
}
```

### Sorting

| R | Polars Rust API |
|---|----------------|
| `df[order(df$x), ]` | `df.sort(["x"], SortMultipleOptions::default())` |
| `df[order(df$x, decreasing=TRUE), ]` | `SortMultipleOptions::default().with_order_descending(true)` |
| `df[order(df$x, df$y), ]` | `df.sort(["x", "y"], SortMultipleOptions::default())` |
| `sort(df)` | Not standard R, but we could support it |

### Summary statistics

`describe()` is Python-only in polars. We build it manually:

```rust
fn r_summary(df: &DataFrame) -> Result<DataFrame, RError> {
    // For each column, compute count, mean, std, min, 25%, 50%, 75%, max
    let mut stat_rows: Vec<DataFrame> = Vec::new();

    for col_name in df.get_column_names() {
        let s = df.column(col_name)?.as_materialized_series();
        let stats = df.clone().lazy()
            .select([
                lit(col_name.to_string()).alias("column"),
                col(col_name).count().cast(DataType::Float64).alias("count"),
                col(col_name).mean().alias("mean"),
                col(col_name).std(1).alias("sd"),
                col(col_name).min().cast(DataType::Float64).alias("min"),
                col(col_name).quantile(lit(0.25), QuantileMethod::Linear)
                    .alias("25%"),
                col(col_name).median().alias("median"),
                col(col_name).quantile(lit(0.75), QuantileMethod::Linear)
                    .alias("75%"),
                col(col_name).max().cast(DataType::Float64).alias("max"),
            ])
            .collect()?;
        stat_rows.push(stats);
    }

    // vstack all rows
    let mut result = stat_rows.remove(0);
    for row in stat_rows {
        result = result.vstack(&row)?;
    }
    Ok(result)
}
```

### Binding rows and columns

| R | Polars Rust API |
|---|----------------|
| `rbind(df1, df2)` | `df1.vstack(&df2)?` (same schema) or `concat([df1.lazy(), df2.lazy()], UnionArgs { diagonal: true, .. })` (different schemas) |
| `cbind(df1, df2)` | `polars::functions::concat_df_horizontal(&[df1, df2], true)?` |
| `rbind.data.frame(...)` | Same as rbind |
| `cbind.data.frame(...)` | Same as cbind |

### I/O

| R | Polars Rust API |
|---|----------------|
| `read.csv("file.csv")` | `CsvReadOptions::default().with_has_header(true).try_into_reader_with_file_path(Some("file.csv".into()))?.finish()?` |
| `read.csv("file.csv", sep="\t")` | `.map_parse_options(\|opts\| opts.with_separator(b'\t'))` |
| `read.csv("file.csv", header=FALSE)` | `.with_has_header(false)` |
| `write.csv(df, "out.csv")` | `CsvWriter::new(&mut file).include_header(true).finish(&mut df)?` |
| `write.csv(df, row.names=FALSE)` | Default — polars has no row names |
| `read.csv("file.csv.gz")` | Requires `decompress` feature; polars auto-detects |

Lazy CSV reading (for large files):

```rust
// Only reads the rows/columns that are actually needed
let df = LazyCsvReader::new("big_data.csv")
    .with_has_header(true)
    .finish()?
    .filter(col("sales").gt(lit(1000)))
    .select([col("date"), col("sales"), col("region")])
    .collect()?;
```

### Factor support

R factors are integer-coded categorical variables with labels. Polars' `Categorical` dtype is the natural mapping.

```rust
fn rvalue_to_factor_column(name: &str, values: &[Option<String>]) -> Column {
    // Create string series then cast to Categorical
    let s = Series::new(name.into(), values);
    s.cast(&DataType::Categorical(None, CategoricalOrdering::Physical))
        .unwrap()
        .into()
}

fn factor_to_rvalue(series: &Series) -> RValue {
    // Extract the string representation (labels) and integer codes
    let cat = series.categorical().unwrap();
    // Get labels as character vector
    let labels: Vec<Option<String>> = cat.iter_str()
        .map(|opt| opt.map(|s| s.to_string()))
        .collect();
    // Return as character vector with "factor" class and "levels" attr
    // ...
}
```

### Conversion between RValue and polars

These are the critical boundary functions. Every R operation that touches a polars DataFrame goes through these.

#### RValue → polars Column

```rust
fn rvalue_to_column(name: &str, val: &RValue, target_len: usize) -> Result<Column, RError> {
    let col = match val {
        RValue::Vector(rv) => match &rv.inner {
            Vector::Double(d) => {
                let vals: Vec<Option<f64>> = recycle(d.as_slice(), target_len);
                Column::new(name.into(), vals)
            }
            Vector::Integer(i) => {
                let vals: Vec<Option<i64>> = recycle(i.as_slice(), target_len);
                Column::new(name.into(), vals)
            }
            Vector::Character(c) => {
                let vals: Vec<Option<&str>> = recycle_ref(c.as_slice(), target_len);
                Column::new(name.into(), vals)
            }
            Vector::Logical(l) => {
                let vals: Vec<Option<bool>> = recycle(l.as_slice(), target_len);
                Column::new(name.into(), vals)
            }
        },
        RValue::Null => {
            // NULL column → all-null Float64 column
            Column::new_empty(name.into(), &DataType::Float64)
        }
        _ => return Err(RError::Other(format!(
            "cannot coerce {} to data frame column", val.type_name()
        ))),
    };
    Ok(col)
}
```

#### polars Series → RValue

```rust
fn series_to_rvalue(series: &Series) -> RValue {
    match series.dtype() {
        DataType::Float64 => {
            let vals: Vec<Option<f64>> = series.f64().unwrap().into_iter().collect();
            RValue::vec(Vector::Double(vals.into()))
        }
        DataType::Float32 => {
            // Upcast to f64 for R
            let vals: Vec<Option<f64>> = series.f32().unwrap()
                .into_iter().map(|v| v.map(|x| x as f64)).collect();
            RValue::vec(Vector::Double(vals.into()))
        }
        DataType::Int64 => {
            let vals: Vec<Option<i64>> = series.i64().unwrap().into_iter().collect();
            RValue::vec(Vector::Integer(vals.into()))
        }
        DataType::Int32 | DataType::Int16 | DataType::Int8 => {
            // Upcast to i64 for R
            let s = series.cast(&DataType::Int64).unwrap();
            let vals: Vec<Option<i64>> = s.i64().unwrap().into_iter().collect();
            RValue::vec(Vector::Integer(vals.into()))
        }
        DataType::Boolean => {
            let vals: Vec<Option<bool>> = series.bool().unwrap().into_iter().collect();
            RValue::vec(Vector::Logical(vals.into()))
        }
        DataType::String => {
            let vals: Vec<Option<String>> = series.str().unwrap()
                .into_iter().map(|v| v.map(|s| s.to_string())).collect();
            RValue::vec(Vector::Character(vals.into()))
        }
        DataType::Categorical(_, _) => {
            // Factor: extract string labels
            let vals: Vec<Option<String>> = series.categorical().unwrap()
                .iter_str().map(|v| v.map(|s| s.to_string())).collect();
            // TODO: set class=factor, levels attr
            RValue::vec(Vector::Character(vals.into()))
        }
        _ => {
            // Fallback: cast to string
            let s = series.cast(&DataType::String).unwrap_or_else(|_| {
                Series::new(series.name().clone(), vec!["<unconvertible>"; series.len()])
            });
            let vals: Vec<Option<String>> = s.str().unwrap()
                .into_iter().map(|v| v.map(|s| s.to_string())).collect();
            RValue::vec(Vector::Character(vals.into()))
        }
    }
}
```

#### polars DataFrame → RList (materialization)

For R code that treats a data frame as a list (e.g., `lapply(df, mean)`):

```rust
fn dataframe_to_rlist(pdf: &PolarsDataFrame) -> RValue {
    let names: Vec<Option<String>> = pdf.col_names().into_iter().map(Some).collect();
    let values: Vec<(Option<String>, RValue)> = pdf.inner
        .get_columns()
        .iter()
        .map(|col| {
            let name = col.name().to_string();
            let val = series_to_rvalue(col.as_materialized_series());
            (Some(name), val)
        })
        .collect();

    let mut list = RList::new(values);
    list.set_attr("names", RValue::vec(Vector::Character(names.into())));
    list.set_attr("class",
        RValue::vec(Vector::Character(vec![Some("data.frame".to_string())].into())));

    // row.names
    let nrow = pdf.nrow();
    let row_names = match &pdf.row_names {
        Some(rn) => rn.clone(),
        None => (1..=nrow).map(|i| Some(i.to_string())).collect(),
    };
    list.set_attr("row.names", RValue::vec(Vector::Character(row_names.into())));

    RValue::List(list)
}
```

## Printing

R's `print.data.frame` shows a formatted table. With polars we get `fmt` for free, but it doesn't match R's output format. We should implement our own `print.data.frame` that:

1. Shows column names as header
2. Shows row names/numbers on the left
3. Right-aligns numeric columns, left-aligns character columns
4. Truncates to terminal width
5. Shows `# ... with N more rows` for large frames
6. Shows column types below the header (like tibble): `<dbl>`, `<chr>`, `<int>`, `<lgl>`

We can use the `tabled` crate (already in our deps) for this, or build it with manual formatting.

## Backward compatibility

### Dual representation

For the transition period, `is.data.frame()` must check both:

```rust
fn is_data_frame(val: &RValue) -> bool {
    match val {
        RValue::DataFrame(_) => true,
        RValue::List(l) => has_class_list(l, "data.frame"),
        _ => false,
    }
}
```

### Fallback to list-based

Some R patterns can't be represented as polars DataFrames:

- Columns containing lists or nested structures (unless we use `dtype-struct`)
- Columns of different lengths (shouldn't happen, but defensive)
- Columns containing functions, environments, or language objects
- Mixed-type columns (polars is strictly typed per column)

In these cases, fall back to the current list-with-attrs approach. The wrapper should detect this at construction time and refuse to build a polars DataFrame, returning an `RList` instead.

### S3 dispatch

With `RValue::DataFrame` as a new variant, we need S3 dispatch to recognize it. The `class()` builtin must return `"data.frame"` for `RValue::DataFrame`, and `inherits(df, "data.frame")` must return TRUE. The S3 dispatcher should extract the class from the polars wrapper just like it does from list attrs.

## Implementation order

1. **Add polars dependency** with features above; verify it builds
2. **Create `PolarsDataFrame` wrapper** in `value.rs` with basic methods
3. **Implement `rvalue_to_column` and `series_to_rvalue`** conversion functions
4. **Rewrite `data.frame()`** to construct polars DataFrames (with list fallback)
5. **Implement `$`, `[[`, `[,]` indexing** on the new type
6. **Implement `print.data.frame`** using polars column iteration + tabled or manual formatting
7. **Rewrite `read.csv` / `write.csv`** to use polars CsvReader/CsvWriter
8. **Add `merge()`** using polars joins
9. **Add `subset()`, `with()`** using polars lazy filter/eval
10. **Add `aggregate()` / `tapply()`** using polars `group_by`
11. **Add `summary()`** using manual lazy stat expressions
12. **Bridge `rbind` / `cbind`** to polars `vstack` / `concat_df_horizontal`
13. **Factor support** via `Categorical` dtype
14. **Parquet/JSON I/O** (`read.parquet` / `write.parquet` as new builtins)
15. **Add `order()` / `sort()`** using polars `sort()`
16. **`as.data.frame()`** coercion from matrices, lists, vectors
17. **`lapply(df, ...)` bridge** — auto-materialize to list when apply functions receive a DataFrame
18. **Performance**: lazy CSV scanning for large files

## Testing strategy

Create `tests/polars-dataframe.R`:

```r
# Construction
df <- data.frame(x = 1:5, y = c("a","b","c","d","e"))
stopifnot(nrow(df) == 5)
stopifnot(ncol(df) == 2)
stopifnot(identical(names(df), c("x", "y")))

# Column access
stopifnot(identical(df$x, 1:5))
stopifnot(identical(df[["y"]], c("a","b","c","d","e")))

# Row filtering
sub <- df[df$x > 3, ]
stopifnot(nrow(sub) == 2)

# 2D indexing
cell <- df[2, "y"]
stopifnot(cell == "b")

# merge
df2 <- data.frame(y = c("a","c","e"), z = c(10, 30, 50))
m <- merge(df, df2, by = "y")
stopifnot(nrow(m) == 3)

# aggregate
agg <- aggregate(x ~ y, df, sum)
stopifnot(is.data.frame(agg))

# rbind / cbind
df3 <- rbind(df, df)
stopifnot(nrow(df3) == 10)

# CSV round-trip
write.csv(df, "/tmp/test_polars.csv", row.names = FALSE)
df4 <- read.csv("/tmp/test_polars.csv")
stopifnot(identical(names(df4), names(df)))

# summary
s <- summary(df)
cat("All polars data frame tests passed!\n")
```

## Open questions

1. **row.names**: Polars has no concept of row names. Store them in the wrapper? Or phase them out (tibbles don't use them either)?
2. **Copy semantics**: R data frames have copy-on-modify. Polars DataFrames are reference-counted internally. We need to ensure `df2 <- df; df2$x <- 99` doesn't mutate `df`. Solution: clone the polars DataFrame on assignment.
3. **NA vs null**: R's NA is a value within the type (NA_integer_ is a special i32). Polars uses null bitmaps. This is semantically equivalent but we need to be careful with `is.na()` — it should check the polars null bitmap, not look for a sentinel value.
4. **String interning**: Polars uses Arrow string representation (offset + data buffer). Converting to/from `Vec<Option<String>>` on every column access is wasteful. Consider lazy conversion with caching.
5. **Large DataFrames**: For DataFrames that exceed memory, polars' lazy scanning (CSV, parquet) can stream. Should we expose this via `read.csv()` automatically for large files?
