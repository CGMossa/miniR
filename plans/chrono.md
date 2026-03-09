# chrono integration plan

> `chrono` 0.4 — Date and time library for Rust.
> Already vendored as a transitive dependency of reedline.

## What it does

Full-featured date/time library: `NaiveDate`, `NaiveTime`, `NaiveDateTime`, `DateTime<Tz>`, `Duration`, `Utc`, `Local`. Parsing via `strftime`-compatible format strings. Timezone support via `chrono-tz` (optional).

## Where it fits in newr

### R's date/time system

R has three date/time representations:

| R class | Internal storage | chrono equivalent |
| ------- | --------------- | ----------------- |
| `Date` | double (days since 1970-01-01) | `NaiveDate` |
| `POSIXct` | double (seconds since epoch) | `DateTime<Utc>` or `DateTime<Local>` |
| `POSIXlt` | named list (sec, min, hour, mday, mon, year, wday, yday, isdst) | Break down via `.hour()`, `.minute()`, etc. |
| `difftime` | double with `units` attr | `chrono::Duration` |

### Integration points

| R function | chrono API |
| ---------- | ---------- |
| `Sys.time()` | `Utc::now().timestamp() as f64 + nanos` |
| `Sys.Date()` | `Local::now().date_naive()` → days since epoch |
| `as.Date("2024-01-15")` | `NaiveDate::parse_from_str(s, fmt)` |
| `as.Date(18000, origin="1970-01-01")` | `NaiveDate::from_num_days_from_ce(...)` |
| `as.POSIXct("2024-01-15 10:30:00")` | `NaiveDateTime::parse_from_str(s, fmt)` → epoch secs |
| `format(date, "%Y-%m-%d")` | `date.format(fmt).to_string()` |
| `strftime(x, format)` | Same as format |
| `strptime(x, format)` | `NaiveDateTime::parse_from_str(x, fmt)` |
| `difftime(t1, t2)` | `Duration::seconds((t1 - t2) as i64)` |
| `seq.Date(from, to, by)` | Date arithmetic with `Duration::days()` |
| `Sys.sleep(n)` | `std::thread::sleep` (not chrono, but related) |
| `weekdays(date)` | `date.weekday().to_string()` |
| `months(date)` | `date.month()` → month name |
| `quarters(date)` | `(date.month() - 1) / 3 + 1` |
| `julian(date)` | `date.num_days_from_ce()` |

### R format codes → chrono format codes

R and chrono both use `strftime`-style codes, so most map directly:

`%Y`, `%m`, `%d`, `%H`, `%M`, `%S`, `%A`, `%a`, `%B`, `%b`, `%p`, `%Z`, `%z` — all identical.

R-specific: `%OS` (fractional seconds) → chrono `%S%.f`.

### RValue representation

Dates and POSIXct are stored as `RValue::Vector(Double)` with class attributes — this matches R's representation exactly. No new RValue variant needed.

```rust
// Date: double with class="Date"
fn r_date(days_since_epoch: f64) -> RValue {
    let mut rv = RValue::vec(Vector::Double(vec![Some(days_since_epoch)].into()));
    rv.set_class("Date");
    rv
}

// POSIXct: double with class=c("POSIXct", "POSIXt")
fn r_posixct(secs_since_epoch: f64) -> RValue {
    let mut rv = RValue::vec(Vector::Double(vec![Some(secs_since_epoch)].into()));
    rv.set_class_vec(vec!["POSIXct", "POSIXt"]);
    rv
}
```

## Implementation order

1. `Sys.time()` and `Sys.Date()` — immediate, no parsing needed
2. `as.Date()` with string and numeric input
3. `as.POSIXct()` with string input
4. `format.Date()` and `format.POSIXct()` via S3 dispatch
5. `strptime()` / `strftime()`
6. Date arithmetic (`+`, `-` on Date objects)
7. `difftime()` with units conversion
8. `seq.Date()` / `seq.POSIXt()`
9. `weekdays()`, `months()`, `quarters()`
10. `POSIXlt` as named list (break-down conversion)

## Notes

- chrono is already vendored (transitive dep), so zero additional build cost
- The `time.md` and `jiff.md` plans exist as alternatives, but chrono is what we already have
- chrono's `strftime` compatibility makes R format string translation trivial
- No new RValue variant needed — dates are just doubles with class attrs, exactly like R
