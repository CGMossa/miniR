+++
title = "Native Runtime"
weight = 3
description = "miniR's native story is not only about compiling package src/ trees. It also includes a Rust implementation of a large slice of the R C API, callback plumbing back into the interpreter, and an error boundary that can survive longjmp-style native failures."
+++

miniR's native story is not only about compiling package `src/` trees. It also includes a Rust implementation of a large slice of the R C API, callback plumbing back into the interpreter, and an error boundary that can survive longjmp-style native failures.

## The Core Idea

Package shared libraries resolve symbols such as `Rf_allocVector`, `Rf_eval`, `Rf_findVar`, and `R_registerRoutines` against the miniR process itself. That means miniR needs an in-process R ABI surface, not just a loader.

The center of that surface is `src/interpreter/native/runtime.rs`.

## What Lives In The Native Runtime

The native runtime provides:

- exported sentinel globals such as `R_NilValue`, `R_BaseEnv`, and `R_GlobalEnv`
- many `extern "C"` functions implementing the R C API
- native invocation state for the current thread
- callbacks that let native code ask miniR to evaluate, look up, define, or parse objects
- conversion helpers between `RValue` and `SEXP`

This is what lets loaded packages treat miniR like an R runtime instead of like an unrelated host program.

## Thread-Local Native State

During native execution, miniR keeps thread-local runtime state for:

- allocation tracking
- a protect stack
- interpreter callbacks
- temporary language-object stashes used when parsing or evaluating across the boundary

That state is scoped to native entry points and cleared afterwards. The goal is to support reentrant native calls without leaking interpreter state across sessions.

## Crossing The `.Call()` Boundary

The rough `.Call()` flow is:

1. `native/dll.rs` resolves the symbol in a loaded shared library.
2. miniR installs interpreter callbacks into the native runtime.
3. Rust-side values are converted into `SEXP`.
4. The native function runs.
5. Returned `SEXP` values are converted back into `RValue`.
6. Temporary callbacks and runtime state are cleared.

That same layer also handles registered routines and native error propagation.

## Global Initialization

There are two important setup steps before package native code can behave sanely:

- `init_globals()` seeds sentinel SEXPs and well-known symbols.
- `init_global_envs()` maps the current interpreter's base and global environments to `R_BaseEnv` and `R_GlobalEnv`.

Without those mappings, package init code sees meaningless pointers instead of a usable runtime environment.

## Errors And Longjmp Boundaries

`Rf_error()`-style control flow is not implemented as a pure Rust unwind. miniR uses a C trampoline around the setjmp/longjmp boundary and then converts the failure back into a Rust-side error.

That bridge is important because it allows all of these at once:

- native errors unwinding safely back into miniR
- R tracebacks and native backtraces being captured in the same failure
- native code re-entering the interpreter through callbacks

This is one of the sharpest parts of the runtime and should stay explicit in the code.

## Routine Registration

Real packages do not always rely on a bare exported symbol table. Many register routines during package init, and some depend on C-callable lookup helpers.

That is why miniR includes support for things like:

- `R_registerRoutines()`
- registered `.Call()` and `.External()` entry points
- C-callable lookup across package boundaries

If registered routines fail while plain exported symbols work, the problem is in runtime bookkeeping, not in the compiler step.

## Native ABI Coverage

The runtime covers a wide surface area, including:

- allocation and protection
- attribute access and mutation
- symbols and environments
- evaluation helpers
- coercion and type predicates
- strings and character translation
- RNG hooks
- connections and parts of graphics-facing ABI
- Rmath-facing entry points

The important point is not the raw count. The important point is that the runtime needs enough coverage for real packages to survive contact with the interpreter.

## Backtraces Are Part Of The Native Story

miniR couples native runtime support to stack unwinding support so native failures are debuggable:

- `object`, `gimli`, and `addr2line` resolve raw instruction pointers
- the captured frames are stitched into the higher-level error story
- package authors can see both the R stack and the native call chain

That makes native support materially more useful than a loader that only reports "symbol call failed".

## Where To Debug Native Failures

Start in the native runtime when the symptom looks like:

- a package shared library loads but C API calls crash or misbehave
- `Rf_eval` or environment lookups return nonsense from native code
- `R_BaseEnv` or `R_GlobalEnv` appears unset
- `Rf_error()` does not unwind cleanly
- registered routines exist but cannot be called
- native backtraces disappear or resolve badly

Those are runtime integration bugs, not just build-system bugs.
