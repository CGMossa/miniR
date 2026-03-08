# indexmap integration plan

> `indexmap` 2.13 — Insertion-order-preserving hash map.
> https://github.com/indexmap-rs/indexmap

## What it does

`IndexMap<K, V>` — hash map that preserves insertion order. Same API as `HashMap`
plus index-based access (`get_index(i)`, `swap_remove`, `shift_remove`).
`IndexSet<T>` — ordered hash set.

O(1) lookup by key, O(1) lookup by index. Iteration in insertion order.

## Where it fits in newr

### 1. Named lists — the #1 use case

R named lists preserve insertion order:

```r
x <- list(a = 1, b = 2, c = 3)
names(x)  # "a" "b" "c" — always in insertion order
x[["b"]]  # 2 — O(1) by name
x[[2]]    # 2 — O(1) by index
```

Currently we use `Vec<(Option<String>, RValue)>` which is O(n) for name lookup.
`IndexMap<String, RValue>` gives O(1) name lookup AND preserves insertion order
AND supports index-based access.

### 2. Attributes

R object attributes are a named list with preserved order:

```r
attr(x, "names")   # O(1) lookup
attributes(x)      # returns in insertion order
```

### 3. Environments (maybe)

R environments use hashed lookup but `ls()` returns names in a defined order.
`IndexMap` could replace `HashMap` in `EnvInner` if ordering matters.

### 4. Data frame columns

Data frames are named lists of columns. Column lookup by name (`df$col`) needs
to be fast, and column order matters for display.

## Relationship to builtins plan

| Phase | Builtins affected | Impact |
|---|---|---|
| Core (lists) | `list()`, `names()`, `[[`, `$` | O(1) named access |
| Core (attrs) | `attr()`, `attributes()`, `structure()` | ordered attributes |
| Phase 3 (collections) | `match()`, `which()` on named vectors | fast name matching |

## Recommendation

**Add now.** This is a fundamental data structure improvement. Named lists are
everywhere in R — function arguments, attributes, data frames. The current
`Vec<(Option<String>, RValue)>` is O(n) for every name lookup.

**Effort:** 1-2 hours to replace list internals + update accessors.
