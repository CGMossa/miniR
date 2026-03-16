use std::collections::HashMap;

use unicode_width::UnicodeWidthStr;

use crate::interpreter::value::*;
use derive_more::{Display, Error};
use minir_macros::builtin;
use regex::Regex;

use crate::interpreter::value::deparse_expr;

// region: StringError

/// Structured error type for string operations.
#[derive(Debug, Display, Error)]
pub enum StringError {
    #[display("invalid regular expression: {}", source)]
    InvalidRegex {
        #[error(source)]
        source: regex::Error,
    },
}

impl From<StringError> for RError {
    fn from(e: StringError) -> Self {
        RError::from_source(RErrorKind::Argument, e)
    }
}

// endregion

/// Extract common regex options from named args: fixed, ignore.case, perl
fn get_regex_opts(named: &[(String, RValue)]) -> (bool, bool) {
    let fixed = named
        .iter()
        .find(|(n, _)| n == "fixed")
        .and_then(|(_, v)| v.as_vector()?.as_logical_scalar())
        .unwrap_or(false);
    let ignore_case = named
        .iter()
        .find(|(n, _)| n == "ignore.case")
        .and_then(|(_, v)| v.as_vector()?.as_logical_scalar())
        .unwrap_or(false);
    (fixed, ignore_case)
}

/// Build a compiled regex from a pattern string, respecting fixed and ignore.case options.
/// Returns Err(RError) if the pattern is invalid regex.
fn build_regex(pattern: &str, fixed: bool, ignore_case: bool) -> Result<Regex, RError> {
    let pat = if fixed {
        regex::escape(pattern)
    } else {
        pattern.to_string()
    };
    let pat = if ignore_case {
        format!("(?i){}", pat)
    } else {
        pat
    };
    Regex::new(&pat).map_err(|source| -> RError { StringError::InvalidRegex { source }.into() })
}

/// Convert R-style replacement backreferences (\1, \2) to regex crate style ($1, $2)
fn convert_replacement(repl: &str) -> String {
    let mut result = String::with_capacity(repl.len());
    let chars: Vec<char> = repl.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\\' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit() {
            result.push('$');
            result.push(chars[i + 1]);
            i += 2;
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }
    result
}

/// Extract a substring from a character string.
///
/// @param x character string to extract from
/// @param start integer starting position (1-indexed)
/// @param stop integer ending position (inclusive)
/// @return character scalar containing the substring
#[builtin(min_args = 3, names = ["substring"])]
fn builtin_substr(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let s = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();
    let start = usize::try_from(
        args.get(1)
            .and_then(|v| v.as_vector()?.as_integer_scalar())
            .unwrap_or(1),
    )?;
    let default_stop = i64::try_from(s.len())?;
    let stop = usize::try_from(
        args.get(2)
            .and_then(|v| v.as_vector()?.as_integer_scalar())
            .unwrap_or(default_stop),
    )?;
    let start = start.saturating_sub(1); // R is 1-indexed
    let result = if start < s.len() {
        s[start..stop.min(s.len())].to_string()
    } else {
        String::new()
    };
    Ok(RValue::vec(Vector::Character(vec![Some(result)].into())))
}

/// Convert strings to upper case.
///
/// @param x character vector to convert
/// @return character vector with all characters in upper case
#[builtin(min_args = 1)]
fn builtin_toupper(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(rv)) if matches!(rv.inner, Vector::Character(_)) => {
            let Vector::Character(vals) = &rv.inner else {
                unreachable!()
            };
            let result: Vec<Option<String>> = vals
                .iter()
                .map(|s| s.as_ref().map(|s| s.to_uppercase()))
                .collect();
            Ok(RValue::vec(Vector::Character(result.into())))
        }
        _ => Err(RError::new(
            RErrorKind::Argument,
            "argument is not character".to_string(),
        )),
    }
}

/// Convert strings to lower case.
///
/// @param x character vector to convert
/// @return character vector with all characters in lower case
#[builtin(min_args = 1)]
fn builtin_tolower(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(rv)) if matches!(rv.inner, Vector::Character(_)) => {
            let Vector::Character(vals) = &rv.inner else {
                unreachable!()
            };
            let result: Vec<Option<String>> = vals
                .iter()
                .map(|s| s.as_ref().map(|s| s.to_lowercase()))
                .collect();
            Ok(RValue::vec(Vector::Character(result.into())))
        }
        _ => Err(RError::new(
            RErrorKind::Argument,
            "argument is not character".to_string(),
        )),
    }
}

/// Remove leading and/or trailing whitespace from strings.
///
/// @param x character vector to trim
/// @param which character scalar: "both", "left", or "right"
/// @return character vector with whitespace removed
#[builtin(min_args = 1)]
fn builtin_trimws(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let which = named
        .iter()
        .find(|(n, _)| n == "which")
        .and_then(|(_, v)| v.as_vector()?.as_character_scalar())
        .or_else(|| {
            args.get(1)
                .and_then(|v| v.as_vector()?.as_character_scalar())
        })
        .unwrap_or_else(|| "both".to_string());
    let trim_fn: fn(&str) -> &str = match which.as_str() {
        "both" => str::trim,
        "left" => str::trim_start,
        "right" => str::trim_end,
        _ => {
            return Err(RError::new(
                RErrorKind::Argument,
                format!(
                    "invalid 'which' argument: {:?} — must be \"both\", \"left\", or \"right\"",
                    which
                ),
            ))
        }
    };
    match args.first() {
        Some(RValue::Vector(rv)) if matches!(rv.inner, Vector::Character(_)) => {
            let Vector::Character(vals) = &rv.inner else {
                unreachable!()
            };
            let result: Vec<Option<String>> = vals
                .iter()
                .map(|s| s.as_ref().map(|s| trim_fn(s).to_string()))
                .collect();
            Ok(RValue::vec(Vector::Character(result.into())))
        }
        _ => Err(RError::new(
            RErrorKind::Argument,
            "argument is not character".to_string(),
        )),
    }
}

/// Replace all occurrences of a pattern in character strings.
///
/// @param pattern character scalar: regular expression or fixed string
/// @param replacement character scalar: replacement text
/// @param x character vector to search
/// @param fixed logical: if TRUE, pattern is a literal string
/// @param ignore.case logical: if TRUE, matching is case-insensitive
/// @return character vector with all matches replaced
#[builtin(min_args = 3)]
fn builtin_gsub(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let pattern = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();
    let replacement = args
        .get(1)
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();
    let (fixed, ignore_case) = get_regex_opts(named);
    let re = build_regex(&pattern, fixed, ignore_case)?;
    let repl = if fixed {
        replacement.clone()
    } else {
        convert_replacement(&replacement)
    };
    match args.get(2) {
        Some(RValue::Vector(rv)) if matches!(rv.inner, Vector::Character(_)) => {
            let Vector::Character(vals) = &rv.inner else {
                unreachable!()
            };
            let result: Vec<Option<String>> = vals
                .iter()
                .map(|s| {
                    s.as_ref()
                        .map(|s| re.replace_all(s, repl.as_str()).into_owned())
                })
                .collect();
            Ok(RValue::vec(Vector::Character(result.into())))
        }
        _ => Err(RError::new(
            RErrorKind::Argument,
            "argument is not character".to_string(),
        )),
    }
}

