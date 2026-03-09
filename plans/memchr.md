# memchr integration plan

> `memchr` 2.8 — Optimized byte/substring search using SIMD.
> Already vendored as a transitive dependency of regex and reedline.

## What it does

Highly optimized routines for searching bytes in slices:

- `memchr(needle, haystack)` — find first occurrence of a byte
- `memchr2`, `memchr3` — find first of 2 or 3 bytes
- `memrchr` — reverse search
- `memmem` — substring search (Two-Way + SIMD)

Typically 2-10x faster than naive iteration for string searching.

## Where it fits in newr

### R string functions that search

| R function | How memchr could help |
| ---------- | -------------------- |
| `grep(pattern, x, fixed=TRUE)` | `memmem::find(haystack, needle)` for literal pattern search |
| `grepl(pattern, x, fixed=TRUE)` | Same — fast literal contains check |
| `chartr(old, new, x)` | `memchr` to find bytes to translate |
| `startsWith(x, prefix)` | Not needed — slice comparison is already O(n) |
| `strsplit(x, split, fixed=TRUE)` | `memmem::find_iter()` to find all split points |
| `nchar(x, type="bytes")` | Not needed — `str.len()` is already O(1) |
| `which(x == "value")` | memchr on the internal string buffer |

### The key win: `fixed=TRUE`

When `grep/grepl/sub/gsub` are called with `fixed=TRUE`, we currently compile a regex with `regex::escape()`. Using `memchr::memmem` instead avoids regex overhead entirely:

```rust
use memchr::memmem;

fn fixed_grep(pattern: &str, text: &str) -> bool {
    memmem::find(text.as_bytes(), pattern.as_bytes()).is_some()
}

fn fixed_gsub(pattern: &str, replacement: &str, text: &str) -> String {
    let finder = memmem::Finder::new(pattern.as_bytes());
    // ... replace all occurrences using finder.find_iter()
}
```

## Implementation order

1. Use `memmem::Finder` for `grep/grepl` with `fixed=TRUE`
2. Use `memmem::Finder` for `sub/gsub` with `fixed=TRUE`
3. Use `memmem::find_iter` for `strsplit` with `fixed=TRUE`
4. Benchmark against regex with escaped literal

## Priority

Low — performance optimization. The regex crate already handles `fixed=TRUE` reasonably well. This would matter for very large text processing. Already vendored, zero build cost.
