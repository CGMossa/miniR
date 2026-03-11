# exn integration plan

> `exn` 0.3 — Context-aware concrete Error type with tree-structured frames.
> <https://github.com/fast/exn>
> Zero runtime dependencies, no_std compatible.

## What it does

`Exn<E>` wraps a typed error `E` with a tree of frames, each capturing source location automatically. Unlike anyhow (opaque type-erased error) or thiserror (just derive Display/Error), exn gives you:

- **Typed errors** — `Exn<E>` derefs to `E`, pattern matching works
- **Tree-structured context** — child errors nest hierarchically, not just a linear chain
- **Automatic location tracking** — every `.raise()` captures `file:line:col`
- **Zero deps** — no runtime dependencies at all

### Key API

```rust
use exn::{Exn, ErrorExt, ResultExt, bail, ensure};

// Convert any Error into Exn
let e: Exn<MyError> = my_error.raise();

// Wrap lower error as child of higher error
let result = low_level_op()
    .or_raise(|| HighLevelError("context".into()))?;

// bail! and ensure! macros
bail!(MyError("something went wrong".into()));
ensure!(x > 0, MyError("x must be positive".into()));
```

## Where it fits in miniR (preferred over thiserror)

### Current RError

```rust
pub enum RError {
    Name(String),
    Type(String),
    Argument(String),
    Index(String),
    Parse(String),
    Other(String),
    Return(RValue),
}
```

### With exn

Keep `RError` as a plain enum (no derive macros needed), but wrap it in `Exn<RError>` at error sites to get location tracking and context:

```rust
use exn::{Exn, bail, ensure, ResultExt};

// Type alias for our result type
pub type RResult<T> = Result<T, Exn<RError>>;

// In builtins — bail! captures location automatically
fn builtin_sqrt(args: &[RValue], _: &[(String, RValue)]) -> RResult<RValue> {
    ensure!(!args.is_empty(), RError::Argument("argument 'x' is missing".into()));
    let x = args[0].as_double()
        .or_raise(|| RError::Type("non-numeric argument to mathematical function".into()))?;
    Ok(RValue::vec(Vector::Double(vec![Some(x.sqrt())].into())))
}

// Context wrapping — low-level error becomes child of high-level error
fn eval_call(name: &str, args: &[RValue], env: &Environment) -> RResult<RValue> {
    let func = env.get_function(name)
        .ok_or(RError::Name(name.to_string()))
        .map_err(|e| e.raise())
        .or_raise(|| RError::Other(format!("error in {}()", name)))?;
    // ...
}
```

### Error display with tree structure

When an error propagates through multiple call levels, exn shows the full context tree:

```text
Error in aggregate(): failed to compute summary
  └─ Error in mean(): non-numeric argument to mathematical function
       at src/interpreter/builtins/math.rs:42:5
     └─ Type error: expected numeric, got character
          at src/interpreter/value.rs:156:9
```

This is exactly what our CLAUDE.md design goal asks for: "say why it went wrong and what to do about it."

### Advantages over thiserror for our use case

| | thiserror | exn |
|---|---|---|
| Error definition | Need `#[derive(Error)]` on RError | Plain enum, no macros |
| Location tracking | None — must add manually | Automatic on every `.raise()` |
| Context nesting | Manual `#[source]` fields | Automatic tree via `.or_raise()` |
| Dependencies | proc-macro dep (syn, quote, proc-macro2) | Zero deps |
| Pattern matching | Works | Works (Exn derefs to inner error) |

### Migration path

1. Add `exn = "0.3"` to Cargo.toml
2. Create `pub type RResult<T> = Result<T, Exn<RError>>` alias
3. Migrate builtins one module at a time — change return type from `Result<RValue, RError>` to `RResult<RValue>`
4. Use `.raise()` at error creation sites
5. Use `.or_raise()` for context wrapping
6. Update error display to print the frame tree

### Compatibility

`Exn<RError>` implements `Deref<Target=RError>`, so existing `match` arms on `RError` variants still work. Migration can be incremental.

## Implementation order

1. Add `exn` dependency, vendor it
2. Define `RResult<T>` type alias
3. Add `Exn` wrapping in the interpreter eval loop (top-level context)
4. Migrate builtin return types module by module
5. Update error formatting to display the frame tree with locations
6. Integrate with nu-ansi-term for colored error tree output
7. Add "did you mean" suggestions as context frames

## Priority

High — directly supports our design goal of better error messages. Zero dependencies, trivial to vendor. Preferred over thiserror/anyhow.
