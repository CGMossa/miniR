# flate2 integration plan

> `flate2` — gzip/deflate/zlib compression and decompression.
> <https://github.com/rust-lang/flate2-rs>

## What it does

Pure-Rust or system-linked gzip compression. Provides `GzDecoder`,
`GzEncoder`, `DeflateDecoder`, `DeflateEncoder`, `ZlibDecoder`,
`ZlibEncoder` for streaming compress/decompress.

## Where it fits in miniR

### GNU R binary serialization (primary use)

R's `readRDS()`/`saveRDS()`/`load()`/`save()` use gzip compression
by default. The binary RDS format (see `plans/binary-serialization.md`)
wraps the XDR-serialized byte stream in gzip. Without flate2, we
cannot read any real-world .rds or .RData files.

```rust
use flate2::read::GzDecoder;
let file = File::open("data.rds")?;
let mut decoder = GzDecoder::new(file);
let mut bytes = Vec::new();
decoder.read_to_end(&mut bytes)?;
// bytes is now the raw XDR serialization stream
unserialize_xdr(&bytes)
```

### `gzfile()` connections

R has `gzfile(description, open)` for reading/writing gzip-compressed
files as connections. This is used by `readLines(gzfile("log.gz"))`.

### `memCompress()` / `memDecompress()`

R builtins for in-memory compression:
- `memCompress(from, type = "gzip")` — compress raw vector
- `memDecompress(from, type = "gzip")` — decompress raw vector

### `R.utils::gzip()` / `R.utils::gunzip()`

Higher-level file gzip/gunzip used by many CRAN packages.

## Implementation

1. Add `flate2 = { version = "1", optional = true, default-features = false, features = ["rust_backend"] }` with feature `compression`
2. Use `rust_backend` feature for pure-Rust (no system zlib needed)
3. Include `compression` in default features
4. Primary consumer: `src/interpreter/builtins/serialize.rs` (GNU R binary RDS reader/writer)
5. Secondary: `gzfile()` connection type, `memCompress`/`memDecompress` builtins

## Priority: HIGH — required for reading real .rds files (GNU R serialization).
