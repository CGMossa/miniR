# time integration plan

> `time` 0.3 — Date and time library.
> https://github.com/time-rs/time

## What it does

Date, Time, OffsetDateTime, Duration, formatting/parsing. Pure Rust, no system
dependencies.

Key types: `Date`, `Time`, `PrimitiveDateTime`, `OffsetDateTime`, `Duration`,
`UtcOffset`. Formatting via `format_description!()` macro or runtime format strings.

## Where it fits in newr

### Note: Consider jiff instead

BurntSushi's `jiff` is a more modern datetime library with better timezone support,
ISO 8601 duration parsing, and a cleaner API. See `plans/jiff.md`.

### R's date/time system

R has three date/time classes:
- `Date` — days since 1970-01-01 (stored as double)
- `POSIXct` — seconds since epoch (stored as double)
- `POSIXlt` — broken-down time (named list with sec, min, hour, mday, mon, year, etc.)

### Integration points

1. **`Sys.time()`** → `OffsetDateTime::now_utc()` → seconds since epoch as f64
2. **`Sys.Date()`** → `OffsetDateTime::now_utc().date()` → days since epoch
3. **`as.Date()`** → parse string to Date
4. **`as.POSIXct()`** → parse string to epoch seconds
5. **`format.Date()` / `strftime()`** → format with `%Y-%m-%d` etc.
6. **`strptime()`** → parse with format string
7. **`difftime()`** → `Duration` arithmetic
8. **`proc.time()`** → `Instant::elapsed()` (this is `std::time`, not the `time` crate)

## Relationship to builtins plan

| Phase | Builtins affected | Impact |
|---|---|---|
| Phase 7 (time) | `Sys.time()`, `Sys.Date()`, `proc.time()`, `system.time()` | time builtins |
| Phase 7 (time) | `as.Date()`, `as.POSIXct()`, `strftime()`, `strptime()` | date parsing/formatting |
| Phase 7 (time) | `difftime()`, `seq.Date()`, `seq.POSIXt()` | date arithmetic |

## Recommendation

**Consider `jiff` instead** for timezone-aware datetime. Use `std::time::Instant`
for `proc.time()` / `system.time()` (no external crate needed for elapsed time).

If sticking with `time` crate: add when implementing Phase 7 date/time builtins.

**Effort:** 2-3 hours for core date/time builtins.
