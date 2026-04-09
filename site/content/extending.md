+++
title = "Builtins"
weight = 4
description = "How miniR's 800+ builtins register, how the builtin macros differ, and when a missing behavior is not actually a builtin problem"
+++

miniR already exposes 800+ builtin entry points across math, strings, IO, conditions, graphics, packages, native helpers, and more. They are a large part of the public language surface, but they are not the whole interpreter.

Builtins are one of the easiest ways to extend miniR, but they are not the only extension point. A lot of real package compatibility work lives in call semantics, indexing, assignment, package loading, and native support. Add a builtin when the missing behavior is truly a function, not when the real bug is deeper in the evaluator.

## What The Builtin Layer Covers

The builtin modules under `src/interpreter/builtins/` are grouped by runtime area, for example:

| Area | Representative modules |
|------|-------------------------|
| Core language and metaprogramming | `builtins.rs`, `pre_eval.rs`, `interp.rs` |
| Strings, coercion, and collections | `strings.rs`, `coercion.rs`, `collections.rs`, `types.rs` |
| Math and stats | `math.rs`, `stats.rs`, `random.rs` |
| IO and formats | `io.rs`, `json.rs`, `toml.rs`, `serialize.rs`, `parquet.rs` |
| System/runtime integration | `system.rs`, `connections.rs`, `conditions.rs`, `datetime.rs` |
| Graphics | `graphics.rs`, `grid.rs`, `graphics/color.rs` |
| Package and native support | `native_code.rs`, `rlang_ffi.rs`, package-related pre-eval builtins |

That breadth is why builtins matter for package compatibility. The project is not relying on a tiny core plus a giant standard library written in R.

## How Builtins Register

miniR uses two Rust-side pieces together:

- `minir-macros` defines the attribute macros such as `#[builtin]`.
- `linkme` collects builtin descriptors into one distributed registry at link time.

At interpreter startup, `Interpreter::new()` creates the base environment, registers every builtin from the registry into that environment, and synthesizes builtin help pages from Rust doc comments.

That registration path is also why WASM support is not trivial today. The current builtin registry depends on `linkme` distributed slices, and that auto-registration approach does not yet work on `wasm32`.

## Pick The Right Builtin Macro

| Macro | Use it when... | Signature shape |
|------|-----------------|-----------------|
| `#[builtin]` | The function only needs already-evaluated `RValue`s and no interpreter state | `fn(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError>` |
| `#[interpreter_builtin]` | The function needs interpreter or environment access through `BuiltinContext` | `fn(args, named, context: &BuiltinContext) -> Result<RValue, RError>` |
| `#[pre_eval_builtin]` | The function needs unevaluated AST arguments, NSE, or lazy/special evaluation rules | `fn(args: &[Arg], env: &Environment, context: &BuiltinContext) -> Result<RValue, RError>` |

Practical examples:

- `stop()` is a plain `#[builtin]`.
- `warning()` and `message()` are `#[interpreter_builtin]` because they write diagnostics and interact with handlers.
- `library()`, `quote()`, `substitute()`, `missing()`, and `system.time()` are `#[pre_eval_builtin]` because they need unevaluated arguments or special evaluation.

## A Minimal Builtin Skeleton

```rust
use crate::interpreter::value::{RError, RValue};
use minir_macros::builtin;

/// Example builtin used in documentation.
///
/// @param x value to return
/// @return `x`, unchanged
#[builtin(name = "identity_example", min_args = 1, max_args = 1)]
fn builtin_identity_example(
    args: &[RValue],
    _named: &[(String, RValue)],
) -> Result<RValue, RError> {
    Ok(args[0].clone())
}
```

The important parts are not the body. They are the doc comment, arity metadata, and the right macro for the evaluation semantics.

## Typical Workflow

1. Choose the domain file under `src/interpreter/builtins/` that matches the feature, or split a new submodule if the file is getting too large.
2. Add the builtin with doc comments and the correct attribute macro.
3. Use `BuiltinContext` for interpreter access instead of raw TLS.
4. Feature-gate the module or builtin if it depends on optional crates such as `jiff`, `nalgebra`, `rustls`, or the native runtime.
5. Add tests with `Session::eval_source()` or direct value checks through the public API.
6. Run `cargo fmt`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test`.

## What Good Builtins Look Like

- They use the smallest macro that matches the semantics.
- They produce specific error messages that explain what went wrong and what to fix.
- They keep per-interpreter state on `Interpreter`.
- They lean on existing coercion, value, and helper types instead of open-coding conversions everywhere.
- They come with tests, especially when argument matching, recycling, or S3 behavior is involved.

## When A Builtin Is The Wrong Fix

Do **not** add a builtin just because a CRAN package failed.

The right fix is often elsewhere:

- Lazy or special argument behavior belongs in `call_eval.rs`, `arguments.rs`, or `pre_eval.rs`.
- Wrong subset or replacement behavior belongs in `indexing.rs` or `assignment.rs`.
- Namespace and package-loading failures belong in `packages/` or package-related builtins.
- Native extension failures belong in `native/`, not in a Rust reimplementation of every helper function the package uses.

miniR gets stronger when builtin work follows the architecture instead of fighting it.
