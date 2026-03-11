# Optimize sort(unique()) and unique(sort()) with BTreeSet

Detect `sort(unique(x))` and `unique(sort(x))` patterns and implement them as a single BTreeSet pass instead of two separate operations.

## Motivation

In R, `sort(unique(x))` is an extremely common idiom. The naive implementation:

1. `unique(x)` — O(n) scan building a HashSet, then collect → Vec
2. `sort(result)` — O(m log m) sort where m = unique count

With a BTreeSet, this is a single O(n log m) pass that produces sorted unique values directly.

## Implementation options

### Option A: Interpreter-level pattern detection

In the evaluator, detect when `sort()` wraps `unique()` (or vice versa) at the AST level:

```rust
// In eval_call, before evaluating arguments:
if func_name == "sort" {
    if let Some(Expr::Call(inner_fn, inner_args)) = args.first() {
        if inner_fn_name == "unique" {
            return eval_sort_unique(inner_args, env);
        }
    }
}
```

This is fragile (doesn't work with variable bindings or piped calls).

### Option B: Builtin-level optimization (preferred)

Add an internal `sort_unique()` helper and have both `sort()` and `unique()` check if their input came from the other:

Actually simpler: just make `sort()` and `unique()` individually faster using BTreeSet when appropriate, and add a dedicated `sort_unique()` builtin:

```rust
#[builtin(name = "sort_unique")]
fn builtin_sort_unique(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    // Single BTreeSet pass
}
```

### Option C: Optimize unique() to return sorted when input is sorted

If `unique()` detects its input is already sorted (check if `is_sorted()`), use a single-pass dedup. If `sort()` detects its input has no duplicates, skip dedup. This gives the optimization without special detection.

Also, add an argument that says `sorted=FALSE` which will return a sorted, dedup result.

## Recommended approach

Option B — add `sort_unique()` as an explicit builtin. Don't try to be clever with AST pattern matching. Users who want the optimization can call it directly:

```r
sort_unique(x)  # newr extension: faster than sort(unique(x))
```

Also optimize individually:

- `unique()`: use a `HashSet` for O(n) dedup (already does this?) But there is a threshold where this is slower than doing the linear scan. We need to find this. 
- `sort()`: if `na.last = NA` (drop NAs), use unstable sort

Option A and Option C should be considered as well.

## Scope

- Add `sort_unique()` builtin using `BTreeSet` for numeric, character, and integer vectors
- Document as a newr extension in the manual
