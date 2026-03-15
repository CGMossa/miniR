//! Date and time builtins: Sys.Date, as.Date, as.POSIXct, format.Date, etc.
//!
//! Uses the `jiff` crate for timezone-aware date/time operations.
//! R stores dates as doubles with class attributes:
//! - Date: days since 1970-01-01
//! - POSIXct: seconds since epoch, class = c("POSIXct", "POSIXt")

use derive_more::{Display, Error};
use jiff::civil::Date;
use jiff::Timestamp;

use crate::interpreter::value::*;
use minir_macros::builtin;

// region: DateTimeError

#[derive(Debug, Display, Error)]
pub enum DateTimeError {
    #[display("character string is not in a standard unambiguous format")]
    AmbiguousFormat,

    #[display("invalid 'origin' argument")]
    InvalidOrigin,

    #[display("invalid date/time format: {}", reason)]
    InvalidFormat { reason: String },
}

impl From<DateTimeError> for RError {
    fn from(e: DateTimeError) -> Self {
        RError::from_source(RErrorKind::Argument, e)
    }
}

// endregion

// region: helpers

/// The Unix epoch as a jiff Date.
const EPOCH: Date = Date::constant(1970, 1, 1);

/// Convert days-since-epoch (f64) to a jiff Date.
fn days_to_date(days: f64) -> Option<Date> {
    let days_i32 = days.round() as i32; // f64→i32: no TryFrom in std; valid dates always fit
    EPOCH
        .checked_add(jiff::Span::new().days(i64::from(days_i32)))
        .ok()
}

/// Convert a jiff Date to days-since-epoch (f64).
fn date_to_days(date: Date) -> f64 {
    // until() returns Result but cannot fail for two valid Dates with default config
    let span = EPOCH.until(date).expect("valid date span");
    f64::from(span.get_days())
}

/// Build an RValue with class = "Date".
fn r_date(days_since_epoch: f64) -> RValue {
    let mut rv = RVector::from(Vector::Double(vec![Some(days_since_epoch)].into()));
    rv.set_attr(
        "class".to_string(),
        RValue::vec(Vector::Character(vec![Some("Date".to_string())].into())),
    );
    rv.into()
}

/// Build an RValue with class = c("POSIXct", "POSIXt").
fn r_posixct(secs_since_epoch: f64, tz: Option<&str>) -> RValue {
    let mut rv = RVector::from(Vector::Double(vec![Some(secs_since_epoch)].into()));
    rv.set_attr(
        "class".to_string(),
        RValue::vec(Vector::Character(
            vec![Some("POSIXct".to_string()), Some("POSIXt".to_string())].into(),
        )),
    );
    if let Some(tz) = tz {
        rv.set_attr(
            "tzone".to_string(),
            RValue::vec(Vector::Character(vec![Some(tz.to_string())].into())),
        );
    }
    rv.into()
}

/// Build a POSIXct vector from multiple seconds values.
fn r_posixct_vec(secs: Vec<Option<f64>>, tz: Option<&str>) -> RValue {
    let mut rv = RVector::from(Vector::Double(secs.into()));
    rv.set_attr(
        "class".to_string(),
        RValue::vec(Vector::Character(
            vec![Some("POSIXct".to_string()), Some("POSIXt".to_string())].into(),
        )),
    );
    if let Some(tz) = tz {
        rv.set_attr(
            "tzone".to_string(),
            RValue::vec(Vector::Character(vec![Some(tz.to_string())].into())),
        );
    }
    rv.into()
}

/// Map R strftime codes to jiff strftime codes.
/// Most are identical; R's %OS (fractional seconds) maps to %S%.f.
fn translate_format(fmt: &str) -> String {
    fmt.replace("%OS", "%S%.f")
}

/// Extract seconds-since-epoch from a Timestamp.
fn timestamp_to_secs(ts: &Timestamp) -> f64 {
    // i64→f64: epoch seconds always fit in f64 mantissa
    (ts.as_second() as f64) + f64::from(ts.subsec_nanosecond()) / 1e9
}

