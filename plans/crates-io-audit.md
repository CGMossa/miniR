# crates.io Audit: New Crates for Investigation

Source: `analysis/crates-io-overview.csv` (4,880 crates, ≥1M downloads, updated within 18 months, with repository).
Cross-referenced against: `Cargo.toml` (current deps), `analysis/vendor-crate-audit.md` (evaluated crates), `plans/done/` (written plans).

**Scope**: Only crates NOT already evaluated or integrated. Infrastructure crates (proc-macros, build tools, async runtimes, logging) excluded unless they directly implement R functionality.

---

## Statistics & Distributions

| Crate | Version | Downloads | Why |
|-------|---------|-----------|-----|
| **statrs** | 0.18.0 | 21.7M | Full distribution suite (Normal, Gamma, Beta, Chi-sq, F, t, etc.) with PDF/CDF/quantile — maps to `dnorm`/`pnorm`/`qnorm` family. Currently miniR implements distributions inline; statrs centralizes them. |
| **ndarray-stats** | 0.7.0 | 4.8M | Mean, std, quartiles, histograms, correlation for ndarray — maps to `summary()`, `quantile()`, `cor()`. |
| **linregress** | 0.5.4 | 8.2M | OLS regression with R², p-values, confidence intervals — could augment `lm()` / `summary.lm()`. |
| **average** | 0.16.0 | 6.5M | Streaming/incremental statistics (mean, variance, skewness, kurtosis) — useful for large data without full materialization. |

**Recommendation**: statrs is the highest-value single crate here. It replaces ~500 lines of hand-rolled distribution code and adds distributions we don't have yet (Multinomial, Dirichlet, InverseGamma, etc.). ndarray-stats pairs naturally with our existing ndarray dependency.

## Linear Algebra

| Crate | Version | Downloads | Why |
|-------|---------|-----------|-----|
| **faer** | 0.24.0 | 1.9M | Modern pure-Rust linear algebra. Faster than nalgebra for large matrices, SIMD-optimized. Has SVD, QR, Cholesky, eigendecomposition. Alternative/complement to nalgebra. |
| **sprs** | 0.11.4 | 4.8M | Sparse matrix library (CSR/CSC) — maps to R's `Matrix::sparseMatrix()`. Currently missing entirely. |
| **gemm** | 0.19.0 | 7.8M | Optimized matrix multiply. Could speed up `%*%` for large matrices. |

**Recommendation**: sprs fills a real gap (sparse matrices are used heavily in R). faer worth benchmarking against nalgebra for our decomposition operations.

## Numerical Methods

| Crate | Version | Downloads | Why |
|-------|---------|-----------|-----|
| **argmin** | 0.11.0 | 2.9M | Numerical optimization (L-BFGS, Nelder-Mead, conjugate gradient, etc.) — maps directly to `optim()`. Currently unimplemented. |
| **rustfft** | 6.4.1 | 15.7M | High-performance FFT — maps to `fft()`, `mvfft()`, `convolve()`. Currently unimplemented. |
| **realfft** | 3.5.0 | 8.9M | Real-valued FFT (companion to rustfft) — more efficient for real data which is the common case in R. |
| **peroxide** | 0.41.0 | 1.1M | All-in-one scientific computing: ODE solvers, interpolation, root finding, numerical integration. Maps to `integrate()`, `uniroot()`, `spline()`, `approxfun()`. |
| **interp** | 2.1.2 | 3.0M | Interpolation (linear, cubic) — maps to `approx()`, `approxfun()`, `spline()`. |

**Recommendation**: argmin and rustfft fill critical R functionality gaps. `optim()` and `fft()` are heavily used in CRAN packages. peroxide is broad but could supply `integrate()`, `uniroot()`, ODE solving.

## Data Frames & Columnar

