# Preferred Dependencies for miniR project

User-specified dependency versions to use when needed:

- crossterm
- ctrlc
- dirs
- lexopt
- log
- miette 7.6+fancy-no-backtrace+fancy
- multipart-rs
- openssl+vendored (static-link-openssl, not windows)
- reedline 0.46+bashisms
- serde_json
- simplelog
- time
- winresource (build, windows only)

Future crates to consider:

- indexmap (insertion-order-preserving hash map, for named lists/attributes)
- indicatif (progress bars for long-running operations)
- itertools (extended iterator adaptors)
- rayon (data parallelism for vectorized operations)
- readonly 0.2 (struct fields readable outside module but only writable inside)
- serde+derive (serialization/deserialization)
- zmij 1.0 (fast f64-to-string conversion, Schubfach/yy algorithm)

- gc-arena (garbage collection arena, for GC'd R values)
- arrow (Apache Arrow, for columnar data / data frames)
- rand (random number generation for runif, rnorm, sample, set.seed)
- rand_distr (statistical distributions for rbinom, rpois, etc.)
- openblas-src (BLAS backend for linear algebra)
- num-traits (numeric trait abstractions)
- nalgebra (linear algebra: solve, qr, svd, eigen, chol)
- num-complex (complex number support)
- oxiblas (Rust BLAS bindings)
- ndarray (N-dimensional arrays for matrix/array operations)
- slotmap (slot-based arena allocator, for interned/GC'd value handles)

BurntSushi crates:

- jiff (datetime library: Zoned, Timestamp, Span — replaces `time` for R date/time)
- csv (CSV reader/writer for read.csv/write.csv)
- globset (glob pattern matching for Sys.glob, list.files)
- walkdir (recursive directory traversal for list.files(recursive=TRUE))
- tabwriter (elastic tabstops for formatted table output)
- termcolor (colored terminal output for REPL/error formatting)
- bstr (byte strings not required to be UTF-8, for R's mixed-encoding strings)
- aho-corasick (fast multi-pattern string matching for grep with multiple patterns)
- memchr (optimized byte/string search for fast fixed-pattern grep/grepl)

Dev/bench dependencies:

- assert_cmd
- fancy-regex
- pretty_assertions
- rstest 0.23
- serial_test
- divan (benchmarking framework)
- tempfile
- quickcheck (property-based testing for R builtin correctness)
