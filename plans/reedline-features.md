# Reedline REPL Features

Implement all major reedline features demonstrated in `vendor/reedline/examples/`. The current REPL uses only `Reedline::create()` + `DefaultPrompt` — all advanced features are unused.

## Current state

- Basic REPL with `DefaultPrompt` and `DefaultPromptSegment::Basic`
- Manual multi-line detection in `is_likely_incomplete()` (tracks parens/braces/brackets, unclosed strings, trailing operators)
- No history, completions, hints, highlighting, or customization

## Features to implement (priority order)

1. **Persistent history** — `FileBackedHistory` stored in `~/.miniR_history`. Most impactful quality-of-life feature. See `examples/history.rs`.

2. **Validator for multi-line input** — Replace the manual `is_likely_incomplete()` with reedline's `Validator` trait. Cleaner integration, and reedline handles the multi-line prompt automatically. See `examples/validator.rs`.

3. **Syntax highlighting** — Implement `Highlighter` trait for R syntax. Color keywords (`if`, `for`, `function`, `TRUE`, `FALSE`), strings, numbers, comments, operators. See `examples/highlighter.rs`.

4. **History-based hints** — Fish-shell style inline suggestions from history via `DefaultHinter`. See `examples/hinter.rs`.

5. **Tab completion** — `Completer` trait with R-aware completions: builtin function names, variable names from current environment, file paths for string arguments. Use `ColumnarMenu` for display. See `examples/completions.rs`.

6. **IDE-style completions** — Upgrade to `IdeMenu` with descriptions (function signatures, argument docs). See `examples/ide_completions.rs`.

7. **Custom prompt** — Show current working directory, environment name, or session info in prompt. Right-side prompt for elapsed time or memory usage. See `examples/custom_prompt.rs`.

8. **Vi/Emacs edit modes** — Let users choose editing mode via `.Rprofile` or environment variable. See `examples/demo.rs`.

9. **Transient prompt** — Previous lines get simplified prompt (just `>`), current input line gets full prompt. See `examples/transient_prompt.rs`.

10. **Mouse click positioning** — Click to position cursor in input. See `examples/mouse_click.rs`.

11. **Semantic markers** — OSC 133/633 markers for terminal-aware navigation (Ghostty, VS Code). See `examples/semantic_prompt_interactive.rs`.

12. **External printer** — Background thread printing (for async task output). Requires `external_printer` feature. See `examples/external_printer.rs`.

## Architecture notes

- All reedline features are configured via builder methods on `Reedline::create()`
- The `Completer` trait needs access to the interpreter's current environment — use `with_interpreter()` pattern
- History file location: `~/.miniR_history` (or `$NEWR_HISTFILE` if set)
- Highlighting and completion should be implemented as separate modules in `src/repl/` (new directory)