/// Replace the first occurrence of a pattern in character strings.
///
/// @param pattern character scalar: regular expression or fixed string
/// @param replacement character scalar: replacement text
/// @param x character vector to search
/// @param fixed logical: if TRUE, pattern is a literal string
/// @param ignore.case logical: if TRUE, matching is case-insensitive
/// @return character vector with first match replaced
#[builtin(min_args = 3)]
fn builtin_sub(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let pattern = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();
    let replacement = args
        .get(1)
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();
    let (fixed, ignore_case) = get_regex_opts(named);
    let re = build_regex(&pattern, fixed, ignore_case)?;
    let repl = if fixed {
        replacement.clone()
    } else {
        convert_replacement(&replacement)
    };
    match args.get(2) {
        Some(RValue::Vector(rv)) if matches!(rv.inner, Vector::Character(_)) => {
            let Vector::Character(vals) = &rv.inner else {
                unreachable!()
            };
            let result: Vec<Option<String>> = vals
                .iter()
                .map(|s| {
                    s.as_ref()
                        .map(|s| re.replace(s, repl.as_str()).into_owned())
                })
                .collect();
            Ok(RValue::vec(Vector::Character(result.into())))
        }
        _ => Err(RError::new(
            RErrorKind::Argument,
            "argument is not character".to_string(),
        )),
    }
}

/// Test whether a pattern matches each element of a character vector.
///
/// @param pattern character scalar: regular expression or fixed string
/// @param x character vector to search
/// @param fixed logical: if TRUE, pattern is a literal string
/// @param ignore.case logical: if TRUE, matching is case-insensitive
/// @return logical vector indicating which elements match
#[builtin(min_args = 2)]
fn builtin_grepl(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let pattern = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();
    let (fixed, ignore_case) = get_regex_opts(named);
    let re = build_regex(&pattern, fixed, ignore_case)?;
    match args.get(1) {
        Some(RValue::Vector(rv)) if matches!(rv.inner, Vector::Character(_)) => {
            let Vector::Character(vals) = &rv.inner else {
                unreachable!()
            };
            let result: Vec<Option<bool>> = vals
                .iter()
                .map(|s| Some(s.as_ref().map(|s| re.is_match(s)).unwrap_or(false)))
                .collect();
            Ok(RValue::vec(Vector::Logical(result.into())))
        }
        _ => Err(RError::new(
            RErrorKind::Argument,
            "argument is not character".to_string(),
        )),
    }
}

/// Search for pattern matches in a character vector.
///
/// @param pattern character scalar: regular expression or fixed string
/// @param x character vector to search
/// @param value logical: if TRUE, return matching elements instead of indices
/// @param fixed logical: if TRUE, pattern is a literal string
/// @param ignore.case logical: if TRUE, matching is case-insensitive
/// @return integer vector of indices (default) or character vector of matching elements
#[builtin(min_args = 2)]
fn builtin_grep(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let pattern = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();
    let value = named
        .iter()
        .find(|(n, _)| n == "value")
        .and_then(|(_, v)| v.as_vector()?.as_logical_scalar())
        .unwrap_or(false);
    let (fixed, ignore_case) = get_regex_opts(named);
    let re = build_regex(&pattern, fixed, ignore_case)?;

    match args.get(1) {
        Some(RValue::Vector(rv)) if matches!(rv.inner, Vector::Character(_)) => {
            let Vector::Character(vals) = &rv.inner else {
                unreachable!()
            };
            if value {
                let result: Vec<Option<String>> = vals
                    .iter()
                    .filter(|s| s.as_ref().map(|s| re.is_match(s)).unwrap_or(false))
                    .cloned()
                    .collect();
                Ok(RValue::vec(Vector::Character(result.into())))
            } else {
                let result: Result<Vec<Option<i64>>, RError> = vals
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| s.as_ref().map(|s| re.is_match(s)).unwrap_or(false))
                    .map(|(i, _)| Ok(Some(i64::try_from(i)? + 1)))
                    .collect();
                Ok(RValue::vec(Vector::Integer(result?.into())))
            }
        }
        _ => Err(RError::new(
            RErrorKind::Argument,
            "argument is not character".to_string(),
        )),
    }
}

/// Find the first match of a pattern in each element of a character vector.
///
/// @param pattern character scalar: regular expression or fixed string
/// @param text character vector to search
/// @param fixed logical: if TRUE, pattern is a literal string
/// @param ignore.case logical: if TRUE, matching is case-insensitive
/// @return integer vector of match positions with "match.length" attribute
#[builtin(min_args = 2)]
fn builtin_regexpr(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let pattern = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();
    let (fixed, ignore_case) = get_regex_opts(named);
    let re = build_regex(&pattern, fixed, ignore_case)?;
    match args.get(1) {
        Some(RValue::Vector(rv)) if matches!(rv.inner, Vector::Character(_)) => {
            let Vector::Character(vals) = &rv.inner else {
                unreachable!()
            };
            let mut positions = Vec::new();
            let mut lengths = Vec::new();
            for s in vals.iter() {
                match s.as_ref().and_then(|s| re.find(s)) {
                    Some(m) => {
                        positions.push(Some(i64::try_from(m.start())? + 1)); // R is 1-indexed
                        lengths.push(Some(i64::try_from(m.len())?));
                    }
                    None => {
                        positions.push(Some(-1));
                        lengths.push(Some(-1));
                    }
                }
            }
            let mut rv = RVector::from(Vector::Integer(positions.into()));
            rv.set_attr(
                "match.length".to_string(),
                RValue::vec(Vector::Integer(lengths.into())),
            );
            Ok(RValue::Vector(rv))
        }
        _ => Err(RError::new(
            RErrorKind::Argument,
            "argument is not character".to_string(),
        )),
    }
}

/// Find all matches of a pattern in each element of a character vector.
///
/// @param pattern character scalar: regular expression or fixed string
/// @param text character vector to search
/// @param fixed logical: if TRUE, pattern is a literal string
/// @param ignore.case logical: if TRUE, matching is case-insensitive
/// @return list of integer vectors with match positions and "match.length" attributes
#[builtin(min_args = 2)]
fn builtin_gregexpr(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let pattern = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();
    let (fixed, ignore_case) = get_regex_opts(named);
    let re = build_regex(&pattern, fixed, ignore_case)?;
    match args.get(1) {
        Some(RValue::Vector(rv)) if matches!(rv.inner, Vector::Character(_)) => {
            let Vector::Character(vals) = &rv.inner else {
                unreachable!()
            };
            let mut list_items = Vec::new();
            for s in vals.iter() {
                let (positions, lengths): (Vec<Option<i64>>, Vec<Option<i64>>) = match s.as_ref() {
                    Some(s) => {
                        let matches: Vec<_> = re.find_iter(s).collect();
                        if matches.is_empty() {
                            (vec![Some(-1)], vec![Some(-1)])
                        } else {
                            let (positions, lengths): (Vec<_>, Vec<_>) = matches
                                .iter()
                                .map(|m| -> Result<_, RError> {
                                    Ok((
                                        Some(i64::try_from(m.start())? + 1),
                                        Some(i64::try_from(m.len())?),
                                    ))
                                })
                                .collect::<Result<Vec<_>, _>>()?
                                .into_iter()
                                .unzip();
                            (positions, lengths)
                        }
                    }
                    None => (vec![Some(-1)], vec![Some(-1)]),
                };
                let mut match_rv = RVector::from(Vector::Integer(positions.into()));
                match_rv.set_attr(
                    "match.length".to_string(),
                    RValue::vec(Vector::Integer(lengths.into())),
                );
                list_items.push((None, RValue::Vector(match_rv)));
            }
            Ok(RValue::List(RList::new(list_items)))
        }
        _ => Err(RError::new(
            RErrorKind::Argument,
            "argument is not character".to_string(),
        )),
    }
}

