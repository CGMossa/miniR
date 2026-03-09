# signal-hook integration plan

> `signal-hook` 0.3 — Unix signal handling.
> Already vendored as a transitive dependency of crossterm/reedline.

## What it does

Register signal handlers (SIGINT, SIGTERM, SIGHUP, etc.) safely from Rust. Provides both low-level signal registration and high-level flag-based checking.

## Where it fits in newr

### The problem

When a user runs a long R computation and presses Ctrl+C:
- Currently: the entire process is killed (default SIGINT behavior)
- Expected: the current computation is interrupted, control returns to the REPL

R has `tryCatch(expr, interrupt = function(e) ...)` to catch interrupts. Long-running C code in R checks `R_CheckUserInterrupt()` periodically.

### Integration points

| R feature | signal-hook API |
| --------- | --------------- |
| Ctrl+C interrupts computation | `signal_hook::flag::register(SIGINT, Arc<AtomicBool>)` |
| `Sys.sleep(n)` is interruptible | Check interrupt flag during sleep loop |
| `tryCatch(..., interrupt=)` | Check flag, throw RError::Interrupt |
| Long loops (for, while, sapply) | Check flag at loop iteration boundaries |

### Implementation

```rust
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use signal_hook::consts::SIGINT;

// Global interrupt flag
lazy_static! {
    static ref INTERRUPTED: Arc<AtomicBool> = {
        let flag = Arc::new(AtomicBool::new(false));
        signal_hook::flag::register(SIGINT, Arc::clone(&flag)).unwrap();
        flag
    };
}

fn check_interrupt() -> Result<(), RError> {
    if INTERRUPTED.load(Ordering::Relaxed) {
        INTERRUPTED.store(false, Ordering::Relaxed);
        Err(RError::Interrupt)
    } else {
        Ok(())
    }
}
```

Then insert `check_interrupt()?` calls at:
- Top of every loop iteration (for, while, repeat)
- Every N iterations of vectorized operations (sapply, lapply)
- Inside `Sys.sleep()` polling loop
- At function call boundaries

### New RError variant

```rust
pub enum RError {
    // ... existing variants ...
    Interrupt,  // Ctrl+C
}
```

`tryCatch(expr, interrupt = handler)` would catch `RError::Interrupt`.

## Implementation order

1. Add `RError::Interrupt` variant
2. Register SIGINT handler at interpreter startup
3. Add `check_interrupt()` to for/while/repeat loop bodies
4. Add `check_interrupt()` to apply family (every N iterations)
5. Make `Sys.sleep()` interruptible
6. Wire `tryCatch(..., interrupt=)` to catch Interrupt errors

## Priority

High — fundamental UX issue. Without this, users must kill the process to stop runaway computations. Already vendored, zero build cost.
