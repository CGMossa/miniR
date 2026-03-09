# DONE — Completed Implementations

Items moved here from TODO.md once fully implemented.

## Interpreter

- S3 dispatch — full dispatch with `UseMethod()` and `NextMethod()` + dispatch stack

## Core Language

- `match.arg(arg, choices)` — match argument to list of choices
- `switch(expr, ...)` — multi-way branch
- `do.call(fn, args)` — call function with arg list
- `sys.call()` — return current call
- `nargs()` — number of arguments to current function

## Attributes & OOP

- `attr(x, which)` — get/set attribute
- `attributes(x)` — get/set all attributes
- `structure(x, ...)` — set attributes inline
- `class(x)` — get/set class (with attribute support)
- `class<-` — replacement function for setting class
- `names<-` — replacement function for setting names
- `inherits(x, what)` — check class membership
- `UseMethod(generic)` — S3 method dispatch
- `NextMethod()` — call next S3 method

## Data Structures

- `matrix(data, nrow, ncol, byrow)` — create matrix
- `dim(x)` / `dim<-` — get/set dimensions
- `nrow(x)` / `ncol(x)` / `NROW(x)` / `NCOL(x)` — row/column count
- `t(x)` — transpose
- `crossprod(x)` / `tcrossprod(x)` — cross products (via ndarray)

## Apply Family

- `sapply(X, FUN)` — simplified apply
- `lapply(X, FUN)` — list apply
- `vapply(X, FUN, FUN.VALUE)` — typed apply
- `Vectorize(FUN)` — vectorize a function
- `Reduce(f, x)` — fold/reduce
- `Filter(f, x)` — filter elements
- `Map(f, ...)` — map over multiple lists

## Metaprogramming

- `eval(expr, envir)` — evaluate expression
- `parse(text=)` — parse string to expression
- `quote(expr)` — return unevaluated expression
- `substitute(expr, env)` — substitute in expression

## Environments

- `environment(fun)` — get closure environment
- `new.env(parent)` — create environment
- `globalenv()` / `baseenv()` / `emptyenv()` — special environments
- `parent.env(env)` — parent environment
- `environmentName(env)` — environment name
- `is.environment(x)` — environment type check
- `ls()` / `objects()` — list bindings
- `exists(name, envir)` — check binding exists

## Error Handling

- `tryCatch(expr, error, warning)` — structured error handling
- `try(expr)` — simple error handling

## File I/O

- `readLines(con)` — read text lines
- `writeLines(text, con)` — write text lines
- `source(file)` — execute R file
- `file.exists(path)` — check file exists

## Architecture

- add linkme
- decouple builtins with proc-macros (interpreter_builtin, pre_eval_builtin, noop_builtin)
