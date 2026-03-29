# rlang hang: eager argument evaluation triggers C abort

## What happened

`library(rlang)` hangs indefinitely. The hang is a CPU spin at 100%.

## TRUE Root cause (2026-03-29)

**miniR evaluates function arguments eagerly instead of lazily (promises).**

rlang's R code uses `on_package_load("glue", .Call(ffi_glue_is_here))` and
similar patterns where the second argument is captured by `substitute(expr)`
inside the function. In GNU R, `expr` is NOT evaluated because function
arguments are lazy (wrapped in promises). In miniR, the `.Call(...)` is
evaluated immediately, calling into rlang's C code which then fails and
hits `r_abort` → `while(1)`.

**Fix: implement lazy argument evaluation (R promise semantics).** This is
a fundamental interpreter feature, not a native code issue.

## Proximate cause (the hang mechanism)

rlang's C code (in `rlang.dylib`) has `r_abort()` which:
1. Calls `r_peek_frame()` → `Rf_eval(peek_frame_call, r_envs.base)`
2. Calls `r_alloc_environment()` → `Rf_eval(new_env_call, r_envs.base)`
3. Calls `r_exec_n()` → `Rf_eval(call_to_abort, mask_env)`
4. Falls into `while(1);` when none of the above longjmps

The fundamental issue: rlang expects `Rf_eval` to longjmp on error (via `Rf_error` → `longjmp`). Our `Rf_eval`:
- Receives a LANGSXP pairlist (from `Rf_lcons`)
- Doesn't know how to evaluate it as an R call
- Returns `R_NilValue` instead of longjmping

## Why LANGSXP decompilation doesn't work yet

The LANGSXP decompilation code was added but `ffi_is_character` is called
during R file sourcing. The stack trace shows:
```
r_abort → ffi_is_character → _minir_call_protected → dot_call → builtin_dot_call
→ eval_call → eval_in → source_r_directory → load_namespace
```

One of rlang's R files calls `.Call(ffi_is_character, ...)` at the top level
(likely inside a `local({})` block or during argument evaluation). The C function
receives an argument of the wrong type and calls `r_abort("...")`.

## What needs to happen

1. `Rf_eval` must evaluate LANGSXP pairlist calls correctly — decompiling to
   text is fragile; a proper approach would directly interpret the pairlist
   as a function call (extract CAR=function, CDR=args, dispatch)

2. `r_abort` → `Rf_eval(abort_call)` → our interpreter evaluates `abort()` →
   `abort()` calls `stop()` → `stop()` should longjmp via `Rf_error`

3. The whole chain depends on error propagation through Rf_eval correctly
   calling Rf_error on failure, which longjmps back to _minir_call_protected

## Alternative: stub ffi_is_character

`ffi_is_character(x, n, missing, empty)` just checks if x is a character vector
of length n. We could implement this as a pure R function or as a Rust builtin,
bypassing the C code entirely. This would fix the immediate hang for rlang.

## Affected packages

**83 packages** (out of 260 tested) hang because they transitively depend on rlang.
This includes the entire tidyverse: dplyr, tidyr, ggplot2, purrr, stringr, tibble,
forcats, readr, and all their reverse dependencies. Fixing rlang would unlock
roughly 80+ more packages, taking us from 102/260 (39%) to ~180/260 (69%).

## Non-longjmp design options

1. **Reimplement rlang's type-check FFI in Rust** — `ffi_is_character`, `ffi_is_logical`,
   etc. are simple type checks. Replace the C implementations with Rust builtins that
   never call `r_abort`. The `.Call` dispatch could check for these names and route
   to pure-Rust implementations.

2. **Error flag in Rf_eval** — Instead of longjmp, set a thread-local error flag
   that C code can check. Requires patching rlang's C source (add `if (r_eval_errored())`
   checks) which is fragile.

3. **Thread-local error SEXP** — When Rf_eval fails, store the error condition in a
   thread-local. C code that calls r_eval checks the error flag. Similar to R_tryEval's
   error_occurred parameter but implicit.

4. **Compile rlang with miniR error handling** — Patch rlang's `r_abort` to call
   `Rf_error` directly instead of `r_exec_n(abort, ...)`, bypassing the Rf_eval round-trip.
   The C trampoline's longjmp would handle it correctly since it only crosses C frames.
