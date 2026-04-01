# Divergences from GNU R

miniR intentionally diverges from GNU R where R's behavior is confusing,
inconsistent, or unnecessarily restrictive. This is not a drop-in replacement
— it's a fresh implementation that respects R's useful semantics while fixing
the bad ones.

## Language Improvements

### Trailing commas everywhere

GNU R rejects trailing commas in function calls and definitions:

```r
# GNU R: Error in c(1, 2, 3, ) : argument 4 is empty
c(1, 2, 3,)
```

miniR allows trailing commas in all contexts — function calls, function
definitions, `c()`, `list()`, `data.frame()`, etc. This matches the
convention in most modern languages and eliminates diff noise when adding
items to the end of a list.

```r
# miniR: works fine
c(1, 2, 3,)
list(a = 1, b = 2,)
data.frame(x = 1:3, y = 4:6,)
function(a, b,) a + b
```

### `data.frame()` forward references

GNU R evaluates each `data.frame()` column independently — later columns
cannot reference earlier ones:

```r
# GNU R: Error in data.frame(x = 1:5, xx = x * x) : object 'x' not found
data.frame(x = 1:5, xx = x * x)
```

miniR evaluates columns left-to-right in a child environment, so each named
column is visible to subsequent column expressions. This matches the behavior
of `dplyr::tibble()`:

```r
# miniR: works, xx = c(1, 4, 9, 16, 25)
data.frame(x = 1:5, xx = x * x)

# Chaining works too
data.frame(a = 1:3, b = a + 10L, c = a + b)
```

Column bindings do not leak into the caller's environment.

### Unified pipe operators

In GNU R, `|>` (base) and `%>%` (magrittr) are different implementations with
different features. miniR unifies them — **`|>` and `%>%` are identical** and
both support `_` and `.` as placeholders:

```r
x |> f(a, _)      # works (R 4.2+ style)
x |> f(a, .)      # also works (magrittr style)
x %>% f(a, .)     # same thing — no library(magrittr) needed
```

The magrittr pipe variants are also native operators:

| Operator | Purpose |
|---|---|
| `%<>%` | Assignment pipe — pipe and assign back to LHS |
| `%T>%` | Tee pipe — pipe for side effect, return original LHS |
| `%$%` | Exposition pipe — expose LHS names to RHS expression |

All available without `library(magrittr)`.

### `if...else` across newlines

GNU R requires braces when `else` is on a new line:

```r
# GNU R: unexpected 'else'
if (TRUE) 1
else 2
```

miniR accepts this — `else` on a new line is unambiguous when preceded by
a complete `if` expression.

### `**` as power operator

miniR accepts `**` as a synonym for `^` (exponentiation). GNU R does not
recognize `**`.

```r
# miniR: 8
2 ** 3
```

## Parser Divergences

### Newline continuation in postfix chains

`x\n(y)` parses as a call `x(y)`, not two separate expressions. Same for
`x\n$y` and `pkg\n::foo`. This is required for CRAN compatibility — many
packages split chained expressions across lines.

### `?` is top-level only

`x <- ?sin` doesn't work — `?` is only at the top level of the precedence
chain. In GNU R, `?` can appear in expression position. Low priority since
`?` is interactive-only.

## Semantic Differences

### `<<-` creates in global, not parent

GNU R's `<<-` walks up the environment chain and creates the binding in the
first enclosing environment where it exists, or the global environment if not
found. miniR always creates missing bindings in the global environment.

### Error messages

miniR aims for better error messages than GNU R — more informative, more
specific, with suggestions for how to fix the problem. This means error
message strings will not match GNU R exactly.

### Tracebacks show C frames with source locations

GNU R tracebacks only show R-level call frames and opaque `.Call` entries:

```
Error in validate(x) : value must be non-negative, got -5
Calls: run_check -> validate -> .Call
Execution halted
```

miniR unwinds the native stack and shows individual C function frames with
DWARF file:line info (when debug symbols are available):

```
Error: value must be non-negative, got -5
Traceback (most recent call last):
2: validate(x)
   [C] deep_helper at test.c:36 (stacktest.dylib)
   [C] middle_helper at test.c:42 (stacktest.dylib)
   [C] C_validate at test.c:47 (stacktest.dylib)
1: run_check(-5)
```

This makes debugging native code issues significantly easier — you can see
exactly which C function errored and where, not just that `.Call` was invoked.

## Serialization

`readRDS()` / `saveRDS()` / `load()` / `save()` support GNU R's binary
XDR format (version 2/3, gzip-compressed) for reading. The miniR-specific
text format (`miniRDS1`) is also supported for round-tripping within miniR.

## Not Yet Implemented

These are not divergences — just features that haven't landed yet:

- Raster graphics devices (`png()` falls back to SVG, `jpeg()`/`bmp()` missing)
- Some device management functions (`dev.list()`, `postscript()`)
