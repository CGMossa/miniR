use std::fmt;

use pest::error::InputLocation;

use super::Rule;

/// A structured parse error with human-friendly messages and source context.
#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub line: usize,
    pub col: usize,
    pub source_line: String,
    pub filename: Option<String>,
    pub suggestion: Option<String>,
    /// The full source code that was being parsed. Populated by `convert_pest_error`
    /// and `parse_program`; used by the miette diagnostic renderer.
    /// Boxed to keep `ParseError` small in the common `Result::Ok` path.
    pub source_code: Option<Box<String>>,
    /// Byte offset into `source_code` where the error occurred.
    pub byte_offset: usize,
    /// Length of the error span in bytes (0 = point, >0 = range).
    pub span_length: usize,
}

impl std::error::Error for ParseError {}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Header: error message
        if let Some(ref filename) = self.filename {
            writeln!(
                f,
                "Error in parse: {}:{}:{}: {}",
                filename, self.line, self.col, self.message
            )?;
        } else {
            writeln!(f, "Error: {}", self.message)?;
        }

        // Source line with caret
        let line_num = format!("{}", self.line);
        let gutter_width = line_num.len();
        writeln!(f, "{} |", " ".repeat(gutter_width))?;
        writeln!(f, "{} | {}", line_num, self.source_line)?;
        let caret_offset = self.col.saturating_sub(1);
        write!(
            f,
            "{} | {}^",
            " ".repeat(gutter_width),
            " ".repeat(caret_offset)
        )?;

        // Suggestion
        if let Some(ref suggestion) = self.suggestion {
            write!(f, "\n{} |", " ".repeat(gutter_width))?;
            write!(f, "\n{} = help: {}", " ".repeat(gutter_width), suggestion)?;
        }

        Ok(())
    }
}

// region: miette Diagnostic implementation

#[cfg(feature = "diagnostics")]
impl miette::Diagnostic for ParseError {
    fn code<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        Some(Box::new("parse::error"))
    }

    fn help<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        self.suggestion
            .as_ref()
            .map(|s| Box::new(s.as_str()) as Box<dyn fmt::Display>)
    }

    fn source_code(&self) -> Option<&dyn miette::SourceCode> {
        self.source_code
            .as_ref()
            .map(|s| s.as_ref() as &String as &dyn miette::SourceCode)
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = miette::LabeledSpan> + '_>> {
        // Only provide labels if we have source code to render them against
        if self.source_code.is_some() {
            let label = miette::LabeledSpan::at(
                self.byte_offset..self.byte_offset + self.span_length.max(1),
                &self.message,
            );
            Some(Box::new(std::iter::once(label)))
        } else {
            None
        }
    }
}

#[cfg(feature = "diagnostics")]
impl ParseError {
    /// Render this error using miette's graphical report handler.
    /// Falls back to the standard Display if rendering fails.
    pub fn render(&self) -> String {
        let handler = miette::GraphicalReportHandler::new();
        let mut buf = String::new();
        match handler.render_report(&mut buf, self) {
            Ok(()) => buf,
            Err(_) => format!("{}", self),
        }
    }
}

#[cfg(not(feature = "diagnostics"))]
impl ParseError {
    /// Render this error. Without the `diagnostics` feature, this is just Display.
    pub fn render(&self) -> String {
        format!("{}", self)
    }
}

// endregion

/// Compute byte offset into `source` given 1-based line and 1-based column.
fn line_col_to_byte_offset(source: &str, line: usize, col: usize) -> usize {
    let mut offset = 0;
    for (i, src_line) in source.lines().enumerate() {
        if i + 1 == line {
            // col is 1-based, clamp to line length
            return offset + (col.saturating_sub(1)).min(src_line.len());
        }
        offset += src_line.len() + 1; // +1 for the newline
    }
    // Past end of source
    source.len()
}

