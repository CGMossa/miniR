# bstr integration plan

> `bstr` 1.12 — Byte strings not required to be valid UTF-8, by BurntSushi.
> <https://github.com/BurntSushi/bstr>

## What it does

String type (`BStr`, `BString`) that works with arbitrary bytes, not just valid
UTF-8. Provides string operations (find, replace, split, trim, case conversion)
that work on byte strings. Gracefully handles mixed encodings.

```rust
use bstr::{ByteSlice, BString};

let s = BString::from(b"caf\xe9".as_slice()); // not valid UTF-8
assert_eq!(s.find("caf"), Some(0));
for word in s.words() { /* ... */ }
```

Also: `BStr` (borrowed, like `&str` for bytes), regex support via `bstr::Regex`.

## Where it fits in newr

### 1. R's string encoding model

R strings can be in multiple encodings: UTF-8, Latin-1, "bytes" (raw), or native
encoding. R's `Encoding()` function returns `"UTF-8"`, `"latin1"`, `"bytes"`, or
`"unknown"`.

Currently newr uses Rust `String` (always UTF-8). This breaks when R code:

- Reads a Latin-1 file without conversion
- Uses `chartr()` or `iconv()` between encodings
- Has `Encoding(x) <- "bytes"` (raw byte strings)

`bstr` handles all of these — it's the right foundation for R's string model.

### 2. `readLines()` / `readBin()` — reading non-UTF-8 files

```r
x <- readLines("latin1_file.txt", encoding="latin1")
```

With bstr, we can read the raw bytes and track the encoding separately:

```rust
struct RString {
    data: BString,       // raw bytes
    encoding: Encoding,  // UTF-8, Latin-1, Bytes, Unknown
}
```

### 3. `iconv()` — encoding conversion

```r
iconv(x, from="latin1", to="UTF-8")
```

bstr provides the byte-level access needed for encoding conversion. Pair with
`encoding_rs` crate for actual charset conversion.

### 4. `nchar(x, type="bytes")` vs `nchar(x, type="chars")`

R distinguishes byte length from character length. `BStr::len()` gives bytes,
`BStr::chars().count()` gives characters (with best-effort UTF-8 decoding).

### 5. `grep()` / `gsub()` on byte strings

bstr integrates with regex for searching byte strings that may not be valid UTF-8.

## Relationship to builtins plan

| Phase | Builtins affected | Impact |
|---|---|---|
| Phase 2 (strings) | All string functions | encoding-aware strings |
| Phase 2 (strings) | `nchar()`, `Encoding()`, `enc2utf8()`, `iconv()` | encoding support |
| Phase 11 (I/O) | `readLines()`, `readBin()`, `scan()` | read non-UTF-8 files |

## Recommendation

**Add when implementing encoding support.** For now, UTF-8-only `String` works
for most R code. When we need Latin-1 or raw byte string support, bstr is the
right foundation.

**Effort:** Medium — 2-3 hours to introduce BString into the string type, more
to propagate through all string operations.
