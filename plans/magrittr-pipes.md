# Magrittr-style Pipe Operators as Native Builtins

## Summary

Add magrittr's pipe variants as native operators with cleaner syntax — no `%` delimiters.
This is a miniR divergence: R requires `%>%`, `%<>%`, `%T>%`, `%$%` from the magrittr package.
miniR makes them built-in operators with shorter symbols: `|>`, `<>`, `T>`, `$>`.

## Operators

| miniR | magrittr | Name | Semantics |
|---|---|---|---|
| `\|>` | `%>%` | Pipe | Forward pipe with `.` placeholder (already implemented, uses `_`) |
| `<>` | `%<>%` | Assignment pipe | Pipe and assign result back to LHS |
| `T>` | `%T>%` | Tee pipe | Pipe forward but return the LHS (for side effects) |
| `$>` | `%$>%` | Exposition pipe | Expose LHS names to RHS expression (like `with()`) |

## Divergence from GNU R

- These are **native operators** parsed by the grammar, not infix functions loaded from a package
- No `%` delimiters — `<>`, `T>`, `$>` are first-class tokens
- Both `_` and `.` work as placeholders in all pipe variants (R only supports `_` in `|>`, `.` in `%>%`)
- Available without `library(magrittr)` — always loaded

## Semantics

### `<>` — Assignment pipe

```r
x <- c(3, 1, 2)
x <> sort()       # equivalent to: x <- x |> sort()
x <> sqrt()       # equivalent to: x <- x |> sqrt()
# x is now sorted and square-rooted
```

Implementation: evaluate the pipe chain, then assign the result back to the LHS symbol.
LHS must be a simple symbol or indexing expression (same targets as `<-`).

### `T>` — Tee pipe

```r
data <>
  transform(z = x + y) T>
  plot(z ~ x, data = _) |>  # plot is called for side effect, result discarded
  summary()                   # summary gets the transform() result, not plot() result
```

Implementation: evaluate `lhs T> rhs` as `{ rhs(lhs); lhs }` — the RHS is called for
its side effect (printing, plotting, logging), but the LHS value is forwarded.

### `$>` — Exposition pipe

```r
data.frame(x = 1:10, y = rnorm(10)) $> cor(x, y)
# equivalent to: with(df, cor(x, y))
# exposes column names as variables in the RHS expression
```

Implementation: create a child environment containing the LHS's named elements
(for data frames: columns; for lists: elements; for environments: bindings).
Evaluate RHS in that child environment.

## Parser Changes

Add tokens to the PEG grammar (`src/parser/r.pest`):

```pest
assign_pipe = { "<>" }
tee_pipe    = { "T>" }
expo_pipe   = { "$>" }
```

These need to be at the same precedence level as `|>` (between `%any%` and `+/-`).

Potential conflicts:
- `<>` could conflict with `!=` or comparison operators — but `<>` is not valid R syntax today
- `T>` starts with `T` which is a symbol — need to ensure `T > 5` (comparison) is not parsed as tee pipe. Solution: `T>` is only a pipe when followed by a call or symbol, not when `T` is followed by space+`>`
- `$>` starts with `$` — `obj$>` could conflict with dollar access. Solution: `$>` is only parsed as pipe at expression level, not after `$` accessor

## AST Changes

Add to `BinaryOp` enum in `src/parser/ast.rs`:

```rust
AssignPipe,  // <>
TeePipe,     // T>
ExpoPipe,    // $>
```

## Interpreter Changes

Add to `eval_binary_op` in `src/interpreter.rs` or `control_flow.rs`:

### AssignPipe (`<>`)
```rust
BinaryOp::AssignPipe => {
    let result = self.eval_pipe(lhs, rhs, env)?;
    self.eval_assign(&AssignOp::LeftArrow, lhs, &Expr::from_value(result), env)?;
    Ok(result)
}
```

### TeePipe (`T>`)
```rust
BinaryOp::TeePipe => {
    let left_val = self.eval_in(lhs, env)?;
    // Evaluate the pipe for side effects, discard result
    let _ = self.eval_pipe_with_value(left_val.clone(), rhs, env)?;
    Ok(left_val) // return original value
}
```

### ExpoPipe (`$>`)
```rust
BinaryOp::ExpoPipe => {
    let left_val = self.eval_in(lhs, env)?;
    // Create child env with LHS's named elements
    let child_env = Environment::new_child(env);
    expose_names(&left_val, &child_env);
    self.eval_in(rhs, &child_env)
}
```

Where `expose_names` binds list/data.frame columns or environment bindings into the child env.

## Placeholder Unification

Both `_` and `.` should work as placeholders in ALL pipe variants:

```r
x |> f(a, _)    # already works
x |> f(a, .)    # should also work (magrittr compat)
x <> f(a, .)    # assignment pipe with placeholder
```

In the pipe evaluator, check for both `Symbol("_")` and `Symbol(".")` as placeholders.

## Implementation Order

1. Add `.` as additional placeholder in `|>` (alongside `_`)
2. Parse `<>` as assignment pipe
3. Implement assignment pipe semantics
4. Parse `T>` as tee pipe (careful with `T > x` ambiguity)
5. Implement tee pipe semantics
6. Parse `$>` as exposition pipe (careful with `obj$>` ambiguity)
7. Implement exposition pipe (reuse `with()` logic)
8. Tests for all variants
9. Document divergences
