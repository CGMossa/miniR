# log + simplelog integration plan

> `log` 0.4 — Logging facade. https://github.com/rust-lang/log
> `simplelog` 0.12 — Simple logging implementations. https://github.com/Drakulix/simplelog.rs

## What they do

`log` provides macros: `error!()`, `warn!()`, `info!()`, `debug!()`, `trace!()`.
`simplelog` provides concrete loggers: `TermLogger`, `WriteLogger`, `CombinedLogger`.

```rust
use simplelog::*;
CombinedLogger::init(vec![
    TermLogger::new(LevelFilter::Warn, Config::default(), TerminalMode::Mixed, ColorChoice::Auto),
    WriteLogger::new(LevelFilter::Debug, Config::default(), File::create("newr.log").unwrap()),
]).unwrap();

log::info!("Starting interpreter");
log::warn!("Deprecated function called: {}", name);
```

## Where they fit in newr

### 1. Interpreter diagnostics

Internal logging for development and debugging:
- `debug!("eval: {:?}", expr)` — trace evaluation
- `info!("Loading source: {}", path)` — file operations
- `warn!("Coercing {} to {}", from, to)` — implicit coercions

### 2. `--verbose` / `--debug` CLI flags

```
newr --verbose script.R    # show info! messages
newr --debug script.R      # show debug! messages
```

### 3. R's `message()` / `warning()` / `stop()`

These are R-level, not Rust `log` — but the Rust log system can be used alongside
to separate interpreter internals from R user-facing messages.

## Relationship to builtins plan

No direct relationship. This is infrastructure for interpreter development, not
R builtins. But useful for debugging builtin implementations.

## Recommendation

**Add when debugging becomes painful.** Currently `eprintln!` works fine for a
small codebase. When the interpreter grows complex enough that tracing execution
matters, add log+simplelog with `--verbose` flag.

**Effort:** 20 minutes for basic setup.
