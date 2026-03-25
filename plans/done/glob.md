# glob integration plan

> `glob` 0.3 -- Unix shell-style glob pattern matching.
> <https://github.com/rust-lang/glob>

## What it does

Matches file paths against shell-style patterns (`*`, `?`, `[...]`).
Returns an iterator of matching paths.

```rust
use glob::glob;

for entry in glob("src/**/*.rs").unwrap() {
    match entry {
        Ok(path) => println!("{}", path.display()),
        Err(e) => eprintln!("{}", e),
    }
}
```

Key API:
- `glob(pattern)` -- iterate over matching paths
- `Pattern::new(pattern)` -- compile a pattern for reuse
- `Pattern::matches(path)` -- test if a path matches

Supports: `*` (any chars except `/`), `**` (recursive), `?` (one char),
`[abc]` (character class), `[!abc]` (negated class).

## How it differs from globset

`glob` (this crate) does filesystem traversal -- it walks directories and
returns matching file paths. `globset` only does pattern matching against
strings, without touching the filesystem.

For `Sys.glob()`, we need actual filesystem traversal, so `glob` is the
right crate. `globset` is useful for `list.files(pattern=)` where we
already have a list of paths and want to filter them.

## Where it fits in miniR

### `Sys.glob()` -- expand glob patterns to file paths

```r
Sys.glob("*.R")                 # all .R files in working dir
Sys.glob("data/*.csv")          # CSV files in data/
Sys.glob("src/**/*.rs")         # recursive Rust source files
Sys.glob(c("*.R", "*.r"))      # vectorized: multiple patterns
```

Implementation:

```rust
fn builtin_sys_glob(args: &CallArgs, ctx: &mut BuiltinContext) -> Result<RValue> {
    let patterns = args.required_character(0)?;
    let mut results = Vec::new();
    let wd = ctx.interpreter().get_working_dir();
    for pattern in patterns {
        let full_pattern = wd.join(pattern);
        for entry in glob::glob(full_pattern.to_str().unwrap())? {
            results.push(entry?.to_string_lossy().to_string());
        }
    }
    Ok(RValue::from(Vector::character(results)))
}
```

### `path.expand()` -- tilde expansion (partial)

While `glob` does not do tilde expansion, the glob infrastructure
is useful alongside `dirs::home_dir()` for `path.expand("~/...")`.

## Status

Already a direct dependency in Cargo.toml (`glob = "0.3"`). Used
internally but `Sys.glob()` builtin may not be wired up yet.

## Implementation

1. Implement `Sys.glob(paths)` builtin in `builtins/os.rs` (or wherever OS builtins live)
2. Use `ctx.interpreter().get_working_dir()` for relative pattern resolution
3. Return character vector of matched paths, sorted
4. Handle `Sys.glob(c("*.R", "*.csv"))` -- vectorized over patterns

## Priority

Medium -- `Sys.glob()` is commonly used in R scripts that process
batches of files. Already vendored as a direct dependency.
