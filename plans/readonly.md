# readonly integration plan

> `readonly` 0.2.13 — Struct fields readable outside module, writable only inside.
> <https://github.com/dtolnay/readonly>

## What it does

`#[readonly::make]` on a struct makes `pub` fields readable from outside the
defining module but not writable. Inside the module, full read+write access.

```rust
mod m {
    #[readonly::make]
    pub struct S {
        pub n: i32,       // read-only outside mod m
    }
}

fn demo(s: &mut m::S) {
    println!("{}", s.n);  // OK — read
    // s.n += 1;          // ERROR — write
}
```

## Where it fits in newr

### 1. `Environment` — prevent external mutation of internals

Currently `EnvInner` is private, but `Environment` exposes `set()` publicly.
With `readonly`, we could make environment metadata readable but not
directly mutable from outside the environment module:

```rust
// src/interpreter/environment.rs
#[readonly::make]
pub struct EnvironmentInfo {
    pub name: Option<String>,
    pub parent: Option<Environment>,
    pub length: usize,
}
```

Callers can inspect `env.name` but must go through methods to modify.

**Verdict: Low value** — we already achieve this with private fields + public
getters. The `EnvInner` behind `Rc<RefCell<>>` is already private.

### 2. `RList` — protect attrs from direct mutation

```rust
#[readonly::make]
pub struct RList {
    pub values: Vec<(Option<String>, RValue)>,
    pub attrs: Option<Box<Attributes>>,
}
```

External code could read `list.values` and `list.attrs` but would need to use
`set_attr()` / `get_attr()` methods. This prevents accidental direct mutation
of the attribute map without going through the proper API.

**Verdict: Medium value** — `attrs` should really only be modified via methods
that maintain invariants (e.g. class attribute validation in the future).

### 3. Vector newtypes — prevent inner Vec mutation

```rust
// src/interpreter/value/double.rs
#[readonly::make]
#[derive(Debug, Clone, PartialEq, Deref, From, Into)]
pub struct Double {
    pub inner: Vec<Option<f64>>,
}
```

External code can read/iterate via `Deref` but can't replace the inner Vec
directly — must construct a new `Double` via `From`/`Into`.

**Verdict: Low value** — `Deref` already provides read access, and `.0` access
is rarely used outside the value module.

### 4. Interpreter state — protect internal counters

```rust
#[readonly::make]
pub struct Interpreter {
    pub global_env: Environment,
    pub call_depth: usize,        // readable for debugging, not settable
    pub max_call_depth: usize,
}
```

**Verdict: Medium value** — prevents builtins from accidentally modifying
interpreter state they shouldn't touch.

## Relationship to builtins plan

No direct relationship to R builtins. This is a code quality / API safety tool.
It helps enforce the principle that internal state should only be modified through
designated methods, which becomes important as more builtins get access to
the interpreter.

## Recommendation

**Add when:** we refactor the Interpreter struct or add more public fields to
RList/Environment that should be read-only externally. Not urgent — the current
private-field pattern works fine, but `readonly` is cleaner when you want
public read + private write without writing boilerplate getters.

**Effort:** Trivial — add dependency, annotate 2-3 structs, remove manual getters.
