# Parser error messages plan

## Current state

Parse errors are pest's default output — a raw `format!("Parse error: {}", e)` string.
Pest errors show grammar rule names that mean nothing to users:

```
> "hello""
Parse error:  --> 1:8
  |
1 | "hello""
  |        ^---
  |
  = expected eq_assign_op, left_assign_op, right_assign_op, or_op, and_op, ...
```

R gives:
```
> "hello""
Error: unexpected string constant in ""hello""
```

## Goals

- Detect common mistakes and explain what went wrong in plain English
- Suggest fixes where possible
- Show the source location with context (the line, a caret pointing to the error)
- Color the output (red for error, cyan for source, yellow for suggestions)
- Work in both REPL and file mode (file mode shows filename:line:col)

## What to build

### Custom error type with spans

Replace `Result<Expr, String>` in `parse_program` with a proper error type:

```rust
pub struct ParseError {
    pub message: String,
    pub line: usize,
    pub col: usize,
    pub source_line: String,
    pub filename: Option<String>,
    pub suggestion: Option<String>,
}
```

### Pest error conversion with human-friendly rule names

Map pest grammar rules to readable descriptions:

| Grammar rule | Human description |
|---|---|
| `eq_assign_op` | `'='` |
| `left_assign_op` | `'<-'` or `'<<-'` |
| `right_assign_op` | `'->'` or `'->>'` |
| `or_op` | `'\|'` or `'\|\|'` |
| `and_op` | `'&'` or `'&&'` |
| `compare_op` | a comparison operator |
| `add_op` | `'+'` or `'-'` |
| `mul_op` | `'*'` or `'/'` |
| `special_op` | a special operator (`%%`, `%in%`, etc.) |
| `pipe_op` | `'\|>'` |
| `power_op` | `'^'` |
| `unary_expr`, `primary_expr`, `expr` | an expression |
| `ident` | a variable name |
| `string` | a string |
| `number` | a number |
| `EOI` | end of input |

### Token classification at error position

Identify what the parser actually found at the error location:

- String literal → "string constant"
- Number literal → "numeric constant"
- Keyword (`if`, `else`, `for`, etc.) → the keyword name in quotes
- Operator → the operator itself
- Punctuation → the character itself
- EOF → "end of input"

Combine into R-style message: `unexpected <what-was-found> in "<context>"`

### Common-mistake detection with suggestions

Before falling through to the generic error, check for known patterns:

| Pattern | Message | Suggestion |
|---|---|---|
| `if x > 0` (no parens) | "missing parentheses around `if` condition" | "try `if (x > 0)`" |
| Unmatched `(`, `{`, `[` | "unmatched opening `(`" | "add closing `)` " |
| Unmatched `)`, `}`, `]` | "unexpected closing `)`" | "remove `)` or add matching `(`" |
| `else` at start of new line (REPL) | "`else` must follow `}` on the same line" | "put `} else {` on one line" |
| Missing comma `f(a b)` | "missing comma between arguments" | "try `f(a, b)`" |
| `=` vs `==` in condition | "`=` is assignment here, not comparison" | "use `==` for comparison" |
| Unterminated string | "unterminated string" | "add closing quote" |
| `}` without `{` | "unexpected `}` without matching `{`" | — |
| Double operator `x + + y` | "unexpected `+`" | "remove the extra `+`" |

### Colored error display

```
error: unexpected string constant
 --> script.R:1:8
  |
1 | "hello""
  |        ^ unexpected string constant after complete expression
  |
  = help: remove the extra quote, or use paste() to concatenate strings
```

Use ANSI codes (crossterm is already a transitive dep):
- "error:" bold red
- File/line/col in blue
- The `^` pointer in bold red
- "help:" in bold cyan

### REPL incomplete-expression detection improvement

The current `is_likely_incomplete()` in `main.rs` manually counts brackets.
Improve by also detecting:

- Trailing binary operators: `x +`, `x &`, `x |>`, `x <-`
- Trailing commas: `f(1, 2,`
- `if (...) { ... } else` at end of line (else expects body)
- `function(x)` without body at end of line

### File-mode errors show filename and surrounding context

```
error: object 'x' not found
 --> script.R:7:12
  |
6 | y <- 42
7 | result <- x + y
  |           ^ not found in this scope
8 | print(result)
```

## Files to change

- `src/parser.rs` — new `ParseError` type, pest error conversion, mistake detection
- `src/main.rs` — use `ParseError`, pass filename, update REPL error display
- `src/interpreter/value.rs` — update `RError::Parse` to hold structured error

## R error message style reference

R follows these patterns:
- `Error: <message>` — errors without call context
- `Error in <call> : <message>` — errors in a specific call
- Parse errors: `Error in parse(...) : <file>:<line>:<col>: <description>`
- Object not found: `Error: object '<name>' not found`
- Unexpected token: `Error: unexpected <token-type> in "<context>"`
