# Interpreter Roadmap

A prioritized plan for bringing newr from "parser works" to "can run real R code."

## Current State (2026-03-08)

- Parser: **100%** — 12,399/12,399 CRAN R files parse
- Builtins: 336 registered, ~133 implemented, ~132 no-ops, ~37 stubs
- Language features: basic vectorized arithmetic, indexing, closures, control flow work
- Missing: S3/S4, attributes, matrices, data frames, factors, lazy eval, metaprogramming, regex

## Priority Framework

Ranked by: *how many CRAN package lines of code would start working*

---

## Phase 1: Attributes & Structure (CRITICAL)

Everything in R hangs on attributes. Classes, dimensions, names, factors — all are attributes.

### 1.1 Attribute storage

Add `attrs: Option<HashMap<String, RValue>>` to `RValue::Vector` and `RValue::List`.

Key attributes:
- `names` — vector/list element names
- `class` — S3 class (character vector)
- `dim` — matrix/array dimensions (integer vector)
- `dimnames` — row/column names
- `levels` — factor levels
- `row.names` — data frame row names

### 1.2 attr() / attributes() / structure()

Implement properly (currently stubs):
- `attr(x, "name")` — get/set single attribute
- `attr(x, "name") <- value` — replacement form
- `attributes(x)` — get all as list
- `attributes(x) <- list(...)` — set all
- `structure(x, class="foo", ...)` — set attributes inline

**CRAN usage:** `attr` 6,698 calls, `attributes` 804 calls, `structure` 1,501 calls

### 1.3 names() proper implementation