/// Extract matched substrings from regexpr/gregexpr results.
///
/// @param x character vector that was searched
/// @param m match data from regexpr() or gregexpr()
/// @return character vector (for regexpr) or list (for gregexpr) of matched substrings
#[builtin(min_args = 2)]
fn builtin_regmatches(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let x = match args.first() {
        Some(RValue::Vector(rv)) if matches!(rv.inner, Vector::Character(_)) => {
            let Vector::Character(vals) = &rv.inner else {
                unreachable!()
            };
            vals.clone()
        }
        _ => {
            return Err(RError::new(
                RErrorKind::Argument,
                "argument is not character".to_string(),
            ))
        }
    };

    // Second arg is regexpr/gregexpr output
    match args.get(1) {
        // regexpr result: single integer vector with match.length attr
        Some(RValue::Vector(rv)) if matches!(rv.inner, Vector::Integer(_)) => {
            let Vector::Integer(positions) = &rv.inner else {
                unreachable!()
            };
            let lengths = match rv.get_attr("match.length") {
                Some(RValue::Vector(lv)) => match &lv.inner {
                    Vector::Integer(l) => l.0.clone(),
                    _ => {
                        return Err(RError::new(
                            RErrorKind::Argument,
                            "invalid match data".to_string(),
                        ))
                    }
                },
                _ => {
                    return Err(RError::new(
                        RErrorKind::Argument,
                        "invalid match data".to_string(),
                    ))
                }
            };
            let mut result = Vec::new();
            for (i, pos) in positions.iter().enumerate() {
                let p = pos.unwrap_or(-1);
                let l = lengths.get(i).copied().flatten().unwrap_or(-1);
                if p > 0 && l > 0 {
                    if let Some(Some(s)) = x.get(i) {
                        let start = usize::try_from(p - 1)?;
                        let end = start + usize::try_from(l)?;
                        if end <= s.len() {
                            result.push(Some(s[start..end].to_string()));
                        } else {
                            result.push(None);
                        }
                    } else {
                        result.push(None);
                    }
                }
                // If no match (p == -1), skip (R returns character(0) effectively)
            }
            Ok(RValue::vec(Vector::Character(result.into())))
        }
        // gregexpr result: list of integer vectors
        Some(RValue::List(list)) => {
            let mut list_items = Vec::new();
            for (i, (_, match_val)) in list.values.iter().enumerate() {
                let RValue::Vector(rv) = match_val else {
                    list_items.push((
                        None,
                        RValue::vec(Vector::Character(Vec::<Option<String>>::new().into())),
                    ));
                    continue;
                };
                let Vector::Integer(positions) = &rv.inner else {
                    list_items.push((
                        None,
                        RValue::vec(Vector::Character(Vec::<Option<String>>::new().into())),
                    ));
                    continue;
                };
                let lengths = match rv.get_attr("match.length") {
                    Some(RValue::Vector(lv)) => match &lv.inner {
                        Vector::Integer(l) => l.0.clone(),
                        _ => vec![],
                    },
                    _ => vec![],
                };
                let s = x.get(i).and_then(|s| s.as_ref());
                let mut matches = Vec::new();
                for (j, pos) in positions.iter().enumerate() {
                    let p = pos.unwrap_or(-1);
                    let l = lengths.get(j).copied().flatten().unwrap_or(-1);
                    if p > 0 && l > 0 {
                        if let Some(s) = s {
                            let start = usize::try_from(p - 1)?;
                            let end = start + usize::try_from(l)?;
                            if end <= s.len() {
                                matches.push(Some(s[start..end].to_string()));
                            }
                        }
                    }
                }
                list_items.push((None, RValue::vec(Vector::Character(matches.into()))));
            }
            Ok(RValue::List(RList::new(list_items)))
        }
        _ => Err(RError::new(
            RErrorKind::Argument,
            "invalid match data".to_string(),
        )),
    }
}

/// Parsed format specifier from a `sprintf` format string.
struct FmtSpec {
    flags: String,
    width: Option<usize>,
    precision: Option<usize>,
    specifier: char,
}

impl FmtSpec {
    /// Apply width and flags to an already-formatted value string.
    ///
    /// Uses `UnicodeWidthStr::width()` so that CJK characters and emoji
    /// (which occupy two terminal columns) are measured correctly.
    fn pad(&self, formatted: &str) -> String {
        let width = match self.width {
            Some(w) => w,
            None => return formatted.to_string(),
        };
        let display_width = UnicodeWidthStr::width(formatted);
        if display_width >= width {
            return formatted.to_string();
        }
        let pad_char = if self.flags.contains('0') && !self.flags.contains('-') {
            '0'
        } else {
            ' '
        };
        let pad_len = width.saturating_sub(display_width);
        if self.flags.contains('-') {
            // left-align: content then spaces
            format!("{}{}", formatted, " ".repeat(pad_len))
        } else if pad_char == '0' {
            // zero-pad: preserve leading sign/minus
            if let Some(rest) = formatted.strip_prefix('-') {
                let pad_len = width.saturating_sub(UnicodeWidthStr::width(rest) + 1);
                format!("-{}{}", "0".repeat(pad_len), rest)
            } else {
                format!("{}{}", "0".repeat(pad_len), formatted)
            }
        } else {
            // right-align with spaces
            format!("{}{}", " ".repeat(pad_len), formatted)
        }
    }

    /// Format an integer value according to this specifier.
    fn format_int(&self, v: i64) -> String {
        let raw = match self.specifier {
            'x' => {
                let hex = format!("{:x}", v);
                if self.flags.contains('#') {
                    format!("0x{}", hex)
                } else {
                    hex
                }
            }
            'X' => {
                let hex = format!("{:X}", v);
                if self.flags.contains('#') {
                    format!("0X{}", hex)
                } else {
                    hex
                }
            }
            'o' => {
                let oct = format!("{:o}", v);
                if self.flags.contains('#') && !oct.starts_with('0') {
                    format!("0{}", oct)
                } else {
                    oct
                }
            }
            _ => {
                if self.flags.contains('+') && v >= 0 {
                    format!("+{}", v)
                } else {
                    v.to_string()
                }
            }
        };
        self.pad(&raw)
    }

    /// Format a float value according to this specifier.
    fn format_float(&self, v: f64) -> String {
        let prec = self.precision.unwrap_or(6);
        let raw = match self.specifier {
            'f' => {
                let s = format!("{:.prec$}", v, prec = prec);
                if self.flags.contains('+') && v >= 0.0 && !v.is_nan() {
                    format!("+{}", s)
                } else {
                    s
                }
            }
            'e' | 'E' => {
                let s = format_scientific(v, prec, self.specifier == 'E');
                if self.flags.contains('+') && v >= 0.0 && !v.is_nan() {
                    format!("+{}", s)
                } else {
                    s
                }
            }
            'g' | 'G' => {
                let s = format_g(v, prec, self.specifier == 'G');
                if self.flags.contains('+') && v >= 0.0 && !v.is_nan() {
                    format!("+{}", s)
                } else {
                    s
                }
            }
            _ => format!("{}", v),
        };
        self.pad(&raw)
    }

    /// Format a string value according to this specifier.
    fn format_str(&self, v: &str) -> String {
        let truncated = match self.precision {
            Some(prec) => &v[..v.len().min(prec)],
            None => v,
        };
        self.pad(truncated)
    }
}

/// Format a float in scientific notation matching R's output (two-digit exponent minimum).
fn format_scientific(v: f64, prec: usize, upper: bool) -> String {
    if v.is_nan() {
        return "NaN".to_string();
    }
    if v.is_infinite() {
        return if v > 0.0 {
            "Inf".to_string()
        } else {
            "-Inf".to_string()
        };
    }
    let e_char = if upper { 'E' } else { 'e' };
    if v == 0.0 {
        return format!("{:.prec$}{}{}", 0.0, e_char, "+00", prec = prec);
    }
    let abs_v = v.abs();
    let exp = abs_v.log10().floor() as i32;
    let mantissa = v / 10f64.powi(exp);
    format!("{:.prec$}{}{:+03}", mantissa, e_char, exp, prec = prec)
}

