# slotmap integration plan

> `slotmap` 1.1 — Slot-based arena allocator with stable keys.
> <https://github.com/orlp/slotmap>

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

## Where it fits in newr

### 1. Interned strings

R has many repeated strings (column names, function names, attribute names).
A `SlotMap<DefaultKey, String>` can intern these — store each string once, pass
around lightweight keys.

### 2. Value arena

Instead of `Rc<RefCell<Vec<...>>>` for shared vectors, store all vectors in a
`SlotMap<VectorKey, Vector>` and reference them by key. This enables:

- Copy-on-write by checking if key has refcount > 1
- Simpler serialization (keys are just integers)
- Better cache locality (values stored contiguously)

### 3. Environment arena

Environments could be stored in a `SlotMap<EnvKey, EnvInner>` instead of
`Rc<RefCell<EnvInner>>`. Parent references become `EnvKey` instead of `Rc`.
This avoids reference cycles entirely (no need for `gc-arena`).

### Alternative to gc-arena

SlotMap provides a simpler alternative to full GC:

- No `'gc` lifetime infecting all types
- Manual management via keys (simpler to reason about)
- But: no automatic cycle collection — must manually track and free

## Relationship to builtins plan

No direct relationship. Infrastructure for value representation.

## Recommendation

**Consider when refactoring value representation.** Simpler than `gc-arena` but
requires manual lifetime management of keys. Good intermediate step between
current `Rc<RefCell<>>` and full GC.

**Effort:** Medium — 1-2 sessions to prototype arena-based values.
