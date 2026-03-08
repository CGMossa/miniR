# aho-corasick + memchr integration plan

> `aho-corasick` — Fast multi-pattern string matching by BurntSushi.
> https://github.com/BurntSushi/aho-corasick
>
> `memchr` — Optimized byte search by BurntSushi.
> https://github.com/BurntSushi/memchr

## What they do

**memchr**: SIMD-accelerated byte search. `memchr(b'x', haystack)` finds a byte
in a slice, 2-10x faster than `slice.iter().position()`. Also `memchr2`, `memchr3`
for searching 2-3 bytes simultaneously, and `memmem` for substring search.

**aho-corasick**: Searches for multiple patterns simultaneously in a single pass.
Built on the Aho-Corasick automaton algorithm. O(n + m) where n is text length
and m is total matches.

```rust
use aho_corasick::AhoCorasick;

let patterns = &["apple", "banana", "cherry"];
let ac = AhoCorasick::new(patterns)?;
let matches: Vec<_> = ac.find_iter("I like apple and banana").collect();
```

## Where they fit in newr

### 1. `grep()` / `grepl()` with `fixed=TRUE`

R's `grep(pattern, x, fixed=TRUE)` does literal string matching. `memchr::memmem`
is the fastest way to do this:

```rust
use memchr::memmem;

fn grep_fixed(pattern: &str, x: &[String]) -> Vec<usize> {
    let finder = memmem::Finder::new(pattern);
    x.iter().enumerate()
        .filter(|(_, s)| finder.find(s.as_bytes()).is_some())
        .map(|(i, _)| i)
        .collect()
}
```

### 2. `grep()` with multiple patterns

When searching for multiple fixed patterns (common in R pipelines):

```r
# Find lines matching any of these patterns
grep("apple|banana|cherry", text, fixed=FALSE)
```

With aho-corasick, the multi-pattern case is faster than regex:

```rust
let ac = AhoCorasick::new(&["apple", "banana", "cherry"])?;
let matches: Vec<usize> = text.iter().enumerate()
    .filter(|(_, line)| ac.is_match(line))
    .map(|(i, _)| i)
    .collect();
```

### 3. `chartr()` — translate characters

R's `chartr(old, new, x)` translates characters. For single-byte translations,
memchr finds positions quickly.

### 4. `strsplit()` with fixed delimiter

`strsplit(x, ",")` splits on a literal string. memchr's `memmem::find_iter()`
finds all split positions efficiently.

### 5. Already transitive dependencies

Both `aho-corasick` and `memchr` are dependencies of the `regex` crate. They're
already in our dependency tree if we use `regex`. Direct use gives access to
the fixed-string fast paths.

## Relationship to builtins plan

| Phase | Builtins affected | Impact |
|---|---|---|
| Phase 2 (strings) | `grep(fixed=TRUE)`, `grepl(fixed=TRUE)` | fast literal matching |
| Phase 2 (strings) | `sub(fixed=TRUE)`, `gsub(fixed=TRUE)` | fast literal replacement |
| Phase 2 (strings) | `strsplit(fixed=TRUE)` | fast literal splitting |
| Phase 2 (strings) | `chartr()` | character translation |
| Phase 5 (regex) | multi-pattern grep | simultaneous pattern search |

## Recommendation

**Available as transitive deps of `regex`.** Add as direct dependencies when
implementing `grep(fixed=TRUE)` and other fixed-string operations — the fast
paths matter for performance on large text data.

**Effort:** 30 minutes to add fixed-string fast paths to grep/gsub/strsplit.