/// Try common date formats in order.
fn parse_date_string(s: &str) -> Result<Date, DateTimeError> {
    // ISO 8601 first
    if let Ok(d) = s.parse::<Date>() {
        return Ok(d);
    }
    // Try common R formats
    for fmt in &["%Y-%m-%d", "%Y/%m/%d", "%m/%d/%Y", "%d/%m/%Y", "%b %d, %Y"] {
        if let Ok(d) = Date::strptime(fmt, s) {
            return Ok(d);
        }
    }
    Err(DateTimeError::AmbiguousFormat)
}

/// Try common datetime formats.
fn parse_datetime_string(s: &str, tz: Option<&str>) -> Result<f64, DateTimeError> {
    // Try parsing as Zoned first (has timezone info)
    if let Ok(z) = s.parse::<jiff::Zoned>() {
        return Ok(timestamp_to_secs(&z.timestamp()));
    }
    // Try as Timestamp
    if let Ok(ts) = s.parse::<Timestamp>() {
        return Ok(timestamp_to_secs(&ts));
    }
    // Try as civil DateTime, then apply timezone
    let fmts = [
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%d %H:%M",
        "%Y/%m/%d %H:%M:%S",
        "%Y-%m-%dT%H:%M:%S",
    ];
    for fmt in &fmts {
        if let Ok(dt) = jiff::civil::DateTime::strptime(fmt, s) {
            let zoned = civil_to_zoned(dt, tz)?;
            return Ok(timestamp_to_secs(&zoned.timestamp()));
        }
    }
    // Try as date-only, treating as midnight
    if let Ok(d) = parse_date_string(s) {
        let dt = d.at(0, 0, 0, 0);
        let zoned = civil_to_zoned(dt, tz)?;
        return Ok(timestamp_to_secs(&zoned.timestamp()));
    }
    Err(DateTimeError::AmbiguousFormat)
}

/// Convert a civil DateTime to a Zoned datetime in the given timezone.
fn civil_to_zoned(
    dt: jiff::civil::DateTime,
    tz: Option<&str>,
) -> Result<jiff::Zoned, DateTimeError> {
    let tz_name = tz.unwrap_or("UTC");
    let tz_obj = jiff::tz::TimeZone::get(tz_name).map_err(|_| DateTimeError::InvalidFormat {
        reason: format!("unknown timezone '{tz_name}'"),
    })?;
    dt.to_zoned(tz_obj)
        .map_err(|e| DateTimeError::InvalidFormat {
            reason: e.to_string(),
        })
}

/// Build a Date vector with class attr from days values.
fn r_date_vec(days: Vec<Option<f64>>) -> RValue {
    let mut rv = RVector::from(Vector::Double(days.into()));
    rv.set_attr(
        "class".to_string(),
        RValue::vec(Vector::Character(vec![Some("Date".to_string())].into())),
    );
    rv.into()
}

// endregion

// region: Sys.Date / Sys.time

/// Get the current date.
///
/// @return a Date object representing today's date
#[builtin(name = "Sys.Date")]
fn builtin_sys_date(_args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    let today = jiff::Zoned::now().date();
    Ok(r_date(date_to_days(today)))
}

/// Get the current date-time as a POSIXct value.
///
/// @return a POSIXct object representing the current instant
#[builtin(name = "Sys.time")]
fn builtin_sys_time(_args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    let ts = Timestamp::now();
    Ok(r_posixct(timestamp_to_secs(&ts), None))
}

// endregion

// region: print.Date / print.POSIXct

/// Print a Date object to stdout.
///
/// @param x a Date object to print
/// @return x, invisibly
#[builtin(name = "print.Date", min_args = 1)]
fn builtin_print_date(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let formatted = builtin_format_date(args, named)?;
    println!("{}", formatted);
    Ok(args.first().cloned().unwrap_or(RValue::Null))
}

/// Print a POSIXct object to stdout.
///
/// @param x a POSIXct object to print
/// @return x, invisibly
#[builtin(name = "print.POSIXct", min_args = 1)]
fn builtin_print_posixct(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let formatted = builtin_format_posixct(args, named)?;
    println!("{}", formatted);
    Ok(args.first().cloned().unwrap_or(RValue::Null))
}

// endregion

// region: as.Date

