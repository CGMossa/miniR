+++
title = "Tracebacks And Backtraces"
weight = 8
description = "R-level call stacks, file-and-line locations, and native `.Call` backtraces"
+++

miniR has two related debugging layers:

- **Traceback** for the R call stack
- **Backtrace** for native C frames reached through `.Call`, `.C`, or `.External`

Both are stored per interpreter. There is no process-global traceback slot shared by unrelated sessions.

## Layer 1: R Tracebacks

When an error propagates through nested closure calls, miniR snapshots the call stack and makes it available through `traceback()` and through session error rendering.

Example:

```r
f <- function() stop("boom")
g <- function() f()
h <- function() g()
h()
```

Typical output:

```text
Error: boom
Traceback (most recent call last):
3: f()
2: g()
1: h()
```

Important details:

- Successful evaluations do not erase the last traceback. A new error replaces it.
- Top-level builtin errors like `stop("x")` have no closure frames, so there may be no traceback to print.
- `traceback()` is implemented as a builtin, but it is reading interpreter-owned traceback state rather than a process-global singleton.

## File And Line Locations

When code is evaluated from a file, miniR pushes source context onto the interpreter so traceback entries can resolve call spans back to `file:line`.

That means sourced code can produce entries like:

```text
2: inner() at /path/to/script.R:14
1: outer() at /path/to/script.R:18
```

This is especially useful for package loading and corpus runs, where "which file and which line" matters more than a generic call name.

## Layer 2: Native Backtraces

When native code calls `Rf_error()`, miniR captures the raw instruction pointer stack before the C frames disappear, then resolves those addresses into symbols and source lines when debug info is available.

That native layer lives behind the `native` feature and is built from a coupled set of crates:

- `libloading` for shared-library loading
- `cc` and `pkg-config` for package compilation
- `addr2line`, `gimli`, and `object` for turning raw addresses into readable stack frames

Example from the native stacktrace test fixture:

```text
Error: value must be non-negative, got -5
Traceback (most recent call last):
2: validate(x)
   [C] deep_helper at test.c:30 (stacktest.dylib)
   [C] middle_helper at test.c:36 (stacktest.dylib)
   [C] C_validate at test.c:41 (stacktest.dylib)
1: run_check(-5)
```

If native code re-enters R through `Rf_eval`, miniR can also mark that boundary in the traceback so you can see where control moved from C back into R.

## Why This Matters For miniR

miniR is trying to run real package code, not only toy examples. That means errors often cross boundaries:

- R code calling into package native code
- native code calling back into R
- sourced package files calling helpers deep in a namespace

A good traceback is not optional in that world. It is part of the interpreter architecture, and it is one of the places where miniR can be more useful than GNU R during development.
