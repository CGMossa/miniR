use std::collections::HashMap;

use bstr::ByteSlice;
use memchr::memmem;
use unicode_width::UnicodeWidthStr;

use super::CallArgs;
use crate::interpreter::value::*;
use crate::interpreter::BuiltinContext;
use derive_more::{Display, Error};
use minir_macros::{builtin, interpreter_builtin};
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

// region: memchr fixed-pattern helpers

/// Check whether `haystack` contains `needle` using SIMD-accelerated memchr.
fn fixed_contains(haystack: &str, needle: &str) -> bool {
    memmem::find(haystack.as_bytes(), needle.as_bytes()).is_some()
}

/// Case-insensitive fixed-pattern containment check.
fn fixed_contains_ignorecase(haystack: &str, needle: &str) -> bool {
    let h = haystack.to_lowercase();
    let n = needle.to_lowercase();
    memmem::find(h.as_bytes(), n.as_bytes()).is_some()
}

/// Replace the first occurrence of `needle` in `haystack` with `replacement`.
fn fixed_sub(haystack: &str, needle: &str, replacement: &str) -> String {
    if let Some(pos) = memmem::find(haystack.as_bytes(), needle.as_bytes()) {
        let mut result = String::with_capacity(haystack.len() - needle.len() + replacement.len());
        result.push_str(&haystack[..pos]);
        result.push_str(replacement);
        result.push_str(&haystack[pos + needle.len()..]);
        result
    } else {
        haystack.to_string()
    }
}

/// Replace all occurrences of `needle` in `haystack` with `replacement`.
fn fixed_gsub(haystack: &str, needle: &str, replacement: &str) -> String {
    if needle.is_empty() {
        // Match R behavior for empty pattern: insert replacement before each char and after the last
        let mut result =
            String::with_capacity(haystack.len() + replacement.len() * (haystack.len() + 1));
        for ch in haystack.chars() {
            result.push_str(replacement);
            result.push(ch);
        }
        result.push_str(replacement);
        return result;
    }
    let finder = memmem::Finder::new(needle.as_bytes());
    let mut result = String::with_capacity(haystack.len());
    let mut last_end = 0;
    for pos in finder.find_iter(haystack.as_bytes()) {
        result.push_str(&haystack[last_end..pos]);
        result.push_str(replacement);
        last_end = pos + needle.len();
    }
    result.push_str(&haystack[last_end..]);
    result
}

/// Split `haystack` on all occurrences of `needle`.
fn fixed_split(haystack: &str, needle: &str) -> Vec<Option<String>> {
    if needle.is_empty() {
        // Empty split: split into individual characters (handled by caller)
        return haystack.chars().map(|c| Some(c.to_string())).collect();
    }
    let finder = memmem::Finder::new(needle.as_bytes());
    let mut parts = Vec::new();
    let mut last_end = 0;
    for pos in finder.find_iter(haystack.as_bytes()) {
        parts.push(Some(haystack[last_end..pos].to_string()));
        last_end = pos + needle.len();
    }
    parts.push(Some(haystack[last_end..].to_string()));
    parts
}

// endregion