/// Convert a character string or numeric value to a Date object.
///
/// @param x character string, numeric (days since origin), or Date to convert
/// @param format strptime-style format string for parsing (optional)
/// @param origin Date or string giving the origin for numeric conversion
/// @return a Date object
#[builtin(name = "as.Date", min_args = 1)]
fn builtin_as_date(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let x = &args[0];

    // Check for format argument
    let format = named
        .iter()
        .find(|(k, _)| k == "format")
        .map(|(_, v)| v)
        .or(args.get(1))
        .and_then(|v| v.as_vector())
        .and_then(|v| v.as_character_scalar());

    // Check for origin argument (for numeric conversion)
    let origin = named
        .iter()
        .find(|(k, _)| k == "origin")
        .map(|(_, v)| v)
        .or(args.get(1));

    match x {
        RValue::Vector(rv) => {
            // Check if already a Date
            if let Some(cls) = rv.get_attr("class") {
                if let Some(c) = cls.as_vector().and_then(|v| v.as_character_scalar()) {
                    if c == "Date" {
                        return Ok(x.clone());
                    }
                }
            }

            match &rv.inner {
                Vector::Character(cv) => {
                    // Parse string(s) to date
                    let days: Vec<Option<f64>> = cv
                        .iter()
                        .map(|opt_s| {
                            opt_s
                                .as_ref()
                                .map(|s| {
                                    if let Some(ref fmt) = format {
                                        let jiff_fmt = translate_format(fmt);
                                        Date::strptime(&jiff_fmt, s)
                                            .map(date_to_days)
                                            .map_err(|_| DateTimeError::AmbiguousFormat)
                                    } else {
                                        parse_date_string(s).map(date_to_days)
                                    }
                                })
                                .transpose()
                        })
                        .collect::<Result<_, _>>()?;
                    Ok(r_date_vec(days))
                }
                Vector::Double(dv) => {
                    // Numeric: needs origin
                    let origin_days = resolve_origin(origin)?;
                    let days: Vec<Option<f64>> = dv
                        .iter()
                        .map(|opt_d| opt_d.map(|d| d + origin_days))
                        .collect();
                    Ok(r_date_vec(days))
                }
                Vector::Integer(iv) => {
                    // Integer: same as double, needs origin
                    let origin_days = resolve_origin(origin)?;
                    let days: Vec<Option<f64>> = iv
                        .iter()
                        .map(|opt_i| opt_i.map(|i| i as f64 + origin_days))
                        .collect();
                    Ok(r_date_vec(days))
                }
                _ => Err(RError::new(
                    RErrorKind::Type,
                    format!(
                        "expected character or numeric, got {}",
                        rv.inner.type_name()
                    ),
                )),
            }
        }
        _ => Err(RError::new(
            RErrorKind::Type,
            format!("expected character or numeric, got {}", x.type_name()),
        )),
    }
}

/// Resolve the 'origin' argument for numeric→Date conversion.
fn resolve_origin(origin: Option<&RValue>) -> Result<f64, RError> {
    if let Some(orig) = origin {
        if let Some(s) = orig.as_vector().and_then(|v| v.as_character_scalar()) {
            let d = parse_date_string(&s).map_err(|_| DateTimeError::InvalidOrigin)?;
            Ok(date_to_days(d))
        } else if let Some(d) = orig.as_vector().and_then(|v| v.as_double_scalar()) {
            Ok(d)
        } else {
            Err(DateTimeError::InvalidOrigin.into())
        }
    } else {
        Err(RError::other(
            "'origin' must be supplied for numeric conversion",
        ))
    }
}

// endregion

// region: as.POSIXct