/// Convert a pest error into a human-friendly ParseError.
pub(super) fn convert_pest_error(e: pest::error::Error<Rule>, source: &str) -> ParseError {
    let (line, col) = match e.line_col {
        pest::error::LineColLocation::Pos((l, c)) => (l, c),
        pest::error::LineColLocation::Span((l, c), _) => (l, c),
    };

    let source_line = source.lines().nth(line - 1).unwrap_or("").to_string();

    // Get byte offset for token classification
    let byte_offset = match e.location {
        InputLocation::Pos(p) => p,
        InputLocation::Span((s, _)) => s,
    };

    // Compute span length from the token at the error position
    let span_length = token_length_at(source, byte_offset);

    // Try common-mistake detection first
    if let Some(mut err) = detect_common_mistakes(source, &source_line, line, col) {
        err.source_code = Some(Box::new(source.to_string()));
        if err.byte_offset == 0 && (err.line > 1 || err.col > 1) {
            err.byte_offset = line_col_to_byte_offset(source, err.line, err.col);
        }
        return err;
    }

    // Classify what was found at the error position
    let found_token = classify_token(source, byte_offset);

    // Build R-style "unexpected <token> in <context>" message
    let context = build_context(&source_line, col);
    let message = if context.is_empty() {
        format!("unexpected {}", found_token)
    } else {
        format!("unexpected {} in \"{}\"", found_token, context)
    };

    // Try to generate a suggestion from what was expected
    let suggestion = suggest_from_expected(&e, &found_token, &source_line, col);

    ParseError {
        message,
        line,
        col,
        source_line,
        filename: None,
        suggestion,
        source_code: Some(Box::new(source.to_string())),
        byte_offset,
        span_length,
    }
}

/// Compute the byte length of the token at the given offset for span highlighting.
fn token_length_at(source: &str, offset: usize) -> usize {
    let remaining = &source[offset..];
    if remaining.is_empty() {
        return 0;
    }

    let ch = remaining
        .chars()
        .next()
        .expect("non-empty string has a first char");

    // String literal — highlight the opening quote
    if ch == '"' || ch == '\'' {
        return 1;
    }

    // Number
    if ch.is_ascii_digit()
        || (ch == '.' && remaining.len() > 1 && remaining.as_bytes()[1].is_ascii_digit())
    {
        return remaining
            .find(|c: char| !c.is_ascii_digit() && c != '.' && c != 'e' && c != 'E' && c != 'L')
            .unwrap_or(remaining.len());
    }

    // Keyword or identifier
    if ch.is_ascii_alphabetic() || ch == '.' || ch == '_' {
        return remaining
            .find(|c: char| !c.is_ascii_alphanumeric() && c != '.' && c != '_')
            .unwrap_or(remaining.len());
    }

    // Multi-char operators
    for op in &[
        "<<-", "<-", "->>", "->", "|>", "||", "&&", "==", "!=", ">=", "<=", "%%", "**",
    ] {
        if remaining.starts_with(op) {
            return op.len();
        }
    }

    // Single char
    ch.len_utf8()
}

/// Map a pest grammar rule to a human-readable description.
fn humanize_rule(rule: &Rule) -> &'static str {
    match rule {
        Rule::expr | Rule::unary_expr | Rule::primary_expr => "an expression",
        Rule::ident | Rule::plain_ident | Rule::dotted_ident => "a variable name",
        Rule::number | Rule::decimal_number | Rule::hex_number => "a number",
        Rule::string => "a string",
        Rule::block => "a block `{ ... }`",
        Rule::paren_expr => "a parenthesized expression",
        Rule::if_expr => "an if-expression",
        Rule::for_expr => "a for-loop",
        Rule::while_expr => "a while-loop",
        Rule::function_def => "a function definition",
        Rule::param_list => "function parameters",
        Rule::arg_list => "function arguments",
        Rule::eq_assign_op => "'='",
        Rule::left_assign_op => "'<-'",
        Rule::right_assign_op => "'->'",
        Rule::or_op => "'|' or '||'",
        Rule::and_op => "'&' or '&&'",
        Rule::compare_op => "a comparison operator",
        Rule::add_op => "'+' or '-'",
        Rule::mul_op => "'*' or '/'",
        Rule::special_op => "a special operator (%%,  %in%, etc.)",
        Rule::pipe_op => "'|>'",
        Rule::power_op => "'^'",
        Rule::EOI => "end of input",
        _ => "an expression",
    }
}

