# tracing integration plan

> `tracing` — Structured, context-aware logging and diagnostics.
> <https://github.com/tokio-rs/tracing>

## Why tracing over log

- **Structured spans**: `tracing::span!` tracks enter/exit of functions, not just point events. Perfect for tracking R function calls, S3 dispatch chains, and eval recursion.
- **Per-interpreter context**: Spans can carry fields like interpreter ID, so logs from parallel interpreters don't interleave confusingly.
- **Compatible with log**: `tracing` emits `log` records too via `tracing-log`, so any `log`-based subscriber still works.
- **Async-ready**: If we ever add async I/O (mio plan), tracing handles it natively.
- **Ecosystem**: `tracing-subscriber` for formatting, `tracing-flame` for flamegraphs, `tracing-chrome` for Chrome devtools traces.

## What to instrument

### Spans (enter/exit tracking)
- `eval_in` — span per expression evaluation
- `call_function` — span per R function call with function name
- `dispatch_s3` — span for S3 method lookup
- `bind_closure_call` — span for argument matching
- Session `eval_source` / `eval_file` — top-level span

### Events (point-in-time)
- `trace!` — symbol lookup, environment creation
- `debug!` — S3 dispatch decisions, argument match results
- `info!` — interpreter creation, file loading, signal handling
- `warn!` — R warnings (suppressible)
- `error!` — R errors (catchable)

## Implementation

1. Add `tracing = "0.1"` as direct dep (facade is zero-cost like `log`)
2. Add `tracing-subscriber = { version = "0.3", optional = true }` behind `tracing-output` feature
3. Initialize in main.rs: `tracing_subscriber::fmt().with_env_filter("MINIR_LOG").init()`
4. Replace any `log::*` calls with `tracing::*` equivalents
5. Add `#[tracing::instrument]` on key functions for automatic span creation

## Priority: Medium — valuable for debugging but not blocking features.
