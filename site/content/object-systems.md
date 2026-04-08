+++
title = "S3 And S4"
weight = 6
description = "How miniR handles S3 dispatch in the evaluator and S4 registries in the runtime"
+++

miniR supports both of R's major object systems, but they sit at different depths in the runtime.

- **S3** is wired directly into evaluation and operator dispatch.
- **S4** is currently more registry- and builtin-driven, centered around class, generic, and method tables stored on the interpreter.

## S3: The Everyday Dispatch Path

S3 support lives primarily in `src/interpreter/s3.rs`, plus operator hooks in `src/interpreter/ops.rs` and package registration through the loader.

The dispatch shape is:

1. Force the dispatch object enough to inspect its class vector
2. Look for `generic.class` in the current environment chain
3. Fall back to the interpreter's S3 method registry
4. Fall back to `generic.default`
5. Push S3 dispatch context so `NextMethod()` can continue the chain

That registry exists because packages register methods through `S3method()` in `NAMESPACE`, and those methods are not always discoverable by ordinary environment lookup alone.

## What S3 Keeps On The Interpreter

The interpreter stores:

- an S3 dispatch stack for active method calls
- a per-interpreter S3 method registry keyed by `(generic, class)`
- ordinary environments where `generic.class` functions may also live

This is why S3 works with both user-defined methods and package-declared methods loaded through the package runtime.

## S3 Entry Points

The main S3-facing pieces are:

- `UseMethod()` and `NextMethod()`
- generic functions such as `print()` and `format()`
- operator dispatch paths in `ops.rs`
- package loader registration of `S3method()` directives

If S3 dispatch looks wrong, the fix is usually in `s3.rs`, `ops.rs`, or package loading - not in the parser.

## S4: Registries On The Interpreter

S4 support lives mainly in `src/interpreter/builtins/s4.rs`. The interpreter stores three tables:

| Registry | Purpose |
|------|---------|
| `s4_classes` | class name -> class definition |
| `s4_generics` | generic name -> generic definition |
| `s4_methods` | `(generic, signature)` -> method function |

That state is per interpreter, which keeps different sessions isolated.

## What `setClass()` Stores

`setClass()` registers:

- slot definitions
- inheritance via `contains`
- prototype defaults
- virtual-class status
- optional validity functions

The implementation collects inherited slots and prototypes by walking the registered class graph, so class definitions are more than just a flat slot map.

## What `new()`, `slot()`, And Validation Do

The current S4 object model is list-backed:

- `new()` constructs an object from the registered class definition
- `slot()` and `slot<-` read and write named slots
- `validObject()` calls any registered validity function
- `showClass()` and `existsMethod()` inspect the registries

This is enough to support a meaningful subset of `methods` behavior while keeping the representation straightforward.

## What `setGeneric()` And `setMethod()` Do

`setGeneric()` stores a generic definition in the S4 generic registry and may also bind a callable definition in the current environment.

`setMethod()` stores a method in the per-interpreter method table keyed by the generic name and signature vector. If no generic was registered yet, the implementation falls back to binding the function directly under the generic name for compatibility.

That makes the current S4 implementation more explicit and table-oriented than S3's evaluator-first model.

## The Practical Difference Between S3 And S4 In miniR

| System | Current center of gravity |
|------|----------------------------|
| S3 | Evaluator and package runtime |
| S4 | Builtins and interpreter registries |

So:

- missing S3 behavior usually means a dispatch, environment, or package-loader issue
- missing S4 behavior usually means class/generic/method registry work in `builtins/s4.rs`

## Where To Extend

| If you need to add... | Start here |
|------|-----------------|
| New S3 dispatch semantics | `src/interpreter/s3.rs` |
| Operator method lookup | `src/interpreter/ops.rs` |
| Package-declared S3 methods | `src/interpreter/packages/loader.rs` and `namespace.rs` |
| New S4 registration/inspection builtin | `src/interpreter/builtins/s4.rs` |
| New S4 runtime state | `src/interpreter.rs` |

The two systems are intentionally not collapsed into one abstraction. R itself treats them differently, and miniR follows that reality.