/// Convert a character string or numeric value to a POSIXct date-time.
///
/// @param x character string, numeric (seconds since epoch), or POSIXct to convert
/// @param tz timezone name (default: UTC for parsing, system for display)
/// @return a POSIXct object
#[builtin(name = "as.POSIXct", min_args = 1)]
fn builtin_as_posixct(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let x = &args[0];

    let tz = named
        .iter()
        .find(|(k, _)| k == "tz")
        .map(|(_, v)| v)
        .and_then(|v| v.as_vector())
        .and_then(|v| v.as_character_scalar());

    match x {
        RValue::Vector(rv) => {
            // If already POSIXct, return as-is
            if let Some(cls) = rv.get_attr("class") {
                if let Some(c) = cls.as_vector().and_then(|v| v.as_character_scalar()) {
                    if c == "POSIXct" {
                        return Ok(x.clone());
                    }
                }
            }

            match &rv.inner {
                Vector::Character(cv) => {
                    let secs: Vec<Option<f64>> = cv
                        .iter()
                        .map(|opt_s| {
                            opt_s
                                .as_ref()
                                .map(|s| parse_datetime_string(s, tz.as_deref()))
                                .transpose()
                        })
                        .collect::<Result<_, _>>()?;
                    Ok(r_posixct_vec(secs, tz.as_deref()))
                }
                Vector::Double(dv) => {
                    // Numeric: treat as seconds since epoch
                    let secs: Vec<Option<f64>> = dv.iter().copied().collect();
                    Ok(r_posixct_vec(secs, tz.as_deref()))
                }
                _ => Err(RError::new(
                    RErrorKind::Type,
                    format!(
                        "expected character or numeric, got {}",
                        rv.inner.type_name()
                    ),
                )),
            }
        }
        _ => Err(RError::new(
            RErrorKind::Type,
            format!("expected character or numeric, got {}", x.type_name()),
        )),
    }
}

// endregion

// region: format.Date / format.POSIXct

/// Format a Date object as a character string.
///
/// @param x a Date object
/// @param format strftime-style format string (default "%Y-%m-%d")
/// @return character vector of formatted date strings
#[builtin(name = "format.Date", min_args = 1)]
fn builtin_format_date(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let x = &args[0];
    let format = named
        .iter()
        .find(|(k, _)| k == "format")
        .map(|(_, v)| v)
        .or(args.get(1))
        .and_then(|v| v.as_vector())
        .and_then(|v| v.as_character_scalar())
        .unwrap_or_else(|| "%Y-%m-%d".to_string());

    let jiff_fmt = translate_format(&format);

    match x {
        RValue::Vector(rv) => match &rv.inner {
            Vector::Double(vals) => {
                let result: Vec<Option<String>> = vals
                    .iter()
                    .map(|opt_d| {
                        opt_d.and_then(|d| {
                            days_to_date(d).map(|date| date.strftime(&jiff_fmt).to_string())
                        })
                    })
                    .collect();
                Ok(RValue::vec(Vector::Character(result.into())))
            }
            _ => Err(RError::new(
                RErrorKind::Type,
                format!("expected Date (numeric), got {}", rv.inner.type_name()),
            )),
        },
        _ => Err(RError::new(
            RErrorKind::Type,
            format!("expected Date, got {}", x.type_name()),
        )),
    }
}

/// Format a POSIXct object as a character string.
///
/// @param x a POSIXct object
/// @param format strftime-style format string (default "%Y-%m-%d %H:%M:%S")
/// @return character vector of formatted datetime strings
#[builtin(name = "format.POSIXct", min_args = 1)]
fn builtin_format_posixct(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let x = &args[0];
    let format = named
        .iter()
        .find(|(k, _)| k == "format")
        .map(|(_, v)| v)
        .or(args.get(1))
        .and_then(|v| v.as_vector())
        .and_then(|v| v.as_character_scalar())
        .unwrap_or_else(|| "%Y-%m-%d %H:%M:%S".to_string());

    let jiff_fmt = translate_format(&format);

    // Get timezone from attribute
    let tz_name = if let RValue::Vector(rv) = x {
        rv.get_attr("tzone")
            .and_then(|v| v.as_vector())
            .and_then(|v| v.as_character_scalar())
    } else {
        None
    };

    let tz = if let Some(ref tz_name) = tz_name {
        jiff::tz::TimeZone::get(tz_name).unwrap_or(jiff::tz::TimeZone::UTC)
    } else {
        jiff::tz::TimeZone::system()
    };

    match x {
        RValue::Vector(rv) => match &rv.inner {
            Vector::Double(vals) => {
                let result: Vec<Option<String>> = vals
                    .iter()
                    .map(|opt_d| {
                        opt_d.and_then(|secs| {
                            secs_to_timestamp(secs)
                                .map(|ts| ts.to_zoned(tz.clone()).strftime(&jiff_fmt).to_string())
                        })
                    })
                    .collect();
                Ok(RValue::vec(Vector::Character(result.into())))
            }
            _ => Err(RError::new(
                RErrorKind::Type,
                format!("expected POSIXct (numeric), got {}", rv.inner.type_name()),
            )),
        },
        _ => Err(RError::new(
            RErrorKind::Type,
            format!("expected POSIXct, got {}", x.type_name()),
        )),
    }
}

