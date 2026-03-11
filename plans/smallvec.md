# smallvec integration plan

> `smallvec` 1.15 — Stack-allocated vector for small sizes, heap-allocated for larger.
> Already vendored as a transitive dependency of reedline.

## What it does

`SmallVec<[T; N]>` stores up to N elements inline (on the stack). If the vector grows beyond N, it spills to the heap like a normal `Vec`. This avoids heap allocation for common small cases.

## Where it fits in miniR

### R is full of small vectors

Most R operations produce small results:

- Function arguments: typically 1-5 args
- Named argument pairs: typically 0-3
- Scalar operations: vectors of length 1
- Short sequences: `1:10`, `c(1, 2, 3)`
- Attribute lists: typically 1-3 attrs (names, class, dim)

Currently every vector allocation goes to the heap. SmallVec could avoid allocation for the common case.

### Candidate replacements

| Current type | SmallVec replacement | Rationale |
| ------------ | -------------------- | --------- |
| `Vec<RValue>` (positional args) | `SmallVec<[RValue; 4]>` | Most R functions take 1-3 args |
| `Vec<(String, RValue)>` (named args) | `SmallVec<[(String, RValue); 2]>` | 0-2 named args is common |
| `Vec<Option<f64>>` (Double vector) | `SmallVec<[Option<f64>; 1]>` | Scalar doubles extremely common |
| `Vec<(Option<String>, RValue)>` (list values) | `SmallVec<[...; 4]>` | Small lists (data frame columns) |
| `HashMap<String, RValue>` (attrs) | `SmallVec<[(String, RValue); 2]>` | Most objects have 0-2 attrs; linear scan is faster for N<5 |

### The attrs case

Attributes (`names`, `class`, `dim`) are stored as `Option<Box<HashMap<String, RValue>>>`. Most objects have 0-2 attributes. A `SmallVec` with linear search would be faster than a HashMap for this:

```rust
pub type Attributes = SmallVec<[(String, RValue); 3]>;

impl RVector {
    pub fn get_attr(&self, name: &str) -> Option<&RValue> {
        self.attrs.as_ref()?.iter().find(|(k, _)| k == name).map(|(_, v)| v)
    }

    pub fn set_attr(&mut self, name: &str, value: RValue) {
        let attrs = self.attrs.get_or_insert_with(Default::default);
        if let Some(entry) = attrs.iter_mut().find(|(k, _)| k == name) {
            entry.1 = value;
        } else {
            attrs.push((name.to_string(), value));
        }
    }
}
```

### Tradeoffs

- **Pro**: Fewer heap allocations for common cases
- **Pro**: Better cache locality for small collections
- **Con**: Larger stack size per SmallVec (N * size_of::<T>() even if empty)
- **Con**: More complex types in function signatures

## Implementation order

1. Replace `Attributes` HashMap with `SmallVec<[(String, RValue); 3]>`
2. Benchmark with real R scripts
3. If measurable win, apply to argument passing
4. Consider for scalar vector optimization

## Priority

Low — micro-optimization. Profile first. The attrs replacement is the most impactful change since every attributed vector allocation benefits.
