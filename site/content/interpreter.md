+++
title = "The Interpreter"
weight = 4
description = "How `Session`, `Interpreter`, `eval_in`, environments, promises, and builtin dispatch fit together during evaluation"
+++

miniR is a tree-walking interpreter. That description is technically true, but it is too small to be useful on its own.

What matters in practice is how a piece of source text moves through the system:

1. parse into an AST
2. evaluate that AST against an environment
3. create promises for closure arguments
4. force those promises at the right boundary
5. dispatch to builtins, closures, S3 methods, package code, or native code
6. capture enough call-stack context to explain failures

This page is the operational view of that path.

For the boundaries around this page, read [Session API And Embedding](@/session-api.md) for the public host-facing API and [Parser And Diagnostics](@/parser-and-diagnostics.md) for the syntax front end.

## The Public Entry Point

The public API lives in `src/session.rs`.

`Session` owns an `Interpreter` instance and exposes methods such as:

- `eval_source()`
- `eval_expr()`
- `eval_file()`
- `auto_print()`
- `format_last_traceback()`

That means most embeddings do not talk to raw interpreter internals directly. They create a session and evaluate code through that boundary.

The [Session API And Embedding](@/session-api.md) page covers that wrapper in more detail.

## The Basic Evaluation Path

At a high level:

1. `Session::eval_source()` parses the source with `parse_program()`.
2. `Session::eval_expr()` installs the session's interpreter through `with_interpreter_state()`.
3. `Interpreter::eval()` drives evaluation for the parsed expression.
4. `eval_in()` and its helpers dispatch on the AST node kind.

This split matters because parsing and runtime evaluation are intentionally separate problems. Syntax bugs belong in the parser. Lazy evaluation, argument matching, and method dispatch do not.

## Where The Real Runtime Lives

`src/interpreter.rs` is the center of the runtime.

The interpreter owns:

- the global environment
- stdout and stderr capture
- call stack and traceback state
- package and help indexes
- working directory and environment variables
- RNG, temp directories, options, and graphics state

The point of that structure is not only neatness. It is what makes miniR reentrant. The runtime state you care about is attached to an interpreter instance rather than scattered across mutable globals.

## Environments

miniR uses lexical environments backed by `Rc<RefCell<_>>`.

The usual environment shape is:

| Layer | Role |
|------|------|
| Base environment | builtins and standard bindings |
| Global environment | user workspace |
| Local call environments | closure calls, promises, and temporary scopes |

That environment chain is what the evaluator walks for symbol lookup, package attachment, and call-time scope behavior.

## Calls, Promises, And Forcing

Call evaluation lives mainly in `src/interpreter/call_eval.rs`.

The important split is:

- **closures** get lazy promise arguments
- **builtins** get forced concrete values

That is how miniR models R's call-by-need behavior without making every builtin manually reason about promise forcing.

The rough flow is:

1. resolve the call target
2. detect special builtins that must intercept before normal evaluation
3. if the target is a closure, create promises and bind them lazily
4. if the target is a builtin, evaluate and force arguments at the builtin boundary
5. reorder builtin arguments against formal names when needed

This boundary is one of the most important semantic seams in the interpreter.

## Builtins Versus Closures

When `call_function_with_call()` receives a callable, it dispatches one of two ways:

- `RFunction::Builtin`
- `RFunction::Closure`

Builtins carry metadata such as:

- name
- implementation kind
- minimum and maximum arity
- formal parameter info

Closures carry:

- parameter list
- body expression
- closure environment

That is why builtins and closures share the language surface while still using different runtime paths internally.

## Special Builtins And Pre-Eval Behavior

Not every callable should receive already-evaluated arguments.

Some R features only make sense if the builtin sees the raw AST, for example:

- `quote()`
- `substitute()`
- `missing()`
- `library()`
- `system.time()`

That is what the pre-eval builtin path is for. It lets the interpreter intercept those calls before normal eager forcing would destroy the semantics.

## Argument Matching

miniR implements R-style three-pass closure argument matching in `src/interpreter/arguments.rs`:

1. exact matches
2. partial matches
3. positional matches

This is one of the highest-leverage pieces of package compatibility work. Many failures that look like "function X is broken" are really argument-matching failures in disguise.

## Error Context And Stack State

The interpreter also owns the live call stack and the most recent traceback snapshot.

That state is updated as calls enter and unwind. When an error escapes, miniR can format:

- the R call stack
- file-and-line locations for sourced code
- native frames when the failure came through `.Call()` or related interfaces

That is why stack traces are part of interpreter architecture rather than a formatting afterthought.

## Why The Interpreter Is Split Across Files

miniR does not keep everything in one evaluator file.

Important logic is intentionally split into:

- `call_eval.rs` for call dispatch
- `arguments.rs` for matching
- `assignment.rs` for replacement semantics
- `indexing.rs` for read-side subsetting
- `ops.rs` for arithmetic and comparisons
- `control_flow.rs` for loops and `if`
- `s3.rs` for S3 dispatch

That split makes failures easier to classify and keeps runtime changes local to the subsystem they actually affect.

## How To Read Bugs Through This Page

Start from interpreter internals when the symptom looks like:

- lazy arguments are forced too early
- builtins receive the wrong value or wrong name binding
- closures bind arguments incorrectly
- stack traces lose frames
- one kind of AST node evaluates in the wrong environment
- a behavior seems shared across many unrelated packages

Those are usually evaluation-model bugs, not isolated builtin bugs.
