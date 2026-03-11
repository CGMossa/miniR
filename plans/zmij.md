# zmij integration plan

> `zmij` 1.0.21 — Fast f64-to-string conversion (Schubfach/yy algorithm).
> <https://github.com/dtolnay/zmij>

## What it does

High-performance double-to-decimal-string conversion. Faster than `format!`
and `std::fmt`, produces shortest round-trip representation.

```rust
let mut buffer = zmij::Buffer::new();
let printed = buffer.format(1.234);
assert_eq!(printed, "1.234");
```

API: `Buffer::new()`, `buffer.format(f64) -> &str`, `buffer.format(f32) -> &str`.
Zero allocation after initial buffer construction. Reusable across calls.

## Where it fits in miniR

### 1. Replace `format_r_double()` — the hot path

Currently in `src/interpreter/value.rs:228`:

```rust
pub fn format_r_double(f: f64) -> String {
    if f.is_nan() { "NaN".to_string() }
    else if f.is_infinite() {
        if f > 0.0 { "Inf".to_string() } else { "-Inf".to_string() }
    } else if f == f.floor() && f.abs() < 1e15 {
        format!("{}", f as i64)    // <-- allocates String
    } else {
        format!("{}", f)           // <-- allocates String, slow formatting
    }
}
```

With zmij:

```rust
pub fn format_r_double(f: f64) -> String {
    if f.is_nan() { return "NaN".to_string(); }
    if f.is_infinite() {
        return if f > 0.0 { "Inf".to_string() } else { "-Inf".to_string() };
    }
    // Integer-valued doubles display without decimal point
    if f == f.floor() && f.abs() < 1e15 {
        return (f as i64).to_string();
    }
    let mut buf = zmij::Buffer::new();
    buf.format(f).to_string()
}
```

Or better — thread-local buffer to avoid repeated allocation:

```rust
thread_local! {
    static ZMIJ_BUF: RefCell<zmij::Buffer> = RefCell::new(zmij::Buffer::new());
}

pub fn format_r_double(f: f64) -> String {
    if f.is_nan() { return "NaN".to_string(); }
    if f.is_infinite() {
        return if f > 0.0 { "Inf".to_string() } else { "-Inf".to_string() };
    }
    if f == f.floor() && f.abs() < 1e15 {
        return (f as i64).to_string();
    }
    ZMIJ_BUF.with(|buf| buf.borrow_mut().format(f).to_string())
}
```

**Impact:** `format_r_double` is called for every numeric value printed —
vector display, `cat()`, `print()`, `paste()`, `sprintf("%g")`, `deparse()`,
`as.character()` on doubles, `format()`, `writeLines()`. This is one of the
hottest paths in the interpreter.

### 2. `format_vector()` — bulk double formatting

`src/interpreter/value.rs:310` formats entire vectors. Currently:

```rust
Vector::Double(vals) => vals.iter()
    .map(|x| match x {
        Some(f) => format_r_double(*f),
        None => "NA".to_string(),
    })
    .collect(),
```

With zmij, we can reuse a single buffer across the entire vector:

```rust
Vector::Double(vals) => {
    let mut buf = zmij::Buffer::new();
    vals.iter()
        .map(|x| match x {
            Some(f) => {
                // Same R formatting rules...
                if f.is_nan() { "NaN".to_string() }
                else if f.is_infinite() {
                    if *f > 0.0 { "Inf".to_string() } else { "-Inf".to_string() }
                } else if *f == f.floor() && f.abs() < 1e15 {
                    (*f as i64).to_string()
                } else {
                    buf.format(*f).to_string()
                }
            }
            None => "NA".to_string(),
        })
        .collect()
}
```

One buffer, zero intermediate allocations for the formatting itself.

### 3. `sprintf()` / `formatC()` — `%g` and `%f` formatting

R's `sprintf("%g", x)` and `formatC(x)` need fast double formatting.
Currently `sprintf` uses `format!("{}", f)`. zmij gives correct shortest
representation for `%g`-style output.

For `%f` (fixed decimal places) and `%e` (scientific notation), zmij alone
isn't enough — but it handles the common `%g` case.

### 4. `write.csv()` / `write.table()` — bulk output

When we implement `write.csv()` (Phase 11 of std-builtins plan), every numeric
cell needs f64→string conversion. zmij would make CSV export significantly
faster compared to `format!`.

### 5. `deparse()` — expression-to-string

`deparse()` on numeric values calls `format_r_double`. Same benefit.

## Relationship to builtins plan

Direct match with several phases:

| Phase | Builtins affected | Impact |
|---|---|---|
| Phase 2 (strings) | `sprintf()`, `formatC()`, `format()` | faster `%g` formatting |
| Phase 11 (I/O) | `write.csv()`, `write.table()` | fast bulk numeric output |
| All phases | `print()`, `cat()`, `paste()`, `deparse()` | every double displayed |

## Recommendation

**Add now.** This is a pure performance win with zero API change. The
integration is 5 lines in `format_r_double()`. zmij has no dependencies,
is MIT-licensed, maintained by dtolnay.

**Effort:** 15 minutes.

1. `cargo add zmij`
2. Replace `format!("{}", f)` in `format_r_double()` with `zmij::Buffer::format()`
3. Optionally add thread-local buffer for hot paths
4. Run tests to verify formatting matches R output
