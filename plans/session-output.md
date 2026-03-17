# Session-scoped output

## Problem

Builtins use `println!()` and `print!()` which write to process stdout.
This violates the reentrancy design — multiple interpreters in the same
process would interleave output. It also prevents capturing output for
tests, embedded use, or piping.

## Current state

- `cat()`, `print()`, `message()`, `warning()` all use `println!()`
- `help()`, `namespaces()`, `str()`, `View()` use `println!()`
- `writeLines()` to stdout uses `println!()`

## Design

Add a `Writer` abstraction to the Interpreter:

```rust
pub struct Interpreter {
    // ... existing fields ...
    stdout: Box<dyn Write + Send>,
    stderr: Box<dyn Write + Send>,
}
```

Default: `std::io::stdout()` / `std::io::stderr()`.
For testing: `Vec<u8>` or `Cursor<Vec<u8>>`.
For embedding: any `Write` impl.

### Migration path

1. Add `stdout`/`stderr` fields to Interpreter with default stdio
2. Add `interp.write_stdout(msg)` and `interp.write_stderr(msg)` methods
3. Change `BuiltinContext` to expose `ctx.write(msg)` / `ctx.write_err(msg)`
4. Migrate builtins from `println!` to `ctx.write()` — one file at a time
5. `cat()` already uses BuiltinContext — just needs the writer swap
6. `message()`/`warning()` should use stderr writer

### Builtins that need migration

Every `println!` and `print!` in:
- builtins.rs (help, namespaces, cat, readline)
- interp.rs (print, print.data.frame, str, summary, format)
- conditions.rs (warning stderr fallback, message)
- graphics.rs (stub messages)
- s4.rs (stub messages)
- stubs.rs (error messages)

### Testing benefit

With captured output, tests can assert on printed output:
```rust
let mut s = Session::new_with_captured_output();
s.eval_source("cat('hello')");
assert_eq!(s.captured_stdout(), "hello");
```

## Priority

Medium — doesn't block CRAN compat, but needed for proper reentrancy
and testability. The current println! works for single-interpreter CLI use.