/// Classify the token found at a byte offset in the source.
fn classify_token(source: &str, offset: usize) -> String {
    let remaining = &source[offset..];
    if remaining.is_empty() {
        return "end of input".to_string();
    }

    let ch = remaining
        .chars()
        .next()
        .expect("non-empty string has a first char");

    // String literal
    if ch == '"' || ch == '\'' {
        return "string constant".to_string();
    }

    // Number
    if ch.is_ascii_digit()
        || (ch == '.' && remaining.len() > 1 && remaining.as_bytes()[1].is_ascii_digit())
    {
        let end = remaining
            .find(|c: char| !c.is_ascii_digit() && c != '.' && c != 'e' && c != 'E' && c != 'L')
            .unwrap_or(remaining.len());
        let token = &remaining[..end];
        return format!("numeric constant {}", token);
    }

    // Keyword or identifier
    if ch.is_ascii_alphabetic() || ch == '.' || ch == '_' {
        let end = remaining
            .find(|c: char| !c.is_ascii_alphanumeric() && c != '.' && c != '_')
            .unwrap_or(remaining.len());
        let word = &remaining[..end];
        return match word {
            "if" | "else" | "for" | "in" | "while" | "repeat" | "function" | "return" | "break"
            | "next" | "TRUE" | "FALSE" | "NULL" | "NA" | "Inf" | "NaN" => format!("'{}'", word),
            _ => format!("symbol '{}'", word),
        };
    }

    // Operator or punctuation
    // Check multi-char operators first
    if remaining.starts_with("<<-") {
        return "'<<-'".to_string();
    }
    if remaining.starts_with("<-") {
        return "'<-'".to_string();
    }
    if remaining.starts_with("->>") {
        return "'->>'".to_string();
    }
    if remaining.starts_with("->") {
        return "'->'".to_string();
    }
    if remaining.starts_with("|>") {
        return "'|>'".to_string();
    }
    if remaining.starts_with("||") {
        return "'||'".to_string();
    }
    if remaining.starts_with("&&") {
        return "'&&'".to_string();
    }
    if remaining.starts_with("==") {
        return "'=='".to_string();
    }
    if remaining.starts_with("!=") {
        return "'!='".to_string();
    }
    if remaining.starts_with(">=") {
        return "'>='".to_string();
    }
    if remaining.starts_with("<=") {
        return "'<='".to_string();
    }
    if remaining.starts_with("%%") {
        return "'%%'".to_string();
    }
    if remaining.starts_with("**") {
        return "'**'".to_string();
    }

    format!("'{}'", ch)
}

/// Build context string showing input up to the error, truncated to ~40 chars.
fn build_context(source_line: &str, col: usize) -> String {
    // col is a byte offset — clamp to the nearest char boundary
    let end = floor_char_boundary(source_line, col.min(source_line.len()));
    let context = &source_line[..end];
    if context.len() > 40 {
        let start = ceil_char_boundary(context, context.len() - 37);
        format!("...{}", &context[start..])
    } else {
        context.to_string()
    }
}

