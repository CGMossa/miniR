use std::collections::HashMap;

use crate::interpreter::value::*;
use linkme::distributed_slice;
use newr_macros::builtin;

#[distributed_slice(crate::interpreter::builtins::BUILTIN_REGISTRY)]
static ALIAS_SUBSTRING: (&str, crate::interpreter::builtins::BuiltinFn, usize) =
    ("substring", builtin_substr, 2);

#[builtin(min_args = 3)]
fn builtin_substr(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let s = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();
    let start = args
        .get(1)
        .and_then(|v| v.as_vector()?.as_integer_scalar())
        .unwrap_or(1) as usize;
    let stop = args
        .get(2)
        .and_then(|v| v.as_vector()?.as_integer_scalar())
        .unwrap_or(s.len() as i64) as usize;
    let start = start.saturating_sub(1); // R is 1-indexed
    let result = if start < s.len() {
        s[start..stop.min(s.len())].to_string()
    } else {
        String::new()
    };
    Ok(RValue::Vector(Vector::Character(vec![Some(result)])))
}

#[builtin(min_args = 1)]
fn builtin_toupper(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(Vector::Character(vals))) => {
            let result: Vec<Option<String>> = vals
                .iter()
                .map(|s| s.as_ref().map(|s| s.to_uppercase()))
                .collect();
            Ok(RValue::Vector(Vector::Character(result)))
        }
        _ => Err(RError::Argument("argument is not character".to_string())),
    }
}

#[builtin(min_args = 1)]
fn builtin_tolower(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(Vector::Character(vals))) => {
            let result: Vec<Option<String>> = vals
                .iter()
                .map(|s| s.as_ref().map(|s| s.to_lowercase()))
                .collect();
            Ok(RValue::Vector(Vector::Character(result)))
        }
        _ => Err(RError::Argument("argument is not character".to_string())),
    }
}

#[builtin(min_args = 1)]
fn builtin_trimws(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(Vector::Character(vals))) => {
            let result: Vec<Option<String>> = vals
                .iter()
                .map(|s| s.as_ref().map(|s| s.trim().to_string()))
                .collect();
            Ok(RValue::Vector(Vector::Character(result)))
        }
        _ => Err(RError::Argument("argument is not character".to_string())),
    }
}

#[builtin(min_args = 3)]
fn builtin_gsub(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let pattern = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();
    let replacement = args
        .get(1)
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();
    match args.get(2) {
        Some(RValue::Vector(Vector::Character(vals))) => {
            let result: Vec<Option<String>> = vals
                .iter()
                .map(|s| s.as_ref().map(|s| s.replace(&pattern, &replacement)))
                .collect();
            Ok(RValue::Vector(Vector::Character(result)))
        }
        _ => Err(RError::Argument("argument is not character".to_string())),
    }
}

#[builtin(min_args = 3)]
fn builtin_sub(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let pattern = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();
    let replacement = args
        .get(1)
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();
    match args.get(2) {
        Some(RValue::Vector(Vector::Character(vals))) => {
            let result: Vec<Option<String>> = vals
                .iter()
                .map(|s| {
                    s.as_ref().map(|s| {
                        if let Some(pos) = s.find(&pattern) {
                            format!("{}{}{}", &s[..pos], replacement, &s[pos + pattern.len()..])
                        } else {
                            s.clone()
                        }
                    })
                })
                .collect();
            Ok(RValue::Vector(Vector::Character(result)))
        }
        _ => Err(RError::Argument("argument is not character".to_string())),
    }
}

#[builtin(min_args = 2)]
fn builtin_grepl(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let pattern = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();
    match args.get(1) {
        Some(RValue::Vector(Vector::Character(vals))) => {
            let result: Vec<Option<bool>> = vals
                .iter()
                .map(|s| Some(s.as_ref().map(|s| s.contains(&pattern)).unwrap_or(false)))
                .collect();
            Ok(RValue::Vector(Vector::Logical(result)))
        }
        _ => Err(RError::Argument("argument is not character".to_string())),
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

    match args.get(1) {
        Some(RValue::Vector(Vector::Character(vals))) => {
            if value {
                let result: Vec<Option<String>> = vals
                    .iter()
                    .filter(|s| s.as_ref().map(|s| s.contains(&pattern)).unwrap_or(false))
                    .cloned()
                    .collect();
                Ok(RValue::Vector(Vector::Character(result)))
            } else {
                let result: Vec<Option<i64>> = vals
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| s.as_ref().map(|s| s.contains(&pattern)).unwrap_or(false))
                    .map(|(i, _)| Some(i as i64 + 1))
                    .collect();
                Ok(RValue::Vector(Vector::Integer(result)))
            }
        }
        _ => Err(RError::Argument("argument is not character".to_string())),
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
    Ok(RValue::Vector(Vector::Character(vec![Some(output)])))
}

#[builtin(min_args = 1)]
fn builtin_format(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(val) => Ok(RValue::Vector(Vector::Character(vec![Some(format!(
            "{}",
            val
        ))]))),
        None => Ok(RValue::Vector(Vector::Character(vec![Some(String::new())]))),
    }
}

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
                    RValue::Vector(Vector::Character(vec![Some(c.to_string())])),
                )
            })
            .collect()
    } else {
        vec![(
            None,
            RValue::Vector(Vector::Character(
                s.split(&split).map(|p| Some(p.to_string())).collect(),
            )),
        )]
    };
    Ok(RValue::List(RList::new(parts)))
}

