+++
title = "Reentrant Runtime"
weight = 3
description = "What reentrancy means in miniR, which state lives on `Interpreter`, and why the project avoids process-global runtime state"
+++

When miniR says the runtime is reentrant, it means more than "the parser can be called twice."

The design target is that multiple interpreters can exist in the same process without sharing mutable process-global state. That matters for embedding miniR as a library, running isolated tests, nesting evaluations, and eventually running multiple sessions side by side.

## The Core Rule

Mutable runtime state belongs on `Interpreter`.

That includes things people often leave process-global in language runtimes:

- environments and namespace state
- RNG state
- temp directories
- environment variables
- working directory
- options
- traceback and source-context state
- package search paths
- graphics and device state

If adding a feature would require a mutable global singleton, the design is probably wrong for this project.

## What Reentrancy Buys You

miniR is trying to support workflows like:

- two `Session` values in one process evaluating unrelated code
- tests creating fresh interpreters without leaking state across cases
- native code calling back into the interpreter without corrupting some shared global runtime slot
- future host applications embedding miniR as one component among several

This is why reentrancy is a product-level goal, not a low-level implementation preference.

## Where The State Lives

The public boundary is `src/session.rs`, which wraps the interpreter in a `Session`.

The actual mutable runtime lives in `src/interpreter.rs`, where the interpreter owns things such as:

- stdout and stderr capture
- search-path and package state
- last traceback and source-context stacks
- RNG and options
- current working directory and environment variables
- graphics state and device output

That keeps the runtime honest. If one session changes an option or loads a package, another session should not silently see it unless the design explicitly says they are shared.

## Thread-Local Storage Is Not The Model

miniR does have thread-local infrastructure, but it is not supposed to be the main state model for builtins. The normal path is:

- store runtime state on `Interpreter`
- access it through `BuiltinContext`
- use TLS only as bridge infrastructure where the runtime boundary requires it

That distinction matters because "TLS everywhere" easily turns into hidden global state with different failure modes.

## Reentrancy And Native Code

Native support is one of the harshest tests of this design.

miniR allows loaded package code to call back into interpreter services such as `Rf_eval()` and environment lookup. That only stays manageable if the interpreter currently serving the call is explicit in the runtime plumbing.

The native runtime therefore uses callback installation and thread-local invocation state around entry points, while the longer-lived state still belongs to the interpreter instance itself.

## Reentrancy And Tests

The best concrete examples live in `tests/reentrancy.rs`.

Those tests exercise session isolation, nested evaluation, and parallel-thread scenarios. They are not side coverage. They are proof that the design promise is still true as the runtime grows.

## How To Read Bugs Through This Lens

Start thinking about reentrancy when a bug looks like:

- one session sees another session's env vars or working directory
- traceback state leaks across tests
- package search paths persist unexpectedly
- graphics state from one interpreter shows up in another
- a builtin wants to use a mutable global because it seems easier

Those are usually architecture bugs, not one-off implementation mistakes.

## Why miniR Is Strict About This

Many language runtimes drift into accidental singleton design because it is convenient early on. miniR is explicitly trying not to make that trade.

The runtime will only get harder to restructure as package loading, native code, graphics, and object systems grow. Reentrancy is therefore a constraint that keeps the rest of the architecture honest.