/// Extract substrings from character strings.
///
/// Vectorized over x, start, and stop with recycling.
///
/// @param x character vector to extract from
/// @param start integer vector of starting positions (1-indexed)
/// @param stop integer vector of ending positions (inclusive)
/// @return character vector containing the substrings
#[builtin(min_args = 3, names = ["substring"])]
fn builtin_substr(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let x_vec = args
        .first()
        .and_then(|v| v.as_vector())
        .map(|v| v.to_characters())
        .unwrap_or_default();
    let start_vec = args
        .get(1)
        .and_then(|v| v.as_vector())
        .map(|v| v.to_integers())
        .unwrap_or_else(|| vec![Some(1)]);
    let stop_vec = args
        .get(2)
        .and_then(|v| v.as_vector())
        .map(|v| v.to_integers())
        .unwrap_or_default();

    if x_vec.is_empty() {
        return Ok(RValue::vec(Vector::Character(vec![].into())));
    }

    let n = x_vec.len().max(start_vec.len()).max(stop_vec.len());
    let result: Vec<Option<String>> = (0..n)
        .map(|i| {
            let s_opt = &x_vec[i % x_vec.len()];
            let start_opt = start_vec[i % start_vec.len()];
            let stop_opt = stop_vec[i % stop_vec.len()];
            match (s_opt, start_opt, stop_opt) {
                (Some(s), Some(start), Some(stop)) => {
                    let start = usize::try_from(start).unwrap_or(0);
                    let stop = usize::try_from(stop).unwrap_or(0);
                    let start = start.saturating_sub(1); // R is 1-indexed
                    if start < s.len() {
                        Some(s[start..stop.min(s.len())].to_string())
                    } else {
                        Some(String::new())
                    }
                }
                (None, _, _) | (_, None, _) | (_, _, None) => None,
            }
        })
        .collect();
    Ok(RValue::vec(Vector::Character(result.into())))
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

    // Fast path: fixed=TRUE without ignore.case uses memchr (SIMD-accelerated)
    if fixed && !ignore_case {
        return match args.get(2) {
            Some(RValue::Vector(rv)) if matches!(rv.inner, Vector::Character(_)) => {
                let Vector::Character(vals) = &rv.inner else {
                    unreachable!()
                };
                let result: Vec<Option<String>> = vals
                    .iter()
                    .map(|s| s.as_ref().map(|s| fixed_gsub(s, &pattern, &replacement)))
                    .collect();
                Ok(RValue::vec(Vector::Character(result.into())))
            }
            _ => Err(RError::new(
                RErrorKind::Argument,
                "argument is not character".to_string(),
            )),
        };
    }

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

    // Fast path: fixed=TRUE without ignore.case uses memchr (SIMD-accelerated)
    if fixed && !ignore_case {
        return match args.get(2) {
            Some(RValue::Vector(rv)) if matches!(rv.inner, Vector::Character(_)) => {
                let Vector::Character(vals) = &rv.inner else {
                    unreachable!()
                };
                let result: Vec<Option<String>> = vals
                    .iter()
                    .map(|s| s.as_ref().map(|s| fixed_sub(s, &pattern, &replacement)))
                    .collect();
                Ok(RValue::vec(Vector::Character(result.into())))
            }
            _ => Err(RError::new(
                RErrorKind::Argument,
                "argument is not character".to_string(),
            )),
        };
    }

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

    // Fast path: fixed=TRUE uses memchr (SIMD-accelerated) instead of regex
    if fixed {
        let contains_fn: Box<dyn Fn(&str) -> bool> = if ignore_case {
            Box::new(|s: &str| fixed_contains_ignorecase(s, &pattern))
        } else {
            Box::new(|s: &str| fixed_contains(s, &pattern))
        };
        return match args.get(1) {
            Some(RValue::Vector(rv)) if matches!(rv.inner, Vector::Character(_)) => {
                let Vector::Character(vals) = &rv.inner else {
                    unreachable!()
                };
                let result: Vec<Option<bool>> = vals
                    .iter()
                    .map(|s| s.as_ref().map(|s| contains_fn(s)))
                    .collect();
                Ok(RValue::vec(Vector::Logical(result.into())))
            }
            _ => Err(RError::new(
                RErrorKind::Argument,
                "argument is not character".to_string(),
            )),
        };
    }

    let re = build_regex(&pattern, fixed, ignore_case)?;
    match args.get(1) {
        Some(RValue::Vector(rv)) if matches!(rv.inner, Vector::Character(_)) => {
            let Vector::Character(vals) = &rv.inner else {
                unreachable!()
            };
            let result: Vec<Option<bool>> = vals
                .iter()
                .map(|s| s.as_ref().map(|s| re.is_match(s)))
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

    // Fast path: fixed=TRUE uses memchr (SIMD-accelerated) instead of regex
    if fixed {
        let contains_fn: Box<dyn Fn(&str) -> bool> = if ignore_case {
            Box::new(|s: &str| fixed_contains_ignorecase(s, &pattern))
        } else {
            Box::new(|s: &str| fixed_contains(s, &pattern))
        };
        return match args.get(1) {
            Some(RValue::Vector(rv)) if matches!(rv.inner, Vector::Character(_)) => {
                let Vector::Character(vals) = &rv.inner else {
                    unreachable!()
                };
                if value {
                    let result: Vec<Option<String>> = vals
                        .iter()
                        .filter(|s| s.as_ref().map(|s| contains_fn(s)).unwrap_or(false))
                        .cloned()
                        .collect();
                    Ok(RValue::vec(Vector::Character(result.into())))
                } else {
                    let result: Result<Vec<Option<i64>>, RError> = vals
                        .iter()
                        .enumerate()
                        .filter(|(_, s)| s.as_ref().map(|s| contains_fn(s)).unwrap_or(false))
                        .map(|(i, _)| Ok(Some(i64::try_from(i)? + 1)))
                        .collect();
                    Ok(RValue::vec(Vector::Integer(result?.into())))
                }
            }
            _ => Err(RError::new(
                RErrorKind::Argument,
                "argument is not character".to_string(),
            )),
        };
    }

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

/// Replace matched substrings using regexpr/gregexpr match data.
///
/// Called as `regmatches(x, m) <- value`. The replacement function receives
/// args `(x, m, value)`:
/// - For regexpr data: value is a character vector of replacements
/// - For gregexpr data: value is a list of character vectors
///
/// @param x character vector that was searched
/// @param m match data from regexpr() or gregexpr()
/// @param value replacement strings
/// @return character vector with matched substrings replaced
#[builtin(name = "regmatches<-", min_args = 3)]
fn builtin_regmatches_assign(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
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
                "argument 'x' is not character".to_string(),
            ))
        }
    };

    let match_data = args.get(1).ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "missing match data argument".to_string(),
        )
    })?;

    let value = args.get(2).ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "missing replacement value".to_string(),
        )
    })?;

    match match_data {
        // regexpr result: single integer vector with match.length attr
        RValue::Vector(rv) if matches!(rv.inner, Vector::Integer(_)) => {
            let Vector::Integer(positions) = &rv.inner else {
                unreachable!()
            };
            let lengths = match rv.get_attr("match.length") {
                Some(RValue::Vector(lv)) => match &lv.inner {
                    Vector::Integer(l) => l.0.clone(),
                    _ => {
                        return Err(RError::new(
                            RErrorKind::Argument,
                            "invalid match data: match.length attribute is not integer".to_string(),
                        ))
                    }
                },
                _ => {
                    return Err(RError::new(
                        RErrorKind::Argument,
                        "invalid match data: missing match.length attribute".to_string(),
                    ))
                }
            };
            let replacements = match value {
                RValue::Vector(rv) => rv.to_characters(),
                _ => {
                    return Err(RError::new(
                        RErrorKind::Argument,
                        "replacement value must be a character vector".to_string(),
                    ))
                }
            };

            let mut result: Vec<Option<String>> = x.to_vec();
            for (i, pos) in positions.iter().enumerate() {
                let p = pos.unwrap_or(-1);
                let l = lengths.get(i).copied().flatten().unwrap_or(-1);
                if p > 0 && l >= 0 {
                    if let Some(Some(s)) = result.get(i) {
                        let start = usize::try_from(p - 1)?;
                        let end = start + usize::try_from(l)?;
                        if end <= s.len() {
                            let repl = replacements
                                .get(i)
                                .and_then(|r| r.as_ref())
                                .map(|r| r.as_str())
                                .unwrap_or("");
                            let new_s = format!("{}{}{}", &s[..start], repl, &s[end..]);
                            result[i] = Some(new_s);
                        }
                    }
                }
            }
            Ok(RValue::vec(Vector::Character(result.into())))
        }
        // gregexpr result: list of integer vectors
        RValue::List(match_list) => {
            let repl_list = match value {
                RValue::List(l) => l,
                _ => {
                    return Err(RError::new(
                        RErrorKind::Argument,
                        "replacement value must be a list for gregexpr match data".to_string(),
                    ))
                }
            };

            let mut result: Vec<Option<String>> = x.to_vec();
            for (i, (_, match_val)) in match_list.values.iter().enumerate() {
                let RValue::Vector(rv) = match_val else {
                    continue;
                };
                let Vector::Integer(positions) = &rv.inner else {
                    continue;
                };
                let lengths = match rv.get_attr("match.length") {
                    Some(RValue::Vector(lv)) => match &lv.inner {
                        Vector::Integer(l) => l.0.clone(),
                        _ => vec![],
                    },
                    _ => vec![],
                };

                let repls: Vec<Option<String>> = match repl_list.values.get(i) {
                    Some((_, RValue::Vector(rv))) => rv.to_characters().to_vec(),
                    _ => vec![],
                };

                if let Some(Some(s)) = result.get(i) {
                    // Collect all match positions and replacements, process from right to left
                    // so earlier positions remain valid after replacement
                    let mut edits: Vec<(usize, usize, &str)> = Vec::new();
                    for (j, pos) in positions.iter().enumerate() {
                        let p = pos.unwrap_or(-1);
                        let l = lengths.get(j).copied().flatten().unwrap_or(-1);
                        if p > 0 && l >= 0 {
                            let start = usize::try_from(p - 1)?;
                            let end = start + usize::try_from(l)?;
                            if end <= s.len() {
                                let repl = repls
                                    .get(j)
                                    .and_then(|r| r.as_ref())
                                    .map(|r| r.as_str())
                                    .unwrap_or("");
                                edits.push((start, end, repl));
                            }
                        }
                    }
                    // Apply edits right-to-left so byte offsets stay valid
                    edits.sort_by(|a, b| b.0.cmp(&a.0));
                    let mut new_s = s.clone();
                    for (start, end, repl) in edits {
                        new_s = format!("{}{}{}", &new_s[..start], repl, &new_s[end..]);
                    }
                    result[i] = Some(new_s);
                }
            }
            Ok(RValue::vec(Vector::Character(result.into())))
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
        let raw = if self.flags.contains('+') && v >= 0 {
            format!("+{}", v)
        } else {
            v.to_string()
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

/// Collect the format specifiers from a format string, returning a list of
/// `(FmtSpec, arg_index)` pairs (0-based among the data args, i.e. excluding fmt).
fn collect_fmt_specs(fmt: &str) -> Vec<(FmtSpec, usize)> {
    let chars: Vec<char> = fmt.chars().collect();
    let mut specs = Vec::new();
    let mut i = 0;
    let mut arg_idx: usize = 0;
    while i < chars.len() {
        if chars[i] == '%' && i + 1 < chars.len() {
            i += 1;
            if chars[i] == '%' {
                i += 1;
                continue;
            }
            if let Some((spec, consumed)) = parse_fmt_spec(&chars[i..]) {
                i += consumed;
                match spec.specifier {
                    'd' | 'i' | 'f' | 'e' | 'E' | 'g' | 'G' | 's' => {
                        specs.push((spec, arg_idx));
                        arg_idx += 1;
                    }
                    _ => {}
                }
            }
        } else {
            i += 1;
        }
    }
    specs
}

/// Format one string from the format template, using element `elem_idx` from
/// each data-arg vector (with recycling).
fn sprintf_one(fmt: &str, data_args: &[&Vector], elem_idx: usize) -> Result<String, RError> {
    let chars: Vec<char> = fmt.chars().collect();
    let mut output = String::new();
    let mut i = 0;
    let mut arg_idx: usize = 0;

    while i < chars.len() {
        if chars[i] == '%' && i + 1 < chars.len() {
            i += 1;
            if chars[i] == '%' {
                output.push('%');
                i += 1;
                continue;
            }
            if let Some((spec, consumed)) = parse_fmt_spec(&chars[i..]) {
                i += consumed;
                match spec.specifier {
                    'd' | 'i' => {
                        let vec = data_args.get(arg_idx).ok_or_else(|| {
                            RError::new(
                                RErrorKind::Argument,
                                format!(
                                    "too few arguments for sprintf format: \
                                     need argument {} but only {} supplied",
                                    arg_idx + 1,
                                    data_args.len()
                                ),
                            )
                        })?;
                        let ints = vec.to_integers();
                        let v = if ints.is_empty() {
                            0
                        } else {
                            ints[elem_idx % ints.len()].unwrap_or(0)
                        };
                        output.push_str(&spec.format_int(v));
                        arg_idx += 1;
                    }
                    'f' | 'e' | 'E' | 'g' | 'G' => {
                        let vec = data_args.get(arg_idx).ok_or_else(|| {
                            RError::new(
                                RErrorKind::Argument,
                                format!(
                                    "too few arguments for sprintf format: \
                                     need argument {} but only {} supplied",
                                    arg_idx + 1,
                                    data_args.len()
                                ),
                            )
                        })?;
                        let doubles = vec.to_doubles();
                        let v = if doubles.is_empty() {
                            0.0
                        } else {
                            doubles[elem_idx % doubles.len()].unwrap_or(0.0)
                        };
                        output.push_str(&spec.format_float(v));
                        arg_idx += 1;
                    }
                    's' => {
                        let vec = data_args.get(arg_idx).ok_or_else(|| {
                            RError::new(
                                RErrorKind::Argument,
                                format!(
                                    "too few arguments for sprintf format: \
                                     need argument {} but only {} supplied",
                                    arg_idx + 1,
                                    data_args.len()
                                ),
                            )
                        })?;
                        let chars_vec = vec.to_characters();
                        let v = if chars_vec.is_empty() {
                            String::new()
                        } else {
                            chars_vec[elem_idx % chars_vec.len()]
                                .clone()
                                .unwrap_or_default()
                        };
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
    Ok(output)
}

/// Format strings using C-style format specifiers, vectorized over arguments.
///
/// In R, `sprintf(fmt, ...)` is vectorized: if any argument is a vector of
/// length > 1, the result is a character vector of the same length (the
/// longest argument), with shorter arguments recycled.
///
/// @param fmt character scalar: format string with %d, %f, %s, etc.
/// @param ... values to substitute into the format string
/// @return character vector containing the formatted results
#[builtin(min_args = 1)]
fn builtin_sprintf(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let fmt_vec = args.first().and_then(|v| v.as_vector()).ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "sprintf requires a character format string".to_string(),
        )
    })?;

    let fmt_chars = fmt_vec.to_characters();

    // If format string is empty, return character(0)
    if fmt_chars.is_empty() {
        return Ok(RValue::vec(Vector::Character(
            Vec::<Option<String>>::new().into(),
        )));
    }

    let fmt = fmt_chars[0].clone().unwrap_or_default();

    // Collect data-arg vectors (args after the format string)
    let data_vecs: Vec<&Vector> = args[1..].iter().filter_map(|a| a.as_vector()).collect();

    // If any data arg has length 0, return character(0) (R behavior)
    if data_vecs.iter().any(|v| v.is_empty()) {
        return Ok(RValue::vec(Vector::Character(
            Vec::<Option<String>>::new().into(),
        )));
    }

    // Determine the output length: max of all data-arg lengths (minimum 1)
    let max_len = data_vecs.iter().map(|v| v.len()).max().unwrap_or(1);

    // If there are no format specifiers that consume args, produce max_len copies
    // (or just 1 if no data args).  But if data_vecs is empty, just format once.
    let specs = collect_fmt_specs(&fmt);
    let output_len = if specs.is_empty() || data_vecs.is_empty() {
        if data_vecs.is_empty() {
            1
        } else {
            max_len
        }
    } else {
        max_len
    };

    let mut results: Vec<Option<String>> = Vec::with_capacity(output_len);
    for elem_idx in 0..output_len {
        results.push(Some(sprintf_one(&fmt, &data_vecs, elem_idx)?));
    }

    Ok(RValue::vec(Vector::Character(results.into())))
}

// format() is in interp.rs (S3-dispatching interpreter builtin)

/// Split strings by a pattern or fixed delimiter.
///
/// Vectorized over x: returns a list with one element per input string.
///
/// @param x character vector to split
/// @param split character scalar: pattern or fixed string to split on
/// @param fixed logical: if TRUE, split is a literal string
/// @param ignore.case logical: if TRUE, matching is case-insensitive
/// @return list of character vectors, one per input string
#[builtin(min_args = 2)]
fn builtin_strsplit(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let x_vec = args
        .first()
        .and_then(|v| v.as_vector())
        .map(|v| v.to_characters())
        .unwrap_or_default();
    let split = args
        .get(1)
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();
    let (fixed, ignore_case) = get_regex_opts(named);

    // Pre-compile regex once if needed
    let re = if !split.is_empty() && (!fixed || ignore_case) {
        Some(build_regex(&split, fixed, ignore_case)?)
    } else {
        None
    };

    let parts: Vec<(Option<String>, RValue)> = x_vec
        .into_iter()
        .map(|s_opt| {
            let elem = match s_opt {
                None => RValue::vec(Vector::Character(vec![None].into())),
                Some(s) => {
                    if split.is_empty() {
                        // Empty split: split into individual characters
                        let chars: Vec<Option<String>> =
                            s.chars().map(|c| Some(c.to_string())).collect();
                        RValue::vec(Vector::Character(chars.into()))
                    } else if fixed && !ignore_case {
                        // Fixed literal split, case-sensitive — memchr (SIMD-accelerated)
                        let pieces = fixed_split(&s, &split);
                        RValue::vec(Vector::Character(pieces.into()))
                    } else {
                        // Regex split
                        let pieces: Vec<Option<String>> = re
                            .as_ref()
                            .unwrap()
                            .split(&s)
                            .map(|p| Some(p.to_string()))
                            .collect();
                        RValue::vec(Vector::Character(pieces.into()))
                    }
                }
            };
            (None, elem)
        })
        .collect();
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

/// Translate characters in strings (character-by-character substitution).
///
/// Vectorized over x.
///
/// @param old character scalar: characters to replace
/// @param new character scalar: replacement characters (positionally matched)
/// @param x character vector: strings to translate
/// @return character vector with characters substituted
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
    let x_vec = args
        .get(2)
        .and_then(|v| v.as_vector())
        .map(|v| v.to_characters())
        .unwrap_or_default();
    let old_chars: Vec<char> = old.chars().collect();
    let new_chars: Vec<char> = new.chars().collect();
    let result: Vec<Option<String>> = x_vec
        .into_iter()
        .map(|s_opt| {
            s_opt.map(|s| {
                s.chars()
                    .map(|c| {
                        if let Some(pos) = old_chars.iter().position(|&oc| oc == c) {
                            new_chars.get(pos).copied().unwrap_or(c)
                        } else {
                            c
                        }
                    })
                    .collect()
            })
        })
        .collect();
    Ok(RValue::vec(Vector::Character(result.into())))
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

/// Extract the file name from paths.
///
/// Vectorized over path.
///
/// @param path character vector of file paths
/// @return character vector containing the base file names
#[builtin(min_args = 1)]
fn builtin_basename(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let path_vec = args
        .first()
        .and_then(|v| v.as_vector())
        .map(|v| v.to_characters())
        .unwrap_or_default();
    let result: Vec<Option<String>> = path_vec
        .into_iter()
        .map(|p_opt| {
            p_opt.map(|p| {
                std::path::Path::new(&p)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or(p)
            })
        })
        .collect();
    Ok(RValue::vec(Vector::Character(result.into())))
}

/// Extract the directory part from paths.
///
/// Vectorized over path.
///
/// @param path character vector of file paths
/// @return character vector containing the directory components
#[builtin(min_args = 1)]
fn builtin_dirname(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let path_vec = args
        .first()
        .and_then(|v| v.as_vector())
        .map(|v| v.to_characters())
        .unwrap_or_default();
    let result: Vec<Option<String>> = path_vec
        .into_iter()
        .map(|p_opt| {
            p_opt.map(|p| {
                std::path::Path::new(&p)
                    .parent()
                    .map(|par| par.to_string_lossy().to_string())
                    .unwrap_or_else(|| ".".to_string())
            })
        })
        .collect();
    Ok(RValue::vec(Vector::Character(result.into())))
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
/// Uses bstr for graceful handling of non-UTF-8 bytes: invalid sequences
/// are replaced with the Unicode replacement character (U+FFFD) rather than
/// producing an error, matching R's tolerant behavior with `rawToChar()`.
///
/// When `multiple = TRUE`, each byte becomes a separate character element.
///
/// @param x raw vector to convert
/// @param multiple logical: if TRUE, return one string per byte (default FALSE)
/// @return character scalar (or vector if multiple=TRUE) containing the string
#[builtin(name = "rawToChar", min_args = 1)]
fn builtin_raw_to_char(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let bytes = match args.first() {
        Some(RValue::Vector(rv)) => rv.inner.to_raw(),
        _ => {
            return Err(RError::new(
                RErrorKind::Argument,
                "argument must be a raw or integer vector",
            ))
        }
    };

    let multiple = named
        .iter()
        .find(|(k, _)| k == "multiple")
        .and_then(|(_, v)| v.as_vector()?.as_logical_scalar())
        .or_else(|| args.get(1).and_then(|v| v.as_vector()?.as_logical_scalar()))
        .unwrap_or(false);

    if multiple {
        // Each byte becomes a separate character element
        let result: Vec<Option<String>> = bytes
            .iter()
            .map(|&b| {
                // Strip embedded NULs: R silently drops \0 from rawToChar output
                if b == 0 {
                    Some(String::new())
                } else {
                    Some(std::slice::from_ref(&b).to_str_lossy().into_owned())
                }
            })
            .collect();
        Ok(RValue::vec(Vector::Character(result.into())))
    } else {
        // Strip NUL bytes (R strips embedded NULs in rawToChar)
        let filtered: Vec<u8> = bytes.into_iter().filter(|&b| b != 0).collect();
        let s = filtered.as_bstr().to_str_lossy().into_owned();
        Ok(RValue::vec(Vector::Character(vec![Some(s)].into())))
    }
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
#[interpreter_builtin(min_args = 1)]
fn interp_dput(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let s = match args.first() {
        Some(RValue::Language(expr)) => deparse_expr(expr),
        Some(v) => format!("{}", v),
        None => "NULL".to_string(),
    };
    context.write(&format!("{}\n", s));
    Ok(RValue::Null)
}

/// Convert strings to integers using a specified base (radix).
///
/// Vectorized over x.
///
/// @param x character vector: the strings to parse
/// @param base integer scalar: the radix (default 10)
/// @return integer vector, NA where parsing fails
#[builtin(min_args = 1)]
fn builtin_strtoi(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let x_vec = args
        .first()
        .and_then(|v| v.as_vector())
        .map(|v| v.to_characters())
        .unwrap_or_default();
    let base = named
        .iter()
        .find(|(n, _)| n == "base")
        .and_then(|(_, v)| v.as_vector()?.as_integer_scalar())
        .or_else(|| args.get(1).and_then(|v| v.as_vector()?.as_integer_scalar()))
        .unwrap_or(10);
    let base = u32::try_from(base)?;
    let result: Vec<Option<i64>> = x_vec
        .into_iter()
        .map(|s_opt| match s_opt {
            None => None,
            Some(s) => i64::from_str_radix(s.trim(), base).ok(),
        })
        .collect();
    Ok(RValue::vec(Vector::Integer(result.into())))
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

/// Formatted printing of numbers and strings, similar to C's printf family.
///
/// @param x numeric or character vector to format
/// @param width integer: minimum field width (default 0, meaning no padding)
/// @param format character: one of "d" (integer), "f" (fixed), "e" (scientific),
///   "g" (general), "s" (string). Default: "g" for numeric, "s" for character.
/// @param flag character: formatting flags — "-" left-justify, "+" always show sign,
///   " " leading space for positive numbers, "0" zero-pad. Default: ""
/// @param digits integer: number of significant or decimal digits (depends on format).
///   Default: 6.
/// @return character vector of formatted values
#[builtin(name = "formatC", min_args = 1)]
fn builtin_format_c(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let x = args.first().and_then(|v| v.as_vector()).ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "formatC() requires a vector as first argument".to_string(),
        )
    })?;

    // Extract named arguments with defaults
    let width: usize = named
        .iter()
        .find(|(k, _)| k == "width")
        .and_then(|(_, v)| v.as_vector()?.as_integer_scalar())
        .and_then(|i| usize::try_from(i).ok())
        .or_else(|| {
            args.get(1)
                .and_then(|v| v.as_vector()?.as_integer_scalar())
                .and_then(|i| usize::try_from(i).ok())
        })
        .unwrap_or(0);

    let default_format = match x {
        Vector::Character(_) => "s",
        _ => "g",
    };
    let format = named
        .iter()
        .find(|(k, _)| k == "format")
        .and_then(|(_, v)| v.as_vector()?.as_character_scalar())
        .unwrap_or_else(|| default_format.to_string());

    let flag = named
        .iter()
        .find(|(k, _)| k == "flag")
        .and_then(|(_, v)| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();

    let digits: usize = named
        .iter()
        .find(|(k, _)| k == "digits")
        .and_then(|(_, v)| v.as_vector()?.as_integer_scalar())
        .and_then(|i| usize::try_from(i).ok())
        .unwrap_or(6);

    let spec = FmtSpec {
        flags: flag,
        width: if width > 0 { Some(width) } else { None },
        precision: Some(digits),
        specifier: format.chars().next().unwrap_or('g'),
    };

    let result: Vec<Option<String>> = match &format[..] {
        "d" => {
            let ints = x.to_integers();
            ints.iter().map(|v| v.map(|i| spec.format_int(i))).collect()
        }
        "f" | "e" | "E" | "g" | "G" => {
            let doubles = x.to_doubles();
            doubles
                .iter()
                .map(|v| v.map(|f| spec.format_float(f)))
                .collect()
        }
        "s" => {
            let chars = x.to_characters();
            chars
                .iter()
                .map(|v| v.as_ref().map(|s| spec.format_str(s)))
                .collect()
        }
        _ => {
            return Err(RError::new(
                RErrorKind::Argument,
                format!(
                    "formatC(): invalid 'format' argument '{}'. \
                     Use one of: \"d\", \"f\", \"e\", \"g\", \"s\"",
                    format
                ),
            ));
        }
    };

    Ok(RValue::vec(Vector::Character(result.into())))
}

// endregion

// region: format.pval

/// Format p-values for display, showing e.g. "< 2.2e-16" for very small values.
///
/// @param pv numeric vector of p-values
/// @param digits integer: number of significant digits (default 3)
/// @param eps numeric: threshold below which to show "< eps" (default 2.220446e-16,
///   i.e. `.Machine$double.eps`)
/// @return character vector of formatted p-values
#[builtin(name = "format.pval", min_args = 1)]
fn builtin_format_pval(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let x = args.first().and_then(|v| v.as_vector()).ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "format.pval() requires a numeric vector".to_string(),
        )
    })?;

    let digits: usize = named
        .iter()
        .find(|(k, _)| k == "digits")
        .and_then(|(_, v)| v.as_vector()?.as_integer_scalar())
        .and_then(|i| usize::try_from(i).ok())
        .unwrap_or(3);

    let eps: f64 = named
        .iter()
        .find(|(k, _)| k == "eps")
        .and_then(|(_, v)| v.as_vector()?.as_double_scalar())
        .unwrap_or(f64::EPSILON);

    let doubles = x.to_doubles();
    let result: Vec<Option<String>> = doubles
        .iter()
        .map(|v| {
            v.map(|pv| {
                if pv.is_nan() {
                    "NaN".to_string()
                } else if pv < eps {
                    format!("< {:.e_digits$e}", eps, e_digits = digits.saturating_sub(1))
                } else if pv > 1.0 - eps {
                    // Near 1.0 — just show the formatted value
                    format!("{:.prec$}", pv, prec = digits)
                } else {
                    format_g(pv, digits, false)
                }
            })
        })
        .collect();

    Ok(RValue::vec(Vector::Character(result.into())))
}

// endregion

// region: prettyNum

/// Format numbers with separators for readability (e.g., thousand separators).
///
/// @param x character vector (or coerced from numeric) to format
/// @param big.mark character: separator inserted every 3 digits before the decimal
///   point (default "")
/// @param small.mark character: separator inserted every 3 digits after the decimal
///   point (default "")
/// @return character vector with separators inserted
#[builtin(name = "prettyNum", min_args = 1)]
fn builtin_pretty_num(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let x = args.first().and_then(|v| v.as_vector()).ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "prettyNum() requires a vector as first argument".to_string(),
        )
    })?;

    let big_mark = named
        .iter()
        .find(|(k, _)| k == "big.mark")
        .and_then(|(_, v)| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();

    let small_mark = named
        .iter()
        .find(|(k, _)| k == "small.mark")
        .and_then(|(_, v)| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();

    let chars = x.to_characters();
    let result: Vec<Option<String>> = chars
        .iter()
        .map(|v| v.as_ref().map(|s| insert_marks(s, &big_mark, &small_mark)))
        .collect();

    Ok(RValue::vec(Vector::Character(result.into())))
}

/// Insert `big_mark` every 3 digits before the decimal point and `small_mark`
/// every 3 digits after the decimal point.
fn insert_marks(s: &str, big_mark: &str, small_mark: &str) -> String {
    if big_mark.is_empty() && small_mark.is_empty() {
        return s.to_string();
    }

    // Split into sign, integer part, and fractional part
    let (sign, rest) = if let Some(stripped) = s.strip_prefix('-') {
        ("-", stripped)
    } else if let Some(stripped) = s.strip_prefix('+') {
        ("+", stripped)
    } else {
        ("", s)
    };

    // Trim leading/trailing whitespace from rest for the number portion
    let rest = rest.trim();

    let (int_part, frac_part) = match rest.find('.') {
        Some(dot) => (&rest[..dot], Some(&rest[dot + 1..])),
        None => (rest, None),
    };

    let mut out = String::with_capacity(s.len() + 10);
    out.push_str(sign);

    // Insert big.mark in integer part (from right to left, every 3 digits)
    if !big_mark.is_empty() && int_part.len() > 3 {
        // Find where digits start (skip leading non-digit chars like spaces)
        let digit_start = int_part.find(|c: char| c.is_ascii_digit()).unwrap_or(0);
        out.push_str(&int_part[..digit_start]);
        let digits = &int_part[digit_start..];
        let len = digits.len();
        for (i, ch) in digits.chars().enumerate() {
            out.push(ch);
            let pos_from_right = len - 1 - i;
            if pos_from_right > 0 && pos_from_right % 3 == 0 {
                out.push_str(big_mark);
            }
        }
    } else {
        out.push_str(int_part);
    }

    // Append fractional part with small.mark
    if let Some(frac) = frac_part {
        out.push('.');
        if !small_mark.is_empty() && frac.len() > 3 {
            for (i, ch) in frac.chars().enumerate() {
                out.push(ch);
                let pos = i + 1;
                if pos < frac.len() && pos % 3 == 0 {
                    out.push_str(small_mark);
                }
            }
        } else {
            out.push_str(frac);
        }
    }

    out
}

// endregion

// region: encoding builtins

/// Report the encoding of character strings.
///
/// Returns "unknown" for pure-ASCII strings, "UTF-8" for non-ASCII.
/// miniR is UTF-8 everywhere, so this simply checks for non-ASCII bytes.
///
/// @param x character vector
/// @return character vector of encoding names
#[builtin(name = "Encoding", min_args = 1)]
fn builtin_encoding(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let chars = match args.first() {
        Some(RValue::Vector(rv)) => rv.to_characters(),
        _ => {
            return Err(RError::new(
                RErrorKind::Argument,
                "argument is not a character vector".to_string(),
            ))
        }
    };
    let result: Vec<Option<String>> = chars
        .iter()
        .map(|s| match s {
            Some(s) => {
                if s.is_ascii() {
                    Some("unknown".to_string())
                } else {
                    Some("UTF-8".to_string())
                }
            }
            None => Some("unknown".to_string()),
        })
        .collect();
    Ok(RValue::vec(Vector::Character(result.into())))
}

/// Convert character strings between encodings.
///
/// Supports conversions between UTF-8, Latin-1 (ISO-8859-1), ASCII, and "bytes".
/// Uses bstr for byte-level manipulation when dealing with non-UTF-8 data.
///
/// When `sub` is provided, it replaces characters that cannot be represented
/// in the target encoding. The special value `sub = "byte"` uses hex `<xx>`
/// escapes (matching R's behavior).
///
/// @param x character vector to convert
/// @param from source encoding name (default: "")
/// @param to target encoding name (default: "")
/// @param sub substitution string for unconvertible characters (default: NA)
/// @return character vector with converted strings
#[builtin(min_args = 1)]
fn builtin_iconv(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let chars = match args.first() {
        Some(RValue::Vector(rv)) => rv.to_characters(),
        _ => {
            return Err(RError::new(
                RErrorKind::Argument,
                "argument is not a character vector".to_string(),
            ))
        }
    };

    let call_args = super::CallArgs::new(args, named);
    let from_enc = call_args
        .optional_string("from", 1)
        .unwrap_or_default()
        .to_uppercase();
    let to_enc = call_args
        .optional_string("to", 2)
        .unwrap_or_default()
        .to_uppercase();
    let sub = call_args.optional_string("sub", 3);

    // Normalize encoding names
    let from_enc = normalize_encoding_name(&from_enc);
    let to_enc = normalize_encoding_name(&to_enc);

    let result: Vec<Option<String>> = chars
        .iter()
        .map(|s| {
            s.as_ref()
                .map(|s| iconv_one(s, &from_enc, &to_enc, sub.as_deref()))
        })
        .collect();

    Ok(RValue::vec(Vector::Character(result.into())))
}

/// Normalize an encoding name to a canonical form.
fn normalize_encoding_name(name: &str) -> String {
    let upper = name.to_uppercase();
    match upper.as_str() {
        "" | "NATIVE" | "NATIVE.ENC" => "UTF-8".to_string(),
        "LATIN1" | "LATIN-1" | "ISO-8859-1" | "ISO8859-1" | "ISO88591" => "LATIN-1".to_string(),
        "UTF8" | "UTF-8" => "UTF-8".to_string(),
        "ASCII" | "US-ASCII" => "ASCII".to_string(),
        "BYTES" => "BYTES".to_string(),
        _ => upper,
    }
}

/// Convert a single string between encodings using bstr for byte-level access.
fn iconv_one(s: &str, from: &str, to: &str, sub: Option<&str>) -> String {
    // If encodings are the same, return as-is
    if from == to {
        return s.to_string();
    }

    // Since miniR strings are always valid UTF-8 internally, "from" is
    // effectively always UTF-8 regardless of what the user claims.
    // We convert FROM UTF-8 TO the target encoding, then back if needed.
    match to {
        "UTF-8" => {
            // Already UTF-8; nothing to do
            s.to_string()
        }
        "ASCII" => {
            // Replace non-ASCII with substitution or lossy replacement
            s.chars()
                .map(|c| {
                    if c.is_ascii() {
                        c.to_string()
                    } else {
                        match sub {
                            Some("byte") => {
                                // Hex escape each UTF-8 byte
                                let mut buf = [0u8; 4];
                                let bytes = c.encode_utf8(&mut buf).as_bytes();
                                bytes.iter().map(|b| format!("<{b:02x}>")).collect()
                            }
                            Some(replacement) => replacement.to_string(),
                            None => String::new(), // R returns NA for unconvertible without sub, but we use empty
                        }
                    }
                })
                .collect()
        }
        "LATIN-1" => {
            // Convert UTF-8 to Latin-1: chars in 0..=255 map directly,
            // others need substitution
            s.chars()
                .map(|c| {
                    if u32::from(c) <= 255 {
                        c.to_string()
                    } else {
                        match sub {
                            Some("byte") => {
                                let mut buf = [0u8; 4];
                                let bytes = c.encode_utf8(&mut buf).as_bytes();
                                bytes.iter().map(|b| format!("<{b:02x}>")).collect()
                            }
                            Some(replacement) => replacement.to_string(),
                            None => String::new(),
                        }
                    }
                })
                .collect()
        }
        "BYTES" => {
            // Convert to hex representation of UTF-8 bytes
            s.as_bytes().iter().map(|b| format!("\\x{b:02x}")).collect()
        }
        _ => {
            // Unsupported target encoding — return with warning-like behavior
            // In R this would produce NA with a warning; we do lossy passthrough
            s.to_string()
        }
    }
}

/// Convert character vector to UTF-8 encoding (passthrough in miniR).
///
/// Since miniR uses UTF-8 everywhere, this is a no-op that returns its input.
///
/// @param x character vector
/// @return character vector (unchanged)
#[builtin(min_args = 1)]
fn builtin_enc2utf8(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(v @ RValue::Vector(_)) => Ok(v.clone()),
        _ => Err(RError::new(
            RErrorKind::Argument,
            "argument is not a character vector".to_string(),
        )),
    }
}

/// Convert character vector to native encoding (passthrough in miniR).
///
/// Since miniR uses UTF-8 everywhere, this is a no-op that returns its input.
///
/// @param x character vector
/// @return character vector (unchanged)
#[builtin(min_args = 1)]
fn builtin_enc2native(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(v @ RValue::Vector(_)) => Ok(v.clone()),
        _ => Err(RError::new(
            RErrorKind::Argument,
            "argument is not a character vector".to_string(),
        )),
    }
}

// endregion

// region: strtrim

/// Trim character strings to a specified display width.
///
/// Truncates each string to at most `width` display columns. Multi-byte
/// characters and wide characters (CJK) are measured by their terminal width.
///
/// @param x character vector
/// @param width integer vector of maximum widths (recycled)
/// @return character vector of trimmed strings
#[builtin(min_args = 2)]
fn builtin_strtrim(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let chars = match args.first() {
        Some(RValue::Vector(rv)) => rv.to_characters(),
        _ => {
            return Err(RError::new(
                RErrorKind::Argument,
                "non-character argument".to_string(),
            ))
        }
    };
    let widths = match args.get(1) {
        Some(RValue::Vector(rv)) => rv.to_doubles(),
        _ => {
            return Err(RError::new(
                RErrorKind::Argument,
                "'width' must be numeric".to_string(),
            ))
        }
    };
    if widths.is_empty() {
        return Err(RError::new(
            RErrorKind::Argument,
            "invalid 'width' argument — must be a positive number".to_string(),
        ));
    }

    let result: Vec<Option<String>> = chars
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let w = widths[i % widths.len()];
            match (s, w) {
                (Some(s), Some(w)) => {
                    let max_w = w.max(0.0) as usize;
                    Some(trim_to_width(s, max_w))
                }
                (None, _) => None,
                (_, None) => None,
            }
        })
        .collect();
    Ok(RValue::vec(Vector::Character(result.into())))
}

