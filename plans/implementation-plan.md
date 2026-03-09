# Implementation Plan

A concrete, ordered plan for making newr run real R code. Derived from the roadmap, interpreter discoveries, and a full audit of the current codebase.

## Current State (2026-03-08)

- Parser: 100% (12,399/12,399 CRAN .R files)
- Builtins: 336 registered — ~133 implemented, ~132 no-ops, ~37 stubs
- Value system: `Vector` (4 atomic types, no attributes), `RList` (has attributes), `RFunction`, `Environment`, `Null`
- S3 dispatch: skeletal — `dispatch_s3` in interpreter.rs works for closures containing `UseMethod()`, but `UseMethod` itself is a noop stub
- Attributes: only `RList` has `attrs`. `Vector` has zero attribute storage. This blocks matrices, data frames, factors, and `class()`
- Eval pass rate on CRAN: ~0%

## Known Bugs (from interpreter-discoveries.md)

These should be fixed as we encounter them during implementation, not as a separate batch.

1. **`Reduce` doesn't do `match.fun`** — `Reduce("+", 1:10)` fails. Need a `match_fun` helper to resolve strings/symbols to functions.
2. **`eval_apply` uses global env** — sapply/lapply/vapply hardcode `interp.global_env` instead of using the call-site `_env` parameter.
3. **`%in%` uses string comparison** — `eval_in_op` converts everything to `to_characters()`. Should compare within the actual type.
4. **`is_assignment_or_invisible` is crude** — string heuristic with false positives. Should check the parsed AST.
5. **`eval_dollar` treats `@` same as `$`** — fine for now, S4 is very low priority.
6. **Unnecessary interpreter builtins** — `switch`, `get`, `assign`, `exists`, `Vectorize`, `system.time` don't actually need `with_interpreter()`. Could be plain `#[builtin]` with `&Environment`.
7. **Two builtin registries are nearly identical** — `BuiltinFn` vs `InterpreterBuiltinFn` differ only by `&Environment`. Consider merging.

---

## 1. Fix Known Bugs in Apply Family

**Why first:** These are correctness bugs in already-implemented code. Fixing them now means the apply family actually works when we start testing real R scripts.

- Fix `eval_apply` to use `_env` parameter instead of `interp.global_env` (interp.rs:74)
- Add `match_fun` helper: resolve string → function via `env.get_function(name)`, resolve `RValue::Function` → passthrough
- Use `match_fun` in `Reduce`, `Filter`, `Map`, `do.call`, `sapply`, `lapply`, `vapply`
- Fix `%in%` to compare within the actual vector type (doubles as doubles, integers as integers), not via string coercion

## 2. Quick-Win Builtins

**Why second:** Immediate CRAN impact with minimal code. These are all simple functions that don't depend on attributes or S3.

