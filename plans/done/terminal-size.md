# terminal_size integration plan

> `terminal_size` 0.4 -- Query terminal dimensions (rows and columns).
> <https://github.com/eminence/terminal-size>

## What it does

Cross-platform terminal size detection. Returns the number of columns and
rows of the terminal.

```rust
use terminal_size::{terminal_size, Width, Height};

if let Some((Width(w), Height(h))) = terminal_size() {
    println!("Terminal is {}x{}", w, h);
}
```

## Where it fits in miniR

### `getOption("width")` -- default output width

R's `options(width = 80)` controls how wide output is formatted. The
default should match the terminal width:

```r
getOption("width")  # typically 80, or actual terminal width
```

On interpreter startup:

```rust
let width = terminal_size::terminal_size()
    .map(|(Width(w), _)| w as i64)
    .unwrap_or(80);
interpreter.set_option("width", RValue::from(width));
```

### `format()` / `print()` -- adaptive output width

Data frames, matrices, and long vectors wrap their output based on
`getOption("width")`. Knowing the actual terminal width makes
the default formatting work well out of the box.

### `cat()` with `fill` parameter

```r
cat("a", "b", "c", ..., fill = TRUE)  # wraps at getOption("width")
cat("a", "b", "c", ..., fill = 40)    # wraps at 40 columns
```

When `fill = TRUE`, cat uses the terminal width.

## Status

Already vendored as a transitive dependency (via textwrap and miette).
Not a direct dependency -- would need to be added to Cargo.toml for
direct use at interpreter startup.

## Implementation

1. At interpreter startup, query `terminal_size()` and set `options(width = ...)`
2. Ensure `getOption("width")` is wired to the options system
3. Use width in print/format methods for data frames, matrices, long vectors

## Priority

Low -- a nice default, but `options(width = 80)` works fine as a fallback.
Most R code does not depend on exact terminal width.
