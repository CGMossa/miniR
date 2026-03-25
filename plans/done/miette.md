# miette integration plan

> `miette` 7.6 — Rich diagnostic error reporting with source spans.
> <https://github.com/zkat/miette>

## What it does

Error reporting library that renders beautiful diagnostics with:

- Source code snippets with underlined spans
- Colored labels pointing to the error location
- Help text and suggestions
- Related errors and causes

```rust
#[derive(Debug, Diagnostic, Error)]
#[error("unexpected token")]
#[diagnostic(code(parse::unexpected_token), help("did you mean `{suggestion}`?"))]
struct ParseError {
    #[source_code]
    src: NamedSource<String>,
    #[label("this token")]
    span: SourceSpan,
    suggestion: String,
}
```

Features: `fancy` (colored output), `fancy-no-backtrace` (no backtrace in fancy mode).

## Where it fits in miniR

### 1. Parser error reporting

Currently parse errors show line/column but no source context. With miette:

```text
Error: parse::unexpected_token

  × unexpected token `+`
   ╭─[script.R:3:5]
 3 │ x <- + 2
   ·      ╰── expected expression before operator
   ╰────
  help: remove the leading `+` or add an expression before it
```

This requires the parser to track byte offsets (spans), which pest already provides
via `Pair::as_span()`.

### 2. Runtime error reporting

R errors like "object 'x' not found" can show the source line:

```text
Error: eval::not_found

  × object 'x' not found
   ╭─[script.R:7:12]
 7 │ result <- x + y
   ·           ╰── this variable is not defined
   ╰────
```

### 3. Warning display

R warnings can be shown as miette warnings (yellow, non-fatal):

```text
Warning: coerce::na_introduced

  ⚠ NAs introduced by coercion
   ╭─[script.R:2:8]
 2 │ as.integer("abc")
   ·            ╰── cannot convert to integer
   ╰────
```

## Relationship to builtins plan

| Phase | Builtins affected | Impact |
|---|---|---|
| All phases | `stop()`, `warning()`, `message()` | rich error display |
| Parser | all parse errors | source-span diagnostics |

## Recommendation

**Add when we overhaul error reporting.** This is the gold standard for Rust error
display. Requires threading source spans through the AST and into the interpreter,
which is a meaningful refactor but hugely improves user experience.

**Effort:** Medium — 2-3 sessions to add spans to AST and integrate miette.
