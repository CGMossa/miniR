//! Tab completion for R builtin functions and keywords.
//!
//! Provides completions from three sources:
//! 1. R keywords (if, for, function, etc.)
//! 2. R literal constants (TRUE, FALSE, NULL, NA, etc.)
//! 3. All registered builtin function names

use reedline::{Completer, Span, Suggestion};

use crate::interpreter::builtins::BUILTIN_REGISTRY;

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
            let name = descriptor.name;
            // Skip operator-style names like "+", "-", "==", etc.
            if name
                .chars()
                .next()
                .is_some_and(|c| c.is_alphabetic() || c == '.')
            {
                names.push(name.to_string());
            }
        }

        names.sort();
        names.dedup();

        Self { names }
    }
}

impl Completer for RCompleter {
    fn complete(&mut self, line: &str, pos: usize) -> Vec<Suggestion> {
        // Find the start of the current word (R identifiers can contain letters, digits, '.', '_')
        let line_to_pos = &line[..pos];
        let word_start = line_to_pos
            .rfind(|c: char| !c.is_alphanumeric() && c != '.' && c != '_')
            .map(|i| i + 1)
            .unwrap_or(0);

        let prefix = &line[word_start..pos];
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
