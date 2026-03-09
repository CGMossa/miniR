# nu-ansi-term integration plan

> `nu-ansi-term` 0.50 — ANSI terminal styling (colors, bold, underline, etc.).
> Already vendored as a transitive dependency of reedline.

## What it does

Apply ANSI escape codes to strings for colored/styled terminal output. Fork of `ansi_term` maintained by the Nushell team.

```rust
use nu_ansi_term::{Color, Style};

Color::Red.paint("error text");
Color::Yellow.bold().paint("warning");
Style::new().italic().paint("note");
```

## Where it fits in newr

### Colored error messages

Our CLAUDE.md says: "Error messages should be better than GNU R's — more informative, more specific, with suggestions." Color makes errors significantly more readable.

```
Error in eval(expr, envir): object 'x' not found
       ^^^^                         ^^^
       red                          yellow
Did you mean: 'xx'?  (suggestion in green)
```

### R functions / features

| Feature | How nu-ansi-term helps |
| ------- | --------------------- |
| `message()` | Print in a distinct color (R uses stderr + red in some IDEs) |
| `warning()` | Yellow text for warnings |
| `stop()` / error output | Red text for errors with bold function name |
| REPL prompt | Colored prompt (already partially done via reedline) |
| `cat()` output | Could support ANSI codes in cat output |
| `crayon` package emulation | `crayon::red()`, `crayon::bold()` etc. |
| Error suggestions | Green text for "did you mean..." hints |
| Traceback | Dim gray for stack frames, bright for the error line |

### Implementation

```rust
use nu_ansi_term::Color;

fn format_error(err: &RError) -> String {
    match err {
        RError::Name(name) => format!(
            "{}: object '{}' not found",
            Color::Red.bold().paint("Error"),
            Color::Yellow.paint(name),
        ),
        RError::Argument(msg) => format!(
            "{}: {}",
            Color::Red.bold().paint("Error"),
            msg,
        ),
        // ...
    }
}

fn format_warning(msg: &str) -> String {
    format!(
        "{}: {}",
        Color::Yellow.bold().paint("Warning"),
        msg,
    )
}
```

### Terminal detection

Should detect if stdout is a TTY and disable colors for piped output. Use `std::io::IsTerminal` (stable since Rust 1.70):

```rust
use std::io::IsTerminal;

fn use_color() -> bool {
    std::io::stderr().is_terminal()
}
```

## Implementation order

1. Color error messages (stop/error output)
2. Color warning messages
3. Color REPL error output
4. Add `--no-color` CLI flag
5. Color "did you mean..." suggestions
6. Consider `crayon` package emulation as builtins

## Priority

Medium-high — significantly improves developer experience. Already vendored, zero build cost. Error readability is a stated design goal.
