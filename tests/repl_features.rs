//! Tests for REPL features: completer, validator, and highlighter.

#![cfg(feature = "repl")]

use r::repl::{RCompleter, RValidator};
use reedline::{Completer, ValidationResult, Validator};

// region: completer tests

#[test]
fn completer_includes_all_builtin_registry_names() {
    use r::interpreter::builtins::BUILTIN_REGISTRY;

    let mut completer = RCompleter::new();

    // Verify that every alphabetic builtin name can be tab-completed.
    // The completer filters out exact matches (name == prefix), so we use
    // the first character as the prefix. For single-char names, we just
    // verify they exist in the names list by checking the completer returns
    // them for a 1-char prefix that matches.
    for descriptor in BUILTIN_REGISTRY {
        let name = descriptor.name;
        // Skip operator-style names ("+", "-", etc.)
        if !name
            .chars()
            .next()
            .is_some_and(|c| c.is_alphabetic() || c == '.')
        {
            continue;
        }

        // Use the first character as prefix — short enough to not exact-match
        // multi-character names. For single-char names the completer's
        // exact-match filter means they won't appear, so skip those.
        if name.len() <= 1 {
            continue;
        }
        let prefix = &name[..1];
        let suggestions = completer.complete(prefix, prefix.len());
        let found = suggestions.iter().any(|s| s.value == name);
        assert!(
            found,
            "builtin '{}' should be completable from prefix '{}', but wasn't found in {} suggestions",
            name,
            prefix,
            suggestions.len()
        );
    }
}

#[test]
fn completer_returns_suggestions_for_common_prefixes() {
    let mut completer = RCompleter::new();

    let suggestions = completer.complete("pri", 3);
    let names: Vec<&str> = suggestions.iter().map(|s| s.value.as_str()).collect();
    assert!(
        names.contains(&"print"),
        "completing 'pri' should suggest 'print', got: {:?}",
        names
    );
}

#[test]
fn completer_includes_keywords() {
    let mut completer = RCompleter::new();

    let suggestions = completer.complete("func", 4);
    let names: Vec<&str> = suggestions.iter().map(|s| s.value.as_str()).collect();
    assert!(
        names.contains(&"function"),
        "completing 'func' should suggest 'function', got: {:?}",
        names
    );
}

#[test]
fn completer_returns_empty_for_empty_prefix() {
    let mut completer = RCompleter::new();
    let suggestions = completer.complete("", 0);
    assert!(
        suggestions.is_empty(),
        "empty prefix should return no suggestions"
    );
}

#[test]
fn completer_skips_operator_names() {
    let mut completer = RCompleter::new();

    // Completing "+" should not return operator-named builtins
    let suggestions = completer.complete("+", 1);
    assert!(
        suggestions.is_empty(),
        "operator prefix should return no suggestions, got: {:?}",
        suggestions
            .iter()
            .map(|s| s.value.as_str())
            .collect::<Vec<_>>()
    );
}

// endregion

// region: validator tests

#[test]
fn validator_complete_for_simple_expression() {
    let validator = RValidator;
    assert!(matches!(
        validator.validate("1 + 2"),
        ValidationResult::Complete
    ));
}

#[test]
fn validator_incomplete_for_unclosed_paren() {
    let validator = RValidator;
    assert!(matches!(
        validator.validate("f(1, 2"),
        ValidationResult::Incomplete
    ));
}

#[test]
fn validator_incomplete_for_unclosed_brace() {
    let validator = RValidator;
    assert!(matches!(
        validator.validate("if (TRUE) {"),
        ValidationResult::Incomplete
    ));
}

#[test]
fn validator_incomplete_for_trailing_operator() {
    let validator = RValidator;
    assert!(matches!(
        validator.validate("x +"),
        ValidationResult::Incomplete
    ));
}

#[test]
fn validator_incomplete_for_trailing_pipe() {
    let validator = RValidator;
    assert!(matches!(
        validator.validate("x |>"),
        ValidationResult::Incomplete
    ));
}

#[test]
fn validator_complete_for_multiline_block() {
    let validator = RValidator;
    // A complete multi-line block should be accepted in one go
    let code = "{\n  x <- 1\n  y <- 2\n  x + y\n}";
    assert!(
        matches!(validator.validate(code), ValidationResult::Complete),
        "complete multi-line block should validate as complete"
    );
}

#[test]
fn validator_incomplete_for_unclosed_string() {
    let validator = RValidator;
    assert!(matches!(
        validator.validate("x <- \"hello"),
        ValidationResult::Incomplete
    ));
}

#[test]
fn validator_complete_for_pasted_multiline_function() {
    let validator = RValidator;
    // Simulates pasting a complete function definition
    let code = "f <- function(x, y) {\n  x + y\n}";
    assert!(
        matches!(validator.validate(code), ValidationResult::Complete),
        "pasted complete function should validate as complete"
    );
}

#[test]
fn validator_incomplete_for_trailing_comma() {
    let validator = RValidator;
    assert!(matches!(
        validator.validate("c(1,"),
        ValidationResult::Incomplete
    ));
}

#[test]
fn validator_incomplete_for_trailing_assignment() {
    let validator = RValidator;
    assert!(matches!(
        validator.validate("x <-"),
        ValidationResult::Incomplete
    ));
}

// endregion
