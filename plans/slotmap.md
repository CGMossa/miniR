# slotmap integration plan

> `slotmap` 1.1.1 -- Slot-based arena allocator with stable keys.
> <https://github.com/orlp/slotmap>

**Status:** Vendored and available behind `arena` feature gate (`dep:slotmap`).

## What it does

Arena allocator where each insertion returns a `Key` that is stable across
insertions/removals. O(1) insert, remove, and lookup. Keys are versioned to
detect use-after-free.

```rust
let mut sm = SlotMap::new();
let k1 = sm.insert("hello");
let k2 = sm.insert("world");
assert_eq!(sm[k1], "hello");
sm.remove(k2);
// sm[k2] would panic — key is invalidated
```

Variants: `SlotMap`, `HopSlotMap` (stable iteration order), `DenseSlotMap`
(fast iteration, slower insert/remove).

## Where it fits in miniR

### 1. Interned strings (recommended first target)

R has many repeated strings (column names, function names, attribute names).
A `SlotMap<DefaultKey, String>` can intern these -- store each string once, pass
around lightweight keys. This is the lowest-risk integration point because string
interning can be introduced incrementally without changing the `Environment` or
`RValue` type signatures.

### 2. Value arena

Instead of `Rc<RefCell<Vec<...>>>` for shared vectors, store all vectors in a
`SlotMap<VectorKey, Vector>` and reference them by key. This enables:

- Copy-on-write by checking if key has refcount > 1
- Simpler serialization (keys are just integers)
- Better cache locality (values stored contiguously)

### 3. Environment arena (NOT feasible as a feature gate)

Environments could in theory be stored in a `SlotMap<EnvKey, EnvInner>` instead
of `Rc<RefCell<EnvInner>>`. Parent references would become `EnvKey` instead of
`Rc`, avoiding reference cycles entirely.

**However, this is too invasive to implement as a feature-gated alternative.**
See the feasibility analysis below.

## Environment arena feasibility analysis

An audit of the codebase (March 2026) found `Environment` used in **176
occurrences across 20 files**. The type is deeply embedded in:

1. **`RValue::Environment(Environment)`** -- a variant of the core value enum,
   returned by `environment()`, `new.env()`, `.GlobalEnv`, etc.

2. **`RFunction::Closure { env: Environment }`** -- every user-defined function
   closure captures its defining environment.

3. **Every `eval_*` function** takes `&Environment` -- `eval_in`, `eval_call`,
   `eval_assign`, `eval_index`, `eval_if`, `eval_for`, etc.

4. **`BuiltinContext { env: &Environment }`** -- passed to all 200+ builtins.

5. **`CallFrame { env: Environment }`** -- stored on the call stack for each
   active function call.

6. **`ConditionHandler { env: Environment }`** -- stored by
   `withCallingHandlers()`.

### Why a feature gate does not work

A feature-gated alternative would require two different `Environment` types.
Since `Environment` appears in the signatures of `RValue`, `RFunction`,
`BuiltinContext`, `CallFrame`, and every eval function, the entire interpreter
would need to be duplicated or parameterized by an `Env` trait. This is not
feasible without a ground-up redesign.

### What it would take (incremental migration)

If environment-arena is pursued in the future, the path is:

- Add an `EnvArena` (a `SlotMap<EnvKey, EnvInner>`) to the `Interpreter` struct
- Change `Environment` to wrap `EnvKey` instead of `Rc<RefCell<EnvInner>>`
- Every method on `Environment` that borrows `EnvInner` (`get`, `set`,
  `get_function`, etc.) would need `&EnvArena` as an additional parameter
- This means threading the arena through all eval functions and builtins
- The `BuiltinContext` already carries `&Interpreter`, so builtins could access
  the arena through it -- but the method signatures on `Environment` would
  change from `env.get(name)` to `env.get(name, &arena)` everywhere
- Estimated effort: 3-5 sessions, touching every file in `src/interpreter/`

### Benefits if completed

- Keys are `Copy` (8 bytes) vs `Rc` clone (atomic refcount bump)
- No reference cycles possible -- parent pointers are just keys
- All environments contiguous in memory -- better cache locality
- Environment deallocation via `arena.remove(key)` instead of relying on
  Rc drop cascades
- Natural fit with a future GC: arena keys are roots to trace

### Current `Rc<RefCell<>>` is adequate

The current approach works correctly for single-threaded interpretation.
`Rc<RefCell<>>` clone is cheap (pointer-sized + atomic increment). R programs
rarely create enough environments to make arena allocation a performance win.
The main benefit would be architectural (cycle-freedom, Copy keys), not
performance.

## Alternative to gc-arena

SlotMap provides a simpler alternative to full GC:

- No `'gc` lifetime infecting all types
- Manual management via keys (simpler to reason about)
- But: no automatic cycle collection -- must manually track and free

## Relationship to builtins plan

No direct relationship. Infrastructure for value representation.

## Recommendation

1. **String interning** -- lowest risk, highest immediate value. Can be
   introduced behind the `arena` feature gate without changing public types.
2. **Value arena** -- medium risk. Requires changing how vectors are stored
   but `RValue` can wrap a key internally.
3. **Environment arena** -- high risk, high reward. Do NOT attempt as a
   feature-gated alternative. If pursued, commit to a full migration as a
   breaking change on a dedicated branch.

**Effort:** String interning: 1 session. Value arena: 2-3 sessions.
Environment arena: 3-5 sessions (full codebase migration).
