+++
title = "Native Runtime"
weight = 11
description = "The R C API surface compiled into miniR, callback plumbing, and native-call execution"
+++

The existing native-code page is about compiling and loading package code. This page is about the runtime that those shared libraries call into once they are loaded.

## The Core Idea

miniR compiles a Rust implementation of a large slice of the R C API directly into the main binary. Package shared libraries then resolve symbols such as `Rf_allocVector`, `Rf_eval`, `Rf_findVar`, and `R_registerRoutines` against that binary.

The center of that implementation is `src/interpreter/native/runtime.rs`.

## What Lives In The Native Runtime

`runtime.rs` provides:

- exported sentinel globals such as `R_NilValue`, `R_BaseEnv`, and `R_GlobalEnv`
- many `extern "C"` functions implementing the R C API
- thread-local native invocation state
- callback plumbing back into the current Rust interpreter
- helpers for parsing/evaluating language objects across the R/C boundary

This is why native compatibility is not just a loader problem. miniR needs a whole in-process R ABI surface.

## Thread-Local Native State

During native calls, miniR keeps a thread-local runtime state with:

- allocation tracking
- a protect stack
- interpreter callbacks for variable lookup, definition, evaluation, and parsing
- an `RValue` stash used for round-tripping parsed language objects

That state is local to the calling thread and reset around native entry points, which keeps reentrant interpreter calls manageable.

## How `.Call()` Crosses The Boundary

The rough flow is:

1. `native/dll.rs` looks up the symbol in a loaded shared library
2. miniR installs interpreter callbacks into the native runtime
3. Rust values are converted to `SEXP`
4. the native function is called
5. returned `SEXP` values are converted back to `RValue`
6. callbacks and temporary state are cleared

That same layer also handles registered routines and native error propagation.

## Global Symbol Initialization

There are two important initialization steps:

- `init_globals()` seeds sentinel SEXPs and well-known symbols inside the native runtime
- `init_global_envs()` maps the current interpreter's base and global environments to `R_BaseEnv` and `R_GlobalEnv` before package native code runs

Without those, native package init functions would see null or meaningless environment pointers.

## Errors And Protected Calls

`Rf_error()` and related longjmp-based behavior are not implemented directly in Rust. miniR uses a C trampoline for the setjmp/longjmp boundary and then converts failures back into Rust-side errors.

That bridge is what allows:

- native errors to unwind safely back into miniR
- traceback capture to include native frames
- R callbacks from native code to re-enter the interpreter

## Routine Registration And Native ABI Coverage

The native runtime includes support for routine registration such as `R_registerRoutines()` and C-callable lookup helpers. That matters because many real packages do not just export a bare symbol table; they register entry points during package init.

The API surface is broad and includes allocation, coercion, attributes, symbols, evaluation, environments, RNG hooks, connections, and large chunks of Rmath-facing support.

## Backtraces Are Part Of The Runtime Story

Native runtime support is coupled to native stack unwinding:

- `object`, `gimli`, and `addr2line` resolve raw instruction pointers
- `dll.rs` captures frames when native code errors
- `stacktrace.rs` turns those frames into readable `[C] file:line` entries

That is why the `native` feature pulls in more than just `libloading`.

## Where To Extend

| If you need to add... | Start here |
|------|-----------------|
| Another R C API function | `src/interpreter/native/runtime.rs` |
| Better `RValue` <-> `SEXP` conversion | `src/interpreter/native/convert.rs` |
| Shared-library loading or routine lookup changes | `src/interpreter/native/dll.rs` |
| Compiler/build integration | `src/interpreter/native/compile.rs` |
| Native backtrace formatting | `src/interpreter/native/stacktrace.rs` |

The native runtime is one of the highest-leverage parts of miniR. When it gets better, package compatibility moves in large jumps rather than in single-builtin increments.
