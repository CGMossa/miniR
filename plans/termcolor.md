# termcolor integration plan

> `termcolor` 1.4 — Cross-platform colored terminal output by BurntSushi.
> https://github.com/BurntSushi/termcolor

## What it does

Write colored/styled text to terminals. Supports both ANSI escape codes (Unix)
and Windows Console API. Auto-detects terminal capabilities.

```rust
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};
use std::io::Write;

let mut stdout = StandardStream::stdout(ColorChoice::Auto);
stdout.set_color(ColorSpec::new().set_fg(Some(Color::Red)).set_bold(true))?;
write!(&mut stdout, "Error: ")?;
stdout.reset()?;
writeln!(&mut stdout, "something went wrong")?;
```

## Where it fits in newr

### 1. Error/warning/message display

Color-code R's diagnostic output:
- `stop()` → red bold "Error: ..."
- `warning()` → yellow "Warning message: ..."
- `message()` → cyan (or default) stderr output

### 2. REPL prompt styling

Color the `> ` prompt, continuation `+ ` prompt, or output differently from input.

### 3. `cli` package equivalents

R's `cli` package provides styled terminal output. Built-in equivalents:
- `cli::cli_alert_success()` → green checkmark
- `cli::cli_alert_danger()` → red cross
- `cli::cli_text()` → styled text

### Comparison with crossterm

termcolor is simpler and more focused:
- **termcolor** — just colored text output, minimal API
- **crossterm** — full terminal control (cursor, events, raw mode, etc.)

For just coloring output, termcolor is lighter.

## Relationship to builtins plan

| Phase | Builtins affected | Impact |
|---|---|---|
| All phases | `stop()`, `warning()`, `message()` | colored diagnostics |
| Core (REPL) | prompt display | styled prompt |

## Recommendation

**Add when implementing colored error output.** Simpler than crossterm for just
coloring text. Can coexist with crossterm (which reedline already uses).

**Effort:** 30 minutes for error/warning coloring.
