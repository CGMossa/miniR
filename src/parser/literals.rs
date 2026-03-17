//! Literal parsing helpers — numbers, strings, complex values.

use pest::iterators::Pair;

use super::ast::Expr;
use super::Rule;

// region: Complex numbers

pub(super) fn parse_complex(pair: Pair<Rule>) -> Expr {
    let s = pair.as_str();
    // Remove trailing 'i'
    let num_str = &s[..s.len() - 1];
    let val = num_str.parse::<f64>().unwrap_or(0.0);
    Expr::Complex(val)
}

// endregion

// region: Numeric literals

pub(super) fn parse_number(pair: Pair<Rule>) -> Expr {
    let s = pair.as_str();
    // Integer literal (ends with L)
    if let Some(num_str) = s.strip_suffix('L') {
        if num_str.starts_with("0x") || num_str.starts_with("0X") {
            return parse_hex_int(num_str);
        }
        if let Ok(val) = num_str.parse::<i64>() {
            return Expr::Integer(val);
        }
        if let Ok(val) = num_str.parse::<f64>() {
            // Intentional truncation: R `as.integer()` semantics for e.g. 1e5L
            return Expr::Integer(crate::interpreter::coerce::f64_to_i64(val).unwrap_or(0));
        }
    }
    // Hex (without L)
    if s.starts_with("0x") || s.starts_with("0X") {
        return parse_hex_float(s);
    }
    // Float / bare integer
    if let Ok(val) = s.parse::<f64>() {
        // In R, bare integers are still doubles unless suffixed with L
        return Expr::Double(val);
    }
    Expr::Double(0.0)
}

fn parse_hex_int(num_str: &str) -> Expr {
    let hex_part = &num_str[2..];
    // Check for hex float with '.' or 'p'
    if hex_part.contains('.') || hex_part.contains('p') || hex_part.contains('P') {
        let val = parse_hex_float_value(num_str);
        // Intentional truncation: hex float -> integer literal (e.g. 0x1.0p4L)
        return Expr::Integer(crate::interpreter::coerce::f64_to_i64(val).unwrap_or(0));
    }
    let val = i64::from_str_radix(hex_part, 16).unwrap_or(0);
    Expr::Integer(val)
}

fn parse_hex_float(s: &str) -> Expr {
    let val = parse_hex_float_value(s);
    Expr::Double(val)
}

fn parse_hex_float_value(s: &str) -> f64 {
    let s = s.strip_suffix('L').unwrap_or(s);
    let hex_part = &s[2..]; // skip 0x/0X

    if let Some(p_pos) = hex_part.find(['p', 'P']) {
        let mantissa_str = &hex_part[..p_pos];
        let exp_str = &hex_part[p_pos + 1..];

        let mantissa = if let Some(dot_pos) = mantissa_str.find('.') {
            let int_part = &mantissa_str[..dot_pos];
            let frac_part = &mantissa_str[dot_pos + 1..];
            let int_val = if int_part.is_empty() {
                0u64
            } else {
                u64::from_str_radix(int_part, 16).unwrap_or(0)
            };
            let frac_val = if frac_part.is_empty() {
                0.0
            } else {
                let frac_int = u64::from_str_radix(frac_part, 16).unwrap_or(0);
                // u64 -> f64 may lose precision for values > 2^53, acceptable for hex literals
                let frac_digits = i32::try_from(frac_part.len()).unwrap_or(0);
                crate::interpreter::coerce::u64_to_f64(frac_int) / 16f64.powi(frac_digits)
            };
            crate::interpreter::coerce::u64_to_f64(int_val) + frac_val
        } else {
            crate::interpreter::coerce::u64_to_f64(
                u64::from_str_radix(mantissa_str, 16).unwrap_or(0),
            )
        };

        let exp: i32 = exp_str.parse().unwrap_or(0);
        mantissa * 2f64.powi(exp)
    } else if let Some(dot_pos) = hex_part.find('.') {
        // Hex with dot but no exponent
        let int_part = &hex_part[..dot_pos];
        let frac_part = &hex_part[dot_pos + 1..];
        let int_val = if int_part.is_empty() {
            0u64
        } else {
            u64::from_str_radix(int_part, 16).unwrap_or(0)
        };
        let frac_val = if frac_part.is_empty() {
            0.0
        } else {
            let frac_int = u64::from_str_radix(frac_part, 16).unwrap_or(0);
            let frac_digits = i32::try_from(frac_part.len()).unwrap_or(0);
            crate::interpreter::coerce::u64_to_f64(frac_int) / 16f64.powi(frac_digits)
        };
        crate::interpreter::coerce::u64_to_f64(int_val) + frac_val
    } else {
        crate::interpreter::coerce::i64_to_f64(i64::from_str_radix(hex_part, 16).unwrap_or(0))
    }
}

// endregion

// region: String literals

pub(super) fn parse_raw_string(pair: Pair<Rule>) -> Expr {
    let s = pair.as_str();
    // r"(...)" or R'(...)' etc — find the body between outer quotes
    // Also handles dash delimiters: r"---(text)---"
    let quote_pos = s.find('"').or_else(|| s.find('\'')).unwrap();
    let inner = &s[quote_pos + 1..s.len() - 1]; // between outer quotes

    // Strip leading dashes, then the open delimiter, then trailing close + dashes
    let inner = inner.trim_start_matches('-');
    let (open, close) = if inner.starts_with('(') {
        ('(', ')')
    } else if inner.starts_with('[') {
        ('[', ']')
    } else if inner.starts_with('{') {
        ('{', '}')
    } else {
        return Expr::String(inner.to_string());
    };
    // Strip open delimiter from start
    let inner = &inner[1..];
    // Find the matching close delimiter (last occurrence of close + dashes)
    let content = inner.trim_end_matches('-');
    let content = if content.ends_with(close) {
        &content[..content.len() - 1]
    } else {
        content
    };
    let _ = open; // suppress unused warning
    Expr::String(content.to_string())
}

pub(super) fn parse_string_value(pair: Pair<Rule>) -> String {
    let s = pair.as_str();
    let inner = &s[1..s.len() - 1];
    unescape_string(inner)
}

pub(super) fn parse_string(pair: Pair<Rule>) -> Expr {
    Expr::String(parse_string_value(pair))
}

pub(super) fn unescape_string(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => result.push('\n'),
                Some('t') => result.push('\t'),
                Some('r') => result.push('\r'),
                Some('\\') => result.push('\\'),
                Some('"') => result.push('"'),
                Some('\'') => result.push('\''),
                Some('0') => result.push('\0'),
                Some('a') => result.push('\x07'),
                Some('b') => result.push('\x08'),
                Some('f') => result.push('\x0C'),
                Some('v') => result.push('\x0B'),
                Some('x') => {
                    let hex: String = chars.clone().take(2).collect();
                    if let Ok(val) = u8::from_str_radix(&hex, 16) {
                        result.push(val as char);
                        chars.nth(1);
                    }
                }
                Some(other) => {
                    result.push('\\');
                    result.push(other);
                }
                None => result.push('\\'),
            }
        } else {
            result.push(c);
        }
    }
    result
}

// endregion