/// Format using %g/%G: use shorter of %f and %e, removing trailing zeros.
fn format_g(v: f64, prec: usize, upper: bool) -> String {
    if v.is_nan() {
        return "NaN".to_string();
    }
    if v.is_infinite() {
        return if v > 0.0 {
            "Inf".to_string()
        } else {
            "-Inf".to_string()
        };
    }
    let prec = if prec == 0 { 1 } else { prec };
    if v == 0.0 {
        return "0".to_string();
    }
    let abs_v = v.abs();
    let exp = abs_v.log10().floor() as i32;
    // Use %e if exponent < -4 or >= precision
    if exp < -4 || exp >= i32::try_from(prec).unwrap_or(i32::MAX) {
        let sig_prec = prec.saturating_sub(1);
        let s = format_scientific(v, sig_prec, upper);
        // Remove trailing zeros in mantissa before the 'e'
        if let Some(e_pos) = s.find(if upper { 'E' } else { 'e' }) {
            let mantissa_part = s[..e_pos].trim_end_matches('0').trim_end_matches('.');
            format!("{}{}", mantissa_part, &s[e_pos..])
        } else {
            s
        }
    } else {
        // Use %f with enough decimal places
        let decimal_places = if exp >= 0 {
            prec.saturating_sub(usize::try_from(exp + 1).unwrap_or(0))
        } else {
            prec + usize::try_from(-exp - 1).unwrap_or(0)
        };
        let s = format!("{:.prec$}", v, prec = decimal_places);
        // Remove trailing zeros after decimal point
        if s.contains('.') {
            let trimmed = s.trim_end_matches('0').trim_end_matches('.');
            trimmed.to_string()
        } else {
            s
        }
    }
}

/// Parse a format specifier starting after '%'. Returns (FmtSpec, chars consumed).
fn parse_fmt_spec(chars: &[char]) -> Option<(FmtSpec, usize)> {
    let mut i = 0;

    // Parse flags
    let mut flags = String::new();
    while i < chars.len() && "-+ 0#".contains(chars[i]) {
        flags.push(chars[i]);
        i += 1;
    }

    // Parse width
    let mut width = None;
    let width_start = i;
    while i < chars.len() && chars[i].is_ascii_digit() {
        i += 1;
    }
    if i > width_start {
        width = chars[width_start..i]
            .iter()
            .collect::<String>()
            .parse()
            .ok();
    }

    // Parse precision
    let mut precision = None;
    if i < chars.len() && chars[i] == '.' {
        i += 1;
        let prec_start = i;
        while i < chars.len() && chars[i].is_ascii_digit() {
            i += 1;
        }
        precision = Some(
            chars[prec_start..i]
                .iter()
                .collect::<String>()
                .parse()
                .unwrap_or(0),
        );
    }

    // Parse conversion specifier
    if i < chars.len() {
        let specifier = chars[i];
        i += 1;
        Some((
            FmtSpec {
                flags,
                width,
                precision,
                specifier,
            },
            i,
        ))
    } else {
        None
    }
}

/// Format strings using C-style format specifiers.
///
/// @param fmt character scalar: format string with %d, %f, %s, etc.
/// @param ... values to substitute into the format string
/// @return character scalar containing the formatted result
#[builtin(min_args = 1)]
fn builtin_sprintf(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let fmt = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();
    let mut arg_idx = 1;
    let chars: Vec<char> = fmt.chars().collect();
    let mut output = String::new();
    let mut i = 0;

    while i < chars.len() {
        if chars[i] == '%' && i + 1 < chars.len() {
            i += 1;
            // Handle %%
            if chars[i] == '%' {
                output.push('%');
                i += 1;
                continue;
            }
            if let Some((spec, consumed)) = parse_fmt_spec(&chars[i..]) {
                i += consumed;
                match spec.specifier {
                    'd' | 'i' | 'x' | 'X' | 'o' => {
                        let v = args
                            .get(arg_idx)
                            .and_then(|v| v.as_vector()?.as_integer_scalar())
                            .unwrap_or(0);
                        output.push_str(&spec.format_int(v));
                        arg_idx += 1;
                    }
                    'f' | 'e' | 'E' | 'g' | 'G' => {
                        let v = args
                            .get(arg_idx)
                            .and_then(|v| v.as_vector()?.as_double_scalar())
                            .unwrap_or(0.0);
                        output.push_str(&spec.format_float(v));
                        arg_idx += 1;
                    }
                    's' => {
                        let v = args
                            .get(arg_idx)
                            .and_then(|v| v.as_vector()?.as_character_scalar())
                            .unwrap_or_default();
                        output.push_str(&spec.format_str(&v));
                        arg_idx += 1;
                    }
                    _ => {
                        output.push('%');
                        output.push(spec.specifier);
                    }
                }
            }
        } else {
            output.push(chars[i]);
            i += 1;
        }
    }
    Ok(RValue::vec(Vector::Character(vec![Some(output)].into())))
}

// format() is in interp.rs (S3-dispatching interpreter builtin)

/// Split a string by a pattern or fixed delimiter.
///
/// @param x character scalar to split
/// @param split character scalar: pattern or fixed string to split on
/// @param fixed logical: if TRUE, split is a literal string
/// @param ignore.case logical: if TRUE, matching is case-insensitive
/// @return list containing a character vector of the split pieces
#[builtin(min_args = 2)]
fn builtin_strsplit(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let s = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();
    let split = args
        .get(1)
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();
    let (fixed, ignore_case) = get_regex_opts(named);

    let parts: Vec<(Option<String>, RValue)> = if split.is_empty() {
        // Empty split: split into individual characters (same for fixed and regex)
        s.chars()
            .map(|c| {
                (
                    None,
                    RValue::vec(Vector::Character(vec![Some(c.to_string())].into())),
                )
            })
            .collect()
    } else if fixed && !ignore_case {
        // Fixed literal split (no regex), case-sensitive
        vec![(
            None,
            RValue::vec(Vector::Character(
                s.split(&split)
                    .map(|p| Some(p.to_string()))
                    .collect::<Vec<_>>()
                    .into(),
            )),
        )]
    } else {
        // Regex split (or fixed with ignore.case, which uses regex::escape)
        let re = build_regex(&split, fixed, ignore_case)?;
        let pieces: Vec<Option<String>> = re.split(&s).map(|p| Some(p.to_string())).collect();
        vec![(None, RValue::vec(Vector::Character(pieces.into())))]
    };
    Ok(RValue::List(RList::new(parts)))
}

/// Test whether strings start with a given prefix.
///
/// @param x character vector to test
/// @param prefix character scalar: the prefix to look for
/// @return logical vector indicating which elements start with the prefix
#[builtin(name = "startsWith", min_args = 2)]
fn builtin_starts_with(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let prefix = args
        .get(1)
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();
    match args.first() {
        Some(RValue::Vector(rv)) if matches!(rv.inner, Vector::Character(_)) => {
            let Vector::Character(vals) = &rv.inner else {
                unreachable!()
            };
            let result: Vec<Option<bool>> = vals
                .iter()
                .map(|s| s.as_ref().map(|s| s.starts_with(prefix.as_str())))
                .collect();
            Ok(RValue::vec(Vector::Logical(result.into())))
        }
        _ => Err(RError::new(
            RErrorKind::Argument,
            "argument is not character".to_string(),
        )),
    }
}