/// Convert seconds-since-epoch (f64) to a jiff Timestamp.
fn secs_to_timestamp(secs: f64) -> Option<Timestamp> {
    let whole = secs.floor() as i64; // f64→i64: epoch seconds always fit
    let nanos = ((secs - secs.floor()) * 1e9) as i32;
    Timestamp::new(whole, nanos).ok()
}

// endregion

// region: strptime / strftime

/// Parse a character string into a POSIXct date-time using a format specification.
///
/// @param x character string to parse
/// @param format strptime-style format string
/// @param tz timezone name (default: UTC)
/// @return a POSIXct object
#[builtin(min_args = 2)]
fn builtin_strptime(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let x = args[0]
        .as_vector()
        .and_then(|v| v.as_character_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Type,
                format!("expected character, got {}", args[0].type_name()),
            )
        })?;

    let format = named
        .iter()
        .find(|(k, _)| k == "format")
        .map(|(_, v)| v)
        .or(args.get(1))
        .and_then(|v| v.as_vector())
        .and_then(|v| v.as_character_scalar())
        .ok_or_else(|| RError::new(RErrorKind::Type, "expected character format".to_string()))?;

    let tz = named
        .iter()
        .find(|(k, _)| k == "tz")
        .map(|(_, v)| v)
        .and_then(|v| v.as_vector())
        .and_then(|v| v.as_character_scalar());

    let jiff_fmt = translate_format(&format);

    // Try parsing as datetime first, then as date
    if let Ok(dt) = jiff::civil::DateTime::strptime(&jiff_fmt, &x) {
        let zoned = civil_to_zoned(dt, tz.as_deref())?;
        let secs = timestamp_to_secs(&zoned.timestamp());
        return Ok(r_posixct(secs, tz.as_deref()));
    }

    if let Ok(d) = Date::strptime(&jiff_fmt, &x) {
        let dt = d.at(0, 0, 0, 0);
        let zoned = civil_to_zoned(dt, tz.as_deref())?;
        let secs = timestamp_to_secs(&zoned.timestamp());
        return Ok(r_posixct(secs, tz.as_deref()));
    }

    Err(DateTimeError::AmbiguousFormat.into())
}

/// Format a POSIXct date-time as a character string.
///
/// @param x a POSIXct object
/// @param format strftime-style format string (default "%Y-%m-%d %H:%M:%S")
/// @return character string representation
#[builtin(min_args = 1)]
fn builtin_strftime(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    // strftime is essentially format.POSIXct
    let format = named
        .iter()
        .find(|(k, _)| k == "format")
        .map(|(_, v)| v)
        .or(args.get(1))
        .and_then(|v| v.as_vector())
        .and_then(|v| v.as_character_scalar())
        .unwrap_or_else(|| "%Y-%m-%d %H:%M:%S".to_string());

    let named_with_format: Vec<(String, RValue)> = vec![(
        "format".to_string(),
        RValue::vec(Vector::Character(vec![Some(format)].into())),
    )];
    builtin_format_posixct(args, &named_with_format)
}

// endregion

// region: difftime

