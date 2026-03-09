# unicase integration plan

> `unicase` 2.9 — Unicode case-insensitive string comparison.
> Already vendored as a transitive dependency of reedline.

## What it does

Case-insensitive string comparison and hashing that properly handles Unicode case folding (not just ASCII tolower). `UniCase::new("Straße") == UniCase::new("STRASSE")`.

## Where it fits in newr

### R functions with `ignore.case`

Many R string functions have an `ignore.case` parameter:

| R function | Current implementation |
| ---------- | -------------------- |
| `grep(pattern, x, ignore.case=TRUE)` | We use `regex` crate's `(?i)` flag — already correct |
| `grepl(pattern, x, ignore.case=TRUE)` | Same — regex handles it |
| `sub/gsub(..., ignore.case=TRUE)` | Same — regex handles it |
| `match(x, table)` | No ignore.case support yet |
| `%in%` | No ignore.case support |
| `duplicated(x)` | No ignore.case support |
| `unique(x)` | No ignore.case support |
| `sort(x)` | `sort(x, method="radix")` doesn't have ignore.case, but locale-aware sorting does |
| `tolower(x)` / `toupper(x)` | Currently ASCII-only; should handle Unicode |

### Where unicase helps beyond regex

Regex already handles `ignore.case` for pattern matching. Unicase is useful for:

1. **`tolower()` / `toupper()`** — Unicode case conversion (though `str::to_lowercase()` already handles most cases)
2. **Case-insensitive HashMap lookups** — `UniCase` as HashMap key for case-insensitive environments
3. **`match()` with case folding** — comparing strings case-insensitively without regex overhead

### Example

```rust
use unicase::UniCase;

fn r_match_ignore_case(x: &str, table: &[&str]) -> Option<usize> {
    let key = UniCase::new(x);
    table.iter().position(|t| UniCase::new(t) == key).map(|i| i + 1)
}
```

## Priority

Low — regex handles the main use case (`ignore.case` in grep/grepl/sub/gsub). Useful for edge cases in string comparison functions. Already vendored, zero cost.