/// Test whether strings end with a given suffix.
///
/// @param x character vector to test
/// @param suffix character scalar: the suffix to look for
/// @return logical vector indicating which elements end with the suffix
#[builtin(name = "endsWith", min_args = 2)]
fn builtin_ends_with(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let suffix = args
        .get(1)
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();
    match args.first() {
        Some(RValue::Vector(rv)) if matches!(rv.inner, Vector::Character(_)) => {
            let Vector::Character(vals) = &rv.inner else {
                unreachable!()
            };
            let result: Vec<Option<bool>> = vals
                .iter()
                .map(|s| s.as_ref().map(|s| s.ends_with(suffix.as_str())))
                .collect();
            Ok(RValue::vec(Vector::Logical(result.into())))
        }
        _ => Err(RError::new(
            RErrorKind::Argument,
            "argument is not character".to_string(),
        )),
    }
}

/// Translate characters in a string (character-by-character substitution).
///
/// @param old character scalar: characters to replace
/// @param new character scalar: replacement characters (positionally matched)
/// @param x character scalar: string to translate
/// @return character scalar with characters substituted
#[builtin(min_args = 3)]
fn builtin_chartr(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let old = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();
    let new = args
        .get(1)
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();
    let x = args
        .get(2)
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();
    let old_chars: Vec<char> = old.chars().collect();
    let new_chars: Vec<char> = new.chars().collect();
    let result: String = x
        .chars()
        .map(|c| {
            if let Some(pos) = old_chars.iter().position(|&oc| oc == c) {
                new_chars.get(pos).copied().unwrap_or(c)
            } else {
                c
            }
        })
        .collect();
    Ok(RValue::vec(Vector::Character(vec![Some(result)].into())))
}

/// Make syntactically valid R names from character strings.
///
/// @param names character vector of names to sanitize
/// @return character vector of syntactically valid names
#[builtin(min_args = 1)]
fn builtin_make_names(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(rv)) if matches!(rv.inner, Vector::Character(_)) => {
            let Vector::Character(vals) = &rv.inner else {
                unreachable!()
            };
            let result: Vec<Option<String>> = vals
                .iter()
                .map(|s| {
                    s.as_ref().map(|s| {
                        let mut name = String::new();
                        for (i, c) in s.chars().enumerate() {
                            if i == 0 && c.is_ascii_digit() {
                                name.push('X');
                            }
                            if c.is_alphanumeric() || c == '.' || c == '_' {
                                name.push(c);
                            } else {
                                name.push('.');
                            }
                        }
                        if name.is_empty() {
                            name = "X".to_string();
                        }
                        name
                    })
                })
                .collect();
            Ok(RValue::vec(Vector::Character(result.into())))
        }
        _ => Ok(args.first().cloned().unwrap_or(RValue::Null)),
    }
}

/// Make character strings unique by appending sequence numbers to duplicates.
///
/// @param names character vector of names to deduplicate
/// @return character vector with duplicates disambiguated (e.g. "x", "x.1", "x.2")
#[builtin(min_args = 1)]
fn builtin_make_unique(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(rv)) if matches!(rv.inner, Vector::Character(_)) => {
            let Vector::Character(vals) = &rv.inner else {
                unreachable!()
            };
            let mut result = Vec::new();
            let mut counts: HashMap<String, usize> = HashMap::new();
            for v in vals.iter() {
                if let Some(s) = v {
                    let count = counts.entry(s.clone()).or_insert(0);
                    if *count > 0 {
                        result.push(Some(format!("{}.{}", s, count)));
                    } else {
                        result.push(Some(s.clone()));
                    }
                    *count += 1;
                } else {
                    result.push(None);
                }
            }
            Ok(RValue::vec(Vector::Character(result.into())))
        }
        _ => Ok(args.first().cloned().unwrap_or(RValue::Null)),
    }
}

/// Extract the file name from a path.
///
/// @param path character scalar: a file path
/// @return character scalar containing the base file name
#[builtin(min_args = 1)]
fn builtin_basename(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let path = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();
    let base = std::path::Path::new(&path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or(path);
    Ok(RValue::vec(Vector::Character(vec![Some(base)].into())))
}

/// Extract the directory part from a path.
///
/// @param path character scalar: a file path
/// @return character scalar containing the directory component
#[builtin(min_args = 1)]
fn builtin_dirname(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let path = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();
    let dir = std::path::Path::new(&path)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| ".".to_string());
    Ok(RValue::vec(Vector::Character(vec![Some(dir)].into())))
}

/// Convert an R expression or value to its string representation.
///
/// @param expr any R value or language object
/// @return character scalar containing the deparsed representation
#[builtin(min_args = 1)]
fn builtin_deparse(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let s = match args.first() {
        Some(RValue::Language(expr)) => deparse_expr(expr),
        Some(v) => format!("{}", v),
        None => "NULL".to_string(),
    };
    Ok(RValue::vec(Vector::Character(vec![Some(s)].into())))
}

/// Convert integer Unicode code points to a UTF-8 string.
///
/// @param x integer vector of Unicode code points
/// @return character scalar containing the corresponding UTF-8 string
#[builtin(name = "intToUtf8", min_args = 1)]
fn builtin_int_to_utf8(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let ints = match args.first() {
        Some(RValue::Vector(rv)) => rv.to_integers(),
        _ => {
            return Err(RError::new(
                RErrorKind::Argument,
                "argument must be an integer vector".to_string(),
            ))
        }
    };
    let mut result = String::new();
    for val in &ints {
        match val {
            Some(code) if *code >= 0 => match char::from_u32(u32::try_from(*code)?) {
                Some(c) => result.push(c),
                None => {
                    return Err(RError::new(
                        RErrorKind::Argument,
                        format!("invalid Unicode code point: {}", code),
                    ))
                }
            },
            Some(code) => {
                return Err(RError::new(
                    RErrorKind::Argument,
                    format!("invalid Unicode code point: {}", code),
                ))
            }
            None => result.push('\u{FFFD}'), // replacement character for NA
        }
    }
    Ok(RValue::vec(Vector::Character(vec![Some(result)].into())))
}

/// Convert a UTF-8 string to integer Unicode code points.
///
/// @param x character scalar to convert
/// @return integer vector of Unicode code points
#[builtin(name = "utf8ToInt", min_args = 1)]
fn builtin_utf8_to_int(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let s = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "argument must be a single string".to_string(),
            )
        })?;
    let result: Vec<Option<i64>> = s.chars().map(|c| Some(i64::from(u32::from(c)))).collect();
    Ok(RValue::vec(Vector::Integer(result.into())))
}

/// Convert a character string to a raw (byte) vector.
///
/// @param x character scalar to convert
/// @return raw vector of the string's UTF-8 bytes
#[builtin(name = "charToRaw", min_args = 1)]
fn builtin_char_to_raw(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let s = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| RError::new(RErrorKind::Argument, "argument must be a single string"))?;
    let result: Vec<u8> = s.bytes().collect();
    Ok(RValue::vec(Vector::Raw(result)))
}

/// Convert a raw (byte) vector to a character string.
///
/// @param x raw vector to convert
/// @return character scalar containing the UTF-8 string
#[builtin(name = "rawToChar", min_args = 1)]
fn builtin_raw_to_char(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let bytes = match args.first() {
        Some(RValue::Vector(rv)) => rv.inner.to_raw(),
        _ => {
            return Err(RError::new(
                RErrorKind::Argument,
                "argument must be a raw or integer vector",
            ))
        }
    };
    let s = String::from_utf8(bytes).map_err(|e| {
        RError::new(
            RErrorKind::Argument,
            format!("invalid UTF-8 sequence: {}", e),
        )
    })?;
    Ok(RValue::vec(Vector::Character(vec![Some(s)].into())))
}

