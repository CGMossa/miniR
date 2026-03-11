# Arrow Backend for Vector Types

Replace the current `Vec<Option<T>>` vector backend with Apache Arrow arrays for better performance, memory efficiency, and interoperability.

## Current backend

Each vector type is a newtype around `Vec<Option<T>>`:

```rust
pub struct Integer(pub Vec<Option<i64>>);
pub struct Double(pub Vec<Option<f64>>);
pub struct Character(pub Vec<Option<String>>);
pub struct Logical(pub Vec<Option<bool>>);
pub struct ComplexVec(pub Vec<Option<Complex64>>);
```

This has several problems:

- **Memory overhead**: `Option<f64>` is 16 bytes per element (f64 + 8-byte discriminant with alignment). Arrow uses a separate validity bitmap — 1 bit per element for NA tracking.
- **Cache inefficiency**: Values and NA flags are interleaved. Arrow stores them separately for better cache behavior on scans.
- **No zero-copy interop**: Can't share data with Python (pandas/polars), R (via ALTREP), or other Arrow-aware tools without copying.
- **No SIMD**: Arrow arrays are designed for vectorized operations. `Vec<Option<T>>` prevents auto-vectorization due to branch-per-element NA checks.

## Arrow representation

```rust
// Arrow equivalent of Vec<Option<f64>>
struct Float64Array {
    values: Buffer<f64>,      // contiguous f64 values (NA slots have arbitrary values)
    validity: Option<Bitmap>, // 1 bit per element: 1 = valid, 0 = NA
    len: usize,
}
```

Memory for 1M doubles:
- Current: 16 MB (16 bytes × 1M)
- Arrow: ~8.1 MB (8 bytes × 1M + 125KB bitmap)

## Which Arrow crate

Options:
1. **`arrow-rs`** (apache/arrow-rs) — the official Apache Arrow Rust implementation. Large dependency tree (arrow-buffer, arrow-data, arrow-schema, arrow-array, etc.). Very mature.
2. **`arrow2`** — lighter alternative, now mostly merged into arrow-rs. Consider deprecated.
3. **Roll our own** — just the bitmap + buffer parts. No schema, no IPC, no compute kernels. Minimal dependencies.

### Recommendation: Roll our own initially, migrate to arrow-rs later

For miniR's immediate needs, we only need:
- Validity bitmap (compact NA tracking)
- Contiguous value buffer
- Basic operations (index, iterate, filter, map)

This can be ~200 lines of code without any new dependencies. Once we need IPC, Parquet, or cross-language interop, we can adopt `arrow-rs` and the migration will be straightforward since the memory layout is identical.

## Implementation plan

1. **Create `src/interpreter/value/buffer.rs`** — Arena-allocated value buffer with validity bitmap
   ```rust
   pub struct NullableBuffer<T> {
       values: Vec<T>,        // dense values (NA slots are Default::default())
       validity: BitVec,      // 1 = valid, 0 = NA; None if no NAs
       len: usize,
   }
   ```

2. **Implement core traits** — Index, Iterator (yields `Option<&T>`), FromIterator, Clone, PartialEq

3. **Replace newtypes one at a time**:
   - `Double(NullableBuffer<f64>)` first (most performance-critical)
   - `Integer(NullableBuffer<i64>)` next
   - `Logical(NullableBuffer<bool>)` — needs packed bool support
   - `Character` — trickier, needs string offset buffer (Arrow `StringArray` layout)
   - `ComplexVec` — `NullableBuffer<Complex64>`

4. **Benchmark** — Compare vectorized operations (sum, mean, +, *, filter) before and after

5. **Maintain `From<Vec<Option<T>>>` and `Into<Vec<Option<T>>>`** — for compatibility during transition

## Benchmarking plan

Operations to benchmark:
- `sum(x)` for 1M-element double vector
- `x + y` element-wise addition
- `x[x > 0]` logical subsetting
- `is.na(x)` NA detection
- Memory usage for 10M-element vectors

Compare `Vec<Option<f64>>` vs `NullableBuffer<f64>` vs `arrow::Float64Array`.

## Dependencies

- `bitvec` crate for the validity bitmap (already used by some vendored crates?) — or implement manually as `Vec<u64>` with bit manipulation
- No external dependencies required for the initial implementation
