# Per-Module Error Types

Replace the centralized `RError` enum with module-specific error types using `derive_more`'s `Error` derive. Each module defines its own error type, and conversion to `RError` happens at module boundaries.

## Motivation

The current `RError` is a catch-all enum shared by every module:

```rust
pub enum RError {
    Type(String),
    Argument(String),
    Name(String),
    Index(String),
    Parse(String),
    Other(String),
    Condition { condition: RValue, kind: ConditionKind },
    Return(RValue),
    Break,
    Next,
}
```

Problems:
- **Every module depends on `RError`** — tight coupling. Adding `From<TryFromIntError>` for `RError` caused type inference failures across 12 files.
- **No structured context** — errors are just strings. Can't programmatically distinguish "index out of bounds" from "negative index" without string matching.
- **Control flow mixed with errors** — `Return`, `Break`, `Next` are not errors, they're control flow signals. They belong in a separate enum.
- **No source chaining** — can't attach "caused by" context without string concatenation.

## Design

### Separate control flow from errors

```rust
/// Control flow signals (not errors)
pub enum RSignal {
    Return(RValue),
    Break,
    Next,
}

/// Evaluation result type
pub type EvalResult<T> = Result<T, RFlow>;

pub enum RFlow {
    Error(RError),
    Signal(RSignal),
}
```

### Per-module error types

```rust
// src/interpreter/builtins/math.rs
use derive_more::Error;

#[derive(Debug, Error)]
pub enum MathError {
    #[error("non-numeric argument to mathematical function: {0}")]
    NonNumeric(String),
    #[error("value {value} out of range for {target}")]
    OutOfRange { value: String, target: &'static str },
    #[error("NaN produced")]
    NaN,
}

impl From<MathError> for RError {
    fn from(e: MathError) -> Self {
        RError::Type(e.to_string())
    }
}
```

```rust
// src/interpreter/builtins/strings.rs
#[derive(Debug, Error)]
pub enum StringError {
    #[error("invalid multibyte character at position {pos}")]
    InvalidMultibyte { pos: usize },
    #[error("invalid regular expression: {pattern}: {reason}")]
    InvalidRegex { pattern: String, reason: String },
}

impl From<StringError> for RError {
    fn from(e: StringError) -> Self {
        RError::Type(e.to_string())
    }
}
```

```rust
// src/interpreter/builtins/io.rs
#[derive(Debug, Error)]
pub enum IoError {
    #[error("cannot open file '{path}': {reason}")]
    CannotOpen { path: String, reason: String },
    #[error("cannot read from connection: {0}")]
    ReadFailed(#[from] std::io::Error),
}

impl From<IoError> for RError {
    fn from(e: IoError) -> Self {
        RError::Other(e.to_string())
    }
}
```

### Conversion helpers with derive_more

Enable the `error` feature in `derive_more`:

```toml
derive_more = { version = "2.1", features = ["deref", "deref_mut", "from", "into", "constructor", "error", "display"] }
```

Then error types get `Display` and `Error` for free via `#[error("...")]` attributes.

### External error integration

Module-local errors can wrap external crate errors directly:

```rust
#[derive(Debug, Error)]
pub enum IoError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("CSV parse error: {0}")]
    Csv(#[from] csv::Error),
}
```

This eliminates ad-hoc `.map_err(|e| RError::Other(e.to_string()))` sprinkled everywhere. The `From<TryFromIntError>` that caused widespread type inference issues would instead live only in the modules that need it.

## Migration strategy

This is a large refactor. Do it incrementally:

1. **Enable `error` and `display` features** in derive_more — zero code changes
2. **Separate control flow from errors** — `RSignal` enum for Return/Break/Next, `RFlow` enum wrapping both. Update the evaluator's return type. This is the biggest single change.
3. **Extract `ParseError`** from `RError::Parse` — the parser module gets its own error type
4. **Extract `IndexError`** from `RError::Index` — indexing operations get structured errors
5. **One module at a time** — add `MathError`, `StringError`, `IoError`, etc. Each module change is self-contained.
6. **Remove string variants from `RError`** — once all modules have their own types, `RError` becomes a simple sum type of module errors

## Prerequisites

- Enable `error` and `display` features in `derive_more`
- Read `vendor/derive_more/` docs for the `#[error()]` and `#[display()]` attribute syntax