/// Compute the time difference between two date-time values.
///
/// @param time1 first POSIXct or Date value
/// @param time2 second POSIXct or Date value
/// @param units time unit for the result: "secs", "mins", "hours", "days", or "weeks"
/// @return a difftime object (numeric with class and units attributes)
#[builtin(min_args = 2)]
fn builtin_difftime(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let t1 = args[0]
        .as_vector()
        .and_then(|v| v.as_double_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Type,
                format!("expected numeric time, got {}", args[0].type_name()),
            )
        })?;
    let t2 = args[1]
        .as_vector()
        .and_then(|v| v.as_double_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Type,
                format!("expected numeric time, got {}", args[1].type_name()),
            )
        })?;

    let units = named
        .iter()
        .find(|(k, _)| k == "units")
        .map(|(_, v)| v)
        .or(args.get(2))
        .and_then(|v| v.as_vector())
        .and_then(|v| v.as_character_scalar())
        .unwrap_or_else(|| "secs".to_string());

    let diff_secs = t1 - t2;
    let value = match units.as_str() {
        "secs" => diff_secs,
        "mins" => diff_secs / 60.0,
        "hours" => diff_secs / 3600.0,
        "days" => diff_secs / 86400.0,
        "weeks" => diff_secs / 604800.0,
        _ => {
            return Err(RError::other(format!(
                "invalid 'units' argument: '{units}'"
            )))
        }
    };

    let mut rv = RVector::from(Vector::Double(vec![Some(value)].into()));
    rv.set_attr(
        "class".to_string(),
        RValue::vec(Vector::Character(vec![Some("difftime".to_string())].into())),
    );
    rv.set_attr(
        "units".to_string(),
        RValue::vec(Vector::Character(vec![Some(units)].into())),
    );
    Ok(rv.into())
}

// endregion

// region: date component extractors

/// Extract the day-of-week name from a Date object.
///
/// @param x a Date object
/// @return character vector of weekday names (e.g. "Monday", "Tuesday")
#[builtin(min_args = 1)]
fn builtin_weekdays(args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    extract_date_component(&args[0], |date| {
        let name = match date.weekday() {
            jiff::civil::Weekday::Monday => "Monday",
            jiff::civil::Weekday::Tuesday => "Tuesday",
            jiff::civil::Weekday::Wednesday => "Wednesday",
            jiff::civil::Weekday::Thursday => "Thursday",
            jiff::civil::Weekday::Friday => "Friday",
            jiff::civil::Weekday::Saturday => "Saturday",
            jiff::civil::Weekday::Sunday => "Sunday",
        };
        Some(name.to_string())
    })
}

/// Extract the month name from a Date object.
///
/// @param x a Date object
/// @return character vector of month names (e.g. "January", "February")
#[builtin(min_args = 1)]
fn builtin_months(args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    extract_date_component(&args[0], |date| {
        let name = match date.month() {
            1 => "January",
            2 => "February",
            3 => "March",
            4 => "April",
            5 => "May",
            6 => "June",
            7 => "July",
            8 => "August",
            9 => "September",
            10 => "October",
            11 => "November",
            12 => "December",
            _ => unreachable!(),
        };
        Some(name.to_string())
    })
}

/// Extract the quarter from a Date object.
///
/// @param x a Date object
/// @return character vector of quarter labels (e.g. "Q1", "Q2", "Q3", "Q4")
#[builtin(min_args = 1)]
fn builtin_quarters(args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    extract_date_component(&args[0], |date| {
        let q = (date.month() - 1) / 3 + 1;
        Some(format!("Q{q}"))
    })
}

/// Helper: extract a string component from each date in a Date vector.
fn extract_date_component(
    x: &RValue,
    f: impl Fn(Date) -> Option<String>,
) -> Result<RValue, RError> {
    match x {
        RValue::Vector(rv) => match &rv.inner {
            Vector::Double(vals) => {
                let result: Vec<Option<String>> = vals
                    .iter()
                    .map(|opt_d| opt_d.and_then(|d| days_to_date(d).and_then(&f)))
                    .collect();
                Ok(RValue::vec(Vector::Character(result.into())))
            }
            _ => Err(RError::new(
                RErrorKind::Type,
                format!("expected Date (numeric), got {}", rv.inner.type_name()),
            )),
        },
        _ => Err(RError::new(
            RErrorKind::Type,
            format!("expected Date, got {}", x.type_name()),
        )),
    }
}

// endregion

// region: as.POSIXlt (simplified: returns named list)

