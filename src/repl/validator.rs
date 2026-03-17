//! R input validation for multi-line editing.
//!
//! Determines whether an input line is a complete R expression or needs
//! continuation (unclosed brackets, strings, trailing operators, etc.).
//! When the expression is incomplete, reedline shows the `+ ` continuation
//! prompt and lets the user keep typing.

use reedline::{ValidationResult, Validator};

pub struct RValidator;

impl Validator for RValidator {
    fn validate(&self, line: &str) -> ValidationResult {
        if is_likely_incomplete(line) {
            ValidationResult::Incomplete
        } else {
            ValidationResult::Complete
        }
    }
}

fn is_likely_incomplete(input: &str) -> bool {
    let mut open_parens = 0i32;
    let mut open_braces = 0i32;
    let mut open_brackets = 0i32;
    let mut in_string = false;
    let mut string_char = ' ';
    let mut prev_char = ' ';
    let mut in_comment = false;
    let mut in_raw_string = false;
    let mut raw_close_bracket = ' ';
    let mut raw_quote = ' ';

    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        let c = chars[i];

        if in_comment {
            if c == '\n' {
                in_comment = false;
            }
            prev_char = c;
            i += 1;
            continue;
        }

        if in_raw_string {
            // Look for close_bracket followed by matching quote
            if c == raw_close_bracket && i + 1 < len && chars[i + 1] == raw_quote {
                in_raw_string = false;
                prev_char = raw_quote;
                i += 2;
                continue;
            }
            prev_char = c;
            i += 1;
            continue;
        }

        if in_string {
            if c == string_char && prev_char != '\\' {
                in_string = false;
            }
            prev_char = c;
            i += 1;
            continue;
        }

        // Check for raw strings: r"(...)", R"[...]", r'{...}', R'{...}'
        if (c == 'r' || c == 'R') && i + 2 < len {
            let quote = chars[i + 1];
            if quote == '"' || quote == '\'' {
                let open = chars[i + 2];
                let close = match open {
                    '(' => Some(')'),
                    '[' => Some(']'),
                    '{' => Some('}'),
                    _ => None,
                };
                if let Some(close_ch) = close {
                    in_raw_string = true;
                    raw_close_bracket = close_ch;
                    raw_quote = quote;
                    prev_char = open;
                    i += 3; // skip r"( or R"[ etc.
                    continue;
                }
            }
        }

        match c {
            '#' => in_comment = true,
            '"' | '\'' => {
                in_string = true;
                string_char = c;
            }
            '(' => open_parens += 1,
            ')' => open_parens -= 1,
            '{' => open_braces += 1,
            '}' => open_braces -= 1,
            '[' => open_brackets += 1,
            ']' => open_brackets -= 1,
            _ => {}
        }
        prev_char = c;
        i += 1;
    }

    if open_parens > 0 || open_braces > 0 || open_brackets > 0 || in_string || in_raw_string {
        return true;
    }

    // Trailing binary operator means the expression continues
    let trimmed = input.trim_end();
    // Strip trailing comments to find the real trailing token
    let code_end = strip_trailing_comment(trimmed);
    let trailing = code_end.trim_end();

    if trailing.is_empty() {
        return false;
    }

    // Check for trailing binary operators
    if trailing.ends_with('+')
        || trailing.ends_with('*')
        || trailing.ends_with('/')
        || trailing.ends_with(',')
        || trailing.ends_with('|')
        || trailing.ends_with('&')
        || trailing.ends_with('~')
        || trailing.ends_with("<-")
        || trailing.ends_with("<<-")
        || trailing.ends_with("|>")
        || trailing.ends_with("||")
        || trailing.ends_with("&&")
        || trailing.ends_with("%>%")
    {
        return true;
    }

    // Trailing '-' that isn't part of '->' or '->>'
    if trailing.ends_with('-') && !trailing.ends_with("->") {
        return true;
    }

    false
}

/// Strip a trailing comment from a line, returning just the code portion.
/// For multi-line input, only considers the last non-empty line.
fn strip_trailing_comment(input: &str) -> &str {
    // Find the last non-empty line
    let last_line = input
        .rsplit('\n')
        .find(|l| !l.trim().is_empty())
        .unwrap_or(input);

    // Walk through the line respecting strings to find a comment
    let mut in_string = false;
    let mut string_char = ' ';
    let mut prev = ' ';
    for (idx, c) in last_line.char_indices() {
        if in_string {
            if c == string_char && prev != '\\' {
                in_string = false;
            }
            prev = c;
            continue;
        }
        match c {
            '"' | '\'' => {
                in_string = true;
                string_char = c;
            }
            '#' => return &last_line[..idx],
            _ => {}
        }
        prev = c;
    }
    last_line
}
