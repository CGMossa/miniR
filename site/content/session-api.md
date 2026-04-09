+++
title = "Session API And Embedding"
weight = 8
description = "How `Session` wraps `Interpreter`, why it is the public entry point, and what embedding-oriented helpers it exposes"
+++

miniR's public boundary is not the raw interpreter type. It is `Session`.

That is a deliberate design choice. The interpreter owns runtime state, but `Session` is the embedding-facing wrapper that turns that runtime into a usable API for:

- tests
- the CLI
- REPL execution
- host applications embedding miniR as a library

## Why `Session` Exists

`Session` lives in `src/session.rs` and owns an `Interpreter`.

The point of the wrapper is to provide a smaller public surface with sensible entry points rather than making every caller manually manage:

- parser invocation
- source-file reading
- traceback rendering
- output capture
- terminal width syncing
- signal handler setup

That keeps most hosts away from internal runtime plumbing they do not need to know about.

## The Main API Surface

The main methods are:

- `Session::new()`
- `Session::new_with_captured_output()`
- `eval_source()`
- `eval_expr()`
- `eval_file()`
- `auto_print()`
- `render_error()`
- `format_last_traceback()`

These cover the common embedding loop:

1. create a session
2. evaluate source or a file
3. inspect the result
4. render errors with traceback context when something fails

## Captured Output

`new_with_captured_output()` creates a session whose stdout and stderr are backed by shared buffers instead of the process streams.

That is especially useful for:

- tests
- notebook-like environments
- embedding scenarios where the host wants to control how output is displayed

The corresponding accessors are:

- `captured_stdout()`
- `captured_stderr()`

This is a small detail, but it is exactly the kind of thing that makes a runtime pleasant or unpleasant to embed.

## Evaluation Helpers

`Session::eval_source()` parses source text and then evaluates it.

`Session::eval_file()` reads a file, records source context for later traceback formatting, parses it, and evaluates it.

`Session::eval_expr()` evaluates an already-built AST.

That split is useful because different callers care about different layers:

- REPLs and scripts usually start from source text
- tests sometimes want to parse once and evaluate directly
- tooling may want AST-level access

## Error Rendering

`SessionError` separates:

- parse errors
- runtime flow/errors
- file-reading failures

`render_error()` then adds traceback output when the failure came from runtime evaluation and traceback state is available.

That means the host does not need to manually stitch together "base error plus stack trace" output every time.

## Embedding And Reentrancy

Because each session owns its own interpreter, different sessions can keep isolated runtime state.

That is exactly what you want when embedding:

- one host component should not unexpectedly inherit another component's options
- one test case should not leak package state into another
- captured output should belong to the session that produced it

This is one of the clearest places where miniR's reentrant-runtime goal becomes visible in normal API design.

## REPL And CLI Usage

The CLI in `src/main.rs` is intentionally thin and uses the same session API that an embedding host would use.

That is good pressure on the API. If the CLI or REPL needed a secret backdoor to be usable, the public boundary would probably be too weak.

The REPL path also layers in:

- signal handling
- terminal width syncing
- history
- optional plot sender installation

but it still does that through `Session`, not by bypassing it.

## Extra Embedding Helpers

`Session` also exposes helpers that are not about plain evaluation but matter in host environments:

- `set_option()` for interpreter-local option setup
- `sync_terminal_width()` for width-aware printing
- `interrupt_flag()` and `install_signal_handler()`
- `generate_rd_docs()` for builtin documentation generation

These are small but very practical entry points.

## Where To Debug Session-Level Problems

Start in `src/session.rs` when the symptom looks like:

- file evaluation loses source context
- captured output is missing
- error rendering omits tracebacks
- the CLI works but embedding does not
- signal handling or terminal-width behavior is inconsistent across front ends

Those are usually session-boundary issues, not raw interpreter issues.