/// Convert a value to a POSIXlt (broken-down time) list representation.
///
/// @param x character string, numeric, or POSIXct to convert
/// @param tz timezone name (default: system timezone)
/// @return a POSIXlt list with components sec, min, hour, mday, mon, year, wday, yday, isdst
#[builtin(name = "as.POSIXlt", min_args = 1)]
fn builtin_as_posixlt(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let x = &args[0];
    let tz = named
        .iter()
        .find(|(k, _)| k == "tz")
        .map(|(_, v)| v)
        .and_then(|v| v.as_vector())
        .and_then(|v| v.as_character_scalar());

    // First convert to POSIXct seconds
    let secs = match x {
        RValue::Vector(rv) => {
            if let Some(d) = rv.inner.as_double_scalar() {
                d
            } else if let Some(s) = rv.inner.as_character_scalar() {
                parse_datetime_string(&s, tz.as_deref())?
            } else {
                return Err(RError::new(
                    RErrorKind::Type,
                    format!(
                        "expected character or numeric, got {}",
                        rv.inner.type_name()
                    ),
                ));
            }
        }
        _ => {
            return Err(RError::new(
                RErrorKind::Type,
                format!("expected character or numeric, got {}", x.type_name()),
            ));
        }
    };

    let tz_obj = if let Some(ref tz_name) = tz {
        jiff::tz::TimeZone::get(tz_name).unwrap_or(jiff::tz::TimeZone::UTC)
    } else {
        jiff::tz::TimeZone::system()
    };

    let ts = secs_to_timestamp(secs).ok_or_else(|| DateTimeError::InvalidFormat {
        reason: "timestamp out of range".to_string(),
    })?;
    let zoned = ts.to_zoned(tz_obj);
    let dt = zoned.datetime();

    // R's POSIXlt is a named list: sec, min, hour, mday, mon (0-based), year (since 1900),
    // wday (0=Sunday), yday (0-based), isdst
    let wday = match dt.date().weekday() {
        jiff::civil::Weekday::Sunday => 0i64,
        jiff::civil::Weekday::Monday => 1,
        jiff::civil::Weekday::Tuesday => 2,
        jiff::civil::Weekday::Wednesday => 3,
        jiff::civil::Weekday::Thursday => 4,
        jiff::civil::Weekday::Friday => 5,
        jiff::civil::Weekday::Saturday => 6,
    };

    let yday = i64::from(dt.date().day_of_year()) - 1; // 0-based in R

    let components: Vec<(Option<String>, RValue)> = vec![
        (
            Some("sec".to_string()),
            RValue::vec(Vector::Double(
                vec![Some(
                    f64::from(dt.time().second()) + f64::from(dt.time().subsec_nanosecond()) / 1e9,
                )]
                .into(),
            )),
        ),
        (
            Some("min".to_string()),
            RValue::vec(Vector::Integer(
                vec![Some(i64::from(dt.time().minute()))].into(),
            )),
        ),
        (
            Some("hour".to_string()),
            RValue::vec(Vector::Integer(
                vec![Some(i64::from(dt.time().hour()))].into(),
            )),
        ),
        (
            Some("mday".to_string()),
            RValue::vec(Vector::Integer(
                vec![Some(i64::from(dt.date().day()))].into(),
            )),
        ),
        (
            Some("mon".to_string()),
            RValue::vec(Vector::Integer(
                vec![Some(i64::from(dt.date().month()) - 1)].into(),
            )),
        ),
        (
            Some("year".to_string()),
            RValue::vec(Vector::Integer(
                vec![Some(i64::from(dt.date().year()) - 1900)].into(),
            )),
        ),
        (
            Some("wday".to_string()),
            RValue::vec(Vector::Integer(vec![Some(wday)].into())),
        ),
        (
            Some("yday".to_string()),
            RValue::vec(Vector::Integer(vec![Some(yday)].into())),
        ),
        (
            Some("isdst".to_string()),
            RValue::vec(Vector::Integer(vec![Some(-1i64)].into())), // -1 = unknown
        ),
    ];

    let mut list = RList::new(components);
    list.set_attr(
        "class".to_string(),
        RValue::vec(Vector::Character(
            vec![Some("POSIXlt".to_string()), Some("POSIXt".to_string())].into(),
        )),
    );
    Ok(list.into())
}

// endregion