| Crate | Version | Downloads | Why |
|-------|---------|-----------|-----|
| **polars** | 0.53.0 | 9.5M | High-perf DataFrame library on Arrow. Already in audit as planned. |
| **serde_arrow** | 0.14.0 | 1.7M | Bridge serde ↔ Arrow arrays. Could simplify R value → Arrow conversion. |
| **arrow_convert** | 0.11.4 | 2.2M | Nested Rust types to Arrow arrays. Alternative to serde_arrow. |
| **calamine** | 0.34.0 | 6.4M | Read Excel (.xlsx/.xls/.ods) files — maps to `readxl::read_excel()`. Currently unimplemented. |

**Recommendation**: calamine fills a real gap — Excel I/O is extremely common in R. The arrow conversion crates could simplify our NullableBuffer ↔ Arrow interop.

## String & Text Processing

| Crate | Version | Downloads | Why |
|-------|---------|-----------|-----|
| **fancy-regex** | 0.17.0 | 128M | Regex with backreferences and lookahead — R's `grep(perl=TRUE)` uses PCRE which supports these. Our current `regex` crate doesn't. |
| **unicode-normalization** | 0.1.25 | 399M | NFC/NFKC/NFD/NFKD normalization — R's `stri_trans_nfc()` etc. Missing entirely. |
| **aho-corasick** | 1.1.4 | 727M | Already has a plan; noting it's also a dependency of `regex`. |

**Recommendation**: fancy-regex is high priority — PCRE-compatible regex is expected by many R packages. unicode-normalization fills a gap for stringi-equivalent functionality.

## Serialization & Data Formats

| Crate | Version | Downloads | Why |
|-------|---------|-----------|-----|
| **quick-xml** | 0.39.2 | 238M | High-performance XML reader/writer — maps to `xml2::read_xml()`. Currently unimplemented. |
| **roxmltree** | 0.21.1 | 39.6M | Read-only XML tree — simpler API for XML reading. |
| **yaml-rust2** | 0.11.0 | 31M | YAML parser — maps to `yaml::read_yaml()`. |
| **simd-json** | 0.17.0 | 10.8M | SIMD-accelerated JSON — could speed up `jsonlite::fromJSON()` for large files. |
| **bincode** | 3.0.0 | 213M | Efficient binary serialization — could augment/replace text-based RDS format. |
| **rmp-serde** | 1.3.1 | 85.4M | MessagePack — compact binary format, alternative to RDS. |

**Recommendation**: quick-xml is the biggest gap — XML is ubiquitous in R (SVG, HTML, web scraping, configuration). yaml-rust2 fills another common format.

## Networking & HTTP

| Crate | Version | Downloads | Why |
|-------|---------|-----------|-----|
| **reqwest** | 0.13.2 | 414M | High-level HTTP client with async/sync, TLS, redirects, cookies — maps to `httr::GET()`, `download.file()`, `curl::curl_fetch_memory()`. Currently miniR has basic `url()` connections only. |

**Recommendation**: reqwest is the standard Rust HTTP client. It would massively improve `download.file()`, enable `httr`-style functionality, and support CRAN package downloads.

## Database Connectivity

| Crate | Version | Downloads | Why |
|-------|---------|-----------|-----|
| **rusqlite** | 0.39.0 | 53M | Ergonomic SQLite — maps to `RSQLite::dbConnect()`. Most-used database in R. |
| **postgres** | 0.19.12 | 12.5M | Sync PostgreSQL client — maps to `RPostgres::dbConnect()`. |
| **mysql** | 28.0.0 | 3.0M | MySQL client — maps to `RMySQL::dbConnect()`. |
| **odbc-api** | 23.0.1 | 1.3M | ODBC connections — maps to `RODBC`/`odbc` packages. |
| **sqlparser** | 0.61.0 | 56.8M | SQL parser (ANSI SQL:2011) — could enable `sqldf()`-style functionality. |

**Recommendation**: rusqlite is highest priority — SQLite is by far the most common database backend in R. sqlparser could enable interesting SQL-on-dataframe features.

## Compression

