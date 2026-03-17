# Parser: newline continuation in postfix chains

## Issue

GNU R treats `x\n(y)` as two separate expressions, but miniR parses it as
`x(y)` (a function call). Similarly, `x\n$y` becomes dollar access and
`pkg\n::foo` becomes namespace access.

## Why we diverge

R's parser uses a stateful lexer that tracks whether the current expression
is "complete". A newline after a complete expression terminates it; a newline
after an incomplete expression (open paren, trailing operator, etc.) continues
it. This means:

```r
paste         # complete expression → newline terminates
  (x, y)      # new expression → parse error if standalone

paste(x,      # incomplete (open paren) → newline continues
  y)           # continuation → ok
```

Our PEG grammar (pest) is stateless — it can't distinguish "complete" from
"incomplete" expressions at the lexer level. The choice is:

1. **Allow GAP (with newlines) before postfix suffixes** — This means
   `x\n(y)` becomes a call. Diverges from GNU R on this edge case, but
   correctly parses 7014/7014 CRAN R files.

2. **Restrict to horizontal whitespace** — This correctly rejects `x\n(y)`
   but breaks real CRAN code like `paste\n(...)` in base R's logLik.R.

We chose option 1 because CRAN compatibility is more valuable than strict
GNU R newline semantics. The `x\n(y)` pattern almost never appears in real
code as "two expressions" — when it does appear, the `x` is always intended
as a function being called.

## Same reasoning applies to

- `if (cond) expr\nelse expr` — we accept this (GNU R rejects it)
- `x\n$y` — we parse as dollar access (GNU R rejects it)
- `pkg\n::foo` — we parse as namespace access (GNU R rejects it)

## Mitigation

A future improvement could use a two-pass approach: parse greedily, then
validate that cross-line postfix chains are intentional (e.g., preceded by
an obviously-incomplete expression). But this is low priority given 0
CRAN failures.

## Other known parser limitations

### `?` inside assignment RHS

`x <- ?sin` doesn't work because `help_expr` wraps the entire precedence
chain — it's the outermost level. The RHS of `<-` recurses into
`assign_left_expr`, which doesn't include `help_expr`. In GNU R,
`x <- ?sin` assigns the help result to x. In miniR, the `?` is not
reachable in that position. Low priority since `?` is interactive-only.

### Binary `?` drops the RHS

`foo ? bar` (type-based help lookup) parses but the AST builder
(`builder.rs:137`) discards the RHS, producing just `foo`. This is
because binary `?` is rarely used and the help system isn't fully
implemented. The grammar correctly accepts it; the builder just
doesn't preserve the topic.

### `~~` and `:=` are parsed but stubbed

- `~~` (plotmath spacing) parses as `BinaryOp::DoubleTilde` but evaluates
  to `NULL` at runtime (`ops.rs`).
- `:=` (data.table/rlang walrus) parses as `BinaryOp::Special(Walrus)` but
  has no runtime semantics. Needs data.table/rlang metaprogramming support
  to be meaningful.
