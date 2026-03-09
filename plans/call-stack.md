# Call Stack Introspection Plan

> Track call frames so R's `sys.call()`, `sys.frame()`, `parent.frame()`, `missing()`, and `on.exit()` work correctly.

## Current state

The interpreter has **no call stack**. The only frame-like tracking is `s3_dispatch_stack: RefCell<Vec<S3DispatchContext>>` for UseMethod/NextMethod dispatch. Function calls create a child environment but don't record themselves anywhere.

Stubs that need this: `sys.frame()`, `sys.frames()`, `sys.parents()`, `sys.function()`, `sys.on.exit()`, `parent.frame()`, `missing()`, `on.exit()`.

## Design

### CallFrame struct

```rust
pub(crate) struct CallFrame {
    /// The call expression (for sys.call)
    pub call: Expr,
    /// The function being called (for sys.function)
    pub function: RValue,
    /// The call environment (for sys.frame / parent.frame)
    pub env: Environment,
    /// Which arguments were explicitly supplied vs defaulted (for missing())
    pub supplied_args: HashSet<String>,
    /// on.exit expressions — evaluated LIFO when frame is popped
    pub on_exit: Vec<Expr>,
}
```

### Storage

Add to `Interpreter`:

```rust
pub struct Interpreter {
    pub global_env: Environment,
    s3_dispatch_stack: RefCell<Vec<S3DispatchContext>>,
    call_stack: RefCell<Vec<CallFrame>>,  // NEW
}
```

This follows the same pattern as `s3_dispatch_stack` — `RefCell<Vec<>>` for interior mutability through `&self`.

### Push/pop protocol

In `call_function`, around closure evaluation:

```rust
// Push frame BEFORE evaluating body
self.call_stack.borrow_mut().push(CallFrame {
    call: call_expr.clone(),
    function: func.clone(),
    env: call_env.clone(),
    supplied_args,
    on_exit: Vec::new(),
});

let result = self.eval_in(&body, &call_env);

// Pop frame, run on.exit expressions
let frame = self.call_stack.borrow_mut().pop().unwrap();
for expr in frame.on_exit.iter().rev() {
    let _ = self.eval_in(expr, &frame.env);  // errors in on.exit are silently dropped
}

result
```

**Builtin calls don't push frames** — R's `sys.call()` only sees R-level function calls, not .Primitive calls.

### Tracking supplied arguments (for `missing()`)

During parameter binding in `call_function`, track which parameter names received an explicit argument (positional or named) vs which fell through to their default:

```rust
let mut supplied_args = HashSet::new();
for (param, value) in bound_params {
    if was_explicitly_supplied {
        supplied_args.insert(param.name.clone());
    }
}
```

`missing(x)` then checks `!frame.supplied_args.contains("x")` on the top frame.

## Builtins to implement

| Function | Implementation |
|----------|---------------|
| `sys.call(which)` | `call_stack[n].call` — default `which = 0` means current frame |
| `sys.function(which)` | `call_stack[n].function` |
| `sys.frame(which)` | `call_stack[n].env` as `RValue::Environment` |
| `sys.nframe()` | `call_stack.len()` |
| `sys.parents()` | Integer vector of parent frame indices (each frame's caller) |
| `parent.frame(n)` | `call_stack[len - n - 1].env` — the calling frame's environment |
| `missing(x)` | Check `supplied_args` on current frame |
| `on.exit(expr, add)` | Push to current frame's `on_exit` vec; if `add = FALSE` (default), replace |
| `match.arg()` enhancement | Currently works but `missing()` support makes the full R idiom possible |

## Numbering convention

R numbers frames from 0 (global) upward. `sys.call(0)` is the top-level call. Negative indices count from the current frame. Map this to our Vec indices:
- Frame 0 = global (not in the Vec)
- Frame 1 = `call_stack[0]`
- Frame n = `call_stack[n - 1]`
- `sys.call()` with no arg = current frame = `call_stack.last()`

## Implementation order

1. Add `CallFrame` struct and `call_stack` field to `Interpreter`
2. Push/pop frames in `call_function` for closures
3. Track `supplied_args` during parameter binding
4. Implement `sys.call()`, `sys.nframe()`, `sys.frame()` (interpreter builtins — need `with_interpreter`)
5. Implement `missing()` as interpreter builtin
6. Implement `on.exit()` as pre-eval builtin (needs unevaluated expr)
7. Implement `parent.frame()`, `sys.parents()`, `sys.function()`
8. Remove corresponding noop stubs from stubs.rs

## Interaction with S3 dispatch

The S3 dispatch stack is separate and should stay separate. UseMethod() is a special form that *replaces* the current frame's body evaluation with the method body — it doesn't create a new frame. The call_stack frame for the generic should remain on the stack during method dispatch (this is how R works — `sys.call()` inside a method shows the original generic call).

## Priority

High — `missing()` and `on.exit()` are used pervasively in CRAN packages. `sys.call()` is used for error messages. This unblocks a large cluster of TODO items.
