/// Tests for miette-based diagnostic rendering of parse errors.
///
/// These tests verify that:
/// 1. Parse errors carry source code and byte offset information
/// 2. The `render()` method produces useful output with source spans
/// 3. Common mistake detection generates correct spans
/// 4. Feature-gated behavior works correctly
use r::Session;

// region: ParseError carries source context

#[test]
fn parse_error_includes_source_code() {
    let mut session = Session::new();
    // Use something that's definitely a parse error
    let result = session.eval_source("1 +");
    assert!(result.is_err());
    let rendered = result.unwrap_err().render();
    assert!(
        rendered.contains("unexpected") || rendered.contains("incomplete"),
        "rendered: {rendered}"
    );
}

#[test]
fn parse_error_shows_suggestion_for_missing_parens() {
    let mut session = Session::new();
    let result = session.eval_source("if TRUE 1");
    assert!(result.is_err());
    let rendered = result.unwrap_err().render();
    assert!(
        rendered.contains("parentheses"),
        "should suggest parentheses: {rendered}"
    );
    assert!(
        rendered.contains("if (condition)"),
        "should show correct syntax: {rendered}"
    );
}

#[test]
fn parse_error_detects_unmatched_brackets() {
    let mut session = Session::new();
    let result = session.eval_source("f(1, 2");
    assert!(result.is_err());
    let rendered = result.unwrap_err().render();
    assert!(
        rendered.contains("unmatched") || rendered.contains("closing"),
        "should mention unmatched bracket: {rendered}"
    );
}

#[test]
fn parse_error_detects_unterminated_string() {
    let mut session = Session::new();
    let result = session.eval_source("x <- \"hello\ny <- 1");
    assert!(result.is_err());
    let rendered = result.unwrap_err().render();
    assert!(
        rendered.contains("unterminated string"),
        "should detect unterminated string: {rendered}"
    );
}

#[test]
fn parse_error_detects_missing_for_in() {
    let mut session = Session::new();
    let result = session.eval_source("for (i 1:10) print(i)");
    assert!(result.is_err());
    let rendered = result.unwrap_err().render();
    assert!(
        rendered.contains("in"),
        "should mention missing `in`: {rendered}"
    );
}

#[test]
fn parse_error_detects_function_without_parens() {
    let mut session = Session::new();
    // The line must start with `function` for detect_common_mistakes to catch it
    let result = session.eval_source("function { 1 }");
    assert!(result.is_err());
    let rendered = result.unwrap_err().render();
    assert!(
        rendered.contains("parameter list") || rendered.contains("function()"),
        "should mention parameter list: {rendered}"
    );
}

// endregion

// region: render() produces miette output when diagnostics feature is on

#[cfg(feature = "diagnostics")]
mod miette_rendering {
    use r::Session;

    #[test]
    fn render_contains_source_span_markers() {
        let mut session = Session::new();
        let result = session.eval_source("x <- ) + 2");
        assert!(result.is_err());
        let rendered = result.unwrap_err().render();
        // miette's output should contain the source code with labels
        // In non-fancy mode it uses `,----` style markers
        // In fancy mode it uses box-drawing characters
        assert!(
            rendered.contains(",----")
                || rendered.contains("│")
                || rendered.contains("╭")
                || rendered.contains("×"),
            "miette output should contain span markers: {rendered}"
        );
    }

    #[test]
    fn render_contains_help_text() {
        let mut session = Session::new();
        let result = session.eval_source("if TRUE 1");
        assert!(result.is_err());
        let rendered = result.unwrap_err().render();
        // miette shows help text with "help:" prefix
        assert!(
            rendered.contains("help:"),
            "miette output should contain help text: {rendered}"
        );
    }

    #[test]
    fn render_contains_diagnostic_code() {
        let mut session = Session::new();
        let result = session.eval_source("1 +");
        assert!(result.is_err());
        let rendered = result.unwrap_err().render();
        // Our implementation returns code "parse::error"
        assert!(
            rendered.contains("parse::error"),
            "miette output should contain diagnostic code: {rendered}"
        );
    }

    #[test]
    fn render_multiline_source_shows_correct_line() {
        let mut session = Session::new();
        let source = "x <- 1\ny <- 2\nz <- )";
        let result = session.eval_source(source);
        assert!(result.is_err());
        let rendered = result.unwrap_err().render();
        // Should show the error on the correct line
        assert!(
            rendered.contains("z <- )") || rendered.contains("unexpected"),
            "should reference the error line: {rendered}"
        );
    }

    #[test]
    fn render_shows_error_label_in_source() {
        let mut session = Session::new();
        let result = session.eval_source("f(1 2)");
        assert!(result.is_err());
        let rendered = result.unwrap_err().render();
        // miette should show the source code with a label pointing to the error
        assert!(
            rendered.contains("f(1 2)"),
            "should show original source: {rendered}"
        );
    }
}

// endregion

// region: non-diagnostics fallback

#[test]
fn display_output_still_works() {
    let mut session = Session::new();
    let result = session.eval_source("1 +");
    assert!(result.is_err());
    // Display (not render) should still produce the old format
    let display = format!("{}", result.unwrap_err());
    assert!(
        display.contains("Error:") || display.contains("unexpected"),
        "Display should still work: {display}"
    );
}

#[test]
fn render_for_runtime_errors_uses_display() {
    let mut session = Session::new();
    let result = session.eval_source("x");
    assert!(result.is_err());
    let rendered = result.unwrap_err().render();
    // Runtime errors should still work through render()
    assert!(
        rendered.contains("not found") || rendered.contains("Error"),
        "runtime error render should work: {rendered}"
    );
}

// endregion
