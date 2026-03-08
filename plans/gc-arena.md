# gc-arena integration plan

> `gc-arena` 0.5 — Safe, incremental garbage collection.
> https://github.com/kyren/gc-arena

## What it does

Provides a garbage-collected arena where values can reference each other without
causing leaks. Safe Rust API — no unsafe required by users. Incremental collection
(doesn't pause the whole program).

Key types:
- `Arena<R>` — the GC arena, parameterized by a root type
- `Gc<'gc, T>` — a GC'd pointer (like `Rc` but collected)
- `GcCell<'gc, T>` — mutable GC'd cell (like `RefCell` but collected)
- `Collect` trait — derived on types to teach the GC about references

```rust
#[derive(Collect)]
#[collect(no_drop)]
struct MyValue<'gc> {
    name: String,
    children: Vec<Gc<'gc, MyValue<'gc>>>,
}
```

## Where it fits in newr

### 1. Replace Rc<RefCell<>> for RValue

Currently `RValue` uses `Rc<RefCell<>>` for shared mutable access to vectors,
lists, and environments. This causes **reference cycles** (e.g. an environment
that contains a closure that captures that environment) → memory leaks.

With gc-arena:
- `Gc<'gc, Vector>` instead of `Rc<Vec<...>>`
- `GcCell<'gc, EnvInner>` instead of `Rc<RefCell<EnvInner>>`
- Cycles are collected automatically

### 2. Environment parent chains

R environments form a tree (global → package → base). Closures capture their
defining environment, creating cycles. GC handles this correctly.

### 3. R's copy-on-modify semantics

R uses copy-on-modify: `y <- x` shares the underlying data until either is modified.
This is naturally expressed as `Gc` pointers with copy-on-write via reference counting
inside the GC.

### Challenges

- The `'gc` lifetime infects all types — `RValue<'gc>`, `Environment<'gc>`, etc.
- All interpreter state must live inside the `Arena::mutate()` closure
- Significant refactor of the entire value/environment system

## Relationship to builtins plan

No direct relationship to specific builtins. This is a foundational change to
the value representation that affects everything.

## Recommendation

**Add when memory leaks from Rc cycles become a problem.** This is the right
long-term solution but requires a major refactor. The `'gc` lifetime parameter
propagates through the entire codebase.

**Effort:** Major — 1-2 weeks of refactoring.

**Alternative:** Use `Rc` with weak references (`Weak<RefCell<>>`) for parent
environment pointers to break cycles. Simpler but doesn't handle all cases.
