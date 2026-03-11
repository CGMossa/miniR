# ctrlc integration plan

> `ctrlc` 3.5 — Cross-platform Ctrl-C (SIGINT) handler.
> <https://github.com/Detber/rust-ctrlc>

## What it does

Simple API to register a Ctrl-C handler. Sets an `AtomicBool` flag or runs a closure
on SIGINT. Cross-platform (Unix signals + Windows ConsoleCtrlHandler).

```rust
ctrlc::set_handler(|| {
    println!("Ctrl-C pressed!");
    std::process::exit(0);
}).expect("Error setting Ctrl-C handler");
```

Also provides `ctrlc::try_set_handler()` for fallible registration.

## Where it fits in miniR

### 1. Graceful REPL interrupt

Currently pressing Ctrl-C during REPL input is handled by reedline. But during
long-running R computations (e.g. `for(i in 1:1e9) {}`), we need a way to interrupt
execution. ctrlc sets an atomic flag that the interpreter's eval loop can check:

```rust
static INTERRUPTED: AtomicBool = AtomicBool::new(false);

ctrlc::set_handler(|| {
    INTERRUPTED.store(true, Ordering::Relaxed);
}).unwrap();

// In eval loop:
fn eval_expr(&mut self, expr: &Expr) -> Result<RValue, RError> {
    if INTERRUPTED.load(Ordering::Relaxed) {
        INTERRUPTED.store(false, Ordering::Relaxed);
        return Err(RError::Interrupt);
    }
    // ...
}
```

### 2. `on.exit()` / `tryCatch(interrupt = ...)` support

R allows catching interrupts via `tryCatch`. The interrupt mechanism feeds into
the error handling system — `RError::Interrupt` propagates up and can be caught.

### 3. `Sys.sleep()` interruption

Combined with the eval loop check, `Sys.sleep()` can periodically check the
interrupted flag and return early.

## Relationship to builtins plan

| Phase | Builtins affected | Impact |
|---|---|---|
| Core interpreter | eval loop | computation interruption |
| Phase 6 (OS) | `Sys.sleep()` | interruptible sleep |
| Phase 10 (tryCatch) | `tryCatch(interrupt=)` | interrupt handling |

## Recommendation

**Add now.** Essential for any interpreter — without Ctrl-C support, runaway loops
require killing the process. The integration is ~10 lines in `main.rs` plus a check
in the eval loop.

**Effort:** 30 minutes.
