# Error Messages Improvement Plan

## Problem Statement

Error messages across newr are currently either raw internal representations (pest grammar rule names) or generic/vague descriptions that don't help users diagnose problems. R users expect error messages that match GNU R's style and provide actionable context.

Example of current bad output:
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

---

## Phase 1: Parse Error Translation (pest -> human-readable)

### 1.1 Build a rule name -> description mapping

Create a function `humanize_expected(rule: &str) -> &str` that maps internal pest grammar rule names to user-facing descriptions:

| Grammar rule | Human description |
|---|---|
| `eq_assign_op` | `'='` |
| `left_assign_op` | `'<-'` or `'<<-'` |
| `right_assign_op` | `'->'` or `'->>'` |
| `or_op` | `'\|'` or `'\|\|'` |
| `and_op` | `'&'` or `'&&'` |
| `compare_op` | comparison operator |
| `add_op` | `'+'` or `'-'` |
| `mul_op` | `'*'` or `'/'` |
| `special_op` | special operator (`%%`, `%in%`, etc.) |
| `pipe_op` | `'\|>'` |
| `range_op` | `':'` |
| `power_op` | `'^'` |
| `unary_expr` | expression |
| `primary_expr` | expression |
| `ident` | identifier |
| `string` | string |
| `number` | number |
| `program` | end of input |
| `EOI` | end of input |
| `WHITESPACE` | *(skip/hide this)* |

### 1.2 Custom parse error formatter

Replace the raw `format!("Parse error: {}", e)` with a custom formatter that:

1. Extracts the position (line, column) from the pest error
2. Extracts the list of "expected" rule names
3. Determines what token was actually found at the error position
4. Generates an R-style message like:
   - `Error: unexpected string constant in "<context>"`
   - `Error: unexpected ')' in "<context>"`
   - `Error: unexpected symbol in "<context>"`
   - `Error: unexpected end of input`
   - `Error: unexpected 'else' in "<context>"`

The `<context>` should show the input up to and including the unexpected token, truncated to ~40 chars from the right.

### 1.3 Detect common parse error patterns

Add special-case detection for frequent mistakes:

| Pattern | Current message | Improved message |
|---|---|---|
| `"hello""` | `expected eq_assign_op, ...` | `unexpected string constant in ""hello""` |
| `if (x) { } else` at EOL | `expected unary_expr` | `unexpected end of input` (then continue prompt in REPL) |
| `1 +` at EOL | `expected primary_expr` | incomplete expression (continue prompt in REPL) |
| `f(,)` with bad empty arg | depends | `unexpected ',' in "f(,"` |
| `x$` at EOL | `expected string or ident` | `unexpected end of input in "x$"` |
| `pkg::"string"` | `expected ident` | `unexpected string constant (namespace accessor requires unquoted name)` |

### 1.4 Token classification

Write a `classify_token(input: &str, pos: usize) -> &str` function that identifies what the parser actually found:

- String literal → "string constant"
- Number literal → "numeric constant"
- Identifier/keyword → "symbol" or the keyword name (`'if'`, `'else'`, `'for'`, etc.)
- Operator → the operator itself (`'+'`, `'-'`, etc.)
- Punctuation → the character itself (`')'`, `'}'`, `','`, etc.)
- EOF → "end of input"
- Newline → "end of input" (for REPL line-by-line)

### 1.5 Improve REPL incomplete-expression detection

Currently `is_likely_incomplete()` in main.rs does bracket counting. Improve it:

- Also detect trailing operators: `x +`, `x &`, `x &&`, `x |`, `x ||`, `x <-`
- Detect trailing commas: `f(1, 2,`
- Detect `if (...) { ... } else` at end of line (else expects body)
- Detect `function(x)` without body at end of line
- Detect trailing pipe: `x |>`

---

## Phase 2: Runtime Error Messages (interpreter/mod.rs)

### 2.1 Add function name context to errors

Many runtime errors should include which function produced the error. Currently:

```
Error: non-numeric argument to binary operator
```

R says:
```
Error in x + y : non-numeric argument to binary operator
```

Add a call context stack or at minimum pass the current function/operator name to error constructors.

### 2.2 Improve specific interpreter errors

| Location | Current | Improved (match R) |
|---|---|---|
| `mod.rs:181,185` | `invalid argument to unary operator` | `invalid argument to unary operator` (add the operator: `!`, `-`, `+`) |
| `mod.rs:194` | `invalid argument type` | `non-numeric argument to unary operator` |
| `mod.rs:211,216` | `non-numeric argument to binary operator` | `non-numeric argument to binary operator` (add context: `x + y`) |
| `mod.rs:460` | `invalid use of pipe` | `the pipe operator requires a function call on the right side` |
| `mod.rs:506,508` | `invalid assignment target` | `invalid (do_set) left-hand side to assignment` or identify the bad target |
| `mod.rs:521` | `attempt to apply non-function` | `attempt to apply non-function` (add the object type and name) |
| `mod.rs:629` | `attempt to apply non-function` | same as above |
| `mod.rs:740,770` | `invalid index type` | `invalid subscript type 'X'` where X is the actual type |
| `mod.rs:773,894,1012` | `object is not subsettable` | `object of type 'X' is not subsettable` with actual type |
| `mod.rs:939` | `invalid index` | `invalid subscript type 'X'` |
| `mod.rs:944` | `replacement value error` | `replacement has length X, data has length Y` or appropriate R message |
| `mod.rs:1102` | `pkg::name` | `object 'name' not found in namespace 'pkg'` (currently hardcodes "pkg") |
| `mod.rs:1137` | `invalid for() loop sequence` | `invalid for() loop sequence of type 'X'` |

