# reedline integration plan

> `reedline` 0.46+bashisms — Modern readline-style line editor.
> <https://github.com/nushell/reedline>

## What it does

Line editor library from the Nushell project. Features:

- Multiline editing, syntax highlighting, completions
- Vi and Emacs keybinding modes
- History (file-backed, searchable, with deduplication)
- Prompt customization
- `+bashisms` feature: `!!`, `!$`, `!n` history expansion

## Where it fits in newr

### Already in use

reedline is the REPL backend in `src/main.rs`. Current integration includes:

- Basic prompt (`>` for input, `+` for continuation)
- History persistence
- Multiline R expression input

### Future enhancements

1. **R-aware syntax highlighting** — Highlight keywords (`if`, `for`, `function`),
   strings, numbers, comments in the input line
2. **Tab completion** — Complete variable names from the current environment,
   function arguments, file paths
3. **Custom keybindings** — Vi mode for R (like ESS in Emacs)
4. **Multiline continuation** — Detect incomplete expressions (unclosed `{`, `(`,
   unfinished `if`) and continue prompting with `+`
5. **`history()` builtin** — Access reedline's history from R code

## Relationship to builtins plan

| Phase | Builtins affected | Impact |
|---|---|---|
| Phase 11 (I/O) | `readline()`, `menu()` | interactive input |
| Phase 6 (OS) | `history()` | REPL history access |

## Recommendation

**Already added.** Incremental improvements to highlighting and completion as the
interpreter matures. The `+bashisms` feature is already enabled.

**Effort:** Ongoing — each REPL enhancement is 1-2 hours.