/// `raw(length)` — create a raw (byte) vector of zeros.
#[builtin(min_args = 1)]
fn builtin_raw(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let n = args
        .first()
        .and_then(|v| v.as_vector()?.as_integer_scalar())
        .ok_or_else(|| RError::new(RErrorKind::Argument, "argument must be a single integer"))?;
    if n < 0 {
        return Err(RError::new(
            RErrorKind::Argument,
            format!("invalid 'length' argument: {}", n),
        ));
    }
    let len = usize::try_from(n)?;
    Ok(RValue::vec(Vector::Raw(vec![0u8; len])))
}

/// `rawShift(x, n)` — bitwise shift of raw (byte) values.
/// Positive n shifts left, negative n shifts right.
#[builtin(name = "rawShift", min_args = 2)]
fn builtin_raw_shift(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let bytes = match args.first() {
        Some(RValue::Vector(rv)) => rv.inner.to_raw(),
        _ => {
            return Err(RError::new(
                RErrorKind::Argument,
                "argument 'x' must be a raw vector",
            ))
        }
    };
    let shift = args
        .get(1)
        .and_then(|v| v.as_vector()?.as_integer_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "argument 'n' must be a single integer",
            )
        })?;
    if !(-8..=8).contains(&shift) {
        return Err(RError::new(
            RErrorKind::Argument,
            format!("shift amount must be between -8 and 8, got {}", shift),
        ));
    }

    let result: Vec<u8> = bytes
        .iter()
        .map(|&byte| {
            if shift >= 0 {
                byte.wrapping_shl(u32::try_from(shift).unwrap_or(0))
            } else {
                byte.wrapping_shr(u32::try_from(-shift).unwrap_or(0))
            }
        })
        .collect();
    Ok(RValue::vec(Vector::Raw(result)))
}

/// `as.raw(x)` — coerce to raw (byte) values (0-255), truncating to lowest byte.
#[builtin(name = "as.raw", min_args = 1)]
fn builtin_as_raw(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(rv)) => Ok(RValue::vec(Vector::Raw(rv.inner.to_raw()))),
        _ => Err(RError::new(
            RErrorKind::Argument,
            "argument must be a vector",
        )),
    }
}

/// `is.raw(x)` — test if argument is a raw vector.
#[builtin(name = "is.raw", min_args = 1)]
fn builtin_is_raw(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let is_raw =
        matches!(args.first(), Some(RValue::Vector(rv)) if matches!(rv.inner, Vector::Raw(_)));
    Ok(RValue::vec(Vector::Logical(vec![Some(is_raw)].into())))
}

/// Convert a glob (wildcard) pattern to a regular expression.
///
/// @param pattern character scalar: a glob pattern with * and ? wildcards
/// @return character scalar containing the equivalent anchored regex
#[builtin(name = "glob2rx", min_args = 1)]
fn builtin_glob2rx(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let pattern = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "argument must be a character string".to_string(),
            )
        })?;
    let mut result = String::from("^");
    for c in pattern.chars() {
        match c {
            '*' => result.push_str(".*"),
            '?' => result.push('.'),
            '.' | '(' | ')' | '+' | '|' | '{' | '}' | '[' | ']' | '^' | '$' | '\\' => {
                result.push('\\');
                result.push(c);
            }
            _ => result.push(c),
        }
    }
    result.push('$');
    Ok(RValue::vec(Vector::Character(vec![Some(result)].into())))
}

/// Match a pattern with capture groups against a character vector.
///
/// @param pattern character scalar: regular expression with optional capture groups
/// @param text character vector to search
/// @param fixed logical: if TRUE, pattern is a literal string
/// @param ignore.case logical: if TRUE, matching is case-insensitive
/// @return list of integer vectors with match and capture-group positions
#[builtin(min_args = 2)]
fn builtin_regexec(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let pattern = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();
    let (fixed, ignore_case) = get_regex_opts(named);
    let re = build_regex(&pattern, fixed, ignore_case)?;
    match args.get(1) {
        Some(RValue::Vector(rv)) if matches!(rv.inner, Vector::Character(_)) => {
            let Vector::Character(vals) = &rv.inner else {
                unreachable!()
            };
            let mut list_items = Vec::new();
            for s in vals.iter() {
                match s.as_ref().and_then(|s| re.captures(s)) {
                    Some(caps) => {
                        let mut positions = Vec::new();
                        let mut lengths = Vec::new();
                        for i in 0..caps.len() {
                            match caps.get(i) {
                                Some(m) => {
                                    positions.push(Some(i64::try_from(m.start())? + 1));
                                    lengths.push(Some(i64::try_from(m.len())?));
                                }
                                None => {
                                    positions.push(Some(-1));
                                    lengths.push(Some(-1));
                                }
                            }
                        }
                        let mut match_rv = RVector::from(Vector::Integer(positions.into()));
                        match_rv.set_attr(
                            "match.length".to_string(),
                            RValue::vec(Vector::Integer(lengths.into())),
                        );
                        list_items.push((None, RValue::Vector(match_rv)));
                    }
                    None => {
                        let mut match_rv = RVector::from(Vector::Integer(vec![Some(-1)].into()));
                        match_rv.set_attr(
                            "match.length".to_string(),
                            RValue::vec(Vector::Integer(vec![Some(-1)].into())),
                        );
                        list_items.push((None, RValue::Vector(match_rv)));
                    }
                }
            }
            Ok(RValue::List(RList::new(list_items)))
        }
        _ => Err(RError::new(
            RErrorKind::Argument,
            "argument is not character".to_string(),
        )),
    }
}

/// Write a deparsed representation of an R object to stdout.
///
/// @param x any R value or language object
/// @return NULL (invisibly); output is printed to stdout
#[builtin(min_args = 1)]
fn builtin_dput(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let s = match args.first() {
        Some(RValue::Language(expr)) => deparse_expr(expr),
        Some(v) => format!("{}", v),
        None => "NULL".to_string(),
    };
    println!("{}", s);
    Ok(RValue::Null)
}

/// Convert a string to an integer using a specified base (radix).
///
/// @param x character scalar: the string to parse
/// @param base integer scalar: the radix (default 10)
/// @return integer scalar, or NA if parsing fails
#[builtin(min_args = 1)]
fn builtin_strtoi(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let x = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();
    let base = named
        .iter()
        .find(|(n, _)| n == "base")
        .and_then(|(_, v)| v.as_vector()?.as_integer_scalar())
        .or_else(|| args.get(1).and_then(|v| v.as_vector()?.as_integer_scalar()))
        .unwrap_or(10);
    let base = u32::try_from(base)?;
    match i64::from_str_radix(x.trim(), base) {
        Ok(n) => Ok(RValue::vec(Vector::Integer(vec![Some(n)].into()))),
        Err(_) => Ok(RValue::vec(Vector::Integer(vec![None].into()))),
    }
}

/// Test whether character strings are non-empty.
///
/// @param x character vector to test
/// @return logical vector: TRUE for non-empty strings, TRUE for NA
#[builtin(min_args = 1)]
fn builtin_nzchar(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(rv)) if matches!(rv.inner, Vector::Character(_)) => {
            let Vector::Character(vals) = &rv.inner else {
                unreachable!()
            };
            let result: Vec<Option<bool>> = vals
                .iter()
                .map(|s| match s {
                    Some(s) => Some(!s.is_empty()),
                    None => Some(true),
                })
                .collect();
            Ok(RValue::vec(Vector::Logical(result.into())))
        }
        Some(val) => {
            let s = val
                .as_vector()
                .and_then(|v| v.as_character_scalar())
                .unwrap_or_default();
            Ok(RValue::vec(Vector::Logical(
                vec![Some(!s.is_empty())].into(),
            )))
        }
        None => Err(RError::new(
            RErrorKind::Argument,
            "argument is missing".to_string(),
        )),
    }
}

