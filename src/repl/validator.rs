//! R input validation for multi-line editing.
//!
//! Determines whether an input line is a complete R expression or needs
//! continuation (unclosed brackets, strings, trailing operators, etc.).

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

    for c in input.chars() {
        if in_comment {
            if c == '\n' {
                in_comment = false;
            }
            prev_char = c;
            continue;
        }
        if in_string {
            if c == string_char && prev_char != '\\' {
                in_string = false;
            }
        } else {
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
        }
        prev_char = c;
    }

    if open_parens > 0 || open_braces > 0 || open_brackets > 0 || in_string {
        return true;
    }

    // Trailing binary operator means the expression continues
    let trimmed = input.trim_end();
    let trailing = trimmed
        .rfind(|c: char| !c.is_whitespace())
        .map(|i| &trimmed[i..])
        .unwrap_or("");
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
    {
        return true;
    }

    // Trailing '-' that isn't part of '->' or '->>'
    if trailing.ends_with('-') && !trailing.ends_with("->") {
        return true;
    }

    false
}
