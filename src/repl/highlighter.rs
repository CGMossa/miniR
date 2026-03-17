//! R syntax highlighting for the REPL.
//!
//! Colors R keywords, strings, numbers, comments, and operators as the user
//! types, providing immediate visual feedback about syntax structure.

use nu_ansi_term::{Color, Style};
use reedline::{Highlighter, StyledText};

pub struct RHighlighter;

// region: token types

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TokenKind {
    Keyword,
    Literal, // TRUE, FALSE, NULL, NA variants, Inf, NaN
    String,
    Number,
    Comment,
    Operator,
    Bracket,
    Normal,
}

impl TokenKind {
    fn style(self) -> Style {
        match self {
            TokenKind::Keyword => Style::new().bold().fg(Color::Magenta),
            TokenKind::Literal => Style::new().fg(Color::Yellow),
            TokenKind::String => Style::new().fg(Color::Green),
            TokenKind::Number => Style::new().fg(Color::Cyan),
            TokenKind::Comment => Style::new().italic().fg(Color::DarkGray),
            TokenKind::Operator => Style::new().fg(Color::Red),
            TokenKind::Bracket => Style::new().bold(),
            TokenKind::Normal => Style::new(),
        }
    }
}

// endregion

// region: keyword classification

fn classify_word(word: &str) -> TokenKind {
    match word {
        // R keywords
        "if" | "else" | "for" | "while" | "repeat" | "function" | "return" | "next" | "break"
        | "in" | "library" | "require" => TokenKind::Keyword,

        // Literal constants
        "TRUE" | "FALSE" | "NULL" | "NA" | "NA_integer_" | "NA_real_" | "NA_complex_"
        | "NA_character_" | "Inf" | "NaN" | "T" | "F" => TokenKind::Literal,

        _ => TokenKind::Normal,
    }
}

// endregion

// region: raw string detection

/// Check if position `i` starts an R 4.0+ raw string like `r"(...)"`, `R"[...]"`,
/// `r'(...)'`, `R'{...}'`, etc. Returns the closing delimiter char and the number
/// of chars consumed for the prefix (e.g. `r"(` = 3 chars) if it matches.
fn raw_string_prefix(chars: &[char], i: usize) -> Option<(char, usize)> {
    let len = chars.len();
    if i >= len {
        return None;
    }
    let c = chars[i];
    if c != 'r' && c != 'R' {
        return None;
    }
    if i + 2 >= len {
        return None;
    }
    let quote = chars[i + 1];
    if quote != '"' && quote != '\'' {
        return None;
    }
    let open = chars[i + 2];
    let close = match open {
        '(' => ')',
        '[' => ']',
        '{' => '}',
        _ => return None,
    };
    // The closing delimiter is: close_bracket followed by the matching quote
    Some((close, 3))
}

// endregion

// region: tokenizer

