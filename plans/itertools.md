# itertools integration plan

> `itertools` 0.14 — Extra iterator adaptors, functions, and macros.
> <https://github.com/rust-itertools/itertools>

## What it does

Extends Rust's `Iterator` trait with ~80 additional methods:

- `chunks()`, `tuples()` — group elements
- `cartesian_product()` — cross product of iterators
- `sorted()`, `unique()`, `dedup()` — ordering/uniqueness
- `join()` — collect into string with separator
- `interleave()`, `merge()` — combine iterators
- `fold_ok()`, `process_results()` — error handling in iterators
- `iproduct!()`, `izip!()` — macros for combining iterators
- `Itertools::counts()` — frequency counting

## Where it fits in newr

### 1. `paste()` / `paste0()` — `join()`

```rust
// Current: vals.join(sep)  — already in std for &[&str]
// itertools::join() works on any iterator, avoiding collect:
use itertools::Itertools;
values.iter().map(format_value).join(", ")
```

### 2. `expand.grid()` — `cartesian_product()`

R's `expand.grid(a, b, c)` computes the Cartesian product:

```rust
use itertools::Itertools;
let grid = a.iter().cartesian_product(b.iter()).cartesian_product(c.iter());
```

### 3. `table()` — `counts()`

R's `table(x)` counts occurrences of each value:

```rust
use itertools::Itertools;
let freq = values.iter().counts(); // HashMap<&V, usize>
```

### 4. `unique()` — `unique()`

```rust
values.iter().unique().collect()  // preserves first occurrence order
```

### 5. `sort()` + `duplicated()` — `sorted()` + `dedup()`

Sorting and deduplication for vector operations.

### 6. Internal use — cleaner iterator chains

Many builtin implementations iterate over vectors with complex transformations.
itertools makes these chains more readable and sometimes faster.

## Relationship to builtins plan

| Phase | Builtins affected | Impact |
|---|---|---|
| Phase 2 (strings) | `paste()`, `paste0()` | cleaner join |
| Phase 3 (collections) | `expand.grid()`, `table()`, `unique()`, `duplicated()` | direct implementations |
| Phase 13 (iter) | `Filter()`, `Map()`, `Reduce()` | iterator adaptors |

## Recommendation

**Add when implementing Phase 3 collection builtins.** itertools is a zero-cost
abstraction that makes iterator code more readable. The `expand.grid()` and
`table()` implementations are significantly cleaner with it.

**Effort:** 5 minutes to add, used incrementally.