#[builtin(name = "startsWith", min_args = 2)]
fn builtin_starts_with(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let x = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();
    let prefix = args
        .get(1)
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();
    Ok(RValue::Vector(Vector::Logical(vec![Some(
        x.starts_with(&prefix),
    )])))
}

#[builtin(name = "endsWith", min_args = 2)]
fn builtin_ends_with(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let x = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();
    let suffix = args
        .get(1)
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .unwrap_or_default();
    Ok(RValue::Vector(Vector::Logical(vec![Some(
        x.ends_with(&suffix),
    )])))
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
    Ok(RValue::Vector(Vector::Character(vec![Some(result)])))
}

#[builtin(min_args = 1)]
fn builtin_make_names(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(Vector::Character(vals))) => {
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
            Ok(RValue::Vector(Vector::Character(result)))
        }
        _ => Ok(args.first().cloned().unwrap_or(RValue::Null)),
    }
}

#[builtin(min_args = 1)]
fn builtin_make_unique(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(Vector::Character(vals))) => {
            let mut result = Vec::new();
            let mut counts: HashMap<String, usize> = HashMap::new();
            for v in vals {
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
            Ok(RValue::Vector(Vector::Character(result)))
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
    Ok(RValue::Vector(Vector::Character(vec![Some(base)])))
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
    Ok(RValue::Vector(Vector::Character(vec![Some(dir)])))
}

#[builtin(min_args = 1)]
fn builtin_deparse(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let s = args
        .first()
        .map(|v| format!("{}", v))
        .unwrap_or_else(|| "NULL".to_string());
    Ok(RValue::Vector(Vector::Character(vec![Some(s)])))
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
        .unwrap_or(10) as u32;
    match i64::from_str_radix(x.trim(), base) {
        Ok(n) => Ok(RValue::Vector(Vector::Integer(vec![Some(n)]))),
        Err(_) => Ok(RValue::Vector(Vector::Integer(vec![None]))),
    }
}

#[builtin(min_args = 1)]
fn builtin_nzchar(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(Vector::Character(vals))) => {
            let result: Vec<Option<bool>> = vals
                .iter()
                .map(|s| match s {
                    Some(s) => Some(!s.is_empty()),
                    None => Some(true),
                })
                .collect();
            Ok(RValue::Vector(Vector::Logical(result)))
        }
        Some(val) => {
            let s = val
                .as_vector()
                .and_then(|v| v.as_character_scalar())
                .unwrap_or_default();
            Ok(RValue::Vector(Vector::Logical(vec![Some(!s.is_empty())])))
        }
        None => Err(RError::Argument("argument is missing".to_string())),
    }
}

#[builtin(name = "sQuote", min_args = 1)]
fn builtin_squote(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(Vector::Character(vals))) => {
            let result: Vec<Option<String>> = vals
                .iter()
                .map(|s| s.as_ref().map(|s| format!("\u{2018}{}\u{2019}", s)))
                .collect();
            Ok(RValue::Vector(Vector::Character(result)))
        }
        Some(val) => {
            let s = val
                .as_vector()
                .and_then(|v| v.as_character_scalar())
                .unwrap_or_default();
            Ok(RValue::Vector(Vector::Character(vec![Some(format!(
                "\u{2018}{}\u{2019}",
                s
            ))])))
        }
        None => Ok(RValue::Vector(Vector::Character(vec![None]))),
    }
}

#[builtin(name = "dQuote", min_args = 1)]
fn builtin_dquote(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    match args.first() {
        Some(RValue::Vector(Vector::Character(vals))) => {
            let result: Vec<Option<String>> = vals
                .iter()
                .map(|s| s.as_ref().map(|s| format!("\u{201C}{}\u{201D}", s)))
                .collect();
            Ok(RValue::Vector(Vector::Character(result)))
        }
        Some(val) => {
            let s = val
                .as_vector()
                .and_then(|v| v.as_character_scalar())
                .unwrap_or_default();
            Ok(RValue::Vector(Vector::Character(vec![Some(format!(
                "\u{201C}{}\u{201D}",
                s
            ))])))
        }
        _ => Ok(RValue::Vector(Vector::Character(vec![None]))),
    }
}
