# unicode-segmentation integration plan

> `unicode-segmentation` 1.12 — Unicode grapheme cluster and word boundary iteration.
> Already vendored as a transitive dependency of reedline.

## What it does

Implements Unicode Standard Annex #29: text segmentation. Provides iterators for:

- **Grapheme clusters** — what users perceive as "characters" (handles combining marks, emoji, etc.)
- **Word boundaries** — natural word breaks
- **Sentence boundaries** — sentence detection

## Where it fits in miniR

### The problem

R's `nchar()` can count in three modes:

- `nchar(x, type="bytes")` — byte count (trivial: `s.len()`)
- `nchar(x, type="chars")` — Unicode scalar count (trivial: `s.chars().count()`)
- `nchar(x, type="width")` — display width (needs `unicode-width`)

But the "chars" count is wrong for grapheme clusters. `nchar("e\u0301")` should return 1 (one visible character: é), but `chars().count()` returns 2. Real R returns 1 for `nchar("e\u0301", type="chars")` — it counts grapheme clusters.

### Integration points

| R function | unicode-segmentation API |
| ---------- | ------------------------ |
| `nchar(x, type="chars")` | `x.graphemes(true).count()` |
| `substring(x, first, last)` | Iterate graphemes, slice by grapheme index |
| `strsplit(x, "")` | `x.graphemes(true).collect()` — split into grapheme clusters |
| `substr(x, start, stop)` | Grapheme-aware substring extraction |
| `chartr(old, new, x)` | Grapheme-aware character translation |

### Example

```rust
use unicode_segmentation::UnicodeSegmentation;

fn r_nchar(s: &str) -> usize {
    s.graphemes(true).count()  // true = extended grapheme clusters
}

fn r_substring(s: &str, first: usize, last: usize) -> String {
    s.graphemes(true)
        .skip(first - 1)  // R is 1-indexed
        .take(last - first + 1)
        .collect()
}

fn r_strsplit_empty(s: &str) -> Vec<String> {
    s.graphemes(true).map(|g| g.to_string()).collect()
}
```

## Implementation order

1. Fix `nchar()` to use `graphemes(true).count()` for `type="chars"`
2. Fix `substring()` / `substr()` to use grapheme-aware indexing
3. Fix `strsplit(x, "")` to split on grapheme boundaries
4. Audit all string functions for grapheme correctness

## Priority

Medium — correctness issue. Our current `nchar()` is wrong for combining characters and emoji. This is already vendored, so it's free to use.