### 2.3 Fix the namespace error (bug)

`mod.rs:1102` currently hardcodes `"pkg"` in the error message:
```rust
.ok_or_else(|| RError::Name(format!("{}::{}", "pkg", name)))
```
This should use the actual namespace expression.

---

## Phase 3: Builtin Function Errors (builtins.rs)

### 3.1 Include function names in errors

Currently builtin errors are generic:
```
Error in argument: argument is not numeric
```

R says:
```
Error in sum("a") : invalid 'type' (character) of argument
```

For each builtin error, include the function name in the message. Consider adding an `RError::Call { fn_name, message }` variant or passing function context through.

### 3.2 Improve specific builtin errors

| Current (count) | Improved |
|---|---|
| `argument is not numeric` (x8) | `non-numeric argument to mathematical function` or `invalid 'type' (X) of argument` with actual type |
| `argument is not character` (x7) | `non-character argument` with function name |
| `argument is not a vector` (x3) | `argument is not a vector` with actual type |
| `need 2 arguments` (x5) | `X arguments passed to 'fn' which requires Y` |
| `need 3 arguments` (x2) | same pattern |
| `argument "x" is missing` | `argument "x" is missing, with no default` (match R exactly) |
| `argument is missing` | `argument is missing, with no default` |
| `invalid argument` | more specific based on context |
| `sapply/lapply/vapply requires interpreter context` | these should never appear to the user — fix the architecture |

### 3.3 Argument count validation pattern

Create a helper macro or function:
```rust
fn check_min_args(fn_name: &str, args: &[RValue], min: usize) -> Result<(), RError> {
    if args.len() < min {
        Err(RError::Argument(format!(
            "{} argument(s) passed to '{}' which requires at least {}",
            args.len(), fn_name, min
        )))
    } else {
        Ok(())
    }
}
```

---

## Phase 4: RError Enum Redesign

### 4.1 Add structured error variants

Current `RError` uses stringly-typed messages. Add more structured variants:

```rust
pub enum RError {
    // Parse errors
    Parse { message: String, line: usize, col: usize },

    // Runtime errors with call context
    Type { message: String, call: Option<String> },
    Argument { message: String, call: Option<String> },
    Name { name: String },
    Index { message: String },
    Condition { message: String, call: Option<String> },

    // Control flow (not user-visible)
    Return(RValue),
    Break,
    Next,
}
```

### 4.2 Add warning support

R has `warning()` which prints but doesn't stop execution. Currently newr has no warning mechanism. Add:

```rust
pub enum RWarning {
    Coercion(String),   // "NAs introduced by coercion"
    General(String),
}
```

And a warning accumulator in the interpreter that flushes after each top-level expression.

### 4.3 Improve Display formatting

Update the `Display` impl for `RError` to match R's format:

```
Error in <call> : <message>
```

When there's a call context. Without context:
```
Error: <message>
```

---

## Phase 5: Parser Panics and Robustness

### 5.1 Eliminate unwrap() calls in parser

There are 34 `unwrap()` calls in `src/parser/mod.rs`. While most are safe (pest guarantees structure), any that can fail on malformed input should be converted to proper error returns. Audit each:

- `pair.into_inner().next().unwrap()` — safe if grammar guarantees child, but should still use `expect("grammar guarantees X")`
- Any `unwrap()` on user-facing values (string parsing, number parsing) should return errors

### 5.2 Add `expect()` messages

Replace bare `.unwrap()` with `.expect("descriptive message")` for internal invariants, so panics produce useful crash reports instead of `called Option::unwrap() on a None value`.

### 5.3 Handle the CRANtools.R panic

`tests/CRANtools.R` causes a panic at `mod.rs:369:56`. Investigate and fix — this should produce a parse error, not a crash.

---

## Phase 6: Error Location Tracking

### 6.1 Add source positions to AST

Currently the AST has no span/position information. To produce errors like R does:
```
Error in source("file.R") : file.R:42:10: unexpected symbol
42: bad code here
             ^
```

Add `Span { start: usize, end: usize }` to AST nodes (or wrap in `Spanned<Expr>`). This is a large refactor but essential for good error reporting.

### 6.2 Source context in runtime errors

When a runtime error occurs, show the source line and position:
```
Error in file.R:15 : object 'xyz' not found
```

This requires tracking which source line each AST node came from.

---

## Implementation Priority

1. **Phase 1.2 + 1.1** (custom parse error formatter) — highest impact, fixes the ugliest messages
2. **Phase 5.3** (fix CRANtools panic) — crashes are worse than bad messages
3. **Phase 2.3** (fix namespace bug) — actual bug, not just cosmetic
4. **Phase 2.2** (improve interpreter errors) — moderate effort, good improvement
5. **Phase 3.1 + 3.3** (function names in builtin errors) — lots of messages to improve
6. **Phase 1.5** (REPL incomplete detection) — QoL improvement
7. **Phase 5.1 + 5.2** (eliminate unwraps) — robustness
8. **Phase 4** (RError redesign) — larger refactor, do when other phases feel limited
9. **Phase 6** (source positions) — biggest refactor, defer until needed
10. **Phase 4.2** (warnings) — nice to have

---

## Reference: R Error Message Style

R follows these patterns:
- `Error: <message>` — for errors without call context
- `Error in <call> : <message>` — for errors in a specific call
- `Warning message:` / `In <call> : <message>` — for warnings
- Parse errors: `Error in parse(...) : <file>:<line>:<col>: <description>`
- Object not found: `Error: object '<name>' not found`
- Type errors: `Error in <expr> : non-numeric argument to binary operator`
- Subscript errors: `Error in x[i] : subscript out of bounds`
