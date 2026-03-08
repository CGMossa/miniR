# num-complex integration plan

> `num-complex` 0.4 — Complex number type.
> https://github.com/rust-num/num-complex

## What it does

`Complex<T>` type with real and imaginary parts. Full arithmetic, trig functions,
`norm()`, `arg()`, `conj()`, formatting. Implements `Add`, `Mul`, etc. with other
numeric types.

```rust
use num_complex::Complex;
let z = Complex::new(3.0_f64, 4.0);
println!("{}", z.norm());  // 5.0
println!("{}", z.arg());   // 0.9272...
```

## Where it fits in newr

### 1. R's complex type

R has a native complex type:

```r
z <- 3+4i
Mod(z)   # 5
Arg(z)   # 0.9272...
Conj(z)  # 3-4i
Re(z)    # 3
Im(z)    # 4
```

Currently the parser recognizes complex literals but the interpreter stubs them.
`num-complex` provides the backing type.

### 2. Complex vector type

Add `Vector::Complex(Vec<Option<Complex<f64>>>)` to the vector enum:

```rust
enum Vector {
    // ...existing variants...
    Complex(ComplexVec),
}
```

### 3. Complex math

All math functions should work on complex inputs:
- `sqrt(-1+0i)` → `0+1i`
- `exp(z)`, `log(z)`, `sin(z)`, `cos(z)` — all defined for complex
- `polyroot()` — polynomial root finding returns complex

### 4. Coercion chain

R's coercion: logical → integer → double → complex. Adding complex completes
the numeric tower.

## Relationship to builtins plan

| Phase | Builtins affected | Impact |
|---|---|---|
| Phase 1 (math) | `Re()`, `Im()`, `Mod()`, `Arg()`, `Conj()` | complex accessors |
| Phase 1 (math) | All math functions on complex inputs | complex math |
| Phase 3 (collections) | `as.complex()`, `is.complex()` | type conversion |
| Phase 8 (linalg) | `eigen()` — returns complex eigenvalues | complex results |

## Recommendation

**Add when implementing the complex vector type.** R's complex type is relatively
niche (used mainly in signal processing and eigendecomposition) but completing the
type system is important for correctness.

**Effort:** 2-3 hours for Complex vector type + basic operations.
