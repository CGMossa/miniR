# Bug: recursion limit error escapes tryCatch

## Problem

When `eval_in` hits the 256-depth recursion limit, it returns
`RFlow::Error`. But this error sometimes escapes `tryCatch` and
propagates to the top level, crashing the R script.

## Reproduction

```r
tryCatch({
  # Something that triggers deep recursion
  source("cran/base/R/conditions.R", local=TRUE)
}, error=function(e) cat("caught\n"))
# Expected: "caught"
# Actual: uncaught "evaluation nested too deeply" error
```

## Likely cause

The recursion limit check is in `eval_in`, which returns `RFlow::Error`.
But `tryCatch` is a pre-eval builtin that calls `eval_in` for the
expression. If the recursion depth is already at the limit when
`tryCatch` tries to evaluate the error handler, the handler itself
fails with the same recursion error.

## Fix

Reset the recursion depth counter before calling the error handler
in `tryCatch`. The handler should execute at depth 0 (or at least
at the depth where tryCatch was called), not at the depth where
the error occurred.

## Priority

~~High~~ **FIXED** — recursion depth is reset before entering error handlers
in tryCatch. See session-issues-2026-03-19.md item #4.
