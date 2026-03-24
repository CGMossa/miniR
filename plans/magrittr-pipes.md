# Pipe Operators — Unifying |> and %>%

## Summary

In GNU R, `|>` (base, R 4.1+) and `%>%` (magrittr) are two separate pipe
implementations with different features. `|>` has `_` placeholder (R 4.2+) but
no `.`, no tee, no assignment pipe. `%>%` has `.` placeholder, tee, assignment,
exposition — but requires `library(magrittr)`.

miniR unifies them: **`|>` and `%>%` are the same operator** with all features
from both. No need for magrittr. The other magrittr pipe variants (`%<>%`,
`%T>%`, `%$%`) are also native operators.

## Divergence from GNU R

- `|>` and `%>%` are **identical** — both support `_` and `.` as placeholders
- `%<>%`, `%T>%`, `%$%` are **native operators**, not magrittr functions
- All pipes are always available — no `library(magrittr)` needed
- Both `_` and `.` work as placeholders in all pipe variants

## Operators

| Operator | Name | Semantics |
|---|---|---|
| `\|>` / `%>%` | Forward pipe | `x \|> f(y)` → `f(x, y)`. With placeholder: `x \|> f(y, _)` → `f(y, x)` |
| `%<>%` | Assignment pipe | `x %<>% sort()` → `x <- sort(x)`. Pipe and assign back. |
| `%T>%` | Tee pipe | `x %T>% print() \|> f()` → print x for side effect, forward x to f |
| `%$%` | Exposition pipe | `df %$% cor(x, y)` → evaluate RHS with df's columns exposed as variables |

## Placeholder Rules

Both `_` and `.` are recognized as placeholders in all pipe variants:

```r
x |> f(a, _)       # works
x |> f(a, .)       # also works (magrittr compat)
x %>% f(a, .)      # same thing
x %>% f(a, _)      # also works
```

If no placeholder is found in the top-level call args, LHS is prepended as the
first positional argument (current behavior).

If `.` is used **only inside nested calls** (not at top level), LHS is still
prepended as first arg AND `.` is available in nested positions. This matches
magrittr's behavior: `iris %>% subset(1:nrow(.) %% 2 == 0)` prepends iris as
first arg to subset, and `.` inside `nrow()` also refers to iris.

## Current State

- `|>` with `_` placeholder: ✅ implemented
- `|>` with `.` placeholder: not yet
- `%>%` as alias for `|>`: not yet (parser doesn't recognize `%>%` specially)
- `%<>%`: not yet
- `%T>%`: not yet
- `%$%`: not yet

## Implementation

### Phase 1: Unify |> and %>% placeholders

1. Add `.` as a placeholder alongside `_` in `eval_pipe()` in `control_flow.rs`
2. Register `%>%` in the parser as an alias for `|>` (it's already parsed as
   a `%any%` infix — just need the evaluator to treat it as a pipe)

### Phase 2: Assignment pipe %<>%

Parser: `%<>%` is already parseable as `%any%` infix. Evaluator needs to:
1. Evaluate the pipe chain: `result = eval_pipe(lhs_value, rhs)`
2. Assign result back to the LHS symbol: `eval_assign(<-, lhs, result)`

LHS must be a valid assignment target (symbol, index expr, dollar expr).

### Phase 3: Tee pipe %T>%

Evaluator:
1. Evaluate LHS
2. Call RHS with LHS as input (same as regular pipe)
3. Discard the RHS result
4. Return the original LHS value

Use case: side effects (plotting, printing, logging) in a pipe chain.

### Phase 4: Exposition pipe %$%

Evaluator:
1. Evaluate LHS (must be a list, data frame, or environment)
2. Create a child environment with LHS's named elements as bindings
3. Evaluate RHS in that child environment

This is essentially `with(lhs, rhs)` as an operator.

## Parser Notes

`%>%`, `%<>%`, `%T>%`, `%$%` are all valid `%any%` infix operators in the
existing grammar. No parser changes needed — just evaluator dispatch.

The evaluator currently handles `%any%` by looking up the operator name as a
function. For the pipe variants, the evaluator should intercept these specific
operator names and implement pipe semantics directly, rather than looking them
up as user-defined functions.

## Tests

```r
# |> and %>% are identical
stopifnot(identical(1:5 |> sum(), 1:5 %>% sum()))

# . placeholder
stopifnot("hello" %>% paste(., "world") == "hello world")
stopifnot("hello" |> paste(., "world") == "hello world")

# %<>% assignment pipe
x <- c(3, 1, 2)
x %<>% sort()
stopifnot(identical(x, c(1, 2, 3)))

# %T>% tee pipe
x <- c(3, 1, 2)
result <- x %T>% print() %>% sum()  # prints x, then sums x
stopifnot(result == 6)

# %$% exposition pipe
df <- data.frame(a = 1:5, b = 6:10)
result <- df %$% cor(a, b)
stopifnot(result == 1)  # perfect correlation
```
