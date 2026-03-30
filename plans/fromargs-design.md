# FromArgs design: modeling R argument semantics in Rust

## The real problem

R arguments have three states: missing, NULL, or a value. Most Rust arg-decoding
systems flatten this to "present or default" and lose information. That matters
because real R builtins branch on which combination of args was provided:

```r
seq(1, 10)             # from/to path
seq(1, 10, by = 0.5)   # from/to/by path
seq(length.out = 5)    # length.out path
```

And they distinguish NULL from missing:

```r
paste(x, sep = NULL)   # error: sep can't be NULL
paste(x)               # sep defaults to " " (missing, not NULL)
```

The design should make these distinctions natural in Rust, not fight them.

## Core type: `Arg<T>`

```rust
/// The three states of an R function argument.
pub enum Arg<T> {
    /// Caller did not provide this argument. `missing(x)` would return TRUE.
    Missing,
    /// Caller explicitly passed NULL.
    Null,
    /// Caller provided a value (possibly NA — that's inside T).
    Value(T),
}
```

This is the fundamental building block. Everything else composes on top of it.

```rust
impl<T> Arg<T> {
    pub fn is_missing(&self) -> bool { matches!(self, Arg::Missing) }
    pub fn is_null(&self) -> bool { matches!(self, Arg::Null) }
    pub fn value(&self) -> Option<&T> { match self { Arg::Value(v) => Some(v), _ => None } }
    pub fn unwrap_or(self, default: T) -> T { ... }
    pub fn unwrap_or_else(self, f: impl FnOnce() -> T) -> T { ... }
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Arg<U> { ... }

    /// Error if missing, collapse NULL to None, extract value to Some.
    pub fn required(self, param: &str) -> Result<Option<T>, RError> { ... }

    /// Collapse both missing and NULL to None.
    pub fn optional(self) -> Option<T> { ... }
}
```

Usage in a struct:

```rust
#[derive(FromArgs)]
#[builtin(name = "grep")]
struct GrepArgs {
    pattern: String,              // required, error if missing or NULL
    x: RValue,                    // required
    #[default(false)]
    value: bool,                  // missing → false
    #[default(false)]
    fixed: bool,
    #[name = "ignore.case"]
    #[default(false)]
    ignore_case: bool,
}
```

When you need the three-way distinction explicitly:

```rust
#[derive(FromArgs)]
#[builtin(name = "paste")]
struct PasteArgs {
    dots: Dots,
    sep: Arg<String>,      // need to distinguish missing (→ " ") from NULL (→ error)
    collapse: Arg<String>,  // missing = don't collapse, NULL = don't collapse, value = collapse
}

impl Builtin for PasteArgs {
    fn call(self, _ctx: &BuiltinContext) -> Result<RValue, RError> {
        let sep = match self.sep {
            Arg::Missing => " ".to_string(),
            Arg::Null => return Err(RError::arg("'sep' cannot be NULL")),
            Arg::Value(s) => s,
        };
        // ...
    }
}
```

**The rule**: plain `T` or `Option<T>` fields use the simple path (required or
default). `Arg<T>` fields get the full three-way state. You only pay for the
complexity when you need it.

## Variant dispatch: enum FromArgs

This is the interesting part. Some R builtins have genuinely different
implementations depending on which args are present. Today this is a mess of
nested `if let Some(...) = ...` checks. An enum-based FromArgs models it as
Rust pattern matching.

```rust
#[derive(FromArgs)]
#[builtin(name = "seq")]
enum SeqCall {
    /// seq(from, to) or seq(from, to, by = 0.5)
    FromTo {
        from: f64,
        to: f64,
        by: Option<f64>,
    },
    /// seq(from, to, length.out = 10)
    LengthOut {
        from: Option<f64>,
        to: Option<f64>,
        #[name = "length.out"]
        length_out: i64,
    },
    /// seq_len(n) or seq(length.out = n) with no from/to
    Len {
        #[name = "length.out"]
        length_out: i64,
    },
}

impl Builtin for SeqCall {
    fn call(self, _ctx: &BuiltinContext) -> Result<RValue, RError> {
        match self {
            SeqCall::FromTo { from, to, by } => {
                let by = by.unwrap_or(if to >= from { 1.0 } else { -1.0 });
                // ...
            }
            SeqCall::LengthOut { from, to, length_out } => {
                let from = from.unwrap_or(1.0);
                let to = to.unwrap_or(1.0);
                // ...
            }
            SeqCall::Len { length_out } => {
                // seq_len behavior
            }
        }
    }
}
```

**How the derive works for enums**: try each variant in declaration order. A
variant matches if all its required fields (non-`Option`, non-`Arg`) are
present and coercible. First match wins. If no variant matches, generate an
error listing what was expected.

This replaces the current pattern of:
```rust
if let Some(len) = length_out {
    // ...path A...
} else {
    // ...path B...
}
```

with proper exhaustive Rust match arms, which the compiler checks.

### More examples where this helps

