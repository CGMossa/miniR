# dirs integration plan

> `dirs` 6.0 — Platform-specific standard directories.
> https://github.com/dirs-dev/dirs-rs

## What it does

Returns platform-specific paths for home, config, cache, data, etc. 18 functions:
`home_dir()`, `config_dir()`, `data_dir()`, `cache_dir()`, `config_local_dir()`,
`data_local_dir()`, `runtime_dir()`, etc.

No dependencies, no allocations beyond PathBuf.

## Where it fits in newr

### 1. `path.expand()` — tilde expansion

R's `path.expand("~/foo")` expands `~` to the user's home directory:

```rust
fn path_expand(path: &str) -> String {
    if path.starts_with("~/") || path == "~" {
        if let Some(home) = dirs::home_dir() {
            return path.replacen("~", &home.to_string_lossy(), 1);
        }
    }
    path.to_string()
}
```

### 2. `Sys.getenv("HOME")` / `R.home()`

- `Sys.getenv("HOME")` → `dirs::home_dir()`
- `R.home()` → could use `dirs::data_dir()` / "newr" for package library location

### 3. `.libPaths()` — package library search paths

Default library path is typically `~/R/library` or platform-specific data dir.
`dirs::data_dir()` gives the right base on each platform.

### 4. `tempdir()` / `tempfile()`

R's `tempdir()` returns the session temp directory. While `std::env::temp_dir()`
works, `dirs::cache_dir()` can provide a fallback.

### 5. `.Rprofile` / `.Renviron` location

R looks for startup files in `~/.Rprofile` and `~/.Renviron`. We'd look in
`dirs::home_dir()` for these.

## Relationship to builtins plan

| Phase | Builtins affected | Impact |
|---|---|---|
| Phase 6 (OS/env) | `path.expand()`, `Sys.getenv("HOME")`, `R.home()` | correct home dir |
| Phase 6 (OS/env) | `tempdir()`, `tempfile()` | temp directory |
| Phase 11 (I/O) | `.libPaths()`, startup files | package/config locations |

## Recommendation

**Add when implementing path.expand() or R.home().** Pure utility, zero-dep,
cross-platform. Better than hardcoding `$HOME` or `env::var("HOME")`.

**Effort:** 5 minutes to add, used incrementally across builtins.