/// Wrap strings in single (typographic) quotes.
///
/// @param x character vector to quote
/// @return character vector with elements wrapped in single curly quotes
#[builtin(name = "sQuote", min_args = 1)]
fn builtin_squote(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(rv)) if matches!(rv.inner, Vector::Character(_)) => {
            let Vector::Character(vals) = &rv.inner else {
                unreachable!()
            };
            let result: Vec<Option<String>> = vals
                .iter()
                .map(|s| s.as_ref().map(|s| format!("\u{2018}{}\u{2019}", s)))
                .collect();
            Ok(RValue::vec(Vector::Character(result.into())))
        }
        Some(val) => {
            let s = val
                .as_vector()
                .and_then(|v| v.as_character_scalar())
                .unwrap_or_default();
            Ok(RValue::vec(Vector::Character(
                vec![Some(format!("\u{2018}{}\u{2019}", s))].into(),
            )))
        }
        None => Ok(RValue::vec(Vector::Character(vec![None].into()))),
    }
}

/// Wrap strings in double (typographic) quotes.
///
/// @param x character vector to quote
/// @return character vector with elements wrapped in double curly quotes
#[builtin(name = "dQuote", min_args = 1)]
fn builtin_dquote(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(rv)) if matches!(rv.inner, Vector::Character(_)) => {
            let Vector::Character(vals) = &rv.inner else {
                unreachable!()
            };
            let result: Vec<Option<String>> = vals
                .iter()
                .map(|s| s.as_ref().map(|s| format!("\u{201C}{}\u{201D}", s)))
                .collect();
            Ok(RValue::vec(Vector::Character(result.into())))
        }
        Some(val) => {
            let s = val
                .as_vector()
                .and_then(|v| v.as_character_scalar())
                .unwrap_or_default();
            Ok(RValue::vec(Vector::Character(
                vec![Some(format!("\u{201C}{}\u{201D}", s))].into(),
            )))
        }
        _ => Ok(RValue::vec(Vector::Character(vec![None].into()))),
    }
}

// region: strrep

/// Repeat each element of a character vector a specified number of times.
///
/// @param x a character vector
/// @param times an integer vector of repetition counts (recycled)
/// @return a character vector with each element repeated
#[builtin(min_args = 2)]
fn builtin_strrep(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let x = args.first().and_then(|v| v.as_vector()).ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "strrep() requires a character vector as first argument".to_string(),
        )
    })?;
    let times = args.get(1).and_then(|v| v.as_vector()).ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "strrep() requires an integer 'times' argument".to_string(),
        )
    })?;

    let chars = x.to_characters();
    let ints = times.to_integers();
    let max_len = chars.len().max(ints.len());

    let result: Vec<Option<String>> = (0..max_len)
        .map(|i| {
            let s = &chars[i % chars.len()];
            let n = ints[i % ints.len()];
            match (s, n) {
                (Some(s), Some(n)) => {
                    if n < 0 {
                        None // invalid repetition count
                    } else {
                        Some(s.repeat(usize::try_from(n).unwrap_or(0)))
                    }
                }
                _ => None,
            }
        })
        .collect();

    Ok(RValue::vec(Vector::Character(result.into())))
}

// endregion

// region: formatC

/// Format numbers and strings with C-style formatting control.
///
/// @param x vector to format
/// @param width minimum field width (default 1)
/// @param format format type: "d" (integer), "f" (fixed), "e" (scientific),
///   "g" (general), "s" (string), "x" (hex), "o" (octal), "X" (upper hex)
/// @param flag character: "-" (left-justify), "+" (always sign), "0" (zero-pad),
///   " " (space for positive), "#" (alternate form)
/// @return character vector of formatted values
#[builtin(name = "formatC", min_args = 1)]
fn builtin_format_c(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let x = args.first().ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "formatC() requires at least one argument".to_string(),
        )
    })?;
    let width = named
        .iter()
        .find(|(n, _)| n == "width")
        .and_then(|(_, v)| v.as_vector()?.as_integer_scalar())
        .or_else(|| args.get(1).and_then(|v| v.as_vector()?.as_integer_scalar()))
        .map(|w| usize::try_from(w).unwrap_or(0))
        .unwrap_or(1);
    let format = named
        .iter()
        .find(|(n, _)| n == "format")
        .and_then(|(_, v)| v.as_vector()?.as_character_scalar())
        .unwrap_or_else(|| "g".to_string());
    let flag = named
        .iter()
        .find(|(n, _)| n == "flag")
        .and_then(|(_, v)| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();

    let spec = FmtSpec {
        flags: flag,
        width: if width > 0 { Some(width) } else { None },
        precision: named
            .iter()
            .find(|(n, _)| n == "digits")
            .and_then(|(_, v)| v.as_vector()?.as_integer_scalar())
            .map(|d| usize::try_from(d).unwrap_or(6)),
        specifier: format.chars().next().unwrap_or('g'),
    };

    let rv = x.as_vector().ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "formatC() requires a vector argument".to_string(),
        )
    })?;

    match spec.specifier {
        'd' | 'i' | 'x' | 'X' | 'o' => {
            let ints = rv.to_integers();
            let result: Vec<Option<String>> =
                ints.iter().map(|v| v.map(|i| spec.format_int(i))).collect();
            Ok(RValue::vec(Vector::Character(result.into())))
        }
        'f' | 'e' | 'E' | 'g' | 'G' => {
            let doubles = rv.to_doubles();
            let result: Vec<Option<String>> = doubles
                .iter()
                .map(|v| v.map(|d| spec.format_float(d)))
                .collect();
            Ok(RValue::vec(Vector::Character(result.into())))
        }
        's' => {
            let chars = rv.to_characters();
            let result: Vec<Option<String>> = chars
                .iter()
                .map(|v| v.as_ref().map(|s| spec.format_str(s)))
                .collect();
            Ok(RValue::vec(Vector::Character(result.into())))
        }
        _ => Err(RError::new(
            RErrorKind::Argument,
            format!("formatC(): unsupported format '{}'", format),
        )),
    }
}

// endregion

// region: strtrim

/// Trim character strings to a specified display width.
///
/// @param x character vector to trim
/// @param width integer: maximum display width
/// @return character vector with elements trimmed to width
#[builtin(min_args = 2)]
fn builtin_strtrim(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let width = args
        .get(1)
        .and_then(|v| v.as_vector()?.as_integer_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "strtrim() requires a 'width' argument".to_string(),
            )
        })?;
    let width = usize::try_from(width)?;

    match args.first() {
        Some(RValue::Vector(rv)) if matches!(rv.inner, Vector::Character(_)) => {
            let Vector::Character(vals) = &rv.inner else {
                unreachable!()
            };
            let result: Vec<Option<String>> = vals
                .iter()
                .map(|s| {
                    s.as_ref().map(|s| {
                        let mut current_width = 0;
                        let mut trimmed = String::new();
                        for c in s.chars() {
                            let cw = unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
                            if current_width + cw > width {
                                break;
                            }
                            current_width += cw;
                            trimmed.push(c);
                        }
                        trimmed
                    })
                })
                .collect();
            Ok(RValue::vec(Vector::Character(result.into())))
        }
        _ => Err(RError::new(
            RErrorKind::Argument,
            "strtrim() requires a character vector".to_string(),
        )),
    }
}

// endregion

// region: casefold

