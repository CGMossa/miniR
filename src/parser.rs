pub mod ast;

use std::fmt;

use pest::error::InputLocation;
use pest::iterators::Pair;
use pest::Parser;
use pest_derive::Parser;

use ast::*;

#[derive(Parser)]
#[grammar = "parser/r.pest"]
pub struct RParser;

/// A structured parse error with human-friendly messages and source context.
#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub line: usize,
    pub col: usize,
    pub source_line: String,
    pub filename: Option<String>,
    pub suggestion: Option<String>,
}

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

pub fn parse_program(input: &str) -> Result<Expr, ParseError> {
    let pairs = RParser::parse(Rule::program, input).map_err(|e| convert_pest_error(e, input))?;

    let pair = pairs.into_iter().next().unwrap();
    Ok(build_program(pair))
}

/// Convert a pest error into a human-friendly ParseError.
fn convert_pest_error(e: pest::error::Error<Rule>, source: &str) -> ParseError {
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

    // Try common-mistake detection first
    if let Some(err) = detect_common_mistakes(source, &source_line, line, col) {
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
    }
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

    let ch = remaining.chars().next().unwrap();

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
    let end = col.min(source_line.len());
    let context = &source_line[..end];
    if context.len() > 40 {
        format!("...{}", &context[context.len() - 37..])
    } else {
        context.to_string()
    }
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
            return Some(ParseError {
                message: "missing parentheses around `if` condition".to_string(),
                line,
                col: source_line.find("if ").unwrap_or(0) + 4,
                source_line: source_line.to_string(),
                filename: None,
                suggestion: Some("R requires parentheses: `if (condition) ...`".to_string()),
            });
        }
    }

    // `while x > 0` without parentheses
    if let Some(rest) = trimmed.strip_prefix("while ") {
        if !rest.starts_with('(') {
            return Some(ParseError {
                message: "missing parentheses around `while` condition".to_string(),
                line,
                col: source_line.find("while ").unwrap_or(0) + 7,
                source_line: source_line.to_string(),
                filename: None,
                suggestion: Some("R requires parentheses: `while (condition) ...`".to_string()),
            });
        }
    }

    // `for i in 1:10` without parentheses
    if let Some(rest) = trimmed.strip_prefix("for ") {
        if !rest.starts_with('(') {
            return Some(ParseError {
                message: "missing parentheses around `for` clause".to_string(),
                line,
                col: source_line.find("for ").unwrap_or(0) + 5,
                source_line: source_line.to_string(),
                filename: None,
                suggestion: Some("R requires parentheses: `for (var in sequence) ...`".to_string()),
            });
        }
    }

    // --- `function` without parameter list ---
    // `function { ... }` or `function x + 1`
    if let Some(rest) = trimmed.strip_prefix("function") {
        let rest = rest.trim_start();
        if !rest.starts_with('(') && !rest.is_empty() {
            return Some(ParseError {
                message: "`function` requires a parameter list".to_string(),
                line,
                col: source_line.find("function").unwrap_or(0) + 1,
                source_line: source_line.to_string(),
                filename: None,
                suggestion: Some(
                    "use `function(...) body` — even with no parameters, the parentheses are required: `function() ...`"
                        .to_string(),
                ),
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
                return Some(ParseError {
                    message: "missing `in` keyword in `for` loop".to_string(),
                    line,
                    col: source_line.find("for").unwrap_or(0) + 1,
                    source_line: source_line.to_string(),
                    filename: None,
                    suggestion: Some(format!("use `for ({} in {}) ...`", parts[0], after_var)),
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
        return Some(ParseError {
            message: msg.to_string(),
            line: bl,
            col: bc,
            source_line: bline,
            filename: None,
            suggestion: Some(suggestion.to_string()),
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
    let mut prev = ' ';
    let mut cur_line = 1usize;
    let mut cur_col = 1usize;
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
                });
            }
            if ch == string_char && prev != '\\' {
                in_string = false;
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
                string_start_line = cur_line;
                string_start_col = cur_col;
            }
            '\n' => {
                cur_line += 1;
                cur_col = 0; // will be incremented below
            }
            _ => {}
        }
        cur_col += 1;
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

fn build_program(pair: Pair<Rule>) -> Expr {
    let mut exprs = Vec::new();
    for p in pair.into_inner() {
        match p.as_rule() {
            Rule::expr_seq => {
                for child in p.into_inner() {
                    if child.as_rule() == Rule::expr {
                        exprs.push(build_expr(child));
                    }
                }
            }
            Rule::EOI => {}
            _ => {}
        }
    }
    if exprs.len() == 1 {
        exprs.into_iter().next().unwrap()
    } else {
        Expr::Program(exprs)
    }
}

fn build_expr(pair: Pair<Rule>) -> Expr {
    match pair.as_rule() {
        Rule::expr => build_expr(pair.into_inner().next().unwrap()),
        Rule::help_expr => build_help(pair),
        Rule::assign_eq_expr => build_assign_eq(pair),
        Rule::assign_left_expr => build_assign_left(pair),
        Rule::assign_right_expr => build_assign_right(pair),
        Rule::formula_expr => build_formula(pair),
        Rule::or_expr => build_binary_left(pair, |op| match op.as_str() {
            "||" => BinaryOp::OrScalar,
            "|" => BinaryOp::Or,
            _ => unreachable!(),
        }),
        Rule::and_expr => build_binary_left(pair, |op| match op.as_str() {
            "&&" => BinaryOp::AndScalar,
            "&" => BinaryOp::And,
            _ => unreachable!(),
        }),
        Rule::not_expr => build_not(pair),
        Rule::compare_expr => build_binary_left(pair, |op| match op.as_str() {
            "==" => BinaryOp::Eq,
            "!=" => BinaryOp::Ne,
            "<" => BinaryOp::Lt,
            ">" => BinaryOp::Gt,
            "<=" => BinaryOp::Le,
            ">=" => BinaryOp::Ge,
            _ => unreachable!(),
        }),
        Rule::add_expr => build_binary_left(pair, |op| match op.as_str() {
            "+" => BinaryOp::Add,
            "-" => BinaryOp::Sub,
            _ => unreachable!(),
        }),
        Rule::mul_expr => build_binary_left(pair, |op| match op.as_str() {
            "*" => BinaryOp::Mul,
            "/" => BinaryOp::Div,
            _ => unreachable!(),
        }),
        Rule::special_pipe_expr => build_special_pipe(pair),
        Rule::colon_expr => build_colon(pair),
        Rule::unary_expr => build_unary(pair),
        Rule::power_expr => build_power(pair),
        Rule::postfix_expr => build_postfix_expr(pair),
        Rule::namespace_expr => build_namespace_expr(pair),
        Rule::primary_expr => build_primary(pair),
        Rule::keyword_constant => build_primary(pair),
        _ => build_primary(pair),
    }
}

// "?" help (unary or binary — just evaluates and returns the expression for now)
fn build_help(pair: Pair<Rule>) -> Expr {
    let mut inner = pair.into_inner();
    let first = inner.next().unwrap();
    if first.as_rule() == Rule::help_expr {
        // Unary: "?" ~ assign_eq_expr — just evaluate the expr
        build_expr(first)
    } else {
        // Binary: expr ~ "?" ~ expr — just evaluate the LHS
        let lhs = build_expr(first);
        // Ignore the RHS (help topic)
        if inner.next().is_some() {
            // just return lhs
        }
        lhs
    }
}

// "=" assignment (right-associative)
fn build_assign_eq(pair: Pair<Rule>) -> Expr {
    let mut inner = pair.into_inner();
    let lhs = build_expr(inner.next().unwrap());
    match inner.next() {
        None => lhs,
        Some(op_pair) => {
            assert!(op_pair.as_rule() == Rule::eq_assign_op);
            let rhs = build_expr(inner.next().unwrap());
            Expr::Assign {
                op: AssignOp::Equals,
                target: Box::new(lhs),
                value: Box::new(rhs),
            }
        }
    }
}

// "<-" "<<-" assignment (right-associative)
fn build_assign_left(pair: Pair<Rule>) -> Expr {
    let mut inner = pair.into_inner();
    let lhs = build_expr(inner.next().unwrap());
    match inner.next() {
        None => lhs,
        Some(op_pair) => {
            let op = match op_pair.as_str() {
                "<-" => AssignOp::LeftAssign,
                "<<-" => AssignOp::SuperAssign,
                _ => unreachable!(),
            };
            let rhs = build_expr(inner.next().unwrap());
            Expr::Assign {
                op,
                target: Box::new(lhs),
                value: Box::new(rhs),
            }
        }
    }
}

// "->" "->>" assignment (right-associative, but target/value are swapped)
fn build_assign_right(pair: Pair<Rule>) -> Expr {
    let mut inner = pair.into_inner();
    let mut result = build_expr(inner.next().unwrap());
    while let Some(op_pair) = inner.next() {
        let op = match op_pair.as_str() {
            "->" => AssignOp::RightAssign,
            "->>" => AssignOp::RightSuperAssign,
            _ => unreachable!(),
        };
        let target = build_expr(inner.next().unwrap());
        result = Expr::Assign {
            op,
            target: Box::new(target),
            value: Box::new(result),
        };
    }
    result
}

// "~" formula (unary or binary)
fn build_formula(pair: Pair<Rule>) -> Expr {
    let mut inner = pair.into_inner();
    let first = inner.next().unwrap();

    if first.as_rule() == Rule::formula_expr {
        // Unary formula: "~" ~ formula_expr
        let rhs = build_expr(first);
        Expr::Formula {
            lhs: None,
            rhs: Some(Box::new(rhs)),
        }
    } else {
        // Binary: or_expr ~ ("~" ~ or_expr)?
        let lhs = build_expr(first);
        match inner.next() {
            None => lhs,
            Some(rhs_pair) => {
                let rhs = build_expr(rhs_pair);
                Expr::Formula {
                    lhs: Some(Box::new(lhs)),
                    rhs: Some(Box::new(rhs)),
                }
            }
        }
    }
}

fn build_binary_left(pair: Pair<Rule>, map_op: impl Fn(&Pair<Rule>) -> BinaryOp) -> Expr {
    let mut inner = pair.into_inner();
    let mut lhs = build_expr(inner.next().unwrap());
    while let Some(op_pair) = inner.next() {
        let op = map_op(&op_pair);
        let rhs = build_expr(inner.next().unwrap());
        lhs = Expr::BinaryOp {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        };
    }
    lhs
}

fn build_not(pair: Pair<Rule>) -> Expr {
    let mut inner = pair.into_inner();
    let first = inner.next().unwrap();
    if first.as_rule() == Rule::compare_expr {
        build_expr(first)
    } else {
        // "!" ~ not_expr
        let operand = build_expr(first);
        Expr::UnaryOp {
            op: UnaryOp::Not,
            operand: Box::new(operand),
        }
    }
}

// %...% and |>
fn build_special_pipe(pair: Pair<Rule>) -> Expr {
    let mut inner = pair.into_inner();
    let mut lhs = build_expr(inner.next().unwrap());
    while let Some(op_pair) = inner.next() {
        let op = match op_pair.as_rule() {
            Rule::pipe_op => BinaryOp::Pipe,
            Rule::special_op => match op_pair.as_str() {
                "%in%" => BinaryOp::Special(SpecialOp::In),
                "%*%" => BinaryOp::Special(SpecialOp::MatMul),
                "%%" => BinaryOp::Mod,
                "%/%" => BinaryOp::IntDiv,
                _ => BinaryOp::Special(SpecialOp::Other),
            },
            _ => unreachable!(),
        };
        let rhs = build_expr(inner.next().unwrap());
        lhs = Expr::BinaryOp {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        };
    }
    lhs
}

// ":" range/sequence (left-associative, chainable)
fn build_colon(pair: Pair<Rule>) -> Expr {
    let mut inner = pair.into_inner();
    let mut lhs = build_expr(inner.next().unwrap());
    for rhs_pair in inner {
        lhs = Expr::BinaryOp {
            op: BinaryOp::Range,
            lhs: Box::new(lhs),
            rhs: Box::new(build_expr(rhs_pair)),
        };
    }
    lhs
}

fn build_unary(pair: Pair<Rule>) -> Expr {
    let mut inner = pair.into_inner();
    let first = inner.next().unwrap();
    match first.as_rule() {
        Rule::unary_op => {
            let op = match first.as_str() {
                "-" => UnaryOp::Neg,
                "+" => UnaryOp::Pos,
                _ => unreachable!(),
            };
            let operand = build_expr(inner.next().unwrap());
            Expr::UnaryOp {
                op,
                operand: Box::new(operand),
            }
        }
        // "!" at unary level (allows a == !b)
        Rule::unary_expr => {
            let operand = build_expr(first);
            Expr::UnaryOp {
                op: UnaryOp::Not,
                operand: Box::new(operand),
            }
        }
        _ => build_expr(first),
    }
}

fn build_power(pair: Pair<Rule>) -> Expr {
    let mut inner = pair.into_inner();
    let base = build_expr(inner.next().unwrap());
    // Skip the power_op token if present
    match inner.next() {
        None => base,
        Some(next) => {
            let rhs_pair = if next.as_rule() == Rule::power_op {
                inner.next().unwrap()
            } else {
                next
            };
            Expr::BinaryOp {
                op: BinaryOp::Pow,
                lhs: Box::new(base),
                rhs: Box::new(build_expr(rhs_pair)),
            }
        }
    }
}

// postfix_expr = { namespace_expr ~ postfix_suffix* }
fn build_postfix_expr(pair: Pair<Rule>) -> Expr {
    let mut inner = pair.into_inner();
    let mut expr = build_expr(inner.next().unwrap());
    for suffix in inner {
        expr = build_postfix_suffix(expr, suffix);
    }
    expr
}

fn build_postfix_suffix(object: Expr, pair: Pair<Rule>) -> Expr {
    // Unwrap postfix_suffix wrapper if present
    let pair = if pair.as_rule() == Rule::postfix_suffix {
        pair.into_inner().next().unwrap()
    } else {
        pair
    };
    match pair.as_rule() {
        Rule::call_suffix => {
            let args = pair
                .into_inner()
                .filter(|p| p.as_rule() == Rule::arg_list)
                .flat_map(build_arg_list)
                .collect();
            Expr::Call {
                func: Box::new(object),
                args,
            }
        }
        Rule::index1_suffix => {
            let indices = pair
                .into_inner()
                .filter(|p| p.as_rule() == Rule::sub_list)
                .flat_map(build_sub_list)
                .collect();
            Expr::Index {
                object: Box::new(object),
                indices,
            }
        }
        Rule::index2_suffix => {
            let indices = pair
                .into_inner()
                .filter(|p| p.as_rule() == Rule::sub_list)
                .flat_map(build_sub_list)
                .collect();
            Expr::IndexDouble {
                object: Box::new(object),
                indices,
            }
        }
        Rule::dollar_suffix => {
            let inner = pair.into_inner().next().unwrap();
            let name = match inner.as_rule() {
                Rule::dots => "...".to_string(),
                _ => parse_ident_or_string(inner),
            };
            Expr::Dollar {
                object: Box::new(object),
                member: name,
            }
        }
        Rule::slot_suffix => {
            let inner = pair.into_inner().next().unwrap();
            let name = parse_ident_str(inner);
            Expr::Slot {
                object: Box::new(object),
                member: name,
            }
        }
        _ => unreachable!("unexpected postfix: {:?}", pair.as_rule()),
    }
}

// namespace_expr = { primary_expr ~ namespace_suffix* }
fn build_namespace_expr(pair: Pair<Rule>) -> Expr {
    let mut inner = pair.into_inner();
    let mut expr = build_expr(inner.next().unwrap());
    for suffix in inner {
        if suffix.as_rule() == Rule::namespace_suffix {
            let mut ns_inner = suffix.into_inner();
            let op_pair = ns_inner.next().unwrap(); // namespace_op
            let op_str = op_pair.as_str();
            let name_pair = ns_inner.next().unwrap();
            let name = parse_ident_or_string(name_pair);
            expr = if op_str == ":::" {
                Expr::NsGetInt {
                    namespace: Box::new(expr),
                    name,
                }
            } else {
                Expr::NsGet {
                    namespace: Box::new(expr),
                    name,
                }
            };
        }
    }
    expr
}

fn build_primary(pair: Pair<Rule>) -> Expr {
    let pair = match pair.as_rule() {
        Rule::primary_expr | Rule::keyword_constant => pair.into_inner().next().unwrap(),
        _ => pair,
    };

    match pair.as_rule() {
        Rule::null_lit => Expr::Null,
        Rule::na_lit => {
            let s = pair.as_str();
            let na_type = if s.starts_with("NA_complex") {
                NaType::Complex
            } else if s.starts_with("NA_character") {
                NaType::Character
            } else if s.starts_with("NA_real") {
                NaType::Real
            } else if s.starts_with("NA_integer") {
                NaType::Integer
            } else {
                NaType::Logical
            };
            Expr::Na(na_type)
        }
        Rule::inf_lit => Expr::Inf,
        Rule::nan_lit => Expr::NaN,
        Rule::bool_lit => {
            let val = pair.as_str().starts_with('T');
            Expr::Bool(val)
        }
        Rule::complex_number => parse_complex(pair),
        Rule::number => parse_number(pair),
        Rule::raw_string => parse_raw_string(pair),
        Rule::string => parse_string(pair),
        Rule::dots => Expr::Dots,
        Rule::dotdot => {
            let s = pair.as_str();
            let n: u32 = s[2..].parse().unwrap_or(1);
            Expr::DotDot(n)
        }
        Rule::ident => {
            let name = parse_ident_str(pair);
            Expr::Symbol(name)
        }
        Rule::if_expr => build_if(pair),
        Rule::for_expr => build_for(pair),
        Rule::while_expr => build_while(pair),
        Rule::repeat_expr => {
            let body = pair
                .into_inner()
                .find(|p| p.as_rule() == Rule::expr)
                .map(build_expr)
                .unwrap_or(Expr::Null);
            Expr::Repeat {
                body: Box::new(body),
            }
        }
        Rule::break_expr => Expr::Break,
        Rule::next_expr => Expr::Next,
        Rule::return_expr => {
            let val = pair
                .into_inner()
                .find(|p| p.as_rule() == Rule::expr)
                .map(|p| Box::new(build_expr(p)));
            Expr::Return(val)
        }
        Rule::function_def | Rule::lambda_def => build_function(pair),
        Rule::block => build_block(pair),
        Rule::paren_expr => {
            let inner = pair
                .into_inner()
                .find(|p| p.as_rule() == Rule::expr)
                .unwrap();
            build_expr(inner)
        }
        _ => build_expr(pair),
    }
}

fn build_if(pair: Pair<Rule>) -> Expr {
    let mut exprs: Vec<Expr> = pair
        .into_inner()
        .filter(|p| p.as_rule() == Rule::expr)
        .map(build_expr)
        .collect();
    let condition = exprs.remove(0);
    let then_body = exprs.remove(0);
    let else_body = if !exprs.is_empty() {
        Some(Box::new(exprs.remove(0)))
    } else {
        None
    };
    Expr::If {
        condition: Box::new(condition),
        then_body: Box::new(then_body),
        else_body,
    }
}

fn build_for(pair: Pair<Rule>) -> Expr {
    let inner = pair.into_inner();
    // Find ident and exprs
    let mut var = String::new();
    let mut exprs = Vec::new();
    for p in inner {
        match p.as_rule() {
            Rule::ident => var = parse_ident_str(p),
            Rule::expr => exprs.push(build_expr(p)),
            _ => {}
        }
    }
    let iter = exprs.remove(0);
    let body = exprs.remove(0);
    Expr::For {
        var,
        iter: Box::new(iter),
        body: Box::new(body),
    }
}

fn build_while(pair: Pair<Rule>) -> Expr {
    let exprs: Vec<Expr> = pair
        .into_inner()
        .filter(|p| p.as_rule() == Rule::expr)
        .map(build_expr)
        .collect();
    Expr::While {
        condition: Box::new(exprs[0].clone()),
        body: Box::new(exprs[1].clone()),
    }
}

fn build_function(pair: Pair<Rule>) -> Expr {
    let inner = pair.into_inner();
    let mut params = Vec::new();
    let mut body = None;

    for p in inner {
        match p.as_rule() {
            Rule::param_list => {
                params = build_param_list(p);
            }
            Rule::expr => {
                body = Some(build_expr(p));
            }
            _ => {}
        }
    }

    Expr::Function {
        params,
        body: Box::new(body.unwrap_or(Expr::Null)),
    }
}

fn build_param_list(pair: Pair<Rule>) -> Vec<Param> {
    pair.into_inner()
        .filter(|p| p.as_rule() == Rule::param)
        .map(|p| {
            let mut inner = p.into_inner();
            let first = inner.next().unwrap();
            if first.as_rule() == Rule::dots {
                Param {
                    name: "...".to_string(),
                    default: None,
                    is_dots: true,
                }
            } else {
                let name = parse_ident_str(first);
                // Check for = and default value
                let default = inner.find(|p| p.as_rule() == Rule::expr).map(build_expr);
                Param {
                    name,
                    default,
                    is_dots: false,
                }
            }
        })
        .collect()
}

fn build_block(pair: Pair<Rule>) -> Expr {
    let mut exprs = Vec::new();
    for p in pair.into_inner() {
        match p.as_rule() {
            Rule::expr_seq => {
                for child in p.into_inner() {
                    if child.as_rule() == Rule::expr {
                        exprs.push(build_expr(child));
                    }
                }
            }
            Rule::expr => exprs.push(build_expr(p)),
            _ => {}
        }
    }
    if exprs.is_empty() {
        Expr::Null
    } else {
        Expr::Block(exprs)
    }
}

// -------------------- argument lists --------------------

fn build_arg_list(pair: Pair<Rule>) -> Vec<Arg> {
    pair.into_inner()
        .filter(|p| p.as_rule() == Rule::arg_slot)
        .map(|slot| {
            match slot.into_inner().next() {
                None => Arg {
                    name: None,
                    value: None,
                }, // empty arg
                Some(arg_pair) => build_arg(arg_pair),
            }
        })
        .collect()
}

fn build_sub_list(pair: Pair<Rule>) -> Vec<Arg> {
    pair.into_inner()
        .filter(|p| p.as_rule() == Rule::sub_slot)
        .map(|slot| {
            match slot.into_inner().next() {
                None => Arg {
                    name: None,
                    value: None,
                }, // empty slot
                Some(sub_pair) => build_sub_arg(sub_pair),
            }
        })
        .collect()
}

fn build_arg(pair: Pair<Rule>) -> Arg {
    match pair.as_rule() {
        Rule::arg => {
            let inner_pair = pair.into_inner().next().unwrap();
            match inner_pair.as_rule() {
                Rule::named_arg => build_named_arg(inner_pair),
                _ => Arg {
                    name: None,
                    value: Some(build_expr(inner_pair)),
                },
            }
        }
        _ => Arg {
            name: None,
            value: Some(build_expr(pair)),
        },
    }
}

fn build_sub_arg(pair: Pair<Rule>) -> Arg {
    match pair.as_rule() {
        Rule::sub_arg => {
            let inner_pair = pair.into_inner().next().unwrap();
            match inner_pair.as_rule() {
                Rule::named_sub_arg => build_named_arg(inner_pair),
                _ => Arg {
                    name: None,
                    value: Some(build_expr(inner_pair)),
                },
            }
        }
        _ => Arg {
            name: None,
            value: Some(build_expr(pair)),
        },
    }
}

fn build_named_arg(pair: Pair<Rule>) -> Arg {
    let mut inner = pair.into_inner();
    let name_pair = inner.next().unwrap(); // arg_name
    let name = match name_pair.as_rule() {
        Rule::arg_name => {
            let inner_name = name_pair.into_inner().next().unwrap();
            match inner_name.as_rule() {
                Rule::dots => "...".to_string(),
                Rule::dotdot => inner_name.as_str().to_string(),
                Rule::string => parse_string_value(inner_name),
                _ => parse_ident_str(inner_name),
            }
        }
        _ => parse_ident_str(name_pair),
    };
    // Skip named_eq token
    let value = inner.find(|p| p.as_rule() == Rule::expr).map(build_expr);
    Arg {
        name: Some(name),
        value,
    }
}

// -------------------- number parsing --------------------

fn parse_complex(pair: Pair<Rule>) -> Expr {
    let s = pair.as_str();
    // Remove trailing 'i'
    let num_str = &s[..s.len() - 1];
    let val = num_str.parse::<f64>().unwrap_or(0.0);
    Expr::Complex(val)
}

fn parse_number(pair: Pair<Rule>) -> Expr {
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
            return Expr::Integer(val as i64);
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
        return Expr::Integer(val as i64);
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
                frac_int as f64 / 16f64.powi(frac_part.len() as i32)
            };
            int_val as f64 + frac_val
        } else {
            u64::from_str_radix(mantissa_str, 16).unwrap_or(0) as f64
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
            frac_int as f64 / 16f64.powi(frac_part.len() as i32)
        };
        int_val as f64 + frac_val
    } else {
        i64::from_str_radix(hex_part, 16).unwrap_or(0) as f64
    }
}

fn parse_raw_string(pair: Pair<Rule>) -> Expr {
    let s = pair.as_str();
    // r"(...)" or R'(...)' etc - find the body between delimiters
    // Skip r/R and the quote char, then the opening delimiter
    let quote_pos = s.find('"').or_else(|| s.find('\'')).unwrap();
    let inner = &s[quote_pos + 1..s.len() - 1]; // between outer quotes
                                                // inner is like "(...)" — strip the delimiter pair
    let content = if inner.starts_with('(') || inner.starts_with('[') || inner.starts_with('{') {
        &inner[1..inner.len() - 1]
    } else {
        inner
    };
    Expr::String(content.to_string())
}

fn parse_string_value(pair: Pair<Rule>) -> String {
    let s = pair.as_str();
    let inner = &s[1..s.len() - 1];
    unescape_string(inner)
}

fn parse_string(pair: Pair<Rule>) -> Expr {
    Expr::String(parse_string_value(pair))
}

fn unescape_string(s: &str) -> String {
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

fn parse_ident_str(pair: Pair<Rule>) -> String {
    let s = pair.as_str();
    if s.starts_with('`') && s.ends_with('`') {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

fn parse_ident_or_string(pair: Pair<Rule>) -> String {
    let s = pair.as_str();
    if s.starts_with('`') && s.ends_with('`') {
        s[1..s.len() - 1].to_string()
    } else if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\''))
    {
        unescape_string(&s[1..s.len() - 1])
    } else {
        s.to_string()
    }
}
