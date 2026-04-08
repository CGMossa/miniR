+++
title = "Divergences from GNU R"
weight = 12
description = "Intentional differences from GNU R's behavior"
+++

miniR intentionally diverges from GNU R where R's behavior is confusing,
inconsistent, or unnecessarily restrictive. This is not a drop-in replacement
--- it's a fresh implementation that respects R's useful semantics while fixing
the bad ones.

## Language Improvements

### Trailing commas everywhere

GNU R rejects trailing commas in function calls and definitions. miniR allows them in all contexts:

```r
c(1, 2, 3,)
list(a = 1, b = 2,)
data.frame(x = 1:3, y = 4:6,)
function(a, b,) a + b
```

### `data.frame()` forward references

miniR evaluates columns left-to-right, so later columns can reference earlier ones:

```r
data.frame(x = 1:5, xx = x * x)   # works in miniR
data.frame(a = 1:3, b = a + 10L, c = a + b)
```

### Unified pipe operators

`|>` and `%>%` are identical in miniR --- both support `_` and `.` placeholders. No `library(magrittr)` needed:

```r
x |> f(a, _)      # R 4.2+ style
x %>% f(a, .)     # magrittr style --- same thing
```

Also native: `%<>%` (assignment pipe), `%T>%` (tee pipe), `%$%` (exposition pipe).

### `if...else` across newlines

```r
if (TRUE) 1
else 2           # works in miniR (GNU R rejects this)
```

### `**` as power operator

```r
2 ** 3  # 8 --- synonym for ^
```

## Error Messages

miniR aims for better error messages than GNU R --- more informative, more specific, with suggestions for how to fix the problem.

### Tracebacks show C frames

GNU R tracebacks only show opaque `.Call` entries. miniR unwinds the native stack with DWARF source locations:

```
Error: value must be non-negative, got -5
Traceback (most recent call last):
2: validate(x)
   [C] deep_helper at test.c:36 (stacktest.dylib)
   [C] C_validate at test.c:47 (stacktest.dylib)
1: run_check(-5)
```