| Crate | Version | Downloads | Why |
|-------|---------|-----------|-----|
| **bzip2** (bzip2-rs) | 0.6.1 | 107M | bzip2 compression — maps to `bzfile()`, `R.utils::bunzip2()`. Missing entirely. |
| **lz4_flex** | 0.13.0 | 82.6M | Fast LZ4 — useful for data frame serialization, not directly R-facing but improves perf. |
| **zstd-safe** | 7.2.4 | 255M | Zstandard compression — increasingly common format, potential future R addition. |
| **xz2/liblzma** | — | 12.1M | XZ/LZMA — maps to `xzfile()`. Missing entirely. |

**Recommendation**: bzip2 and xz are required — R's `bzfile()` and `xzfile()` connections depend on them. Currently only gzip (flate2) is supported.

## Cryptography & Hashing

| Crate | Version | Downloads | Why |
|-------|---------|-----------|-----|
| **md-5** | 0.10.6 | 202M | MD5 hash — R's `digest::digest(algo="md5")`. Currently returns an error suggesting sha256. |
| **sha1** | 0.10.6 | 294M | SHA-1 — R's `digest::digest(algo="sha1")`. |
| **sha3** | 0.10.8 | 104M | SHA-3 family — future-proofing. |
| **blake2** | 0.10.6 | 99M | BLAKE2 — alternative to BLAKE3, more widely used in some contexts. |
| **xxhash-rust** | 0.8.15 | 55.5M | xxHash — fast non-cryptographic hash, useful for data frame hashing/dedup. |

**Recommendation**: md-5 and sha1 are low-hanging fruit — they complete the digest() algorithm suite.

## Spatial & Geometry

| Crate | Version | Downloads | Why |
|-------|---------|-----------|-----|
| **geo** | 0.32.0 | 14.7M | Geospatial algorithms (area, distance, intersection, convex hull) — maps to `sf` package. |
| **geo-types** | 0.7.18 | 17.3M | Point, LineString, Polygon types — foundation for sf-like functionality. |
| **rstar** | 0.12.2 | 20.4M | R*-tree spatial index — fast spatial queries. |
| **geojson** | 1.0.0 | 7.7M | GeoJSON I/O — maps to `sf::st_read("file.geojson")`. |
| **wkt** | 0.14.0 | 5.7M | Well-Known Text I/O — maps to `sf::st_as_text()`. |
| **gdal** | 0.19.0 | 3.1M | GDAL bindings — maps to `sf::st_read()` / `terra::rast()` for raster/vector data. |

**Recommendation**: The georust ecosystem (geo + geo-types + rstar + geojson + wkt) is a coherent set that would enable basic `sf`-like functionality. Worth investigating as a group.

## Graphics & Visualization

| Crate | Version | Downloads | Why |
|-------|---------|-----------|-----|
| **lyon** | 1.0.19 | 3.5M | 2D path tessellation — could improve vector graphics rendering quality. |
| **imageproc** | 0.26.1 | 7.8M | Image processing (blur, threshold, edge detection, etc.) — maps to `magick` package. |
| **fast_image_resize** | 6.0.0 | 13.2M | SIMD image resizing — faster than image crate's built-in. |
| **cairo-rs** | 0.22.0 | 18.9M | Cairo bindings — R's primary graphics backend uses Cairo. Could enable a cairo device. |
| **lopdf** | 0.40.0 | 5.3M | PDF manipulation (merge, split, modify) — extends beyond our krilla-based PDF device. |

**Recommendation**: cairo-rs is interesting for compatibility with R's cairo device, but we already have krilla for PDF. imageproc adds `magick`-style image manipulation.

## Random Number Generation

| Crate | Version | Downloads | Why |
|-------|---------|-----------|-----|
| **rand_xoshiro** | 0.8.0 | 76M | Xoshiro/xoroshiro generators — fast, high-quality. Alternative to ChaCha for non-crypto use. |
| **rand_pcg** | 0.10.1 | 109M | PCG generators — another high-quality RNG family. |
| **rand_seeder** | 0.5.0 | 1.9M | Universal seeder from any hashable value — could improve `set.seed()` flexibility. |