- `isTRUE(x)` — return `identical(x, TRUE)` (check: scalar logical, value is `Some(true)`)
- `isFALSE(x)` — same for FALSE
- `nzchar(x)` — vectorized `nchar(x) > 0`
- `stopifnot(...)` — check each arg is TRUE, stop with message if not (pre-eval builtin — needs unevaluated args for the error message, or just format the value)
- `match.arg(arg, choices)` — partial matching of `arg` against `choices` character vector
- `on.exit(expr)` — requires call-stack tracking to run at function exit. Stub properly: store expression, execute on Return. This is hard to do right — keep as noop for now but add a TODO
- `missing(x)` — requires tracking which args were actually supplied vs defaulted. Needs a sentinel value or a bitset in the call environment. Implement as pre-eval builtin that checks if the symbol is bound in the call env
- `Sys.time()` — current time as numeric (seconds since epoch)
- `proc.time()` — process elapsed time as named numeric vector
- `is.matrix(x)` — check `dim` attribute has length 2 (depends on #3, but can stub as FALSE for now)
- `as.vector(x)` — strip attributes, return atomic vector

## 3. Attribute Storage on Vectors

**Why third:** This is the critical foundation. Matrices, data frames, factors, S3 classes, `names()` — all are attributes. Nothing else can proceed without this.

### Design Decision: Where to Put Attrs

**Option A:** Add `attrs` to each atomic type (Double, Integer, etc.)

- Pro: data-local, no extra indirection
- Con: 4x duplication of attr methods, breaks the clean newtype pattern

**Option B:** Add `attrs` to the `Vector` enum

- Pro: single location, natural place for "any vector can have attributes"
- Con: Vector enum already has 4 variants, adding a field means changing from bare enum to struct-with-enum

**Option C (recommended):** Wrap `Vector` in a struct like `RList`

```rust
pub struct RVector {
    pub inner: Vector,
    pub attrs: Option<Box<Attributes>>,
}
```

Then `RValue::Vector(RVector)` instead of `RValue::Vector(Vector)`. This parallels `RList` exactly, gives attribute storage without touching the 4 atomic types, and provides a natural place for `get_attr`/`set_attr`/`class()` methods.

### Implementation Steps

- Create `RVector` struct with `inner: Vector` + `attrs: Option<Box<Attributes>>`
- Add `get_attr`, `set_attr`, `class()`, `names()` methods (mirror `RList`)
- Change `RValue::Vector(Vector)` → `RValue::Vector(RVector)`
- Update all match sites (this will be a large mechanical change)
- Add convenience: `RVector::from(Vector)` with `attrs: None` so existing code that constructs vectors just wraps

### Attr Builtins

- `attr(x, "name")` — get single attribute
- `attr(x, "name") <- value` — set single attribute (replacement function)
- `attributes(x)` — return all attrs as named list
- `attributes(x) <- list(...)` — set all attrs
- `structure(x, .Names=..., class=..., ...)` — set attributes inline, return x

## 4. Names

**Why here:** `names()` is the most-used attribute (10,756 CRAN calls). With attribute storage in place, implement properly.

- `names(x)` — return `attr(x, "names")` as character vector (or NULL)
- `names(x) <- value` — set names attribute
- `c()` should propagate names from inputs
- `[` indexing should propagate names
- Named indexing: `x["foo"]` should work via the names attribute

## 5. class() / inherits() / unclass()

**Why here:** Needed for S3 dispatch, and class is just an attribute.

- `class(x)` — return `attr(x, "class")`, or implicit class based on type (`"numeric"`, `"character"`, `"logical"`, `"integer"`, `"list"`, `"function"`, `"NULL"`)
- `class(x) <- value` — set class attribute
- `inherits(x, "classname")` — check if `classname` is in `class(x)`. Also support `which = TRUE` to return position
- `unclass(x)` — strip class attribute, return object
- `is.numeric`, `is.character`, `is.logical`, `is.integer`, `is.list`, `is.function`, `is.null` — most already exist, ensure they work with classed objects
- `is(x, "class")` — alias for inherits

## 6. S3 Dispatch — Full Implementation

**Why here:** With class() working, S3 dispatch becomes real. This unlocks `print.foo`, `format.bar`, `[.data.frame`, etc.

### UseMethod Rewrite

Current state: `UseMethod` is a noop in stubs.rs, but `extract_use_method` in interpreter.rs already detects `UseMethod("generic")` in function bodies and calls `dispatch_s3`. This is actually working for user-defined generics. The issue is:

1. `UseMethod` as a noop means `UseMethod("print")` called directly returns NULL instead of dispatching
2. The `dispatch_s3` method only looks in the current env, not the method registry

### Steps

- Remove `UseMethod` from noop stubs
- Make `UseMethod` a pre-eval builtin that triggers dispatch from within the calling function's context
- OR: keep the current `extract_use_method` approach (it works!) and just ensure `dispatch_s3` searches more broadly:
  - Current env → parent envs → global env → base env
  - Also check `.__S3MethodsTable__.` (R's method registry) — but we can defer this
- Implement `NextMethod()` — call the next method in the class chain
- Register built-in default methods: `print.default`, `format.default`, `[.default`, etc.

### Priority Generics

These generics need `.default` methods:

- `print` — current print logic becomes `print.default`
- `format` — format any R value as character
- `str` — compact display of structure
- `summary` — summary statistics
- `[`, `[[`, `$` — subsetting (already works for basic types, need class dispatch)
- `c` — concatenation (already works, needs to strip class or dispatch)
- `length` — already works
- `as.character`, `as.numeric`, `as.logical`, `as.integer` — coercion generics

## 7. Matrix & Array

**Why here:** Matrices are just vectors with a `dim` attribute. Now that we have attributes, this is mostly indexing.

### 7a. Matrix Construction

- `matrix(data, nrow, ncol, byrow)` — create vector, set `dim` attribute to `c(nrow, ncol)`
- `dim(x)` — get dim attribute
- `dim(x) <- c(r, c)` — set dim attribute (replacement function)
- `nrow(x)` / `ncol(x)` — derived from `dim`
- `NROW(x)` / `NCOL(x)` — work on vectors too (treat as column vector)
- `is.matrix(x)` — `!is.null(dim(x)) && length(dim(x)) == 2`
- `is.array(x)` — `!is.null(dim(x))`
- `t(x)` — transpose (swap dim, rearrange elements)

### 7b. Multi-Dimensional Indexing

The parser already supports `x[i, j]` — it produces `Index { object, indices }` where `indices` is a `Vec<Arg>`. Currently we only handle 1D indexing (first arg).

- When `indices.len() > 1`, dispatch to matrix indexing
- `x[i, j]` — select row i, column j
- `x[i, ]` — select entire row i (empty arg = all)
- `x[, j]` — select entire column j
- `x[i, j] <- val` — matrix assignment
- Convert 2D indices to 1D using column-major order: `idx = (j-1) * nrow + i`

### 7c. Matrix Operations

- `cbind(...)` / `rbind(...)` — column/row binding
- `colnames()` / `rownames()` / `dimnames()` — get/set via attributes
- `apply(X, MARGIN, FUN)` — iterate over rows (MARGIN=1) or columns (MARGIN=2)
- `which(x)` — indices where logical vector is TRUE (already partially done?)
- `%*%` — matrix multiply. Need to add `MatMul` to `SpecialOp` in the grammar and handle in `eval_binary`

## 8. Data Frames

**Why here:** Data frames are lists with `class = "data.frame"`, `names`, and `row.names` attributes. With attributes + S3, this is mostly construction + indexing.

### 8a. Construction

- `data.frame(a = 1:3, b = c("x","y","z"))` — create list, set class, names, row.names
- Recycle shorter columns to longest
- Check all columns same length after recycling
- `stringsAsFactors` parameter (default FALSE in R 4.0+, we should default FALSE)

### 8b. Indexing (S3 dispatch)

- `[.data.frame` — 2D indexing: `df[rows, cols]`
- `df$col` — already works (list $)
- `df[["col"]]` — already works (list [[)
- `df[, "col"]` — column selection, returns vector (drop=TRUE)
- `df[condition, ]` — row filtering with logical vector

### 8c. Operations

- `subset(x, subset, select)` — filter rows and select columns
- `with(data, expr)` — evaluate expr in data frame's column environment
- `merge(x, y, by)` — join two data frames
- `as.data.frame(x)` — coerce various types to data frame
- `rbind.data.frame(...)` / `cbind.data.frame(...)`

## 9. Metaprogramming Foundation

**Why here:** Many CRAN packages use `substitute`, `deparse`, `quote`, `eval`. This is hard but high-value.

### 9a. RValue::Language

Add `RValue::Language(Expr)` — represents an unevaluated R expression (AST node). This is what `quote()` returns.

- `quote(expr)` → pre-eval builtin, return `RValue::Language(expr)` without evaluating
- `substitute(expr, env)` → walk the AST, replace symbols with their values from env
- `deparse(expr)` → convert `RValue::Language(Expr)` to R source code string
- `bquote(expr)` → partial substitution with `.()` splicing

### 9b. eval() / parse()

- `eval(expr, envir)` — if `RValue::Language`, evaluate the AST in `envir`. If character, parse then eval
- `parse(text=)` → call our parser, return `RValue::Language` (or expression vector)
- `sys.call()` → return the current call as a Language object (requires call stack tracking)
- `match.call()` → return the matched call with formal parameter names

### 9c. Call Stack

Many metaprogramming functions need a call stack:

- `sys.call(n)`, `sys.function(n)`, `sys.frame(n)`
- `parent.frame(n)` — caller's environment
- `on.exit(expr)` — register cleanup

Add `call_stack: Vec<CallFrame>` to Interpreter where `CallFrame` stores the call expression, environment, and on.exit expressions.

## 10. Environments as First-Class Objects

- `new.env(parent=)` — create new environment
- `parent.env(env)` — get parent
- `parent.frame()` — caller's environment (needs call stack)
- `environment(fun)` — get closure's captured environment
- `environment(fun) <- env` — set it
- `environmentName(env)` — get name
- `ls(envir=)` — list bindings
- `as.environment(x)` — coerce

## 11. Regex

Current `grep`/`grepl`/`gsub`/`sub` use naive string matching. Need the Rust `regex` crate.

- `grep(pattern, x)` → return indices of matches
- `grepl(pattern, x)` → return logical vector
- `sub(pattern, replacement, x)` → replace first match
- `gsub(pattern, replacement, x)` → replace all matches
- `regexpr(pattern, x)` → return match positions
- `gregexpr(pattern, x)` → return all match positions
- Support `fixed = TRUE` for literal matching
- Support `ignore.case = TRUE`
- `regmatches(x, m)` → extract matches using regexpr output

## 12. File I/O

- `readLines(con)` / `writeLines(text, con)`
- `read.csv(file)` / `write.csv(x, file)` — depends on data frames
- `readRDS(file)` / `saveRDS(object, file)` — needs serialization format
- `source(file)` — already implemented in interp.rs
- `file.exists(path)` — simple file check
- `file.path(...)` — path joining

## 13. Cleanup & Refactoring (Ongoing)

These should be done opportunistically as we touch the relevant code, not as a separate batch.

- Merge `BuiltinFn` and `InterpreterBuiltinFn` into a single type that always receives `&Environment`
- Convert unnecessary interpreter builtins to plain builtins (switch, get, assign, exists, Vectorize, system.time)
- Fix `is_assignment_or_invisible` to check the parsed AST instead of string heuristics
- Add `match_fun` to Interpreter and use it everywhere a function-or-string is accepted

---

## Implementation Order

Strict priority order. Each item builds on the previous.

```text
 1. Fix apply-family bugs (eval_apply env, match_fun, %in%)
 2. Quick-win builtins (isTRUE, nzchar, stopifnot, etc.)
 3. Attribute storage on vectors (RVector wrapper)
 4. names() proper implementation
 5. class() / inherits() / unclass()
 6. S3 dispatch full implementation
 7. Matrix construction + 2D indexing
 8. Data frame construction + indexing
 9. Metaprogramming (RValue::Language, quote, eval, deparse)
10. Environments as first-class
11. Regex (Rust regex crate)
12. File I/O
```

After each numbered item, run:

```sh
cargo test
./scripts/parse-test.sh cran/     # must stay 100%
```

Build an eval-test harness after item 2:

```sh
./scripts/eval-test.sh cran/      # track % of files that eval without error
```

## Success Metrics

| After item | Expected CRAN eval pass rate |
|-----------|------------------------------|
| 2 (quick wins) | ~5% |
| 6 (S3 dispatch) | ~15% |
| 8 (data frames) | ~30% |
| 11 (regex) | ~40% |
