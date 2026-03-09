# walkdir integration plan

> `walkdir` 2.5 — Recursive directory traversal by BurntSushi.
> <https://github.com/BurntSushi/walkdir>

## What it does

Efficient recursive directory walking with sorting, depth limits, symlink
following, and error handling.

```rust
use walkdir::WalkDir;

for entry in WalkDir::new("src").max_depth(2).follow_links(true) {
    let entry = entry?;
    println!("{}", entry.path().display());
}
```

## Where it fits in newr

### 1. `list.files(recursive=TRUE)`

```r
list.files("src", recursive=TRUE)              # all files recursively
list.files("src", pattern="\\.rs$", recursive=TRUE)  # filtered
list.files(".", full.names=TRUE, recursive=TRUE)
```

```rust
fn list_files(path: &str, recursive: bool, pattern: Option<&str>) -> Vec<String> {
    let walker = WalkDir::new(path)
        .max_depth(if recursive { usize::MAX } else { 1 })
        .follow_links(true);
    walker.into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| matches_pattern(e, pattern))
        .map(|e| e.path().to_string_lossy().to_string())
        .collect()
}
```

### 2. `list.dirs()`

```r
list.dirs(".", recursive=TRUE)  # all subdirectories
```

Same as above but filter for `is_dir()`.

### 3. `Sys.glob()` with `**`

Double-star glob (`**/*.R`) requires recursive traversal. walkdir + globset
together handle this:

```rust
let glob = Glob::new("**/*.R")?.compile_matcher();
for entry in WalkDir::new(".") {
    if glob.is_match(entry?.path()) { /* ... */ }
}
```

## Relationship to builtins plan

| Phase | Builtins affected | Impact |
|---|---|---|
| Phase 6 (OS) | `list.files(recursive=TRUE)` | recursive file listing |
| Phase 6 (OS) | `list.dirs()` | directory listing |
| Phase 6 (OS) | `Sys.glob("**/*")` | recursive glob |

## Recommendation

**Add when implementing `list.files(recursive=TRUE)`.** Pairs with `globset` for
pattern filtering. walkdir handles all the edge cases (symlinks, permissions,
cross-platform paths).

**Effort:** 30 minutes for list.files + list.dirs.