**Recommendation**: Low priority — our current ChaCha + SmallRng setup works. But xoshiro is what many statistical packages prefer for speed.

## Numerical Types

| Crate | Version | Downloads | Why |
|-------|---------|-----------|-----|
| **rust_decimal** | 1.40.0 | 87.5M | Arbitrary precision decimal — could support `Rmpfr` or financial calculations. |
| **ordered-float** | 5.2.0 | 279M | Total ordering on floats — could simplify sorting with NAs. |

**Recommendation**: ordered-float could simplify our sort/order implementations that currently handle NA manually.

## Graph Data Structures

| Crate | Version | Downloads | Why |
|-------|---------|-----------|-----|
| **petgraph** | 0.8.3 | 323M | Graph algorithms (BFS, DFS, Dijkstra, toposort, etc.) — maps to `igraph` package. |

**Recommendation**: petgraph maps to the very popular `igraph` R package. Worth investigating when `igraph` compatibility becomes a goal.

## Machine Learning

| Crate | Version | Downloads | Why |
|-------|---------|-----------|-----|
| **linfa** | 0.8.1 | 1.0M | ML framework (k-means, SVM, decision trees, PCA, etc.) — maps to `cluster`, `e1071`, `rpart`. |
| **peroxide** | 0.41.0 | 1.1M | Includes neural networks, gradient descent — broader than pure stats. |

**Recommendation**: Lower priority — focus on getting `lm()`/`glm()` solid before adding ML. But linfa could accelerate `kmeans()`, `prcomp()`.

---

## Priority Tiers for Investigation

### Tier 1 — Fill Critical R Gaps
These crates implement R functionality that is currently **missing and frequently used**:

1. **statrs** — distribution functions (dnorm/pnorm/qnorm family)
2. **argmin** — `optim()` numerical optimization
3. **rustfft** + **realfft** — `fft()`, `convolve()`
4. **fancy-regex** — PCRE-compatible regex (`grep(perl=TRUE)`)
5. **reqwest** — HTTP client (`download.file()`, `httr`)
6. **bzip2** — `bzfile()` compression
7. **quick-xml** — XML I/O (`xml2` package)
8. **rusqlite** — SQLite (`RSQLite` package)
9. **md-5** + **sha1** — complete `digest()` hash suite
10. **calamine** — Excel file reading (`readxl`)

### Tier 2 — Improve Existing Features
These crates enhance functionality that partially exists:

1. **ndarray-stats** — richer stats on arrays (quantile, correlation)
2. **sprs** — sparse matrices (R's Matrix package)
3. **unicode-normalization** — text normalization (stringi)
4. **peroxide** — `integrate()`, `uniroot()`, ODE solvers
5. **interp** — `approx()`, `spline()` interpolation
6. **linregress** — augment `lm()` with more diagnostics
7. **ordered-float** — simplify NA-aware sorting

### Tier 3 — Enable CRAN Package Ecosystems
These unlock large families of CRAN packages:

1. **geo** ecosystem — enables `sf`-like spatial computing
2. **petgraph** — enables `igraph`-like graph analysis
3. **postgres** + **mysql** + **odbc-api** — DBI database backends
4. **linfa** — ML algorithms (kmeans, PCA, SVM)
5. **yaml-rust2** — YAML I/O
6. **sqlparser** — SQL-on-dataframe

### Tier 4 — Performance & Polish
Nice-to-have improvements:

1. **faer** — potentially faster linear algebra
2. **gemm** — optimized matrix multiply
3. **simd-json** — faster JSON parsing
4. **lz4_flex** / **zstd** — additional compression
5. **imageproc** — image manipulation
6. **rand_xoshiro** — faster RNG for simulations