- `names(x)` — return names attribute
- `names(x) <- value` — set names (replacement function already partially works)
- Propagate names through operations (c, [, etc.)

**CRAN usage:** `names` 10,756 calls

### 1.4 class() and inherits()

- `class(x)` — return class attribute, or implicit class (numeric, character, etc.)
- `class(x) <- value` — set class
- `inherits(x, "classname")` — check class membership
- `unclass(x)` — strip class

**CRAN usage:** `class` 3,500 calls, `inherits` 4,263 calls

---

## Phase 2: S3 Dispatch (CRITICAL)

### 2.1 UseMethod()

When a generic function like `print(x)` is called:
1. Get `class(x)` → e.g., `c("data.frame", "list")`
2. For each class, look up `print.data.frame`, `print.list`, `print.default`
3. Call the first match found
4. If none found, call `generic.default`

**CRAN usage:** `UseMethod` 1,502 calls, affects every `print()`, `format()`, `[`, `[[`, `$`, etc.

### 2.2 Method registration

- Track S3 methods in a registry (environment or hashmap)
- Support `generic.class` naming convention
- `NextMethod()` for calling parent class method

### 2.3 Common generics to dispatch

These generics need default methods + class-specific methods:
- `print`, `format`, `str`, `summary`
- `[`, `[[`, `$`, `[<-`, `[[<-`, `$<-`
- `c`, `length`, `names`, `dim`
- `as.character`, `as.numeric`, `as.logical`, etc.

---

## Phase 3: Matrix & Array (HIGH)

### 3.1 Matrix representation

A matrix is a vector with a `dim` attribute. No new RValue variant needed.

- `matrix(data, nrow, ncol, byrow)` — create matrix
- `dim(x)` / `dim(x) <- c(r, c)` — get/set dimensions
- `nrow(x)`, `ncol(x)` — derived from dim
- `t(x)` — transpose

**CRAN usage:** `matrix` 1,924, `nrow` 3,208, `ncol` 2,048, `dim` 2,174, `t` 1,106

### 3.2 Matrix indexing

- `x[i, j]` — 2D subscript (requires parser change: multi-arg `[`)
- `x[i, ]` — row selection (empty j)
- `x[, j]` — column selection (empty i)
- Currently `Arg { name: None, value: None }` represents empty args — leverage this

### 3.3 Matrix operations

- `cbind()`, `rbind()` — column/row binding
- `colnames()`, `rownames()`, `dimnames()`
- `apply(X, MARGIN, FUN)` — apply over rows/columns
- `crossprod()`, `tcrossprod()` — matrix multiplication
- `%*%` — matrix multiply operator (need to add to grammar)

**CRAN usage:** `cbind` 1,479, `rbind` 731, `colnames` 2,069, `rownames` 1,170, `apply` 821

---

## Phase 4: Data Frames (HIGH)

### 4.1 data.frame construction

A data frame is a list with:
- `class = "data.frame"`
- `names` attribute (column names)
- `row.names` attribute
- All columns same length

- `data.frame(a = 1:3, b = c("x","y","z"))`
- Recycle shorter columns

**CRAN usage:** `data.frame` 1,326 calls

### 4.2 Data frame indexing

- `df[i, j]` — matrix-like indexing
- `df$col` — column access (already works via list $)
- `df[["col"]]` — column by name
- `df[, "col"]` — column by name, matrix-style
- `df[condition, ]` — row filtering

### 4.3 Data frame operations

- `merge()`, `subset()`, `transform()`
- `with()`, `within()`
- `rbind.data.frame()`, `cbind.data.frame()`
- `as.data.frame()`

---

## Phase 5: Missing Built-in Functions (HIGH)

Top 50 most-called functions in CRAN that we don't implement properly:

### 5.1 Essential (>2000 calls in CRAN)

| Function | CRAN calls | Status | Notes |
|----------|-----------|--------|-------|
| `missing()` | 4,850 | stub | Check if argument was supplied |
| `inherits()` | 4,263 | stub | S3 class check — needs Phase 2 |
| `on.exit()` | 2,185 | noop | Register cleanup expressions |
| `stopifnot()` | 2,160 | missing | Assert conditions |
| `isTRUE()` | 2,003 | missing | Shorthand for identical(x, TRUE) |
| `switch()` | 1,885 | missing | Multi-way branch |
| `vapply()` | 1,828 | stub | Type-safe apply |
| `match.arg()` | 1,252 | stub | Argument matching |
| `do.call()` | 1,936 | partial | Needs proper named-arg handling |

### 5.2 Important (1000-2000 calls)

| Function | CRAN calls | Status |
|----------|-----------|--------|
| `nzchar()` | 1,353 | missing |
| `eval()` | 1,343 | stub |
| `get()` | 1,279 | missing |
| `substitute()` | 1,270 | stub |
| `parent.frame()` | 1,240 | stub |
| `tryCatch()` | 1,120 | stub |
| `assign()` | 969 | missing |
| `as.vector()` | 1,135 | missing |
| `environment()` | 881 | partial |
| `rep.int()` | 869 | missing |
| `is.matrix()` | 763 | stub |
| `NROW()` / `NCOL()` | 750 | partial |
| `factor()` | 666 | stub |
| `as.data.frame()` | 637 | stub |

### 5.3 Quick wins (easy to implement, high value)

These can be done in an afternoon:
- `isTRUE(x)` → `identical(x, TRUE)`
- `isFALSE(x)` → `identical(x, FALSE)`
- `nzchar(x)` → `nchar(x) > 0`
- `stopifnot(...)` → check each arg, stop if not TRUE
- `switch(expr, ...)` → match expr to named args
- `get(name, envir)` → environment lookup
- `assign(name, value, envir)` → environment assignment
- `exists(name, envir)` → check if name bound
- `is.matrix(x)` → check dim attribute length == 2
- `Sys.time()` → current time as numeric
- `system.time(expr)` → time an expression
- `paste(collapse=)` — already works but verify

---

## Phase 6: Factors (MEDIUM)

A factor is an integer vector with `class = "factor"` and `levels` attribute.

- `factor(x, levels, labels)`
- `levels(x)`, `nlevels(x)`
- `as.integer(factor)` → underlying codes
- `as.character(factor)` → level labels
- Print method shows levels
- Comparison and sorting by level order

**CRAN usage:** `factor` 666, `levels` ~500, `is.factor` ~400

---

## Phase 7: Metaprogramming (MEDIUM)

### 7.1 quote / substitute / deparse

- `quote(expr)` — return unevaluated expression
- `substitute(expr, env)` — substitute variables in expression
- `deparse(expr)` — convert expression to string
- `bquote(expr)` — partial substitution with `.()`

**CRAN usage:** `substitute` 1,270, `deparse` 1,086, `quote` 1,016

### 7.2 eval / parse

- `eval(expr, envir)` — evaluate expression in environment
- `parse(text=)` — parse string to expression
- `sys.call()`, `match.call()` — introspect current call

### 7.3 Requires AST as a value type

Add `RValue::Language(Expr)` to represent unevaluated expressions. This is a significant change but necessary for tidyverse compatibility.

---

## Phase 8: Environments as First-Class (MEDIUM)

- `new.env(parent=)` — create environment
- `parent.env()`, `parent.frame()`
- `ls(envir)`, `get(name, envir)`, `assign(name, val, envir)`
- `environment(fun)` — get closure environment
- `environment(fun) <- env` — set closure environment
- `environmentName()`

---

## Phase 9: Regex (MEDIUM)

Current grep/grepl/gsub use naive string matching. Need actual regex:

- Use Rust `regex` crate
- Support `fixed = TRUE` for literal matching
- Support `perl = TRUE` (default in modern R)
- `regexpr()`, `gregexpr()`, `regmatches()` — proper match objects

---

## Phase 10: File I/O (LOW for CRAN compat, HIGH for usability)

- `readLines()`, `writeLines()`
- `readRDS()`, `saveRDS()`
- `read.csv()`, `write.csv()`
- `source(file)` — execute R file
- `file()`, `connection` objects

---

## Implementation Order

```
Phase 5.3 (quick wins)          ← do first, immediate impact
Phase 1 (attributes)            ← foundation for everything else
Phase 2 (S3 dispatch)           ← unlocks OOP
Phase 3.1-3.2 (matrix basics)   ← with attributes, this is mostly indexing
Phase 4.1-4.2 (data frame basics)
Phase 5.1-5.2 (remaining builtins)
Phase 6 (factors)
Phase 9 (regex)
Phase 7 (metaprogramming)       ← hardest, defer
Phase 8 (environments)
Phase 10 (I/O)
```

---

## Measurement

After each phase, re-run:
```sh
./scripts/parse-test.sh cran/    # should stay 100%
```

And build an eval-test harness:
```sh
./scripts/eval-test.sh cran/     # track % of files that run without error
```

Current eval pass rate is ~0% (almost all files hit missing builtins immediately).
Target after Phase 5.3: ~5-10% of simple files should eval successfully.
Target after Phase 1-4: ~30-40% should at least load without crashing.
