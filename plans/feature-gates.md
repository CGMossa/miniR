# Feature Gate Plan

Every non-core dependency should be optional and feature-gated, included in
`default` but opt-out-able. This enables minimal builds for WASM, embedded,
or sandboxed environments.

## Current State

### Already optional (correct):
- `rand` + `rand_distr` → `random` feature
- `jiff` → `datetime` feature
- `csv` → `io` feature
- `sha2` → `digest` feature
- `serde_json` → `json` feature
- `dirs` → `dirs-support` feature
- `walkdir` → `walkdir-support` feature
- `globset` → `globset-support` feature
- `rayon` → `parallel` feature (NOT in default, opt-in only)
- `collections` → `collections` feature (no dep, just gates the module)

### Always-on but should be optional:
| Dependency | What it provides | Proposed feature |
|---|---|---|
| `ndarray` | Matrix ops, %*%, solve, qr, svd, eigen, det, chol, lm | `linalg` |
| `num-complex` | Complex number support (Vector::Complex) | `complex` |
| `tabled` | View(), kable(), rich table formatting | `tables` |
| `tabwriter` | print.data.frame column alignment | `tables` (same feature) |
| `regex` | grep, gsub, sub, grepl, regexpr, strsplit | `regex` |
| `signal-hook` | Ctrl-C interrupt handling | `signal` |
| `unicode-width` | nchar(type="width"), display alignment | `unicode` |
| `unicode-segmentation` | nchar(type="graphemes") | `unicode` (same) |
| `unicase` | match(ignore.case=TRUE) | `unicode` (same) |
| `libm` | gamma, lgamma, beta, lbeta | `math-special` |
| `itertools` | Code quality (join, sorted, unique) | keep always-on (zero-cost) |
| `nu-ansi-term` | REPL colored output | keep always-on (REPL needs it) |

### Must stay always-on (core infrastructure):
| Dependency | Why |
|---|---|
| `derive_more` | Proc macro for Display/Error/Deref derives |
| `indexmap` | Ordered attributes (fundamental to R semantics) |
| `linkme` | Builtin registration (compile-time) |
| `pest` + `pest_derive` | Parser (cannot parse R without it) |
| `minir-macros` | Our proc macros |
| `reedline` | REPL (could be optional but main.rs needs it) |
| `temp-dir` | tempfile()/tempdir() (fundamental) |
| `glob` | Sys.glob() (fundamental) |
| `itertools` | Used everywhere for join/sorted (zero runtime cost) |

## Proposed feature structure

```toml
[features]
default = [
    "random", "datetime", "io", "collections", "digest", "json",
    "dirs-support", "walkdir-support", "globset-support",
    "linalg", "complex", "tables", "regex-support", "signal",
    "unicode", "math-special",
]

# Data types
complex = ["dep:num-complex"]
linalg = ["dep:ndarray"]

# String/text
regex-support = ["dep:regex"]
unicode = ["dep:unicode-width", "dep:unicode-segmentation", "dep:unicase"]

# Math
random = ["dep:rand", "dep:rand_distr"]
math-special = ["dep:libm"]

# Date/time
datetime = ["dep:jiff"]

# I/O and serialization
io = ["dep:csv"]
json = ["dep:serde_json"]
digest = ["dep:sha2"]

# File system
dirs-support = ["dep:dirs"]
walkdir-support = ["dep:walkdir"]
globset-support = ["dep:globset"]

# Display
tables = ["dep:tabled", "dep:tabwriter"]
signal = ["dep:signal-hook"]

# Data structures
collections = []

# Performance (opt-in)
parallel = ["dep:rayon"]
```

## Minimal build (no features)

`cargo build --no-default-features` should compile a working interpreter with:
- Parsing and evaluation
- Basic vector types (no complex)
- Basic math (no gamma/beta)
- String ops (no regex)
- No I/O, no random, no datetime, no JSON
- No REPL coloring or signal handling

This is the WASM/embedded target.

## Implementation order

1. Gate `ndarray` behind `linalg` — affects math.rs (matrix ops, lm)
2. Gate `num-complex` behind `complex` — affects value.rs (Vector::Complex)
3. Gate `regex` behind `regex-support` — affects strings.rs
4. Gate `signal-hook` behind `signal` — affects interpreter.rs, session.rs
5. Gate `tabled`/`tabwriter` behind `tables` — affects interp.rs
6. Gate `unicode-*` behind `unicode` — affects builtins.rs, strings.rs
7. Gate `libm` behind `math-special` — affects math.rs
8. Verify `cargo check --no-default-features` compiles
9. Verify `cargo test` still passes with all defaults

## cfg patterns

Each gated module uses `#[cfg(feature = "...")]`:
```rust
#[cfg(feature = "complex")]
Vector::Complex(vals) => { ... }

#[cfg(not(feature = "complex"))]
// Complex variant doesn't exist — skip
```

For the `Vector` enum itself, complex becomes conditional:
```rust
pub enum Vector {
    Raw(Vec<u8>),
    Logical(Logical),
    Integer(Integer),
    Double(Double),
    #[cfg(feature = "complex")]
    Complex(Complex),
    Character(Character),
}
```

This is the most invasive change — every match on Vector needs a
`#[cfg(feature = "complex")]` arm or a wildcard. Consider whether
this complexity is worth it. Alternative: keep Complex always-on
(num-complex is tiny) and only gate the heavy deps.

## Pragmatic recommendation

Gate the **heavy** deps that meaningfully affect compile time:
- `ndarray` (~150 lines of generated code, matrixmultiply)
- `tabled` (~800 lines, papergrid, strum)
- `regex` (large, but used everywhere — maybe keep always-on)
- `jiff` (timezone database is large)
- `rand` + `rand_distr` (moderate)

Keep the **tiny** deps always-on:
- `num-complex` (trivial)
- `libm` (trivial)
- `unicode-*` (tiny)
- `itertools` (zero-cost abstractions)
- `signal-hook` (small)
- `tabwriter` (tiny)
- `unicase` (tiny)
