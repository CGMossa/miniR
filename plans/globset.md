# globset integration plan

> `globset` 0.4 — Glob pattern matching by BurntSushi.
> <https://github.com/BurntSushi/globset>

## What it does

Fast glob pattern matching. Compiles glob patterns to regex for efficient matching.
Supports `*`, `**`, `?`, `[abc]`, `{a,b}`, and negation.

```rust
use globset::{Glob, GlobSet, GlobSetBuilder};

let glob = Glob::new("*.R")?.compile_matcher();
assert!(glob.is_match("script.R"));

// Multiple patterns at once:
let mut builder = GlobSetBuilder::new();
builder.add(Glob::new("*.R")?);
builder.add(Glob::new("*.r")?);
let set = builder.build()?;
assert!(set.is_match("test.R"));
```

## Where it fits in newr

### 1. `Sys.glob()` — file glob expansion

```r
Sys.glob("*.R")              # all R files in current dir
Sys.glob("src/**/*.rs")      # recursive glob
Sys.glob("data/*.{csv,tsv}") # multiple extensions
```

```rust
fn builtin_sys_glob(pattern: &str) -> Vec<String> {
    let glob = Glob::new(pattern).unwrap().compile_matcher();
    // walk directory and filter with glob.is_match()
}
```

### 2. `list.files(pattern=)` — filtered file listing

```r
list.files(pattern = "*.csv")  # glob filter on file listing
```

### 3. `.Rprofile` / `.gitignore`-style patterns

Config files may use glob patterns for include/exclude rules.

## Relationship to builtins plan

| Phase | Builtins affected | Impact |
|---|---|---|
| Phase 6 (OS) | `Sys.glob()` | glob expansion |
| Phase 6 (OS) | `list.files(pattern=)` | filtered directory listing |

## Recommendation

**Add when implementing `Sys.glob()` or `list.files()`.** Pairs naturally with
`walkdir` for recursive directory traversal + glob filtering.

**Effort:** 30 minutes for Sys.glob.
