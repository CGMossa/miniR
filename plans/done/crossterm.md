# crossterm integration plan

> `crossterm` 0.29 — Cross-platform terminal manipulation.
> <https://github.com/crossterm-rs/crossterm>

## What it does

Pure-Rust terminal library. Cursor movement, styled output, event reading
(keyboard/mouse/resize), raw mode, alternate screen. Works on Windows, macOS, Linux.

Key APIs:

- `crossterm::style` — `SetForegroundColor`, `SetAttribute`, `Print`, `Stylize` trait
- `crossterm::cursor` — `MoveTo`, `Hide`, `Show`, `SavePosition`, `RestorePosition`
- `crossterm::terminal` — `size()`, `Clear`, `enable_raw_mode`, `EnterAlternateScreen`
- `crossterm::event` — `read()`, `poll()`, `Event::Key`, `Event::Mouse`, `Event::Resize`

## Where it fits in miniR

### 1. REPL — already in use via reedline

`reedline` depends on crossterm internally. We already have crossterm as a transitive
dependency. Direct use would be for:

- **Colored error messages** — `style::Stylize` trait for red errors, yellow warnings
- **Terminal size detection** — `terminal::size()` for `getOption("width")`, `format()`,
  `print.data.frame()` column wrapping

### 2. `readline()` / `menu()` — interactive input

R's `readline(prompt)` reads a line from the terminal. Outside the REPL context
(e.g. in script mode with `--interactive`), crossterm's event loop can handle this.

### 3. `cat()` with ANSI — styled output

R packages like `cli` and `crayon` emit ANSI escape codes. crossterm ensures these
work cross-platform (especially Windows where ANSI support varies).

### 4. `Sys.sleep()` with interrupt — poll for Ctrl-C

`crossterm::event::poll(Duration)` with timeout enables interruptible sleep:

```rust
fn sys_sleep(seconds: f64) {
    let deadline = Instant::now() + Duration::from_secs_f64(seconds);
    while Instant::now() < deadline {
        if crossterm::event::poll(Duration::from_millis(100)).unwrap_or(false) {
            if let Ok(Event::Key(ke)) = crossterm::event::read() {
                if ke.code == KeyCode::Char('c') && ke.modifiers.contains(KeyModifiers::CONTROL) {
                    return; // interrupted
                }
            }
        }
    }
}
```

## Relationship to builtins plan

| Phase | Builtins affected | Impact |
|---|---|---|
| Phase 11 (I/O) | `cat()` styled output, `readline()` | cross-platform terminal I/O |
| All phases | error/warning display | colored diagnostics |
| Phase 6 (OS) | `Sys.sleep()` | interruptible sleep |

## Recommendation

**Already a transitive dependency.** Add as direct dependency only when we need
terminal size or styled output beyond what reedline provides. Low priority —
`termcolor` may be simpler for just colored output.

**Effort:** Trivial to add; medium effort to use throughout.
