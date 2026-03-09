# indicatif integration plan

> `indicatif` 0.18 — Progress bars and spinners for CLI applications.
> <https://github.com/console-rs/indicatif>

## What it does

Renders progress bars, spinners, and multi-progress displays in the terminal.

```rust
let pb = ProgressBar::new(1000);
for i in 0..1000 {
    pb.inc(1);
    // do work
}
pb.finish_with_message("done");
```

Styles: `[####----] 50% (ETA: 2s)`, spinners, multi-bar for parallel tasks.

## Where it fits in newr

### 1. `apply()` family on large data

When `sapply()` / `lapply()` runs over millions of elements, a progress bar
helps users know something is happening:

```r
sapply(1:1e6, slow_function)  # [========>        ] 45% ETA: 12s
```

### 2. `source()` — progress for large scripts

Show progress when sourcing a long R script with many expressions.

### 3. Package installation

If we implement `install.packages()`, progress bars for download + install.

### 4. `txtProgressBar()` — R's built-in progress bar

R has `txtProgressBar(min, max, style)` and `setTxtProgressBar(pb, value)`.
indicatif can back this directly.

## Relationship to builtins plan

| Phase | Builtins affected | Impact |
|---|---|---|
| Phase 3 (collections) | `sapply()`, `lapply()`, `vapply()` | progress on large data |
| Phase 11 (I/O) | `txtProgressBar()`, `setTxtProgressBar()` | R progress bar API |

## Recommendation

**Add when implementing txtProgressBar() or when large-data operations become
common.** Nice quality-of-life feature but not essential for correctness.

**Effort:** 1 hour for txtProgressBar, incremental for apply progress.
