# rlang hang: r_abort while(1) spin

## What happened

`library(rlang)` hangs indefinitely. The hang is a CPU spin at 100%.

## Root cause

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

memoise, withr, reshape2 all depend on rlang and hang because of this.
