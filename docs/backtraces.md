# Tracebacks And Backtraces

miniR has two related debugging layers: R-level tracebacks and native backtraces. Both are stored per interpreter so failures in one session do not overwrite debugging state in another session.

## R Tracebacks

When an error propagates through nested closure calls, miniR snapshots the R call stack and makes it available through `traceback()` and session-level error rendering.

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

- successful evaluations do not erase the last traceback
- a new error replaces the previous stored traceback
- top-level builtin errors may have no closure frames to report

The important design choice is that traceback state lives on the interpreter, not in a process-global slot.

## File And Line Locations

When code is evaluated from a file, miniR pushes source context so traceback entries can resolve to `file:line` locations.

That matters most during:

- package loading
- corpus runs
- sourced scripts
- native callbacks that re-enter R

A generic call name is often not enough when dozens of package files are involved.

## Native Backtraces

When native code calls `Rf_error()`, miniR captures the raw native stack before those frames disappear and then resolves instruction pointers into symbols and source lines when debug info is available.

That native layer is feature-gated behind `native` and uses a stack of supporting crates:

- `libloading` for shared-library loading
- `cc` and `pkg-config` for package compilation
- `addr2line`, `gimli`, and `object` for symbol and line resolution

The goal is not only "there was a native error". The goal is to show where it came from.

## Re-Entering R From Native Code

Some native paths call back into the interpreter through `Rf_eval()` or related helpers. miniR can mark that boundary in the captured traceback so the final error story includes both:

- the R frames that led into native code
- the native frames that failed
- the R frames reached again through callback-based re-entry

That mixed stack is one of the main reasons native diagnostics are worth building out at all.

## Error Presentation

miniR treats error output as part of the product surface. A useful failure report should answer:

- what went wrong
- where it happened
- whether the failing code was R or native
- what the next debugging move should be

Tracebacks and backtraces are the structural part of that answer.

## Where To Debug Diagnostic Failures

Start here when the symptom looks like:

- `traceback()` is empty after nested closure failures
- file-and-line locations disappear for sourced or package code
- native frames are missing after `.Call()` failures
- native and R frames do not line up around callback boundaries
- successful evaluations unexpectedly clear traceback state

Those bugs usually live in call-stack capture, source-context plumbing, or native runtime unwinding rather than in the parser.