/// Trim a string to at most `max_width` display columns.
fn trim_to_width(s: &str, max_width: usize) -> String {
    use unicode_width::UnicodeWidthChar;
    let mut result = String::new();
    let mut current_width = 0;
    for ch in s.chars() {
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if current_width + ch_width > max_width {
            break;
        }
        result.push(ch);
        current_width += ch_width;
    }
    result
}

// endregion

// region: URLencode, URLdecode, casefold, encodeString, substr<-

/// Percent-encode URL strings per RFC 3986.
///
/// Vectorized over URL.
///
/// @param URL character vector of strings to encode
/// @param reserved if TRUE (default), also encode reserved characters
/// @return character vector of percent-encoded strings
#[builtin(name = "URLencode", min_args = 1, namespace = "utils")]
fn builtin_urlencode(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let url_vec = args
        .first()
        .and_then(|v| v.as_vector())
        .map(|v| v.to_characters())
        .unwrap_or_default();
    let reserved = named
        .iter()
        .find(|(k, _)| k == "reserved")
        .and_then(|(_, v)| v.as_vector()?.as_logical_scalar())
        .unwrap_or(true);

    let unreserved = |b: u8| -> bool {
        b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.' || b == b'~'
    };
    let is_reserved = |b: u8| -> bool {
        matches!(
            b,
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
        )
    };

    let result: Vec<Option<String>> = url_vec
        .into_iter()
        .map(|s_opt| {
            s_opt.map(|s| {
                let mut encoded = String::new();
                for &b in s.as_bytes() {
                    if unreserved(b) || (!reserved && is_reserved(b)) {
                        encoded.push(char::from(b));
                    } else {
                        encoded.push_str(&format!("%{:02X}", b));
                    }
                }
                encoded
            })
        })
        .collect();
    Ok(RValue::vec(Vector::Character(result.into())))
}

