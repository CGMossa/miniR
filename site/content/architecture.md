+++
title = "Interpreter Architecture"
weight = 2
description = "How parser, evaluator, runtime values, dispatch, and package layers fit together"
+++

miniR is organized around one rule: interpreter state belongs to the `Interpreter` instance, not to process-global mutable statics. That rule is what makes the runtime reentrant and embeddable.

If you want the focused explanation of that design rule, read the `Reentrant Runtime` page next. If you want the operational evaluation path, read `The Interpreter`. This page is the wider map of the codebase around both.

## Top-Level Layers

- `src/session.rs` exposes `Session`, the public API used by tests, embeddings, and the CLI.
- `src/main.rs` is intentionally thin. It parses command-line flags, starts the REPL when needed, and delegates real work to `Session`.
- `src/parser/r.pest`, `src/parser.rs`, and `src/parser/ast.rs` turn source text into an AST.
- `src/interpreter.rs` is the tree-walking evaluator and the home of per-interpreter state.

## Front End: Parse To AST

- `src/parser/r.pest` defines the grammar and operator precedence.
- `src/parser.rs` converts pest pairs into AST nodes.
- `src/parser/diagnostics.rs` formats parse failures and suggestions when the `diagnostics` feature is enabled.

The parser layer should only answer syntax questions. If a bug is about lazy evaluation, environments, S3 dispatch, or replacement semantics, it almost never belongs here.

## Runtime Core

- `src/interpreter/value.rs` defines `RValue`, vectors, lists, functions, promises, language objects, and `RError`.
- `src/interpreter/environment.rs` implements lexical scoping with `Rc<RefCell<_>>`.
- `src/interpreter.rs` stores stdout/stderr, RNG state, temp dirs, env vars, working directory, options, condition handlers, help indexes, and traceback state on the interpreter itself.

That last point is the architectural center of gravity. miniR is not trying to fake reentrancy with a mostly-global runtime plus a few isolated fields. The runtime state that matters is interpreter-owned.

Environment hierarchy follows the usual miniR shape:

| Layer | Purpose |
|------|---------|
| Base environment | Builtins and standard bindings |
| Global environment | User workspace |
| Local call environments | Closure calls, promises, and temporary scopes |

## Evaluation And Dispatch

- `src/interpreter/call_eval.rs` resolves call targets, creates promises for closures, forces arguments for builtins, and dispatches builtin versus closure calls.
- `src/interpreter/arguments.rs` implements three-pass closure argument matching: exact, partial, then positional.
- `src/interpreter/call.rs` defines `BuiltinContext`, `CallFrame`, and traceback capture.
- `src/interpreter/s3.rs` handles S3 generic dispatch with `UseMethod()` and `NextMethod()`.

This is the layer to change when package code fails because of call semantics, lazy evaluation, argument matching, or traceback behavior.

The dedicated `The Interpreter` page follows that call path in more detail from `Session::eval_source()` through promise creation, builtin dispatch, and stack-trace capture.

## Semantic Subsystems

- `src/interpreter/ops.rs` implements arithmetic, comparison, logical operators, `%in%`, ranges, and matrix operators.
- `src/interpreter/control_flow.rs` handles `if`, loops, `for`, `repeat`, and pipes.
- `src/interpreter/assignment.rs` owns assignment and replacement semantics.
- `src/interpreter/indexing.rs` owns read-side indexing for vectors, lists, matrices, and data frames.

These files matter more than builtin count when real packages fail. Many CRAN corpus regressions are semantic mismatches in these layers, not missing leaf functions.

## Builtins, Packages, Native, And Graphics

- `src/interpreter/builtins.rs` wires builtin registration and builtin help synthesis.
- `src/interpreter/builtins/*.rs` groups builtins by domain such as strings, math, system, conditions, datetime, graphics, and native helpers.
- `src/interpreter/packages/` handles package loading, namespace behavior, and Rd help indexing.
- `src/interpreter/native/` handles compiled code, loading, routine lookup, and native stack unwinding.
- `src/interpreter/graphics/` and `src/interpreter/grid/` hold graphics and device state.

Most of these layers are feature gated. The parser and evaluator core are always present; heavy subsystems such as native loading, GUI plotting, TLS, linalg, and parquet are optional.

If you want the registration mechanics behind the builtin layer, the `Builtin Registry And linkme` page explains how `linkme` and `minir-macros` assemble the builtin registry.

## Where To Make Changes

| If the bug looks like... | Start here |
|--------------------------|------------|
| Parse precedence or newline handling | `parser/r.pest`, `parser.rs` |
| Wrong call semantics or lazy forcing | `call_eval.rs`, `arguments.rs`, `call.rs` |
| Wrong subset or replacement behavior | `indexing.rs`, `assignment.rs` |
| Missing or incorrect builtin | `builtins/*.rs` |
| Namespace or package-loading issue | `packages/`, `builtins/pre_eval.rs` |
| Native `.Call` or C API issue | `native/` |
| Traceback or error-reporting issue | `call.rs`, `session.rs`, `value/error.rs` |

## The Main Architectural Bet

miniR is not trying to win by putting more logic into process-global runtime state. The bet is the opposite: keep the parser separate, keep state on `Interpreter`, keep builtin registration declarative, and make heavy subsystems optional behind feature flags. That structure is what lets the project grow without turning into a single giant evaluator file.
