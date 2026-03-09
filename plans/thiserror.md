# thiserror integration plan

> `thiserror` 2.0 — Derive macro for `std::error::Error`.
> Already vendored as a transitive dependency of reedline.

## What it does

Provides `#[derive(Error)]` to automatically implement `Display` and `Error` for error enums. Removes boilerplate from error type definitions.

## Where it fits in newr

### Current RError

Our `RError` is manually implemented with hand-written `Display`:

```rust
pub enum RError {
    Name(String),
    Type(String),
    Argument(String),
    Index(String),
    Parse(String),
    Other(String),
    Return(RValue),
    // ...
}
```

Each variant wraps a `String` with a message. The `Display` impl is a large `match` block.

### With thiserror

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RError {
    #[error("object '{0}' not found")]
    Name(String),

    #[error("non-numeric argument to binary operator: {0}")]
    Type(String),

    #[error("invalid argument: {0}")]
    Argument(String),

    #[error("subscript out of bounds: {0}")]
    Index(String),

    #[error("parse error: {0}")]
    Parse(String),

    #[error("{0}")]
    Other(String),

    #[error("no function to return from")]
    Return(RValue),

    #[error("user interrupt")]
    Interrupt,

    #[error("{source}")]
    Io {
        #[from]
        source: std::io::Error,
    },
}
```

### Benefits

1. **`#[from]` attribute** — automatic `From<io::Error>` for `RError`, enables `?` operator on I/O operations
2. **Structured display** — format strings next to variants, easier to maintain
3. **`Error` trait impl** — enables `source()` chaining for error cause chains
4. **Less boilerplate** — removes manual `Display` impl

### The bigger opportunity: structured errors

Instead of `RError::Type(String)`, we could have:

```rust
#[derive(Error, Debug)]
pub enum RError {
    #[error("non-numeric argument to '{op}': got {got_type}")]
    Type { op: String, got_type: String },

    #[error("object '{name}' not found{}", suggestion.as_ref().map(|s| format!("\nDid you mean '{}'?", s)).unwrap_or_default())]
    Name { name: String, suggestion: Option<String> },

    #[error("argument '{arg}' is missing with no default")]
    MissingArg { arg: String },

    #[error("subscript out of bounds: index {index} for length {length}")]
    IndexOutOfBounds { index: i64, length: usize },
}
```

This makes errors machine-parseable and enables:
- "Did you mean..." suggestions (using edit distance on the `name` field)
- Precise error context in IDE integrations
- Better error formatting with colors (nu-ansi-term)

## Implementation order

1. Add `#[derive(Error)]` to RError
2. Add `#[from]` for `std::io::Error`
3. Migrate string-message variants to structured fields
4. Add "did you mean" suggestions for `Name` errors
5. Integrate with colored output (nu-ansi-term)

## Priority

Medium — improves code quality and error messages (a stated design goal). Already vendored, zero build cost. The structured error fields are the real win.
