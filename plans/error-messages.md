# Parser Error Messages Follow-up

## Current state

The original parser-error rewrite has already landed:

- `src/parser.rs` has a structured `ParseError`
- parse errors show source context and caret placement
- common mistakes can produce targeted suggestions
- file mode can attach `filename:line:col`

Example:

```text
Error: unterminated string
  |
1 | "hello""
  |        ^
  |
  = help: add a closing `"` to complete the string
```

## Remaining work

- Keep repo tooling and docs aligned with the current parser error prefix (`Error:` / `Error in parse:`), not the old `Parse error:` text
- Decide whether parser and runtime errors should share one colored display style
- Add lightweight CLI smoke coverage once there is a real Rust integration-test harness
- Tighten file-mode consistency if parser and runtime diagnostics start to diverge

## What not to redo

- Do not revert to raw pest errors
- Do not reintroduce stringly `Result<Expr, String>` parsing APIs
- Do not treat this file as the parser rewrite plan; it is now follow-up cleanup
