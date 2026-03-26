# Type stability and attribute preservation

This plan covers review items #7 (type collapse / attribute stripping) and #8
(subsetting semantics gaps). These are the largest remaining correctness
blockers for CRAN package compatibility.

## Problem

The evaluator converts vectors through `to_doubles()` or `to_integers()` in
many places, then rebuilds a fresh `Vector::Double(...)`. This destroys:

- Storage mode (integer → double, character → NA)
- Attributes: `names`, `dim`, `dimnames`, `class`, custom attrs
- Type stability through replacement: `x[1] <- 2L` on an integer vector
  should stay integer, not become double

Affected code paths:

- `assignment.rs` — index replacement always converts to doubles
- `indexing.rs` — matrix subsetting converts to doubles, ignores character type
- `ops.rs` — arithmetic rebuilds plain vectors, drops dim/names/class
- `builtins.rs` — `c()` drops names

## Confirmed failures

```r
x <- 1L; x[1] <- 2L; typeof(x)          # "double" (should be "integer")
m <- matrix(1:4, 2, 2); m[1] <- 9L; dim(m)  # NULL (should be c(2,2))
m <- matrix(1:4, 2, 2); typeof(m[1, ])       # "double" (should be "integer")
m <- matrix(c("a","b","c","d"), 2, 2); m[1,1] # NA (should be "a")
m <- matrix(1:4, 2, 2); dim(m + 1)           # NULL (should be c(2,2))
names(c(a = 1, b = 2))                       # NULL (should be c("a","b"))
x <- 1:4; x[c(TRUE, FALSE)]                  # 1 (should be c(1, 3))
x <- 1:3; x[c(-1, 2)]                        # c(NA, 2) (should error)
```

## Approach

### 1. Type-preserving replacement in assignment.rs

Replace the current `to_doubles()` path with type-aware replacement:

- Match the replacement value's type to the target vector's type
- Integer target + integer replacement → stays integer
- Integer target + double replacement → coerce target to double first
- Character/logical/complex targets → replace in their native type
- Copy attributes from the original object to the result

### 2. Type-preserving indexing in indexing.rs

The matrix indexing path (`eval_matrix_index`) currently:
- Converts everything to doubles via `data.to_doubles()`
- Should instead dispatch on the actual vector type

Add a generic `index_into_vector(v: &Vector, flat_indices: &[usize]) -> Vector`
helper that preserves the vector variant.

### 3. Attribute propagation in ops.rs

After vectorized arithmetic, copy attributes from the longer operand to the
result (R's rule). Key attributes to preserve:

- `dim` / `dimnames` — matrix shape survives arithmetic
- `names` — named vector arithmetic preserves names from the first operand
- `class` — stripped for base types, preserved for S3 classes via group generics

### 4. Logical index recycling in indexing.rs

`index_by_logical` currently takes the mask as-is. It needs to recycle the mask
to the target length before filtering.

### 5. Index validation

Before indexing, validate:
- No mixing of positive and negative indices (error)
- Zero indices are silently dropped
- NA indices produce NA in the result
- Character indices look up `names` / `dimnames`

### 6. `c()` name preservation

The `c()` builtin should:
- Carry forward `names` from input vectors
- Use argument names as element names when inputs are unnamed scalars

## Suggested order

1. Type-preserving `index_into_vector` helper (unblocks #2 and #4)
2. Logical index recycling
3. Index validation (mixed sign, character lookup)
4. Type-preserving replacement
5. Attribute propagation in arithmetic
6. `c()` name preservation
7. Matrix dimname indexing

## Tests needed

- Replacement on every vector type: integer, double, character, logical, complex, raw
- Matrix subsetting preserves type and dim/dimnames
- Arithmetic on matrices preserves dim
- `c(a=1, b=2)` preserves names
- Logical recycling: `x[c(TRUE, FALSE)]` on length-4 vector
- Mixed index error: `x[c(-1, 2)]`
- Character indexing: `m["r1", "c1"]` with dimnames
- Data frame row-name preservation through subsetting