/// Largest byte index <= pos that is a char boundary.
fn floor_char_boundary(s: &str, pos: usize) -> usize {
    let pos = pos.min(s.len());
    let mut i = pos;
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// Smallest byte index >= pos that is a char boundary.
fn ceil_char_boundary(s: &str, pos: usize) -> usize {
    let mut i = pos.min(s.len());
    while i < s.len() && !s.is_char_boundary(i) {
        i += 1;
    }
    i
}

/// Try to suggest a fix based on what was expected.
fn suggest_from_expected(
    e: &pest::error::Error<Rule>,
    found: &str,
    source_line: &str,
    col: usize,
) -> Option<String> {
    let expected_rules: Vec<Rule> = match &e.variant {
        pest::error::ErrorVariant::ParsingError { positives, .. } => positives.clone(),
        _ => vec![],
    };

    // If we found a closing bracket where an expression was expected
    if (found.contains("')'") || found.contains("'}'") || found.contains("']'"))
        && expected_rules
            .iter()
            .any(|r| matches!(r, Rule::expr | Rule::unary_expr | Rule::primary_expr))
    {
        return Some("remove the extra bracket, or add an expression before it".to_string());
    }

    // If end of input where expression expected
    if found == "end of input"
        && expected_rules
            .iter()
            .any(|r| matches!(r, Rule::expr | Rule::unary_expr | Rule::primary_expr))
    {
        return Some("the expression is incomplete — add the missing part".to_string());
    }

    // If a value (number, string, symbol) appears where an operator or comma was expected,
    // this likely means a missing comma inside a function call or vector
    if (found.starts_with("numeric constant")
        || found.starts_with("string constant")
        || found.starts_with("symbol"))
        && is_inside_call_or_vector(source_line, col)
    {
        return Some("did you forget a comma between arguments?".to_string());
    }

    // Describe what was expected using human-friendly names
    if !expected_rules.is_empty() {
        let unique: Vec<&str> = expected_rules
            .iter()
            .map(humanize_rule)
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        if unique.len() == 1 {
            return Some(format!("expected {}", unique[0]));
        }
        if unique.len() <= 3 {
            return Some(format!("expected one of: {}", unique.join(", ")));
        }
    }

    None
}

/// Heuristic: is the error position inside a function call or c() vector?
fn is_inside_call_or_vector(source_line: &str, col: usize) -> bool {
    // Check if there's an unmatched `(` before the error position
    let before = &source_line[..col.min(source_line.len())];
    let open_parens = before.chars().filter(|&c| c == '(').count();
    let close_parens = before.chars().filter(|&c| c == ')').count();
    open_parens > close_parens
}

/// Detect common R mistakes and return a tailored ParseError.
fn detect_common_mistakes(
    source: &str,
    source_line: &str,
    line: usize,
    col: usize,
) -> Option<ParseError> {
    let trimmed = source_line.trim();

    // --- Unterminated string ---
    // Check if there's an unclosed string in the source (before bracket checks,
    // since unclosed strings make bracket counting wrong)
    if let Some(err) = detect_unterminated_string(source) {
        return Some(err);
    }

    // --- Missing parentheses around control flow conditions ---

    // `if x > 0` without parentheses
    if let Some(rest) = trimmed.strip_prefix("if ") {
        if !rest.starts_with('(') {
            let err_col = source_line.find("if ").unwrap_or(0) + 4;
            return Some(ParseError {
                message: "missing parentheses around `if` condition".to_string(),
                line,
                col: err_col,
                source_line: source_line.to_string(),
                filename: None,
                suggestion: Some("R requires parentheses: `if (condition) ...`".to_string()),
                source_code: None,
                byte_offset: line_col_to_byte_offset(source, line, err_col),
                span_length: 1,
            });
        }
    }

    // `while x > 0` without parentheses
    if let Some(rest) = trimmed.strip_prefix("while ") {
        if !rest.starts_with('(') {
            let err_col = source_line.find("while ").unwrap_or(0) + 7;
            return Some(ParseError {
                message: "missing parentheses around `while` condition".to_string(),
                line,
                col: err_col,
                source_line: source_line.to_string(),
                filename: None,
                suggestion: Some("R requires parentheses: `while (condition) ...`".to_string()),
                source_code: None,
                byte_offset: line_col_to_byte_offset(source, line, err_col),
                span_length: 1,
            });
        }
    }

    // `for i in 1:10` without parentheses
    if let Some(rest) = trimmed.strip_prefix("for ") {
        if !rest.starts_with('(') {
            let err_col = source_line.find("for ").unwrap_or(0) + 5;
            return Some(ParseError {
                message: "missing parentheses around `for` clause".to_string(),
                line,
                col: err_col,
                source_line: source_line.to_string(),
                filename: None,
                suggestion: Some("R requires parentheses: `for (var in sequence) ...`".to_string()),
                source_code: None,
                byte_offset: line_col_to_byte_offset(source, line, err_col),
                span_length: 1,
            });
        }
    }

    // --- `function` without parameter list ---
    // `function { ... }` or `function x + 1`
    if let Some(rest) = trimmed.strip_prefix("function") {
        let rest = rest.trim_start();
        if !rest.starts_with('(') && !rest.is_empty() {
            let err_col = source_line.find("function").unwrap_or(0) + 1;
            return Some(ParseError {
                message: "`function` requires a parameter list".to_string(),
                line,
                col: err_col,
                source_line: source_line.to_string(),
                filename: None,
                suggestion: Some(
                    "use `function(...) body` — even with no parameters, the parentheses are required: `function() ...`"
                        .to_string(),
                ),
                source_code: None,
                byte_offset: line_col_to_byte_offset(source, line, err_col),
                span_length: "function".len(),
            });
        }
    }

    // --- `for (i 1:10)` — missing `in` keyword ---
    if let Some(for_content) = extract_for_parens(trimmed) {
        // Check if the content after the variable name has `in`
        let parts: Vec<&str> = for_content.splitn(2, char::is_whitespace).collect();
        if parts.len() >= 2 {
            let after_var = parts[1].trim_start();
            if !after_var.starts_with("in") {
                let err_col = source_line.find("for").unwrap_or(0) + 1;
                return Some(ParseError {
                    message: "missing `in` keyword in `for` loop".to_string(),
                    line,
                    col: err_col,
                    source_line: source_line.to_string(),
                    filename: None,
                    suggestion: Some(format!("use `for ({} in {}) ...`", parts[0], after_var)),
                    source_code: None,
                    byte_offset: line_col_to_byte_offset(source, line, err_col),
                    span_length: "for".len(),
                });
            }
        }
    }

    // --- Unmatched brackets ---
    let (opens, closes) = count_brackets(source);

    // More closes than opens
    if closes.0 > opens.0 {
        return Some(ParseError {
            message: "unexpected `)` without matching `(`".to_string(),
            line,
            col,
            source_line: source_line.to_string(),
            filename: None,
            suggestion: Some("remove the extra `)` or add a matching `(`".to_string()),
            source_code: None,
            byte_offset: line_col_to_byte_offset(source, line, col),
            span_length: 1,
        });
    }
    if closes.1 > opens.1 {
        return Some(ParseError {
            message: "unexpected `}` without matching `{`".to_string(),
            line,
            col,
            source_line: source_line.to_string(),
            filename: None,
            suggestion: Some("remove the extra `}` or add a matching `{`".to_string()),
            source_code: None,
            byte_offset: line_col_to_byte_offset(source, line, col),
            span_length: 1,
        });
    }
    if closes.2 > opens.2 {
        return Some(ParseError {
            message: "unexpected `]` without matching `[`".to_string(),
            line,
            col,
            source_line: source_line.to_string(),
            filename: None,
            suggestion: Some("remove the extra `]` or add a matching `[`".to_string()),
            source_code: None,
            byte_offset: line_col_to_byte_offset(source, line, col),
            span_length: 1,
        });
    }

    // More opens than closes — find where the unmatched bracket is
    if opens.0 > closes.0 {
        let (bl, bc) = find_unmatched_open(source, '(', ')');
        let bline = source.lines().nth(bl - 1).unwrap_or("").to_string();
        return Some(ParseError {
            message: "unmatched `(` — expected a closing `)`".to_string(),
            line: bl,
            col: bc,
            source_line: bline,
            filename: None,
            suggestion: Some("add a closing `)` to match this opening `(`".to_string()),
            source_code: None,
            byte_offset: line_col_to_byte_offset(source, bl, bc),
            span_length: 1,
        });
    }
    if opens.1 > closes.1 {
        let (bl, bc) = find_unmatched_open(source, '{', '}');
        let bline = source.lines().nth(bl - 1).unwrap_or("").to_string();
        return Some(ParseError {
            message: "unmatched `{` — expected a closing `}`".to_string(),
            line: bl,
            col: bc,
            source_line: bline,
            filename: None,
            suggestion: Some("add a closing `}` to match this opening `{`".to_string()),
            source_code: None,
            byte_offset: line_col_to_byte_offset(source, bl, bc),
            span_length: 1,
        });
    }
    if opens.2 > closes.2 {
        // Check for `[[` without `]]`
        let has_double_bracket = source.contains("[[");
        let (bl, bc) = find_unmatched_open(source, '[', ']');
        let bline = source.lines().nth(bl - 1).unwrap_or("").to_string();
        let msg = if has_double_bracket {
            "unmatched `[[` — expected a closing `]]`"
        } else {
            "unmatched `[` — expected a closing `]`"
        };
        let suggestion = if has_double_bracket {
            "use `]]` to close double-bracket indexing (not just `]`)"
        } else {
            "add a closing `]` to match this opening `[`"
        };
        let span_len = if has_double_bracket { 2 } else { 1 };
        return Some(ParseError {
            message: msg.to_string(),
            line: bl,
            col: bc,
            source_line: bline,
            filename: None,
            suggestion: Some(suggestion.to_string()),
            source_code: None,
            byte_offset: line_col_to_byte_offset(source, bl, bc),
            span_length: span_len,
        });
    }

    None
}

/// Detect unterminated strings in the source.
fn detect_unterminated_string(source: &str) -> Option<ParseError> {
    let mut in_string = false;
    let mut string_char = ' ';
    let mut string_start_line = 0;
    let mut string_start_col = 0;
    let mut string_start_byte = 0;
    let mut prev = ' ';
    let mut cur_line = 1usize;
    let mut cur_col = 1usize;
    let mut cur_byte = 0usize;
    let mut in_comment = false;

    for ch in source.chars() {
        if in_comment {
            if ch == '\n' {
                in_comment = false;
                cur_line += 1;
                cur_col = 1;
            } else {
                cur_col += 1;
            }
            cur_byte += ch.len_utf8();
            prev = ch;
            continue;
        }
        if in_string {
            if ch == '\n' {
                // Strings in R can span lines, but only raw strings
                // Regular strings can't contain unescaped newlines
                // This is an unterminated string
                let source_line = source
                    .lines()
                    .nth(string_start_line - 1)
                    .unwrap_or("")
                    .to_string();
                return Some(ParseError {
                    message: "unterminated string".to_string(),
                    line: string_start_line,
                    col: string_start_col,
                    source_line,
                    filename: None,
                    suggestion: Some(format!(
                        "add a closing `{}` to complete the string",
                        string_char
                    )),
                    source_code: None,
                    byte_offset: string_start_byte,
                    span_length: cur_byte - string_start_byte,
                });
            }
            if ch == string_char && prev != '\\' {
                in_string = false;
            }
            cur_col += 1;
            cur_byte += ch.len_utf8();
            prev = ch;
            continue;
        }
        match ch {
            '#' => in_comment = true,
            '"' | '\'' => {
                in_string = true;
                string_char = ch;
                string_start_line = cur_line;
                string_start_col = cur_col;
                string_start_byte = cur_byte;
            }
            '\n' => {
                cur_line += 1;
                cur_col = 0; // will be incremented below
            }
            _ => {}
        }
        cur_col += 1;
        cur_byte += ch.len_utf8();
        prev = ch;
    }

    // String still open at EOF
    if in_string {
        let source_line = source
            .lines()
            .nth(string_start_line - 1)
            .unwrap_or("")
            .to_string();
        return Some(ParseError {
            message: "unterminated string".to_string(),
            line: string_start_line,
            col: string_start_col,
            source_line,
            filename: None,
            suggestion: Some(format!(
                "add a closing `{}` to complete the string",
                string_char
            )),
            source_code: None,
            byte_offset: string_start_byte,
            span_length: source.len() - string_start_byte,
        });
    }

    None
}

/// Find the line and column of the first unmatched opening bracket.
fn find_unmatched_open(source: &str, open: char, close: char) -> (usize, usize) {
    let mut stack: Vec<(usize, usize)> = Vec::new();
    let mut in_string = false;
    let mut string_char = ' ';
    let mut prev = ' ';
    let mut in_comment = false;
    let mut cur_line = 1usize;
    let mut cur_col = 1usize;

    for ch in source.chars() {
        if in_comment {
            if ch == '\n' {
                in_comment = false;
                cur_line += 1;
                cur_col = 1;
                prev = ch;
                continue;
            }
            cur_col += 1;
            prev = ch;
            continue;
        }
        if in_string {
            if ch == string_char && prev != '\\' {
                in_string = false;
            }
            if ch == '\n' {
                cur_line += 1;
                cur_col = 0;
            }
            cur_col += 1;
            prev = ch;
            continue;
        }
        match ch {
            '#' => in_comment = true,
            '"' | '\'' => {
                in_string = true;
                string_char = ch;
            }
            c if c == open => stack.push((cur_line, cur_col)),
            c if c == close => {
                stack.pop();
            }
            '\n' => {
                cur_line += 1;
                cur_col = 0;
            }
            _ => {}
        }
        cur_col += 1;
        prev = ch;
    }

    // The first remaining item in the stack is the unmatched open
    stack.into_iter().next().unwrap_or((1, 1))
}

/// Extract the content inside `for (...)` parentheses, if present.
fn extract_for_parens(trimmed: &str) -> Option<&str> {
    let rest = trimmed.strip_prefix("for")?;
    let rest = rest.trim_start();
    let rest = rest.strip_prefix('(')?;
    // Find matching close paren (simple, not nested-aware for this heuristic)
    let end = rest.find(')')?;
    Some(&rest[..end])
}

/// Count opening and closing brackets in source, respecting strings and comments.
/// Returns ((parens, braces, brackets), (parens, braces, brackets))
fn count_brackets(source: &str) -> ((i32, i32, i32), (i32, i32, i32)) {
    let mut opens = (0i32, 0i32, 0i32);
    let mut closes = (0i32, 0i32, 0i32);
    let mut in_string = false;
    let mut string_char = ' ';
    let mut prev = ' ';
    let mut in_comment = false;

    for ch in source.chars() {
        if in_comment {
            if ch == '\n' {
                in_comment = false;
            }
            prev = ch;
            continue;
        }
        if in_string {
            if ch == string_char && prev != '\\' {
                in_string = false;
            }
            prev = ch;
            continue;
        }
        match ch {
            '#' => in_comment = true,
            '"' | '\'' => {
                in_string = true;
                string_char = ch;
            }
            '(' => opens.0 += 1,
            ')' => closes.0 += 1,
            '{' => opens.1 += 1,
            '}' => closes.1 += 1,
            '[' => opens.2 += 1,
            ']' => closes.2 += 1,
            _ => {}
        }
        prev = ch;
    }
    (opens, closes)
}
