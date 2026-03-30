# Arrow Backend for Vector Types

Migrate from hand-rolled `NullableBuffer<T>` to real `arrow-rs` arrays.

## Status

- Double/Integer: currently use hand-rolled `NullableBuffer<T>` (bitmap + Vec<T>)
- Character: still `Vec<Option<String>>`
- Logical: still `Vec<Option<bool>>`
- ComplexVec: still `Vec<Option<Complex64>>`
- `arrow` crate v58 is already vendored (via parquet feature)

## Migration target

| Current | Arrow replacement |
|---|---|
| `Double(NullableBuffer<f64>)` | `Double(Float64Array)` |
| `Integer(NullableBuffer<i64>)` | `Integer(Int64Array)` |
| `Character(Vec<Option<String>>)` | `Character(StringArray)` |
| `Logical(Vec<Option<bool>>)` | `Logical(BooleanArray)` |
| `ComplexVec(Vec<Option<Complex64>>)` | Keep as-is (no arrow complex type) |

## Key API changes

Arrow arrays are immutable. Mutation pattern changes:

```rust
// Before (mutable)
let mut buf: NullableBuffer<f64> = vec![Some(1.0), None].into();
buf.push(Some(2.0));
buf.set(0, Some(42.0));

// After (builder → freeze)
let mut builder = Float64Builder::with_capacity(3);
builder.append_value(1.0);
builder.append_null();
builder.append_value(2.0);
let arr: Float64Array = builder.finish();
```

For read paths (vast majority), the API is similar:
- `get_opt(i)` → `arr.value(i)` / `arr.is_null(i)`
- `iter_opt()` → `arr.iter()`
- `values_slice()` → `arr.values()`
- `len()` → `arr.len()`

## Mutation points (need builders)

Only ~10 places in the codebase mutate vectors:
- `assignment.rs`: `.set()` for replacement (`x[i] <- v`)
- `ops.rs`: `.push()` building results element-by-element
- `indexing.rs`: `.push()` building subsets
- `graphics/color.rs`: `.push()` building RGBA vectors

These all build up a result incrementally → natural builder pattern.

## Implementation order

1. Make `arrow` non-optional (move from parquet feature to default)
2. Replace `Double(NullableBuffer<f64>)` → `Double(Float64Array)`
3. Replace `Integer(NullableBuffer<i64>)` → `Integer(Int64Array)`
4. Replace `Logical(Vec<Option<bool>>)` → `Logical(BooleanArray)`
5. Replace `Character(Vec<Option<String>>)` → `Character(StringArray)`
6. Remove `NullableBuffer<T>` and `BitVec` from buffer.rs
7. Update native/convert.rs for zero-copy SEXP ↔ Arrow

## Feature gating

Arrow should be a default feature (like `compression`). The `minimal` build
profile can exclude it for WASM targets. Add feature: `arrow-vectors`.