**`format()`** — behaves differently for numeric vs character:

```rust
#[derive(FromArgs)]
#[builtin(name = "format")]
enum FormatCall {
    Numeric {
        x: Vector,   // must be numeric
        #[default(0)]
        nsmall: i64,
        #[default(0)]
        width: i64,
        #[name = "big.mark"]
        #[default("".to_string())]
        big_mark: String,
        #[default(false)]
        scientific: bool,  // Arg<bool> if NULL vs missing matters
    },
    Generic {
        x: RValue,
    },
}
```

**`read.table()` / `read.csv()`** — `read.csv` is just `read.table` with
different defaults. With FromArgs, you could share the struct:

```rust
#[derive(FromArgs)]
#[builtin(name = "read.table")]
struct ReadTableArgs {
    file: String,
    #[default(false)]
    header: bool,
    #[default("\t".to_string())]
    sep: String,
}

// read.csv reuses the same struct with different defaults — could be
// a second #[builtin] attribute on the same struct, or a wrapper.
```

## Coercion with context: `CoerceArg` improvements

Current `CoerceArg` gives bad errors: `"'x' must be numeric"`. No function name,
no hint about what was actually passed. Better:

```rust
pub trait CoerceArg: Sized {
    fn coerce(val: &RValue, param: &str, func: &str) -> Result<Self, RError>;
    fn type_name() -> &'static str;  // for error messages
}
```

Error becomes: `Error in grep(): argument 'pattern' must be character, got numeric vector`

The `func` name comes from the `#[builtin(name = "...")]` attribute on the struct.

## CoerceArg impls needed

```rust
// Scalars (already exist)
impl CoerceArg for f64 { ... }
impl CoerceArg for i64 { ... }
impl CoerceArg for bool { ... }
impl CoerceArg for String { ... }
impl CoerceArg for RValue { ... }   // passthrough

// New: usize (very common for sizes/indices, avoids manual TryFrom)
impl CoerceArg for usize { ... }

// New: Vector types
impl CoerceArg for Vector { ... }   // the raw Vector enum

// New: Environment
impl CoerceArg for Environment { ... }

// New: three-way arg state (generated by the derive, not CoerceArg)
// Arg<T> is handled in the derive expansion, not via CoerceArg
```

`Option<T>` is handled by the derive: if the positional/named slot is empty,
field gets `None`. If present, coerce via `T::coerce` and wrap in `Some`.

## Dots

Variadic `...` captures all positional args not consumed by named fields:

```rust
pub struct Dots(pub Vec<RValue>);

// Also useful:
pub struct NamedDots(pub Vec<(Option<String>, RValue)>);  // preserves names
```

In the derive: `Dots` fields are always last (or first, before named-only fields).
The derive consumes named args by name, puts everything else into dots.

## Field attributes

| Attribute | Meaning |
|---|---|
| `#[default(expr)]` | Value when arg is missing |
| `#[name = "na.rm"]` | R-visible parameter name (for named arg matching) |

No other attributes needed. The field type drives behavior:
- `T` → required, error if missing
- `Option<T>` → optional, `None` if missing
- `Arg<T>` → three-way (missing/null/value)
- `Dots` → variadic rest

## What this replaces

| Current pattern | FromArgs equivalent |
|---|---|
| `args[0]` | struct field (positional order = field order) |
| `args.get(1)` | `Option<T>` field |
| `named.iter().find(\|(n, _)\| n == "sep")` | named field `sep: String` |
| `CallArgs::new(args, named).string("file", 0)` | field `file: String` |
| `CallArgs::logical_flag("recursive", 2, false)` | `#[default(false)] recursive: bool` |
| `if let Some(x) = ... { path_a } else { path_b }` | enum variant dispatch |
| checking `is.null` vs missing | `Arg<T>` field |

## What this does NOT replace

- **Pre-eval builtins** — they receive `&[Arg]` (AST nodes), not values
- **Builtins that inspect the call expression** — `match.call()`, `sys.call()`
- **Builtins that evaluate args conditionally** — `tryCatch`, `switch`
- **`c()` and `list()`** — these have deeply special concatenation semantics
  that can't be expressed as arg decoding

## Implementation order

1. Define `Arg<T>` enum in `value/traits.rs`
2. Add `CoerceArg` for `usize`, `Vector`, `Environment`
3. Add `Option<T>` handling in the derive (field type detection)
4. Add `Arg<T>` handling in the derive (three-way: check for missing, check
   for NULL, then coerce)
5. Add `#[name = "..."]` field attribute parsing in the derive
6. Add `Dots` type and derive support (collect remaining positional args)
7. Add enum `FromArgs` derive (try variants in order, first match wins)
8. Add function name to error messages (`CoerceArg::coerce` gets `func` param)
9. Convert one real builtin as proof of concept (`formatC` — 5 params, no ctx)
10. Convert one enum-dispatch builtin (`seq` — multiple paths based on args)
11. Update CLAUDE.md with the new pattern
