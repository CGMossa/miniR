# jiff integration plan

> `jiff` 0.2 — Datetime library by BurntSushi.
> <https://github.com/BurntSushi/jiff>

## What it does

High-level datetime library with first-class timezone support. Inspired by
Temporal (TC39) and java.time. Better timezone handling than the `time` crate.

Key types:

- `Zoned` — datetime with timezone (like R's POSIXct with tzone attr)
- `Timestamp` — instant in time (UTC epoch, like R's POSIXct numeric value)
- `civil::Date` — calendar date (like R's Date)
- `civil::Time` — time of day
- `civil::DateTime` — date + time without timezone
- `Span` — duration (like R's difftime)
- `SignedDuration` — absolute duration in seconds/nanoseconds

Parsing: `"2024-03-15T10:30:00-05:00[America/New_York]".parse::<Zoned>()`
Formatting: `strftime`-compatible format strings
Arithmetic: `date + Span::new().days(30)`, timezone-aware addition

## Where it fits in miniR

### 1. R's Date class → `civil::Date`

```r
Sys.Date()                    # → jiff::civil::Date::today()
as.Date("2024-03-15")        # → "2024-03-15".parse::<civil::Date>()
as.Date("15/03/2024", "%d/%m/%Y")  # → strptime-style parsing
seq(as.Date("2024-01-01"), by="month", length.out=12)  # → Date + Span arithmetic
```

Internally, R stores Date as f64 (days since 1970-01-01). jiff's `Date` handles
the calendar logic; we convert to/from f64 for R compatibility.

### 2. R's POSIXct class → `Timestamp` / `Zoned`

```r
Sys.time()                   # → jiff::Timestamp::now()
as.POSIXct("2024-03-15 10:30:00", tz="America/New_York")
                              # → parse as Zoned
format(x, "%Y-%m-%d %H:%M:%S")  # → zoned.strftime(...)
```

R's POSIXct is seconds since epoch (f64). jiff `Timestamp` stores
nanoseconds since epoch — higher precision, same concept.

### 3. R's difftime → `Span` / `SignedDuration`

```r
difftime(t2, t1, units="secs")  # → SignedDuration
```

### 4. Date arithmetic

```r
Sys.Date() + 30               # → date + Span::new().days(30)
seq(as.Date("2024-01-01"), as.Date("2024-12-31"), by="month")
                              # → iterate with Span::new().months(1)
```

jiff handles DST transitions, leap seconds, and timezone changes correctly —
something the `time` crate struggles with.

### 5. `strftime()` / `strptime()`

jiff supports `strftime`-style format strings natively:

```rust
let z: Zoned = "2024-03-15T10:30:00-05:00[America/New_York]".parse()?;
let s = z.strftime("%Y-%m-%d %H:%M:%S %Z").to_string();
```

### Advantages over `time` crate

- First-class timezone support (IANA timezone database)
- `Zoned` type carries the timezone with it (R's `tzone` attribute equivalent)
- DST-aware arithmetic (adding 1 day across DST transition works correctly)
- `Span` type for human-readable durations ("1 month 3 days")
- Maintained by BurntSushi — excellent quality, thorough testing

## Relationship to builtins plan

| Phase | Builtins affected | Impact |
|---|---|---|
| Phase 7 (time) | `Sys.time()`, `Sys.Date()` | current time/date |
| Phase 7 (time) | `as.Date()`, `as.POSIXct()`, `as.POSIXlt()` | date/time parsing |
| Phase 7 (time) | `format.Date()`, `format.POSIXct()`, `strftime()` | formatting |
| Phase 7 (time) | `strptime()` | parsing with format string |
| Phase 7 (time) | `difftime()`, `seq.Date()`, `seq.POSIXt()` | arithmetic |
| Phase 7 (time) | `Sys.timezone()`, `OlsonNames()` | timezone support |

## Recommendation

**Use instead of `time` crate.** jiff is superior for R's datetime needs because
R's date/time system is fundamentally timezone-aware. jiff's `Zoned` type maps
directly to R's POSIXct with tzone attribute.

**Add when implementing Phase 7 (date/time builtins).**

**Effort:** 3-4 hours for core date/time builtins with timezone support.