impl Highlighter for RHighlighter {
    fn highlight(&self, line: &str, _cursor: usize) -> StyledText {
        let mut styled = StyledText::new();
        let chars: Vec<char> = line.chars().collect();
        let len = chars.len();
        let mut i = 0;

        while i < len {
            let c = chars[i];

            // Comment: # to end of line
            if c == '#' {
                let start = i;
                while i < len && chars[i] != '\n' {
                    i += 1;
                }
                let text: String = chars[start..i].iter().collect();
                styled.push((TokenKind::Comment.style(), text));
                continue;
            }

            // R 4.0+ raw strings: r"(...)", R"[...]", r'{...}', etc.
            // Must be checked before regular identifiers since 'r' and 'R' are valid
            // identifier starts.
            if let Some((close_bracket, prefix_len)) = raw_string_prefix(&chars, i) {
                let start = i;
                let quote = chars[i + 1];
                i += prefix_len; // skip r"( or R"[ etc.
                                 // Scan for close_bracket followed by matching quote
                while i < len {
                    if chars[i] == close_bracket && i + 1 < len && chars[i + 1] == quote {
                        i += 2; // skip )' or }" etc.
                        break;
                    }
                    i += 1;
                }
                let text: String = chars[start..i].iter().collect();
                styled.push((TokenKind::String.style(), text));
                continue;
            }

            // Strings: "..." or '...'
            if c == '"' || c == '\'' {
                let quote = c;
                let start = i;
                i += 1;
                while i < len {
                    if chars[i] == '\\' && i + 1 < len {
                        i += 2; // skip escaped char
                    } else if chars[i] == quote {
                        i += 1;
                        break;
                    } else {
                        i += 1;
                    }
                }
                let text: String = chars[start..i].iter().collect();
                styled.push((TokenKind::String.style(), text));
                continue;
            }

            // Numbers: digits, hex (0x...), with optional L/i suffix
            if c.is_ascii_digit() || (c == '.' && i + 1 < len && chars[i + 1].is_ascii_digit()) {
                let start = i;
                if c == '0' && i + 1 < len && (chars[i + 1] == 'x' || chars[i + 1] == 'X') {
                    i += 2; // skip 0x
                    while i < len && chars[i].is_ascii_hexdigit() {
                        i += 1;
                    }
                } else {
                    while i < len && (chars[i].is_ascii_digit() || chars[i] == '.') {
                        i += 1;
                    }
                    // Exponent
                    if i < len && (chars[i] == 'e' || chars[i] == 'E') {
                        i += 1;
                        if i < len && (chars[i] == '+' || chars[i] == '-') {
                            i += 1;
                        }
                        while i < len && chars[i].is_ascii_digit() {
                            i += 1;
                        }
                    }
                }
                // L (integer) or i (complex) suffix
                if i < len && (chars[i] == 'L' || chars[i] == 'i') {
                    i += 1;
                }
                let text: String = chars[start..i].iter().collect();
                styled.push((TokenKind::Number.style(), text));
                continue;
            }

            // Identifiers and keywords
            if c.is_alphabetic() || c == '.' || c == '_' {
                let start = i;
                while i < len && (chars[i].is_alphanumeric() || chars[i] == '.' || chars[i] == '_')
                {
                    i += 1;
                }
                let word: String = chars[start..i].iter().collect();
                let kind = classify_word(&word);
                styled.push((kind.style(), word));
                continue;
            }

            // Lambda shorthand: \(x) x + 1
            if c == '\\' && i + 1 < len && chars[i + 1] == '(' {
                styled.push((TokenKind::Keyword.style(), "\\".to_string()));
                i += 1;
                continue;
            }

            // Backtick-quoted identifiers
            if c == '`' {
                let start = i;
                i += 1;
                while i < len && chars[i] != '`' {
                    i += 1;
                }
                if i < len {
                    i += 1; // closing backtick
                }
                let text: String = chars[start..i].iter().collect();
                styled.push((TokenKind::String.style(), text));
                continue;
            }

            // Multi-character operators
            if i + 1 < len {
                let two: String = chars[i..i + 2].iter().collect();
                match two.as_str() {
                    "<-" | "<<" | "->" | ">>" | "|>" | "||" | "&&" | "!=" | "==" | "<=" | ">="
                    | "%%" | "::" => {
                        // Check for <<- and ->> and :::
                        if i + 2 < len {
                            let three: String = chars[i..i + 3].iter().collect();
                            if three == "<<-" || three == "->>" || three == ":::" {
                                styled.push((TokenKind::Operator.style(), three));
                                i += 3;
                                continue;
                            }
                        }
                        styled.push((TokenKind::Operator.style(), two));
                        i += 2;
                        continue;
                    }
                    _ => {}
                }

                // %any% operators
                if c == '%' {
                    let start = i;
                    i += 1;
                    while i < len && chars[i] != '%' {
                        i += 1;
                    }
                    if i < len {
                        i += 1; // closing %
                    }
                    let text: String = chars[start..i].iter().collect();
                    styled.push((TokenKind::Operator.style(), text));
                    continue;
                }
            }

            // Single-character operators
            if matches!(
                c,
                '+' | '-'
                    | '*'
                    | '/'
                    | '^'
                    | '~'
                    | '!'
                    | '<'
                    | '>'
                    | '='
                    | '&'
                    | '|'
                    | ':'
                    | '$'
                    | '@'
                    | '?'
            ) {
                styled.push((TokenKind::Operator.style(), c.to_string()));
                i += 1;
                continue;
            }

            // Brackets and parentheses — bold for visibility
            if matches!(c, '(' | ')' | '[' | ']' | '{' | '}') {
                styled.push((TokenKind::Bracket.style(), c.to_string()));
                i += 1;
                continue;
            }

            // Everything else (whitespace, commas, semicolons, etc.)
            styled.push((Style::new(), c.to_string()));
            i += 1;
        }

        styled
    }
}

// endregion
