# R Condition System Plan

> Implement R's condition handling: `stop()`, `warning()`, `message()`, condition objects, `withCallingHandlers()`, `tryCatch()` restructuring.

## Current state

- `tryCatch()` exists in pre_eval.rs — catches `RError` variants, calls handler with error message as character string
- `stop()`, `warning()`, `message()` exist as basic builtins
- `withCallingHandlers()` is a noop stub
- `simpleError()`, `simpleWarning()`, `simpleMessage()`, `conditionMessage()`, `conditionCall()` are all noop stubs
- No condition objects — errors are plain `RError` enum variants with string messages

## R's condition model

In R, conditions are S3 objects (lists with a class attribute):

```r
simpleError("bad input", call = quote(f(x)))
# => list(message = "bad input", call = quote(f(x)))
#    with class c("simpleError", "error", "condition")
```

Two handling mechanisms:
1. **tryCatch** — unwinds the stack to the handler, like Rust's `?` / catch. The handler runs in the *tryCatch frame*, not the signaling frame.
2. **withCallingHandlers** — runs the handler *in the signaling frame* without unwinding. Handler can invoke `invokeRestart()` or return to resume.

Key difference: tryCatch = catch-and-handle (stack unwound). withCallingHandlers = intercept-and-maybe-resume (stack intact).

## Design

### Condition as RValue

Conditions are just lists with class attributes. We already have `RList` and attributes. A condition is:

```rust
// No new types needed — just construct an RList with attributes
fn make_condition(message: &str, call: Option<Expr>, classes: &[&str]) -> RValue {
    let mut entries = vec![
        ("message".to_string(), RValue::character(message)),
    ];
    if let Some(call_expr) = call {
        entries.push(("call".to_string(), RValue::Language(Language::new(call_expr))));
    }
    let list = RValue::List(RList { values: entries, attrs: None });
    // Set class attribute to classes (e.g., ["simpleError", "error", "condition"])
    list.with_class(classes)
}
```

### Signal propagation via RError

Extend `RError` to carry condition objects:

```rust
pub enum RError {
    // ... existing variants ...
    /// R condition signal (stop/warning/message)
    Condition {
        condition: RValue,  // the condition list object
        kind: ConditionKind,
    },
}

pub enum ConditionKind {
    Error,    // from stop()
    Warning,  // from warning()
    Message,  // from message()
}
```

When `stop("bad")` is called, it creates a `simpleError` condition and returns `Err(RError::Condition { ... })`. This propagates up the call stack via `?` until caught by tryCatch or withCallingHandlers.

### Handler stack

For `withCallingHandlers`, we need a stack of active handlers that are checked *before* unwinding:

```rust
pub(crate) struct ConditionHandler {
    /// Which condition class this handler catches (e.g., "error", "warning", "message")
    pub class: String,
    /// The handler function
    pub handler: RValue,
    /// The environment where the handler was established
    pub env: Environment,
}

// In Interpreter:
pub struct Interpreter {
    // ...
    condition_handlers: RefCell<Vec<Vec<ConditionHandler>>>,  // stack of handler sets
}
```

Each `withCallingHandlers()` call pushes a `Vec<ConditionHandler>` (one per handler argument). When a condition is signaled, walk the handler stack top-down looking for a matching class. If found, call the handler *without* unwinding. If the handler returns normally, continue signaling (look for more handlers). If no handler catches it, fall through to tryCatch or default behavior.

### Signal flow

```
stop("bad input")
  │
  ├─ Create simpleError condition object
  ├─ Walk condition_handlers stack (withCallingHandlers)
  │   ├─ Found matching handler? Call it in-place
  │   │   └─ Handler returns → continue walking
  │   └─ No more handlers → fall through
  ├─ Return Err(RError::Condition { kind: Error, ... })
  │   └─ Propagates up via ? through eval_in, call_function, etc.
  └─ tryCatch catches it → calls error handler with condition
```

For warnings, the flow is different — they don't unwind by default:

```
warning("careful")
  │
  ├─ Create simpleWarning condition object
  ├─ Walk condition_handlers stack
  │   ├─ Found handler? Call it, check if it "muffles" the warning
  │   └─ No handler or not muffled → print warning, continue execution
  └─ Return Ok(original_result)  // warnings don't abort
```

### Restructure tryCatch

Current tryCatch catches `RError` by variant. Restructure to match on condition class:

```rust
// In pre_eval_try_catch:
match result {
    Err(RError::Condition { condition, kind }) => {
        // Find matching handler by condition class
        let class = get_class(&condition);
        for (handler_class, handler_fn) in &handlers {
            if class.iter().any(|c| c == handler_class) {
                return call_function(handler_fn, &[condition], &[], env);
            }
        }
        // No matching handler — re-signal
        Err(RError::Condition { condition, kind })
    }
    other => other,
}
```

## Builtins to implement

| Function | Type | Implementation |
|----------|------|---------------|
| `stop(message, call.)` | builtin | Create simpleError, signal via RError::Condition |
| `warning(message, call.)` | builtin | Create simpleWarning, walk handlers, print if not muffled |
| `message(...)` | builtin | Create simpleMessage, walk handlers, print to stderr |
| `simpleError(msg, call)` | builtin | Construct condition list with class `c("simpleError", "error", "condition")` |
| `simpleWarning(msg, call)` | builtin | Construct condition list with class `c("simpleWarning", "warning", "condition")` |
| `simpleMessage(msg, call)` | builtin | Construct condition list with class `c("simpleMessage", "message", "condition")` |
| `conditionMessage(c)` | builtin | Extract `c$message` |
| `conditionCall(c)` | builtin | Extract `c$call` |
| `withCallingHandlers(expr, ...)` | pre_eval | Push handlers, eval expr, pop handlers |
| `suppressWarnings(expr)` | pre_eval | withCallingHandlers with warning muffler |
| `suppressMessages(expr)` | pre_eval | withCallingHandlers with message muffler |
| `tryCatch(expr, ...)` | pre_eval | Restructure to use condition objects |

## Implementation order

1. Add `ConditionKind` enum and `RError::Condition` variant
2. Implement `simpleError()`, `simpleWarning()`, `simpleMessage()` constructors
3. Restructure `stop()` to create condition objects and signal via `RError::Condition`
4. Restructure `tryCatch()` to match on condition class
5. Add `condition_handlers` stack to `Interpreter`
6. Implement `withCallingHandlers()` as pre_eval builtin
7. Restructure `warning()` to use condition signaling (non-unwinding)
8. Implement `suppressWarnings()`, `suppressMessages()`
9. Implement `conditionMessage()`, `conditionCall()`
10. Remove noop stubs

## Dependency

- Benefits from call-stack.md (for `conditionCall` to show the calling expression)
- But can be implemented independently — `conditionCall` can return NULL until call stack exists

## Priority

High — `stop()` with proper condition objects and `tryCatch` with class matching is used by virtually every CRAN package. `withCallingHandlers` is used by tidyverse extensively (rlang's `abort()` relies on it).