/// Convert case of character strings.
///
/// @param x character vector to convert
/// @param upper logical: if TRUE convert to upper case, if FALSE (default) to lower
/// @return character vector with case converted
#[builtin(min_args = 1)]
fn builtin_casefold(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let upper = named
        .iter()
        .find(|(n, _)| n == "upper")
        .and_then(|(_, v)| v.as_vector()?.as_logical_scalar())
        .unwrap_or(false);

    match args.first() {
        Some(RValue::Vector(rv)) if matches!(rv.inner, Vector::Character(_)) => {
            let Vector::Character(vals) = &rv.inner else {
                unreachable!()
            };
            let result: Vec<Option<String>> = vals
                .iter()
                .map(|s| {
                    s.as_ref().map(|s| {
                        if upper {
                            s.to_uppercase()
                        } else {
                            s.to_lowercase()
                        }
                    })
                })
                .collect();
            Ok(RValue::vec(Vector::Character(result.into())))
        }
        _ => Err(RError::new(
            RErrorKind::Argument,
            "argument is not character".to_string(),
        )),
    }
}

// endregion

// region: encodeString

/// Encode character strings with quoting and escaping.
///
/// @param x character vector to encode
/// @param quote character scalar: quote character to wrap strings in (default none)
/// @param na.encode logical: if TRUE (default), encode NA as "NA"
/// @return character vector with encoded strings
#[builtin(name = "encodeString", min_args = 1)]
fn builtin_encode_string(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let quote = named
        .iter()
        .find(|(n, _)| n == "quote")
        .and_then(|(_, v)| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();
    let na_encode = named
        .iter()
        .find(|(n, _)| n == "na.encode")
        .and_then(|(_, v)| v.as_vector()?.as_logical_scalar())
        .unwrap_or(true);

    match args.first() {
        Some(RValue::Vector(rv)) if matches!(rv.inner, Vector::Character(_)) => {
            let Vector::Character(vals) = &rv.inner else {
                unreachable!()
            };
            let result: Vec<Option<String>> = vals
                .iter()
                .map(|s| match s {
                    Some(s) => {
                        let escaped = s
                            .replace('\\', "\\\\")
                            .replace('\n', "\\n")
                            .replace('\r', "\\r")
                            .replace('\t', "\\t");
                        let escaped = if quote == "\"" {
                            escaped.replace('"', "\\\"")
                        } else if quote == "'" {
                            escaped.replace('\'', "\\'")
                        } else {
                            escaped
                        };
                        if quote.is_empty() {
                            Some(escaped)
                        } else {
                            Some(format!("{}{}{}", quote, escaped, quote))
                        }
                    }
                    None => {
                        if na_encode {
                            Some("NA".to_string())
                        } else {
                            None
                        }
                    }
                })
                .collect();
            Ok(RValue::vec(Vector::Character(result.into())))
        }
        _ => Err(RError::new(
            RErrorKind::Argument,
            "argument is not character".to_string(),
        )),
    }
}

// endregion

// region: URLencode / URLdecode

/// Percent-encode a URL string.
///
/// @param URL character scalar: the URL to encode
/// @param reserved logical: if TRUE (default), also encode reserved characters
/// @return character scalar with percent-encoded URL
#[builtin(name = "URLencode", min_args = 1)]
fn builtin_urlencode(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let url = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "URLencode() requires a character string".to_string(),
            )
        })?;
    let reserved = named
        .iter()
        .find(|(n, _)| n == "reserved")
        .and_then(|(_, v)| v.as_vector()?.as_logical_scalar())
        .unwrap_or(false);

    let mut encoded = String::with_capacity(url.len());
    for byte in url.bytes() {
        // Unreserved characters per RFC 3986
        let is_unreserved = byte.is_ascii_alphanumeric()
            || byte == b'-'
            || byte == b'_'
            || byte == b'.'
            || byte == b'~';
        // Reserved characters that URLencode keeps when reserved=FALSE
        let is_reserved = matches!(
            byte,
            b':' | b'/'
                | b'?'
                | b'#'
                | b'['
                | b']'
                | b'@'
                | b'!'
                | b'$'
                | b'&'
                | b'\''
                | b'('
                | b')'
                | b'*'
                | b'+'
                | b','
                | b';'
                | b'='
        );
        if is_unreserved || (!reserved && is_reserved) {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{:02X}", byte));
        }
    }
    Ok(RValue::vec(Vector::Character(vec![Some(encoded)].into())))
}

/// Decode a percent-encoded URL string.
///
/// @param URL character scalar: the percent-encoded URL
/// @return character scalar with decoded URL
#[builtin(name = "URLdecode", min_args = 1)]
fn builtin_urldecode(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let url = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "URLdecode() requires a character string".to_string(),
            )
        })?;

    let mut decoded = Vec::with_capacity(url.len());
    let bytes = url.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = bytes[i + 1];
            let lo = bytes[i + 2];
            if let (Some(h), Some(l)) = (hex_digit(hi), hex_digit(lo)) {
                decoded.push(h << 4 | l);
                i += 3;
                continue;
            }
        }
        decoded.push(bytes[i]);
        i += 1;
    }
    let s = String::from_utf8(decoded).map_err(|e| {
        RError::new(
            RErrorKind::Argument,
            format!("URLdecode(): invalid UTF-8 after decoding: {}", e),
        )
    })?;
    Ok(RValue::vec(Vector::Character(vec![Some(s)].into())))
}

/// Convert a hex ASCII digit to its numeric value.
fn hex_digit(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

// endregion

// region: iconv

/// Convert character encoding (stub: returns input unchanged for UTF-8).
///
/// @param x character vector to convert
/// @param from character scalar: source encoding (default "")
/// @param to character scalar: target encoding (default "")
/// @return character vector with encoding converted
#[builtin(min_args = 1)]
fn builtin_iconv(args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    // Stub: miniR is internally UTF-8, so just return the input unchanged
    match args.first() {
        Some(v @ RValue::Vector(_)) => Ok(v.clone()),
        _ => Ok(RValue::vec(Vector::Character(vec![None].into()))),
    }
}

// endregion

// region: substr<-

/// Replace a substring within a character string.
///
/// This is the replacement function for `substr(x, start, stop) <- value`.
///
/// @param x character scalar: original string
/// @param start integer: starting position (1-indexed)
/// @param stop integer: ending position (inclusive)
/// @param value character scalar: replacement text
/// @return character scalar with the substring replaced
#[builtin(name = "substr<-", names = ["substring<-"], min_args = 4)]
fn builtin_substr_assign(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let s = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();
    let start = usize::try_from(
        args.get(1)
            .and_then(|v| v.as_vector()?.as_integer_scalar())
            .unwrap_or(1),
    )?;
    let stop = usize::try_from(
        args.get(2)
            .and_then(|v| v.as_vector()?.as_integer_scalar())
            .unwrap_or(1),
    )?;
    let value = args
        .get(3)
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();

    let start = start.saturating_sub(1); // R is 1-indexed
    let stop = stop.min(s.len());

    if start >= s.len() || start >= stop {
        return Ok(RValue::vec(Vector::Character(vec![Some(s)].into())));
    }

    // R's substr<- replaces at most (stop - start) characters from value
    let replace_len = stop - start;
    let replacement: String = value.chars().take(replace_len).collect();
    // If replacement is shorter than the range, only replace that many chars
    let actual_replace_len = replacement.len().min(replace_len);

    let mut result = String::with_capacity(s.len());
    result.push_str(&s[..start]);
    result.push_str(&replacement);
    result.push_str(&s[start + actual_replace_len..]);
    Ok(RValue::vec(Vector::Character(vec![Some(result)].into())))
}

// endregion
