# Nice Error Message for ..0

Add a helpful error message when users try `..0`, since R uses 1-based indexing for `...` arguments.

## Current behavior

`..0` is parsed as a dotdot token with index 0. At runtime, it likely produces a confusing index error or silently returns the wrong thing.

## Desired behavior

```r
> f <- function(...) ..0
> f(1, 2, 3)
Error: ..0 is not valid — R uses 1-based indexing for ... arguments.
  Did you mean ..1? (..1 is the first element, ..2 is the second, etc.)
```

## Implementation

In `src/interpreter.rs`, in the dotdot evaluation path, check for index 0 specifically and return a descriptive error:

```rust
Expr::DotDot(n) => {
    if *n == 0 {
        return Err(RError::Index(
            "..0 is not valid — R uses 1-based indexing for ... arguments. \
             Did you mean ..1? (..1 is the first element, ..2 is the second, etc.)"
            .to_string(),
        ));
    }
    // existing ..n logic
}
```

## Scope

- Single-line change in the interpreter's dotdot handler
- Add a test in `tests/dotdot.R` that verifies the error message
