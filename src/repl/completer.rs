//! Tab completion for R builtin functions, keywords, and named parameters.
//!
//! Provides completions from four sources:
//! 1. R keywords (if, for, function, etc.)
//! 2. R literal constants (TRUE, FALSE, NULL, NA, etc.)
//! 3. All registered builtin function names
//! 4. Named parameters when inside a function call (e.g. `runif(n = 10, m` → `min =`)

use reedline::{Completer, Span, Suggestion};

use crate::interpreter::builtins::{find_builtin, BUILTIN_REGISTRY};

pub struct RCompleter {
    names: Vec<String>,
}

impl Default for RCompleter {
    fn default() -> Self {
        Self::new()
    }
}

impl RCompleter {
    pub fn new() -> Self {
        let mut names: Vec<String> = Vec::new();

        // R keywords
        for kw in &[
            "if",
            "else",
            "for",
            "while",
            "repeat",
            "function",
            "return",
            "next",
            "break",
            "in",
            "TRUE",
            "FALSE",
            "NULL",
            "NA",
            "NA_integer_",
            "NA_real_",
            "NA_complex_",
            "NA_character_",
            "Inf",
            "NaN",
        ] {
            names.push((*kw).to_string());
        }

        // Collect all builtin names from the registry
        for descriptor in BUILTIN_REGISTRY {
            for name in std::iter::once(descriptor.name).chain(descriptor.aliases.iter().copied()) {
                // Skip operator-style names like "+", "-", "==", etc.
                if name
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_alphabetic() || c == '.')
                {
                    names.push(name.to_string());
                }
            }
        }

        names.sort();
        names.dedup();

        Self { names }
    }
}

/// Try to find the enclosing function name for the cursor position.
///
/// Walks backward from `pos` through `line`, tracking parenthesis depth.
/// When we find an unmatched `(`, the identifier before it is the function name.
/// Returns `None` if the cursor is not inside a function call.
fn find_enclosing_function(line: &str) -> Option<&str> {
    let bytes = line.as_bytes();
    let mut depth: i32 = 0;

    let mut i = bytes.len();
    while i > 0 {
        i -= 1;
        match bytes[i] {
            b')' => depth += 1,
            b'(' => {
                if depth == 0 {
                    // Found an unmatched ( — extract the function name before it
                    let before = &line[..i];
                    let name_start = before
                        .rfind(|c: char| !c.is_alphanumeric() && c != '.' && c != '_')
                        .map(|j| j + 1)
                        .unwrap_or(0);
                    let name = &before[name_start..i];
                    if !name.is_empty() {
                        return Some(name);
                    }
                    return None;
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    None
}

impl Completer for RCompleter {
    fn complete(&mut self, line: &str, pos: usize) -> Vec<Suggestion> {
        let line_to_pos = &line[..pos];
        let word_start = line_to_pos
            .rfind(|c: char| !c.is_alphanumeric() && c != '.' && c != '_')
            .map(|i| i + 1)
            .unwrap_or(0);

        let prefix = &line[word_start..pos];

        // Check if we're inside a function call — if so, offer parameter completions.
        if let Some(func_name) = find_enclosing_function(line_to_pos) {
            if let Some(descriptor) = find_builtin(func_name) {
                if !descriptor.formals.is_empty() {
                    // Also extract parameter names from @param docs for builtins
                    // that have empty formals but have doc params (variadic functions)
                    let param_suggestions = complete_params(
                        descriptor.formals,
                        descriptor.doc,
                        prefix,
                        word_start,
                        pos,
                    );
                    if !param_suggestions.is_empty() {
                        return param_suggestions;
                    }
                } else {
                    // For variadic builtins, try extracting params from docs
                    let doc_params = extract_doc_params(descriptor.doc);
                    if !doc_params.is_empty() {
                        let param_suggestions =
                            complete_params_from_strings(&doc_params, prefix, word_start, pos);
                        if !param_suggestions.is_empty() {
                            return param_suggestions;
                        }
                    }
                }
            }
        }

        if prefix.is_empty() {
            return vec![];
        }

        self.names
            .iter()
            .filter(|name| name.starts_with(prefix) && name.as_str() != prefix)
            .map(|name| Suggestion {
                value: name.clone(),
                description: None,
                style: None,
                extra: None,
                span: Span::new(word_start, pos),
                append_whitespace: false,
                display_override: None,
                match_indices: None,
            })
            .collect()
    }
}

fn complete_params(
    formals: &[&str],
    doc: &str,
    prefix: &str,
    word_start: usize,
    pos: usize,
) -> Vec<Suggestion> {
    // Combine formal names with any extra doc params
    let mut param_names: Vec<&str> = formals.to_vec();

    // Also add params from doc that aren't in formals (e.g. "..." params)
    for line in doc.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("@param ") {
            if let Some(name) = rest.split_whitespace().next() {
                if name != "..." && !param_names.contains(&name) {
                    param_names.push(name);
                }
            }
        }
    }

    param_names
        .iter()
        .filter(|name| {
            if prefix.is_empty() {
                true
            } else {
                name.starts_with(prefix) && **name != prefix
            }
        })
        .map(|name| Suggestion {
            value: format!("{name} = "),
            description: None,
            style: None,
            extra: None,
            span: Span::new(word_start, pos),
            append_whitespace: false,
            display_override: None,
            match_indices: None,
        })
        .collect()
}

fn complete_params_from_strings(
    params: &[String],
    prefix: &str,
    word_start: usize,
    pos: usize,
) -> Vec<Suggestion> {
    params
        .iter()
        .filter(|name| {
            if prefix.is_empty() {
                true
            } else {
                name.starts_with(prefix) && name.as_str() != prefix
            }
        })
        .map(|name| Suggestion {
            value: format!("{name} = "),
            description: None,
            style: None,
            extra: None,
            span: Span::new(word_start, pos),
            append_whitespace: false,
            display_override: None,
            match_indices: None,
        })
        .collect()
}

/// Extract parameter names from @param doc lines (for variadic builtins).
fn extract_doc_params(doc: &str) -> Vec<String> {
    doc.lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            let rest = trimmed.strip_prefix("@param ")?;
            let name = rest.split_whitespace().next()?;
            if name == "..." {
                None
            } else {
                Some(name.to_string())
            }
        })
        .collect()
}