/// Decode percent-encoded URL strings.
///
/// Vectorized over URL.
///
/// @param URL character vector of percent-encoded strings
/// @return character vector of decoded strings
#[builtin(name = "URLdecode", min_args = 1, namespace = "utils")]
fn builtin_urldecode(args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    let url_vec = args
        .first()
        .and_then(|v| v.as_vector())
        .map(|v| v.to_characters())
        .unwrap_or_default();

    let result: Vec<Option<String>> = url_vec
        .into_iter()
        .map(|s_opt| {
            s_opt.map(|s| {
                let mut bytes = Vec::new();
                let mut chars = s.bytes();
                while let Some(b) = chars.next() {
                    if b == b'%' {
                        let hi = chars.next().unwrap_or(b'0');
                        let lo = chars.next().unwrap_or(b'0');
                        let hex = [hi, lo];
                        if let Ok(val) =
                            u8::from_str_radix(std::str::from_utf8(&hex).unwrap_or("00"), 16)
                        {
                            bytes.push(val);
                        }
                    } else if b == b'+' {
                        bytes.push(b' ');
                    } else {
                        bytes.push(b);
                    }
                }
                String::from_utf8_lossy(&bytes).into_owned()
            })
        })
        .collect();
    Ok(RValue::vec(Vector::Character(result.into())))
}

