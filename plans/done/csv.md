# csv integration plan

> `csv` 1.4 — Fast CSV reader/writer by BurntSushi.
> <https://github.com/BurntSushi/rust-csv>

## What it does

High-performance CSV reading and writing. Zero-copy parsing, configurable
delimiters, quoting, headers. Built on BurntSushi's performance principles.

```rust
let mut rdr = csv::ReaderBuilder::new()
    .has_headers(true)
    .delimiter(b',')
    .from_path("data.csv")?;

for result in rdr.records() {
    let record = result?;
    println!("{}", &record[0]);
}
```

Also: `Writer`, `ByteRecord` (zero-copy), `StringRecord`, serde integration
for typed deserialization.

## Where it fits in miniR

### 1. `read.csv()` / `read.table()`

R's most-used I/O functions:

```r
df <- read.csv("data.csv")
df <- read.csv("data.csv", header=TRUE, sep=",", stringsAsFactors=FALSE)
df <- read.table("data.tsv", sep="\t", header=TRUE)
df <- read.delim("data.tsv")  # tab-separated
```

Mapping to csv crate:

| R parameter | csv equivalent |
|---|---|
| `sep = ","` | `.delimiter(b',')` |
| `header = TRUE` | `.has_headers(true)` |
| `quote = "\""` | `.quote(b'"')` |
| `comment.char = "#"` | `.comment(Some(b'#'))` |
| `na.strings = "NA"` | post-processing: replace "NA" with `None` |
| `skip = 5` | skip first N records |
| `nrows = 100` | take only N records |
| `colClasses` | type-guided column parsing |

### 2. `write.csv()` / `write.table()`

```r
write.csv(df, "output.csv", row.names=FALSE)
write.table(df, "output.tsv", sep="\t", row.names=FALSE)
```

```rust
let mut wtr = csv::WriterBuilder::new()
    .delimiter(b',')
    .from_path("output.csv")?;
wtr.write_record(&["name", "age", "city"])?;  // header
wtr.write_record(&["Alice", "30", "NYC"])?;   // data
```

### 3. Column type inference

When `colClasses` is not specified, R infers column types by scanning the data.
The csv crate reads strings; we add type inference on top:

1. Read all rows as strings
2. For each column, try: integer → double → keep as character
3. Convert `"NA"`, `"NaN"`, `""` to appropriate NA values

### 4. Performance

The csv crate is extremely fast — BurntSushi optimized it for the `xsv` tool.
Combined with zmij for double formatting, CSV I/O would be competitive with
R's `data.table::fread()`.

## Relationship to builtins plan

| Phase | Builtins affected | Impact |
|---|---|---|
| Phase 11 (I/O) | `read.csv()`, `read.csv2()`, `read.table()`, `read.delim()` | CSV reading |
| Phase 11 (I/O) | `write.csv()`, `write.csv2()`, `write.table()` | CSV writing |

## Recommendation

**Add when implementing Phase 11 (I/O).** This is the right crate — fast, correct,
well-maintained. CSV I/O is one of R's most important features.

**Effort:** 2-3 hours for read.csv/write.csv with type inference.
