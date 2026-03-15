use std::collections::HashMap;

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

#[builtin(min_args = 1)]
fn builtin_sprintf(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    // Very simplified sprintf - handles %d, %f, %s, %e, %g
    let fmt = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();
    let result = fmt.clone();
    let mut arg_idx = 1;

    let mut i = 0;
    let chars: Vec<char> = result.chars().collect();
    let mut output = String::new();

    while i < chars.len() {
        if chars[i] == '%' && i + 1 < chars.len() {
            i += 1;
            // Skip flags, width, precision
            while i < chars.len() && "-+ 0#".contains(chars[i]) {
                i += 1;
            }
            while i < chars.len() && chars[i].is_ascii_digit() {
                i += 1;
            }
            if i < chars.len() && chars[i] == '.' {
                i += 1;
                while i < chars.len() && chars[i].is_ascii_digit() {
                    i += 1;
                }
            }
            if i < chars.len() {
                let spec = chars[i];
                match spec {
                    'd' | 'i' => {
                        if let Some(v) = args
                            .get(arg_idx)
                            .and_then(|v| v.as_vector()?.as_integer_scalar())
                        {
                            output.push_str(&v.to_string());
                        }
                        arg_idx += 1;
                    }
                    'f' | 'e' | 'g' => {
                        if let Some(v) = args
                            .get(arg_idx)
                            .and_then(|v| v.as_vector()?.as_double_scalar())
                        {
                            output.push_str(&format!("{}", v));
                        }
                        arg_idx += 1;
                    }
                    's' => {
                        if let Some(v) = args
                            .get(arg_idx)
                            .and_then(|v| v.as_vector()?.as_character_scalar())
                        {
                            output.push_str(&v);
                        }
                        arg_idx += 1;
                    }
                    '%' => output.push('%'),
                    _ => {
                        output.push('%');
                        output.push(spec);
                    }
                }
                i += 1;
            }
        } else {
            output.push(chars[i]);
            i += 1;
        }
    }
    Ok(RValue::vec(Vector::Character(vec![Some(output)].into())))
}

// format() is in interp.rs (S3-dispatching interpreter builtin)

#[builtin(min_args = 2)]
fn builtin_strsplit(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let s = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();
    let split = args
        .get(1)
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();

    let parts: Vec<(Option<String>, RValue)> = if split.is_empty() {
        s.chars()
            .map(|c| {
                (
                    None,
                    RValue::vec(Vector::Character(vec![Some(c.to_string())].into())),
                )
            })
            .collect()
    } else {
        vec![(
            None,
            RValue::vec(Vector::Character(
                s.split(&split)
                    .map(|p| Some(p.to_string()))
                    .collect::<Vec<_>>()
                    .into(),
            )),
        )]
    };
    Ok(RValue::List(RList::new(parts)))
}

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

#[builtin(min_args = 1)]
fn builtin_deparse(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let s = match args.first() {
        Some(RValue::Language(expr)) => deparse_expr(expr),
        Some(v) => format!("{}", v),
        None => "NULL".to_string(),
    };
    Ok(RValue::vec(Vector::Character(vec![Some(s)].into())))
}

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

#[builtin(name = "charToRaw", min_args = 1)]
fn builtin_char_to_raw(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let s = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| RError::new(RErrorKind::Argument, "argument must be a single string"))?;
    let result: Vec<u8> = s.bytes().collect();
    Ok(RValue::vec(Vector::Raw(result)))
}

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