/// Convert case of a character vector.
///
/// @param x character vector
/// @param upper if TRUE, convert to uppercase; if FALSE (default), to lowercase
/// @return character vector with converted case
#[builtin(min_args = 1)]
fn builtin_casefold(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let upper = named
        .iter()
        .find(|(k, _)| k == "upper")
        .and_then(|(_, v)| v.as_vector()?.as_logical_scalar())
        .unwrap_or(false);

    match args.first() {
        Some(RValue::Vector(rv)) => {
            let result: Vec<Option<String>> = rv
                .to_characters()
                .into_iter()
                .map(|opt| {
                    opt.map(|s| {
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
        _ => Ok(RValue::Null),
    }
}

/// Encode a character string with optional quoting and escape handling.
///
/// @param x character vector
/// @param quote quote character to wrap each string in (default none)
/// @param na.encode if TRUE (default), encode NA as "NA"
/// @return character vector with encoded strings
#[builtin(name = "encodeString", min_args = 1)]
fn builtin_encode_string(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let quote = named
        .iter()
        .find(|(k, _)| k == "quote")
        .and_then(|(_, v)| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();

    match args.first() {
        Some(RValue::Vector(rv)) => {
            let result: Vec<Option<String>> = rv
                .to_characters()
                .into_iter()
                .map(|opt| {
                    opt.map(|s| {
                        let escaped = s
                            .replace('\\', "\\\\")
                            .replace('\n', "\\n")
                            .replace('\r', "\\r")
                            .replace('\t', "\\t");
                        let escaped = if !quote.is_empty() {
                            escaped.replace(&quote, &format!("\\{quote}"))
                        } else {
                            escaped
                        };
                        if quote.is_empty() {
                            escaped
                        } else {
                            format!("{quote}{escaped}{quote}")
                        }
                    })
                })
                .collect();
            Ok(RValue::vec(Vector::Character(result.into())))
        }
        _ => Ok(RValue::Null),
    }
}

/// Replace substrings in character strings.
///
/// Vectorized over x, with start, stop, and value recycled.
///
/// @param x character vector (modified in place conceptually)
/// @param start integer vector of start positions (1-based)
/// @param stop integer vector of stop positions (1-based)
/// @param value character vector of replacement strings
/// @return character vector with substrings replaced
#[builtin(name = "substr<-", min_args = 4)]
fn builtin_substr_assign(args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    let x_vec = args
        .first()
        .and_then(|v| v.as_vector())
        .map(|v| v.to_characters())
        .unwrap_or_default();
    let start_vec = args
        .get(1)
        .and_then(|v| v.as_vector())
        .map(|v| v.to_integers())
        .unwrap_or_else(|| vec![Some(1)]);
    let stop_vec = args
        .get(2)
        .and_then(|v| v.as_vector())
        .map(|v| v.to_integers())
        .unwrap_or_else(|| vec![Some(1)]);
    let value_vec = args
        .get(3)
        .and_then(|v| v.as_vector())
        .map(|v| v.to_characters())
        .unwrap_or_default();

    if x_vec.is_empty() {
        return Ok(RValue::vec(Vector::Character(vec![].into())));
    }

    let n = x_vec.len();
    let result: Vec<Option<String>> = (0..n)
        .map(|i| {
            let x_opt = &x_vec[i];
            let start_opt = if start_vec.is_empty() {
                Some(1)
            } else {
                start_vec[i % start_vec.len()]
            };
            let stop_opt = if stop_vec.is_empty() {
                Some(1)
            } else {
                stop_vec[i % stop_vec.len()]
            };
            let value_opt = if value_vec.is_empty() {
                None
            } else {
                value_vec[i % value_vec.len()].clone()
            };

            match (x_opt, start_opt, stop_opt, value_opt) {
                (Some(x), Some(start_i), Some(stop_i), Some(value)) => {
                    let start = usize::try_from(start_i).unwrap_or(0);
                    let stop = usize::try_from(stop_i).unwrap_or(0);
                    let chars: Vec<char> = x.chars().collect();
                    let start = start.saturating_sub(1).min(chars.len());
                    let stop = stop.min(chars.len());
                    let range_len = stop.saturating_sub(start);
                    let repl_chars: Vec<char> = value.chars().take(range_len).collect();

                    let mut result: Vec<char> = chars[..start].to_vec();
                    result.extend(&repl_chars);
                    if repl_chars.len() < range_len {
                        result.extend(&chars[start + repl_chars.len()..stop]);
                    }
                    result.extend(&chars[stop..]);

                    Some(result.into_iter().collect())
                }
                (None, _, _, _) | (_, None, _, _) | (_, _, None, _) | (_, _, _, None) => None,
            }
        })
        .collect();

    Ok(RValue::vec(Vector::Character(result.into())))
}

// endregion

// region: strwrap

/// Wrap character strings to a specified width.
///
/// Breaks long strings into lines of at most `width` characters. Supports
/// first-line indentation (`indent`) and subsequent-line indentation (`exdent`).
///
/// @param x character vector to wrap
/// @param width target line width (default: 0.9 * getOption("width"))
/// @param indent indentation of first line (default: 0)
/// @param exdent indentation of subsequent lines (default: 0)
/// @param prefix prefix for each line (default: "")
/// @param simplify if TRUE return a character vector, if FALSE return a list
/// @return character vector (or list) of wrapped strings
/// @namespace base
#[builtin(min_args = 1)]
fn builtin_strwrap(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);

    let x = match call_args.value("x", 0) {
        Some(v) => match v.as_vector() {
            Some(v) => v.to_characters(),
            None => {
                return Err(RError::new(
                    RErrorKind::Argument,
                    "invalid 'x' argument".to_string(),
                ))
            }
        },
        None => {
            return Err(RError::new(
                RErrorKind::Argument,
                "argument 'x' is missing".to_string(),
            ))
        }
    };

    let width = call_args.integer_or("width", 1, 80) as usize;
    let indent = call_args.integer_or("indent", 2, 0) as usize;
    let exdent = call_args.integer_or("exdent", 3, 0) as usize;
    let prefix = call_args.optional_string("prefix", 4).unwrap_or_default();
    let initial = call_args
        .optional_string("initial", 5)
        .unwrap_or_else(|| prefix.clone());
    let simplify = call_args.logical_flag("simplify", 6, true);

    let indent_str = " ".repeat(indent);
    let exdent_str = " ".repeat(exdent);

    let mut all_lines: Vec<Vec<Option<String>>> = Vec::new();
    for s_opt in &x {
        match s_opt {
            None => all_lines.push(vec![None]),
            Some(s) => {
                let effective_width = width.saturating_sub(initial.len() + indent);
                if effective_width == 0 {
                    all_lines.push(vec![Some(format!("{initial}{indent_str}{s}"))]);
                    continue;
                }
                let wrapped = textwrap::wrap(s, effective_width);
                let mut lines = Vec::new();
                for (i, line) in wrapped.iter().enumerate() {
                    if i == 0 {
                        lines.push(Some(format!("{initial}{indent_str}{line}")));
                    } else {
                        let ew = width.saturating_sub(prefix.len() + exdent);
                        let rewrapped = if ew > 0 && line.len() > ew {
                            textwrap::wrap(line, ew)
                        } else {
                            vec![std::borrow::Cow::Borrowed(line.as_ref())]
                        };
                        for subline in rewrapped {
                            lines.push(Some(format!("{prefix}{exdent_str}{subline}")));
                        }
                    }
                }
                if lines.is_empty() {
                    lines.push(Some(format!("{initial}{indent_str}")));
                }
                all_lines.push(lines);
            }
        }
    }

    if simplify {
        let flat: Vec<Option<String>> = all_lines.into_iter().flatten().collect();
        Ok(RValue::vec(Vector::Character(flat.into())))
    } else {
        let list_vals: Vec<(Option<String>, RValue)> = all_lines
            .into_iter()
            .map(|lines| (None, RValue::vec(Vector::Character(lines.into()))))
            .collect();
        Ok(RValue::List(RList::new(list_vals)))
    }
}

// endregion

// region: CRAN-compat string builtins (toString, gettext, type.convert, locale)

/// Collapse a vector into a single comma-separated string.
///
/// @param x vector to collapse
/// @param sep separator (default ", ")
/// @return character scalar
/// @namespace base
#[builtin(name = "toString", min_args = 1)]
fn builtin_to_string(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let sep = call_args
        .optional_string("sep", 1)
        .unwrap_or_else(|| ", ".to_string());

    let chars = match args.first() {
        Some(RValue::Vector(rv)) => rv.to_characters(),
        Some(RValue::Null) => Vec::new(),
        Some(RValue::List(l)) => l
            .values
            .iter()
            .map(|(_, v)| v.as_vector().and_then(|vec| vec.as_character_scalar()))
            .collect(),
        _ => vec![],
    };

    let parts: Vec<String> = chars.into_iter().flatten().collect();
    Ok(RValue::vec(Vector::Character(
        vec![Some(parts.join(&sep))].into(),
    )))
}

/// Translate a message (i18n stub — returns the message unchanged).
///
/// @param ... character strings to return
/// @param domain translation domain (ignored)
/// @return character vector
/// @namespace base
#[builtin(min_args = 1)]
fn builtin_gettext(args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    // Stub: just return the first argument unchanged
    match args.first() {
        Some(v) => Ok(v.clone()),
        None => Ok(RValue::vec(Vector::Character(
            vec![Some(String::new())].into(),
        ))),
    }
}

/// Translate a message with singular/plural forms (i18n stub).
///
/// @param n count for plural selection
/// @param msg1 singular message
/// @param msg2 plural message
/// @return character scalar
/// @namespace base
#[builtin(min_args = 3)]
fn builtin_ngettext(args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    let n = args
        .first()
        .and_then(|v| v.as_vector()?.as_integer_scalar())
        .unwrap_or(1);
    let msg = if n == 1 { args.get(1) } else { args.get(2) };
    match msg {
        Some(v) => Ok(v.clone()),
        None => Ok(RValue::vec(Vector::Character(
            vec![Some(String::new())].into(),
        ))),
    }
}

/// Format and translate a message (i18n stub — delegates to sprintf).
///
/// @param fmt format string
/// @param ... format arguments
/// @return character scalar
/// @namespace base
#[builtin(min_args = 1)]
fn builtin_gettextf(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    // Delegate to sprintf
    builtin_sprintf(args, named)
}

/// Auto-convert character vector to appropriate type.
///
/// @param x character vector to convert
/// @param as.is if TRUE, don't convert to factor (default TRUE in miniR)
/// @return converted vector (numeric, integer, logical, or character)
/// @namespace utils
#[builtin(name = "type.convert", min_args = 1)]
fn builtin_type_convert(args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    let chars = match args.first() {
        Some(RValue::Vector(rv)) => rv.to_characters(),
        _ => return Ok(args.first().cloned().unwrap_or(RValue::Null)),
    };

    // Try logical first
    let all_logical = chars
        .iter()
        .all(|c| matches!(c.as_deref(), Some("TRUE" | "FALSE" | "T" | "F") | None));
    if all_logical && !chars.is_empty() {
        let vals: Vec<Option<bool>> = chars
            .iter()
            .map(|c| match c.as_deref() {
                Some("TRUE") | Some("T") => Some(true),
                Some("FALSE") | Some("F") => Some(false),
                _ => None,
            })
            .collect();
        return Ok(RValue::vec(Vector::Logical(vals.into())));
    }

    // Try integer
    let all_int = chars.iter().all(|c| match c {
        None => true,
        Some(s) => s.parse::<i64>().is_ok(),
    });
    if all_int && !chars.is_empty() {
        let vals: Vec<Option<i64>> = chars
            .iter()
            .map(|c| match c {
                None => None,
                Some(s) => s.parse::<i64>().ok(),
            })
            .collect();
        return Ok(RValue::vec(Vector::Integer(vals.into())));
    }

    // Try double
    let all_double = chars.iter().all(|c| match c {
        None => true,
        Some(s) => s.parse::<f64>().is_ok() || s == "NA" || s == "NaN" || s == "Inf" || s == "-Inf",
    });
    if all_double && !chars.is_empty() {
        let vals: Vec<Option<f64>> = chars
            .iter()
            .map(|c| match c {
                None => None,
                Some(s) => match s.as_str() {
                    "NA" => None,
                    "NaN" => Some(f64::NAN),
                    "Inf" => Some(f64::INFINITY),
                    "-Inf" => Some(f64::NEG_INFINITY),
                    _ => s.parse::<f64>().ok(),
                },
            })
            .collect();
        return Ok(RValue::vec(Vector::Double(vals.into())));
    }

    // Keep as character
    Ok(args.first().cloned().unwrap_or(RValue::Null))
}

/// Get the current locale setting (stub — returns "C" locale).
///
/// @param category locale category (default "LC_ALL")
/// @return character scalar with locale name
/// @namespace base
#[builtin(name = "Sys.getlocale", min_args = 0)]
fn builtin_sys_getlocale(_args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    Ok(RValue::vec(Vector::Character(
        vec![Some("C".to_string())].into(),
    )))
}

/// Set the locale (stub — accepts but ignores the setting).
///
/// @param category locale category
/// @param locale locale string
/// @return character scalar with previous locale
/// @namespace base
#[builtin(name = "Sys.setlocale", min_args = 0)]
fn builtin_sys_setlocale(_args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    Ok(RValue::vec(Vector::Character(
        vec![Some("C".to_string())].into(),
    )))
}

// endregion
