//! Parser for R documentation (.Rd) files.
//!
//! Rd is a LaTeX-like documentation format used by R packages. Each `.Rd` file
//! in a package's `man/` directory documents one or more R objects (functions,
//! datasets, classes, etc.).
//!
//! This module implements a metadata-first parser: it reliably extracts the
//! structural sections needed for `help()` lookup, topic resolution, and
//! example extraction, without attempting full GNU-R-compatible Rd rendering.
//!
//! The parser is a hand-written stateful lexer that handles:
//! - Rd commands (`\name{}`, `\title{}`, etc.)
//! - Nested brace groups
//! - Escaped characters (`\%`, `\{`, `\}`, `\\`)
//! - Comment lines (starting with `%`)
//! - `#ifdef` / `#ifndef` / `#endif` preprocessor directives (treated as text)
//! - `\dontrun{}`, `\donttest{}`, `\dontshow{}` blocks in examples

use std::collections::HashMap;
use std::path::Path;

// region: Data types

/// A parsed R documentation file.
///
/// Contains the extracted metadata and section content from an `.Rd` file.
/// Text content has markup stripped to produce plain-text representations
/// suitable for terminal display.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RdDoc {
    /// The documented object's name (`\name{}`).
    pub name: Option<String>,
    /// Topic aliases that resolve to this doc page (`\alias{}`).
    pub aliases: Vec<String>,
    /// Short one-line title (`\title{}`).
    pub title: Option<String>,
    /// Description section (`\description{}`).
    pub description: Option<String>,
    /// Usage examples showing function signatures (`\usage{}`).
    pub usage: Option<String>,
    /// Argument descriptions (`\arguments{}`), keyed by parameter name.
    pub arguments: Vec<(String, String)>,
    /// Return value description (`\value{}`).
    pub value: Option<String>,
    /// Runnable examples (`\examples{}`).
    pub examples: Option<String>,
    /// See-also cross-references (`\seealso{}`).
    pub seealso: Option<String>,
    /// Document type (`\docType{}`), e.g. "package", "data", "class".
    pub doc_type: Option<String>,
    /// Keywords (`\keyword{}`).
    pub keywords: Vec<String>,
    /// Author information (`\author{}`).
    pub author: Option<String>,
    /// Details section (`\details{}`).
    pub details: Option<String>,
    /// Note section (`\note{}`).
    pub note: Option<String>,
    /// References section (`\references{}`).
    pub references: Option<String>,
    /// Named custom sections (`\section{Name}{...}`).
    pub sections: Vec<(String, String)>,
}

/// Errors that can occur when parsing an Rd file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RdError {
    /// Unbalanced braces in the input.
    UnbalancedBraces { line: usize },
    /// An I/O error occurred reading the file.
    IoError(String),
}

impl std::fmt::Display for RdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RdError::UnbalancedBraces { line } => {
                write!(f, "Rd parse error: unbalanced braces near line {line}")
            }
            RdError::IoError(msg) => write!(f, "Rd I/O error: {msg}"),
        }
    }
}

impl std::error::Error for RdError {}

// endregion

// region: Parser

impl RdDoc {
    /// Parse an Rd document from its text content.
    pub fn parse(input: &str) -> Result<Self, RdError> {
        let mut doc = RdDoc::default();
        let mut parser = RdParser::new(input);
        parser.parse_toplevel(&mut doc)?;
        Ok(doc)
    }

    /// Parse an Rd file from disk.
    pub fn parse_file(path: &Path) -> Result<Self, RdError> {
        let content = std::fs::read_to_string(path).map_err(|e| RdError::IoError(e.to_string()))?;
        Self::parse(&content)
    }

    /// Format the document as plain text for terminal display.
    pub fn format_text(&self) -> String {
        let mut out = String::new();

        // Header
        if let Some(name) = &self.name {
            out.push_str(name);
            if let Some(pkg) = self.keywords.first() {
                out.push_str(&format!(" ({pkg})"));
            }
            out.push('\n');
            out.push_str(&"\u{2500}".repeat(name.len().max(20)));
            out.push('\n');
        }

        // Title
        if let Some(title) = &self.title {
            out.push('\n');
            out.push_str(title);
            out.push_str("\n\n");
        }

        // Description
        if let Some(desc) = &self.description {
            out.push_str("Description:\n");
            for line in desc.lines() {
                out.push_str("  ");
                out.push_str(line);
                out.push('\n');
            }
            out.push('\n');
        }

        // Usage
        if let Some(usage) = &self.usage {
            out.push_str("Usage:\n");
            for line in usage.lines() {
                out.push_str("  ");
                out.push_str(line);
                out.push('\n');
            }
            out.push('\n');
        }

        // Arguments
        if !self.arguments.is_empty() {
            out.push_str("Arguments:\n");
            for (name, desc) in &self.arguments {
                out.push_str(&format!("  {:<12} {}\n", name, desc));
            }
            out.push('\n');
        }

        // Details
        if let Some(details) = &self.details {
            out.push_str("Details:\n");
            for line in details.lines() {
                out.push_str("  ");
                out.push_str(line);
                out.push('\n');
            }
            out.push('\n');
        }

        // Value
        if let Some(value) = &self.value {
            out.push_str("Value:\n");
            for line in value.lines() {
                out.push_str("  ");
                out.push_str(line);
                out.push('\n');
            }
            out.push('\n');
        }

        // Custom sections
        for (name, content) in &self.sections {
            out.push_str(name);
            out.push_str(":\n");
            for line in content.lines() {
                out.push_str("  ");
                out.push_str(line);
                out.push('\n');
            }
            out.push('\n');
        }

        // Note
        if let Some(note) = &self.note {
            out.push_str("Note:\n");
            for line in note.lines() {
                out.push_str("  ");
                out.push_str(line);
                out.push('\n');
            }
            out.push('\n');
        }

        // Author
        if let Some(author) = &self.author {
            out.push_str("Author(s):\n");
            out.push_str("  ");
            out.push_str(author);
            out.push_str("\n\n");
        }

        // References
        if let Some(refs) = &self.references {
            out.push_str("References:\n");
            for line in refs.lines() {
                out.push_str("  ");
                out.push_str(line);
                out.push('\n');
            }
            out.push('\n');
        }

        // See Also
        if let Some(seealso) = &self.seealso {
            out.push_str("See Also:\n");
            out.push_str("  ");
            out.push_str(seealso);
            out.push_str("\n\n");
        }

        // Examples
        if let Some(examples) = &self.examples {
            out.push_str("Examples:\n");
            for line in examples.lines() {
                out.push_str("  ");
                out.push_str(line);
                out.push('\n');
            }
            out.push('\n');
        }

        // Aliases (if different from name)
        let non_name_aliases: Vec<&String> = self
            .aliases
            .iter()
            .filter(|a| self.name.as_ref() != Some(a))
            .collect();
        if !non_name_aliases.is_empty() {
            out.push_str("Aliases: ");
            out.push_str(
                &non_name_aliases
                    .iter()
                    .map(|a| a.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
            );
            out.push('\n');
        }

        out
    }

    /// Extract the examples section as runnable R code.
    ///
    /// Returns `None` if no examples section exists. The returned code has
    /// `\dontrun{}` blocks removed and `\donttest{}` / `\dontshow{}` blocks
    /// unwrapped (their content is included).
    pub fn examples_code(&self) -> Option<&str> {
        self.examples.as_deref()
    }

    /// Serialize this document to R's `.Rd` format.
    ///
    /// Produces a well-formed Rd file that can be placed in a package's `man/`
    /// directory. Text content is escaped so that `%`, `{`, `}`, and `\` in
    /// user-facing strings don't break Rd parsing, while Rd structural commands
    /// are emitted verbatim.
    pub fn to_rd(&self) -> String {
        let mut out = String::new();

        // \name{}
        if let Some(name) = &self.name {
            out.push_str(&format!("\\name{{{}}}\n", escape_rd(name)));
        }

        // \alias{} — one per alias
        for alias in &self.aliases {
            out.push_str(&format!("\\alias{{{}}}\n", escape_rd(alias)));
        }

        // \title{}
        if let Some(title) = &self.title {
            out.push_str(&format!("\\title{{{}}}\n", escape_rd(title)));
        }

        // \description{}
        if let Some(desc) = &self.description {
            out.push_str("\\description{\n");
            out.push_str(&escape_rd(desc));
            out.push('\n');
            out.push_str("}\n");
        }

        // \usage{}
        if let Some(usage) = &self.usage {
            out.push_str("\\usage{\n");
            // Usage is R code — escape only %, not braces (braces are valid R syntax
            // and Rd parsers expect them in usage blocks).
            out.push_str(&escape_rd_usage(usage));
            out.push('\n');
            out.push_str("}\n");
        }

        // \arguments{}
        if !self.arguments.is_empty() {
            out.push_str("\\arguments{\n");
            for (param, desc) in &self.arguments {
                out.push_str(&format!(
                    "  \\item{{{}}}{{{}}}",
                    escape_rd(param),
                    escape_rd(desc)
                ));
                out.push('\n');
            }
            out.push_str("}\n");
        }

        // \details{}
        if let Some(details) = &self.details {
            out.push_str("\\details{\n");
            out.push_str(&escape_rd(details));
            out.push('\n');
            out.push_str("}\n");
        }

        // \value{}
        if let Some(value) = &self.value {
            out.push_str("\\value{\n");
            out.push_str(&escape_rd(value));
            out.push('\n');
            out.push_str("}\n");
        }

        // \note{}
        if let Some(note) = &self.note {
            out.push_str("\\note{\n");
            out.push_str(&escape_rd(note));
            out.push('\n');
            out.push_str("}\n");
        }

        // \author{}
        if let Some(author) = &self.author {
            out.push_str(&format!("\\author{{{}}}\n", escape_rd(author)));
        }

        // \references{}
        if let Some(refs) = &self.references {
            out.push_str("\\references{\n");
            out.push_str(&escape_rd(refs));
            out.push('\n');
            out.push_str("}\n");
        }

        // \seealso{}
        if let Some(seealso) = &self.seealso {
            out.push_str("\\seealso{\n");
            out.push_str(&escape_rd(seealso));
            out.push('\n');
            out.push_str("}\n");
        }

        // \section{Name}{...} — custom sections
        for (sec_name, sec_content) in &self.sections {
            out.push_str(&format!("\\section{{{}}}{{", escape_rd(sec_name)));
            out.push('\n');
            out.push_str(&escape_rd(sec_content));
            out.push('\n');
            out.push_str("}\n");
        }

        // \examples{}
        if let Some(examples) = &self.examples {
            out.push_str("\\examples{\n");
            // Examples are R code — wrap in \dontrun{} since these are
            // synthesized from doc comments, not tested runnable examples.
            out.push_str("\\dontrun{\n");
            out.push_str(&escape_rd_usage(examples));
            out.push('\n');
            out.push_str("}\n");
            out.push_str("}\n");
        }

        // \keyword{}
        for kw in &self.keywords {
            out.push_str(&format!("\\keyword{{{}}}\n", escape_rd(kw)));
        }

        // \docType{}
        if let Some(doc_type) = &self.doc_type {
            out.push_str(&format!("\\docType{{{}}}\n", escape_rd(doc_type)));
        }

        out
    }
}

/// Escape text for inclusion in Rd markup.
///
/// In Rd format, `%` starts a comment, `{` and `}` delimit arguments, and `\`
/// introduces commands. All four must be escaped when they appear in plain text.
fn escape_rd(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '%' => out.push_str("\\%"),
            '{' => out.push_str("\\{"),
            '}' => out.push_str("\\}"),
            _ => out.push(ch),
        }
    }
    out
}

/// Escape text for Rd usage/examples blocks (R code).
///
/// In R code sections, only `%` needs escaping (it's still an Rd comment
/// character). Braces and backslashes are valid R syntax and should be
/// left alone so the code remains valid.
fn escape_rd_usage(text: &str) -> String {
    text.replace('%', "\\%")
}

/// Stateful parser for Rd files.
///
/// The parser processes input character by character, tracking brace depth
/// and recognizing Rd commands, comments, and escape sequences.
struct RdParser<'a> {
    input: &'a str,
    pos: usize,
    line: usize,
}

impl<'a> RdParser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input,
            pos: 0,
            line: 1,
        }
    }

    /// Peek at the current character without advancing.
    fn peek(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    /// Peek at the next n characters as a string slice (char-aware, not byte-aware).
    fn peek_str(&self, n: usize) -> &'a str {
        let remaining = &self.input[self.pos..];
        let end = remaining
            .char_indices()
            .nth(n)
            .map(|(i, _)| i)
            .unwrap_or(remaining.len());
        &remaining[..end]
    }

    /// Advance by one character and return it.
    fn advance(&mut self) -> Option<char> {
        let ch = self.input[self.pos..].chars().next()?;
        self.pos += ch.len_utf8();
        if ch == '\n' {
            self.line += 1;
        }
        Some(ch)
    }

    /// Skip whitespace (but not newlines).
    fn skip_spaces(&mut self) {
        while let Some(ch) = self.peek() {
            if ch == ' ' || ch == '\t' {
                self.advance();
            } else {
                break;
            }
        }
    }

    /// Check if we're at end of input.
    fn at_end(&self) -> bool {
        self.pos >= self.input.len()
    }

    /// Read a command name after `\`. Returns the command name (e.g., "name", "title").
    fn read_command_name(&mut self) -> String {
        let mut name = String::new();
        while let Some(ch) = self.peek() {
            if ch.is_ascii_alphanumeric() || ch == '.' || ch == '_' {
                name.push(ch);
                self.advance();
            } else {
                break;
            }
        }
        name
    }

    /// Read a brace-delimited argument `{...}`, handling nesting and Rd markup.
    /// Returns the content with markup stripped to plain text.
    fn read_brace_arg(&mut self) -> Result<String, RdError> {
        self.read_brace_arg_inner(false)
    }

    /// Read a brace-delimited argument in verbatim mode.
    fn read_brace_arg_verbatim(&mut self) -> Result<String, RdError> {
        self.read_brace_arg_ex(true, false)
    }

    /// Read a brace-delimited argument in R-code mode.
    ///
    /// Like normal mode (Rd commands still expanded via `\` + alpha → break),
    /// but tracks R string literals so `{` inside `"..."` or `'...'` doesn't
    /// affect brace depth. `%` still acts as Rd comment. `#` comments still
    /// count braces (matching GNU R's gramRd.y behavior — `## END}` closes).
    ///
    /// Used for `\examples{}` (RSECTIONHEADER) and `\code{}` (RCODEMACRO).
    fn read_brace_arg_rcode(&mut self) -> Result<String, RdError> {
        self.read_brace_arg_ex(false, true)
    }

    fn read_brace_arg_inner(&mut self, verbatim: bool) -> Result<String, RdError> {
        self.read_brace_arg_ex(verbatim, false)
    }

    /// Core brace-delimited argument reader.
    ///
    /// `verbatim`: no Rd command expansion, `%` is literal.
    /// `rlike`: track R string literals (don't count braces inside strings).
    fn read_brace_arg_ex(&mut self, verbatim: bool, rlike: bool) -> Result<String, RdError> {
        // Expect opening brace
        if self.peek() != Some('{') {
            return Ok(String::new());
        }
        self.advance(); // consume '{'

        let mut depth: usize = 1;
        let mut text = String::new();
        let start_line = self.line;
        // R string tracking (RLIKE mode from GNU R's gramRd.y).
        // When inside "...", '...', or `...`, braces don't affect depth.
        let mut in_r_string: Option<char> = None;

        while !self.at_end() && depth > 0 {
            let ch = self.peek().unwrap();

            // Inside an R string: consume chars, don't count braces.
            // Only the matching unescaped close quote exits string mode.
            // Backslash inside string: if followed by the quote char or
            // another backslash, it's an escape — consume both chars.
            if rlike {
                if let Some(quote) = in_r_string {
                    self.advance();
                    text.push(ch);
                    if ch == '\\' {
                        // Could be R escape (\") or Rd escape (\\).
                        // In both cases, consume the next char to avoid
                        // mistaking it for a closing quote.
                        if let Some(next) = self.peek() {
                            self.advance();
                            text.push(next);
                        }
                    } else if ch == quote {
                        in_r_string = None;
                    }
                    continue;
                }
                // Outside string: enter string mode on quote characters.
                if ch == '"' || ch == '\'' || ch == '`' {
                    in_r_string = Some(ch);
                    self.advance();
                    text.push(ch);
                    continue;
                }
                // R comment (#): consume to end of line, but STILL count
                // braces (matching GNU R). Don't enter string mode for '
                // inside comments like "# it's a test".
                if ch == '#' {
                    while !self.at_end() {
                        let c = self.peek().unwrap();
                        self.advance();
                        text.push(c);
                        if c == '\\' {
                            // Rd escape in comment — check for \{ \} \\
                            if let Some(next) = self.peek() {
                                if next == '{' || next == '}' || next == '\\' || next == '%' {
                                    self.advance();
                                    text.push(next);
                                    continue;
                                }
                            }
                        }
                        if c == '{' {
                            depth += 1;
                        } else if c == '}' {
                            depth -= 1;
                            if depth == 0 {
                                // Brace that closes the section — put it back
                                // so the outer loop handles it.
                                text.pop(); // remove the }
                                self.pos -= 1; // un-advance
                                break;
                            }
                        }
                        if c == '\n' {
                            break;
                        }
                    }
                    continue;
                }
            }

            match ch {
                '{' => {
                    depth += 1;
                    self.advance();
                    text.push('{');
                }
                '}' => {
                    depth -= 1;
                    self.advance();
                    if depth > 0 {
                        text.push('}');
                    }
                }
                '\\' => {
                    self.advance();
                    if let Some(next) = self.peek() {
                        match next {
                            // Escaped specials — always recognized, even in verbatim mode
                            '%' | '{' | '}' | '\\' => {
                                text.push(next);
                                self.advance();
                            }
                            _ if verbatim => {
                                // In verbatim mode, backslash + non-special is literal
                                text.push('\\');
                                text.push(next);
                                self.advance();
                            }
                            _ => {
                                // This is a command inside the brace group
                                let cmd = self.read_command_name();
                                if cmd.is_empty() {
                                    // Just a backslash followed by non-alpha
                                    // (e.g. `\n` in preformatted or `\ `)
                                    text.push(next);
                                    self.advance();
                                } else {
                                    self.handle_inline_command(&cmd, &mut text)?;
                                }
                            }
                        }
                    }
                }
                '%' if !verbatim => {
                    // Comment — skip to end of line
                    self.skip_comment_line();
                }
                '#' if !verbatim => {
                    // Preprocessor directives (#ifdef, #ifndef, #endif) are only
                    // recognized at the start of a line. Mid-line '#' is just text.
                    if self.is_at_line_start(&text) && self.is_preprocessor_directive() {
                        self.skip_to_eol();
                    } else {
                        text.push(ch);
                        self.advance();
                    }
                }
                '\n' => {
                    text.push('\n');
                    self.advance();
                }
                _ => {
                    text.push(ch);
                    self.advance();
                }
            }
        }

        if depth > 0 {
            return Err(RdError::UnbalancedBraces { line: start_line });
        }

        Ok(text)
    }

    /// Handle an inline Rd command encountered inside a brace group.
    /// Strips the markup and appends plain-text content to `out`.
    fn handle_inline_command(&mut self, cmd: &str, out: &mut String) -> Result<(), RdError> {
        match cmd {
            // Commands whose content is included as-is
            // NOTE: \code{} uses normal mode, not RLIKE, because \code{}
            // appears in text sections where ' is an apostrophe, not a string.
            // RLIKE is only safe for top-level sections (\examples, \usage).
            "code" | "bold" | "strong" | "emph" | "samp" | "file" | "pkg" | "var" | "env"
            | "option" | "command" | "dfn" | "cite" | "acronym" | "sQuote" | "dQuote" => {
                if self.peek() == Some('{') {
                    let content = self.read_brace_arg()?;
                    out.push_str(&content);
                }
            }
            // Verbatim commands — `%` is literal, commands not expanded
            "preformatted" | "verb" => {
                if self.peek() == Some('{') {
                    let content = self.read_brace_arg_verbatim()?;
                    out.push_str(&content);
                }
            }
            // \link[pkg]{text} or \link{text} — extract the display text
            "link" | "linkS4class" => {
                // Skip optional [...] argument
                if self.peek() == Some('[') {
                    self.skip_bracket_arg();
                }
                if self.peek() == Some('{') {
                    let content = self.read_brace_arg()?;
                    out.push_str(&content);
                }
            }
            // \href{url}{text} — show the text
            "href" => {
                if self.peek() == Some('{') {
                    let _url = self.read_brace_arg()?;
                }
                if self.peek() == Some('{') {
                    let text = self.read_brace_arg()?;
                    out.push_str(&text);
                }
            }
            // \url{...} — show the URL
            "url" => {
                if self.peek() == Some('{') {
                    let url = self.read_brace_arg()?;
                    out.push_str(&url);
                }
            }
            // \email{...} — show the email
            "email" => {
                if self.peek() == Some('{') {
                    let email = self.read_brace_arg()?;
                    out.push_str(&email);
                }
            }
            // \eqn{latex}{text} or \eqn{latex} — prefer text alt if present
            "eqn" => {
                if self.peek() == Some('{') {
                    let first = self.read_brace_arg()?;
                    if self.peek() == Some('{') {
                        let text_alt = self.read_brace_arg()?;
                        out.push_str(&text_alt);
                    } else {
                        out.push_str(&first);
                    }
                }
            }
            // \deqn{latex}{text} — centered equation
            "deqn" => {
                if self.peek() == Some('{') {
                    let first = self.read_brace_arg()?;
                    if self.peek() == Some('{') {
                        let text_alt = self.read_brace_arg()?;
                        out.push('\n');
                        out.push_str(text_alt.trim());
                        out.push('\n');
                    } else {
                        out.push('\n');
                        out.push_str(first.trim());
                        out.push('\n');
                    }
                }
            }
            // \item{name}{desc} — used in \arguments and \describe
            "item" => {
                if self.peek() == Some('{') {
                    let name = self.read_brace_arg()?;
                    out.push_str(&name);
                    if self.peek() == Some('{') {
                        let desc = self.read_brace_arg()?;
                        out.push_str(": ");
                        out.push_str(&desc);
                    }
                }
            }
            // \dots, \ldots — ellipsis
            "dots" | "ldots" => {
                out.push_str("...");
            }
            // \R — the R name
            "R" => {
                out.push('R');
                // Consume optional empty braces
                if self.peek() == Some('{') {
                    let _ = self.read_brace_arg()?;
                }
            }
            // \cr — line break
            "cr" => {
                out.push('\n');
            }
            // \tab — tab separator (in \tabular)
            "tab" => {
                out.push('\t');
            }
            // \Sexpr[...]{...} — skip (cannot evaluate R code)
            "Sexpr" => {
                if self.peek() == Some('[') {
                    self.skip_bracket_arg();
                }
                if self.peek() == Some('{') {
                    let _ = self.read_brace_arg()?;
                }
            }
            // \if{format}{text}\else{text} — skip conditionals
            "ifelse" | "if" => {
                if self.peek() == Some('{') {
                    let _ = self.read_brace_arg()?;
                }
                if self.peek() == Some('{') {
                    let _ = self.read_brace_arg()?;
                }
                // Check for \else
                self.skip_spaces();
                if self.peek_str(5) == "\\else" {
                    for _ in 0..5 {
                        self.advance();
                    }
                    if self.peek() == Some('{') {
                        let text = self.read_brace_arg()?;
                        out.push_str(&text);
                    }
                }
            }
            // \tabular{fmt}{rows} — extract content
            "tabular" => {
                if self.peek() == Some('{') {
                    let _ = self.read_brace_arg()?; // format spec
                }
                if self.peek() == Some('{') {
                    let content = self.read_brace_arg()?;
                    out.push_str(&content);
                }
            }
            // \describe{...} and \enumerate{...} and \itemize{...}
            "describe" | "enumerate" | "itemize" | "value" => {
                if self.peek() == Some('{') {
                    let content = self.read_brace_arg()?;
                    out.push_str(&content);
                }
            }
            // \dontrun{...} — skip content
            "dontrun" => {
                if self.peek() == Some('{') {
                    let _ = self.read_brace_arg()?;
                }
            }
            // \donttest{...} and \dontshow{...} — include content
            "donttest" | "dontshow" | "testonly" => {
                if self.peek() == Some('{') {
                    let content = self.read_brace_arg()?;
                    out.push_str(&content);
                }
            }
            // \newcommand, \renewcommand — skip
            "newcommand" | "renewcommand" => {
                if self.peek() == Some('{') {
                    let _ = self.read_brace_arg()?;
                }
                if self.peek() == Some('{') {
                    let _ = self.read_brace_arg()?;
                }
            }
            // Package macros — consume braces, output nothing meaningful
            "packageDESCRIPTION" | "packageIndices" | "packageAuthor" | "packageMaintainer"
            | "packageTitle" => {
                if self.peek() == Some('{') {
                    let pkg = self.read_brace_arg()?;
                    out.push_str(&format!("[{cmd}: {pkg}]"));
                }
            }
            // \method{generic}{class} — format as generic.class
            "method" | "S3method" | "S4method" => {
                if self.peek() == Some('{') {
                    let generic = self.read_brace_arg()?;
                    if self.peek() == Some('{') {
                        let class = self.read_brace_arg()?;
                        out.push_str(&format!("{generic}.{class}"));
                    } else {
                        out.push_str(&generic);
                    }
                }
            }
            // \Rdversion{...}, \RdOpts{...}, \encoding{...} — skip
            "Rdversion" | "RdOpts" | "encoding" | "concept" | "source" => {
                if self.peek() == Some('{') {
                    let _ = self.read_brace_arg()?;
                }
            }
            // \out{...} — raw output (HTML/LaTeX), just include as text
            "out" => {
                if self.peek() == Some('{') {
                    let content = self.read_brace_arg_verbatim()?;
                    // Strip HTML tags for plain text display
                    out.push_str(&content);
                }
            }
            // \subsection{title}{body} — nested section
            "subsection" => {
                if self.peek() == Some('{') {
                    let title = self.read_brace_arg()?;
                    out.push('\n');
                    out.push_str(&title);
                    out.push('\n');
                }
                if self.peek() == Some('{') {
                    let body = self.read_brace_arg()?;
                    out.push_str(&body);
                    out.push('\n');
                }
            }
            // \special{...}
            "special" => {
                if self.peek() == Some('{') {
                    let content = self.read_brace_arg()?;
                    out.push_str(&content);
                }
            }
            // Unknown command — try to extract brace content
            _ => {
                if self.peek() == Some('{') {
                    let content = self.read_brace_arg()?;
                    out.push_str(&content);
                }
            }
        }
        Ok(())
    }

    /// Skip a [...] optional argument.
    fn skip_bracket_arg(&mut self) {
        if self.peek() != Some('[') {
            return;
        }
        self.advance(); // consume '['
        let mut depth = 1;
        while !self.at_end() && depth > 0 {
            match self.peek() {
                Some('[') => {
                    depth += 1;
                    self.advance();
                }
                Some(']') => {
                    depth -= 1;
                    self.advance();
                }
                Some('\\') => {
                    self.advance();
                    self.advance(); // skip escaped char
                }
                _ => {
                    self.advance();
                }
            }
        }
    }

    /// Skip a `%` comment line (from `%` to end of line).
    fn skip_comment_line(&mut self) {
        while let Some(ch) = self.advance() {
            if ch == '\n' {
                break;
            }
        }
    }

    /// Skip to end of line (for preprocessor directives).
    fn skip_to_eol(&mut self) {
        while let Some(ch) = self.peek() {
            if ch == '\n' {
                self.advance();
                break;
            }
            self.advance();
        }
    }

    /// Check if we're at the start of a line (nothing but whitespace before
    /// the current position on this line).
    fn is_at_line_start(&self, text_so_far: &str) -> bool {
        // Check if the last line in text_so_far is all whitespace
        match text_so_far.rfind('\n') {
            Some(pos) => text_so_far[pos + 1..]
                .chars()
                .all(|c| c == ' ' || c == '\t'),
            None => text_so_far.chars().all(|c| c == ' ' || c == '\t'),
        }
    }

    /// Check if the current position starts a preprocessor directive
    /// (#ifdef, #ifndef, #endif).
    fn is_preprocessor_directive(&self) -> bool {
        let remaining = &self.input[self.pos..];
        remaining.starts_with("#ifdef")
            || remaining.starts_with("#ifndef")
            || remaining.starts_with("#endif")
    }

    /// Parse the top-level structure of an Rd file.
    fn parse_toplevel(&mut self, doc: &mut RdDoc) -> Result<(), RdError> {
        while !self.at_end() {
            let ch = self.peek().unwrap();

            match ch {
                '%' => {
                    self.skip_comment_line();
                }
                '#' if self.is_preprocessor_directive() => {
                    self.skip_to_eol();
                }
                '\\' => {
                    self.advance(); // consume '\'
                    let cmd = self.read_command_name();
                    if cmd.is_empty() {
                        // Escaped character at top level
                        self.advance();
                        continue;
                    }
                    self.parse_toplevel_command(&cmd, doc)?;
                }
                '\n' | ' ' | '\t' | '\r' => {
                    self.advance();
                }
                _ => {
                    // Stray text at top level — skip
                    self.advance();
                }
            }
        }
        Ok(())
    }

    /// Parse a top-level Rd command and store its content in the doc.
    fn parse_toplevel_command(&mut self, cmd: &str, doc: &mut RdDoc) -> Result<(), RdError> {
        match cmd {
            "name" => {
                let content = self.read_brace_arg()?;
                doc.name = Some(content.trim().to_string());
            }
            "alias" => {
                let content = self.read_brace_arg()?;
                let alias = content.trim().to_string();
                if !alias.is_empty() {
                    doc.aliases.push(alias);
                }
            }
            "title" => {
                let content = self.read_brace_arg()?;
                doc.title = Some(normalize_whitespace(&content));
            }
            "description" => {
                let content = self.read_brace_arg()?;
                doc.description = Some(clean_text(&content));
            }
            "usage" => {
                // Normal mode (not RLIKE) because \usage{} mixes R code with
                // Rd markup like \method{generic}{class} and \dots.
                let content = self.read_brace_arg()?;
                doc.usage = Some(clean_usage(&content));
            }
            "arguments" => {
                self.parse_arguments(doc)?;
            }
            "value" => {
                let content = self.read_brace_arg()?;
                doc.value = Some(clean_text(&content));
            }
            "examples" => {
                let content = self.read_brace_arg_rcode()?;
                doc.examples = Some(clean_examples(&content));
            }
            "seealso" => {
                let content = self.read_brace_arg()?;
                doc.seealso = Some(clean_text(&content));
            }
            "details" => {
                let content = self.read_brace_arg()?;
                doc.details = Some(clean_text(&content));
            }
            "note" => {
                let content = self.read_brace_arg()?;
                doc.note = Some(clean_text(&content));
            }
            "references" => {
                let content = self.read_brace_arg()?;
                doc.references = Some(clean_text(&content));
            }
            "author" => {
                let content = self.read_brace_arg()?;
                doc.author = Some(normalize_whitespace(&content));
            }
            "keyword" => {
                let content = self.read_brace_arg()?;
                let kw = content.trim().to_string();
                if !kw.is_empty() {
                    doc.keywords.push(kw);
                }
            }
            "docType" => {
                let content = self.read_brace_arg()?;
                doc.doc_type = Some(content.trim().to_string());
            }
            "section" => {
                // \section{Title}{Content}
                let title = self.read_brace_arg()?;
                let content = self.read_brace_arg()?;
                doc.sections
                    .push((normalize_whitespace(&title), clean_text(&content)));
            }
            // Top-level commands we want to skip entirely (single arg)
            "Rdversion" | "RdOpts" | "encoding" | "concept" | "source" => {
                if self.peek() == Some('{') {
                    let _ = self.read_brace_arg()?;
                }
            }
            // Macro definitions have two brace args
            "newcommand" | "renewcommand" => {
                if self.peek() == Some('{') {
                    let _ = self.read_brace_arg()?;
                }
                if self.peek() == Some('{') {
                    let _ = self.read_brace_arg()?;
                }
            }
            // Unknown top-level command — consume its brace argument if present
            _ => {
                if self.peek() == Some('{') {
                    let _ = self.read_brace_arg()?;
                }
            }
        }
        Ok(())
    }

    /// Parse the `\arguments{...}` section, extracting `\item{name}{desc}` pairs.
    fn parse_arguments(&mut self, doc: &mut RdDoc) -> Result<(), RdError> {
        if self.peek() != Some('{') {
            return Ok(());
        }
        self.advance(); // consume '{'

        let mut depth: usize = 1;
        let start_line = self.line;

        while !self.at_end() && depth > 0 {
            let ch = self.peek().unwrap();

            match ch {
                '{' => {
                    depth += 1;
                    self.advance();
                }
                '}' => {
                    depth -= 1;
                    self.advance();
                }
                '%' => {
                    self.skip_comment_line();
                }
                '#' if self.is_preprocessor_directive() => {
                    self.skip_to_eol();
                }
                '\\' => {
                    self.advance();
                    let cmd = self.read_command_name();
                    if cmd == "item" {
                        let name_raw = self.read_brace_arg()?;
                        let desc_raw = self.read_brace_arg()?;
                        let name = normalize_whitespace(&name_raw);
                        let desc = clean_text(&desc_raw);
                        if !name.is_empty() {
                            doc.arguments.push((name, desc));
                        }
                    } else if !cmd.is_empty() {
                        // Some other command inside arguments — skip its args
                        if self.peek() == Some('{') {
                            let _ = self.read_brace_arg()?;
                        }
                    }
                }
                _ => {
                    self.advance();
                }
            }
        }

        if depth > 0 {
            return Err(RdError::UnbalancedBraces { line: start_line });
        }

        Ok(())
    }
}

// endregion

// region: Text cleaning utilities

/// Normalize whitespace: collapse runs of whitespace (including newlines) into single spaces,
/// then trim.
fn normalize_whitespace(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Clean section text: trim leading/trailing blank lines, normalize internal
/// paragraph breaks (double newlines) while preserving single newlines for
/// line structure.
fn clean_text(s: &str) -> String {
    let lines: Vec<&str> = s.lines().collect();

    // Trim leading and trailing blank lines
    let start = lines.iter().position(|l| !l.trim().is_empty()).unwrap_or(0);
    let end = lines
        .iter()
        .rposition(|l| !l.trim().is_empty())
        .map(|i| i + 1)
        .unwrap_or(0);

    if start >= end {
        return String::new();
    }

    lines[start..end]
        .iter()
        .map(|l| l.trim())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Clean usage text: preserve line structure but trim each line.
fn clean_usage(s: &str) -> String {
    let lines: Vec<&str> = s.lines().collect();
    let start = lines.iter().position(|l| !l.trim().is_empty()).unwrap_or(0);
    let end = lines
        .iter()
        .rposition(|l| !l.trim().is_empty())
        .map(|i| i + 1)
        .unwrap_or(0);

    if start >= end {
        return String::new();
    }

    lines[start..end]
        .iter()
        .map(|l| l.trim())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Clean examples text: strip comment-only lines (starting with `%`) and
/// trim leading/trailing blank lines.
fn clean_examples(s: &str) -> String {
    let lines: Vec<&str> = s
        .lines()
        .filter(|l| !l.trim_start().starts_with('%'))
        .collect();

    let start = lines.iter().position(|l| !l.trim().is_empty()).unwrap_or(0);
    let end = lines
        .iter()
        .rposition(|l| !l.trim().is_empty())
        .map(|i| i + 1)
        .unwrap_or(0);

    if start >= end {
        return String::new();
    }

    lines[start..end].join("\n")
}

// endregion

// region: Help index

/// An index of Rd documentation, mapping topic aliases to parsed docs.
///
/// Used by `help()` to look up documentation by topic name. The index scans
/// package `man/` directories and builds an alias-to-doc mapping.
#[derive(Debug, Default)]
pub struct RdHelpIndex {
    /// Maps topic alias -> (package name, parsed doc).
    entries: HashMap<String, Vec<RdIndexEntry>>,
}

/// A single entry in the help index.
#[derive(Debug, Clone)]
pub struct RdIndexEntry {
    /// Package this doc belongs to.
    pub package: String,
    /// Path to the source .Rd file.
    pub file_path: String,
    /// The parsed documentation.
    pub doc: RdDoc,
}

impl RdHelpIndex {
    /// Create a new empty help index.
    pub fn new() -> Self {
        Self::default()
    }

    /// Index all `.Rd` files in a package's `man/` directory.
    ///
    /// Parses each file and registers all its aliases in the index.
    /// Files that fail to parse are silently skipped.
    pub fn index_package_dir(&mut self, package_name: &str, man_dir: &Path) {
        let entries = match std::fs::read_dir(man_dir) {
            Ok(entries) => entries,
            Err(_) => return,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("Rd") {
                continue;
            }

            let doc = match RdDoc::parse_file(&path) {
                Ok(doc) => doc,
                Err(_) => continue,
            };

            let file_path = path.to_string_lossy().to_string();
            let index_entry = RdIndexEntry {
                package: package_name.to_string(),
                file_path,
                doc,
            };

            // Register under each alias
            for alias in &index_entry.doc.aliases {
                self.entries
                    .entry(alias.clone())
                    .or_default()
                    .push(index_entry.clone());
            }

            // Also register under the \name if not already an alias
            if let Some(name) = &index_entry.doc.name {
                if !index_entry.doc.aliases.contains(name) {
                    self.entries
                        .entry(name.clone())
                        .or_default()
                        .push(index_entry.clone());
                }
            }
        }
    }

    /// Register a single entry under a topic name.
    pub fn register_entry(&mut self, topic: &str, entry: RdIndexEntry) {
        self.entries
            .entry(topic.to_string())
            .or_default()
            .push(entry);
    }

    /// Look up documentation for a topic.
    ///
    /// Returns all matching entries (there may be multiple from different packages).
    pub fn lookup(&self, topic: &str) -> Vec<&RdIndexEntry> {
        self.entries
            .get(topic)
            .map(|entries| entries.iter().collect())
            .unwrap_or_default()
    }

    /// Look up documentation for a topic in a specific package.
    pub fn lookup_in_package(&self, topic: &str, package: &str) -> Option<&RdIndexEntry> {
        self.entries
            .get(topic)
            .and_then(|entries| entries.iter().find(|e| e.package == package))
    }

    /// Get all indexed topics.
    pub fn topics(&self) -> Vec<&str> {
        self.entries.keys().map(|s| s.as_str()).collect()
    }

    /// Get all indexed topics for a specific package.
    pub fn package_topics(&self, package: &str) -> Vec<&str> {
        self.entries
            .iter()
            .filter(|(_, entries)| entries.iter().any(|e| e.package == package))
            .map(|(topic, _)| topic.as_str())
            .collect()
    }
}

// endregion

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_rd() {
        let input = r#"\name{f}
\alias{f}
\title{Function f -- a Test}
\description{ An Rd test only. }
\usage{
f(a)
}
\arguments{
  \item{a}{a number.}
}
\value{a number.}
\examples{
f(42)
}
"#;
        let doc = RdDoc::parse(input).unwrap();
        assert_eq!(doc.name.as_deref(), Some("f"));
        assert_eq!(doc.aliases, vec!["f"]);
        assert_eq!(doc.title.as_deref(), Some("Function f -- a Test"));
        assert_eq!(doc.description.as_deref(), Some("An Rd test only."));
        assert_eq!(doc.usage.as_deref(), Some("f(a)"));
        assert_eq!(doc.arguments.len(), 1);
        assert_eq!(doc.arguments[0].0, "a");
        assert_eq!(doc.arguments[0].1, "a number.");
        assert_eq!(doc.value.as_deref(), Some("a number."));
        assert_eq!(doc.examples.as_deref(), Some("f(42)"));
    }

    #[test]
    fn parse_multiple_aliases() {
        let input = r#"\name{PkgC-package}
\alias{PkgC-package}
\alias{PkgC}
\docType{package}
\title{Base R Regression Testing Dummy Package - C}
\keyword{ package }
"#;
        let doc = RdDoc::parse(input).unwrap();
        assert_eq!(doc.name.as_deref(), Some("PkgC-package"));
        assert_eq!(doc.aliases, vec!["PkgC-package", "PkgC"]);
        assert_eq!(doc.doc_type.as_deref(), Some("package"));
        assert_eq!(
            doc.title.as_deref(),
            Some("Base R Regression Testing Dummy Package - C")
        );
        assert_eq!(doc.keywords, vec!["package"]);
    }

    #[test]
    fn parse_escaped_chars() {
        let input = r#"\name{test}
\title{Test escaping}
\description{
  Use \code{\%} for percent and \code{\{} for brace.
}
"#;
        let doc = RdDoc::parse(input).unwrap();
        let desc = doc.description.as_deref().unwrap();
        assert!(desc.contains('%'));
        assert!(desc.contains('{'));
    }

    #[test]
    fn parse_comments_stripped() {
        let input = "% This is a comment\n\\name{test}\n% Another comment\n\\title{Test}\n";
        let doc = RdDoc::parse(input).unwrap();
        assert_eq!(doc.name.as_deref(), Some("test"));
        assert_eq!(doc.title.as_deref(), Some("Test"));
    }

    #[test]
    fn parse_examples_strips_dontrun() {
        let input = r#"\name{test}
\examples{
x <- 1
\dontrun{stop("fail")}
y <- 2
}
"#;
        let doc = RdDoc::parse(input).unwrap();
        let ex = doc.examples.as_deref().unwrap();
        assert!(ex.contains("x <- 1"));
        assert!(ex.contains("y <- 2"));
        assert!(!ex.contains("stop"));
    }

    #[test]
    fn parse_examples_includes_donttest() {
        let input = r#"\name{test}
\examples{
x <- 1
\donttest{y <- 2}
z <- 3
}
"#;
        let doc = RdDoc::parse(input).unwrap();
        let ex = doc.examples.as_deref().unwrap();
        assert!(ex.contains("x <- 1"));
        assert!(ex.contains("y <- 2"));
        assert!(ex.contains("z <- 3"));
    }

    #[test]
    fn parse_nested_markup() {
        let input = r#"\name{test}
\description{
  See \code{\link[stats]{weighted.mean}} for details.
}
"#;
        let doc = RdDoc::parse(input).unwrap();
        let desc = doc.description.as_deref().unwrap();
        assert!(desc.contains("weighted.mean"));
    }

    #[test]
    fn parse_link_with_display_text() {
        let input = r#"\name{test}
\seealso{
  \link[=Paren]{\{} and \link[stats:weighted.mean]{ditto}
}
"#;
        let doc = RdDoc::parse(input).unwrap();
        let sa = doc.seealso.as_deref().unwrap();
        assert!(sa.contains('{'));
        assert!(sa.contains("ditto"));
    }

    #[test]
    fn parse_section_custom() {
        let input = r#"\name{test}
\section{Warning}{
  Do not use in production.
}
"#;
        let doc = RdDoc::parse(input).unwrap();
        assert_eq!(doc.sections.len(), 1);
        assert_eq!(doc.sections[0].0, "Warning");
        assert!(doc.sections[0].1.contains("Do not use in production."));
    }

    #[test]
    fn parse_multi_item_arguments() {
        let input = r#"\name{test}
\arguments{
  \item{x}{the input value}
  \item{y}{the output value}
  \item{...}{additional arguments}
}
"#;
        let doc = RdDoc::parse(input).unwrap();
        assert_eq!(doc.arguments.len(), 3);
        assert_eq!(doc.arguments[0].0, "x");
        assert_eq!(doc.arguments[0].1, "the input value");
        assert_eq!(doc.arguments[1].0, "y");
        assert_eq!(doc.arguments[1].1, "the output value");
        assert_eq!(doc.arguments[2].0, "...");
        assert_eq!(doc.arguments[2].1, "additional arguments");
    }

    #[test]
    fn parse_combined_item_args() {
        // R allows \item{x, y}{description} for combined arguments
        let input = r#"\name{test}
\arguments{
  \item{
    x,
    y
  }{
    combined arguments
  }
}
"#;
        let doc = RdDoc::parse(input).unwrap();
        assert_eq!(doc.arguments.len(), 1);
        assert_eq!(doc.arguments[0].0, "x, y");
        assert!(doc.arguments[0].1.contains("combined arguments"));
    }

    #[test]
    fn parse_eqn_with_text_alt() {
        let input = r#"\name{test}
\description{
  The formula \eqn{\alpha}{alpha} is important.
}
"#;
        let doc = RdDoc::parse(input).unwrap();
        let desc = doc.description.as_deref().unwrap();
        assert!(desc.contains("alpha"));
    }

    #[test]
    fn parse_href() {
        let input = r#"\name{test}
\description{
  See \href{https://example.org}{the website} for info.
}
"#;
        let doc = RdDoc::parse(input).unwrap();
        let desc = doc.description.as_deref().unwrap();
        assert!(desc.contains("the website"));
    }

    #[test]
    fn parse_dots_expansion() {
        let input = r#"\name{test}
\description{
  Pass \dots to the function. Also \ldots works.
}
"#;
        let doc = RdDoc::parse(input).unwrap();
        let desc = doc.description.as_deref().unwrap();
        // \dots and \ldots should both become "..."
        let dot_count = desc.matches("...").count();
        assert_eq!(dot_count, 2);
    }

    #[test]
    fn format_text_output() {
        let input = r#"\name{myFunc}
\alias{myFunc}
\title{My Function Title}
\description{This function does something useful.}
\usage{myFunc(x, y = 1)}
\arguments{
  \item{x}{the input}
  \item{y}{optional parameter}
}
\value{A numeric value.}
\examples{
myFunc(1, 2)
}
"#;
        let doc = RdDoc::parse(input).unwrap();
        let text = doc.format_text();
        assert!(text.contains("myFunc"));
        assert!(text.contains("My Function Title"));
        assert!(text.contains("This function does something useful."));
        assert!(text.contains("myFunc(x, y = 1)"));
        assert!(text.contains("the input"));
        assert!(text.contains("A numeric value."));
        assert!(text.contains("myFunc(1, 2)"));
    }

    #[test]
    fn help_index_lookup() {
        let mut index = RdHelpIndex::new();

        let doc = RdDoc {
            name: Some("myFunc".to_string()),
            aliases: vec!["myFunc".to_string(), "mf".to_string()],
            title: Some("My Function".to_string()),
            ..Default::default()
        };

        let entry = RdIndexEntry {
            package: "testPkg".to_string(),
            file_path: "/path/to/myFunc.Rd".to_string(),
            doc,
        };

        // Manually register
        for alias in &entry.doc.aliases {
            index
                .entries
                .entry(alias.clone())
                .or_default()
                .push(entry.clone());
        }

        assert_eq!(index.lookup("myFunc").len(), 1);
        assert_eq!(index.lookup("mf").len(), 1);
        assert_eq!(index.lookup("nonexistent").len(), 0);

        let result = index.lookup("myFunc")[0];
        assert_eq!(result.package, "testPkg");
        assert_eq!(result.doc.title.as_deref(), Some("My Function"));
    }

    #[test]
    fn help_index_package_filter() {
        let mut index = RdHelpIndex::new();

        let doc1 = RdDoc {
            name: Some("func".to_string()),
            aliases: vec!["func".to_string()],
            title: Some("From pkg A".to_string()),
            ..Default::default()
        };

        let doc2 = RdDoc {
            name: Some("func".to_string()),
            aliases: vec!["func".to_string()],
            title: Some("From pkg B".to_string()),
            ..Default::default()
        };

        index
            .entries
            .entry("func".to_string())
            .or_default()
            .push(RdIndexEntry {
                package: "pkgA".to_string(),
                file_path: "a.Rd".to_string(),
                doc: doc1,
            });
        index
            .entries
            .entry("func".to_string())
            .or_default()
            .push(RdIndexEntry {
                package: "pkgB".to_string(),
                file_path: "b.Rd".to_string(),
                doc: doc2,
            });

        // Unfiltered: both results
        assert_eq!(index.lookup("func").len(), 2);

        // Package-filtered
        let a = index.lookup_in_package("func", "pkgA").unwrap();
        assert_eq!(a.doc.title.as_deref(), Some("From pkg A"));

        let b = index.lookup_in_package("func", "pkgB").unwrap();
        assert_eq!(b.doc.title.as_deref(), Some("From pkg B"));

        assert!(index.lookup_in_package("func", "pkgC").is_none());
    }

    #[test]
    fn parse_real_testit_rd() {
        // Test against the real testit.Rd file from the R test suite
        let input = r#"% A regression test example of Rd conversion
\name{testit}
\title{An Rd Regression Test}
\alias{\{}
\usage{
\\x \\y \%\{\}

foo(\var{x}, \var{y}, ...)
}
\arguments{
  \item{
    x,
    y
  }{
    combined arguments, in multiple Rd lines

    paragraph
  }
  \item{...}{description of \dots: \ldots}
}
\value{
  [NULL]\cr\cr\dots
}
\examples{
\\x
\%\{\}

\dontrun{stop("doomed to fail")}

foo(\var{x},
% pure comment lines should be dropped
    \var{y})
}
"#;
        let doc = RdDoc::parse(input).unwrap();
        assert_eq!(doc.name.as_deref(), Some("testit"));
        assert_eq!(doc.title.as_deref(), Some("An Rd Regression Test"));
        assert!(doc.aliases.contains(&"{".to_string()));
        assert_eq!(doc.arguments.len(), 2);
        assert_eq!(doc.arguments[0].0, "x, y");
        assert!(doc.arguments[1].1.contains("..."));

        // Examples should not contain \dontrun content
        let ex = doc.examples.as_deref().unwrap();
        assert!(!ex.contains("doomed to fail"));
        // Comment lines should be stripped from examples
        assert!(!ex.contains("pure comment lines"));
    }

    #[test]
    fn parse_rd_with_author_keyword() {
        let input = r#"\name{pkg}
\alias{pkg}
\title{A Package}
\author{Jane Doe}
\keyword{package}
\keyword{utilities}
"#;
        let doc = RdDoc::parse(input).unwrap();
        assert_eq!(doc.author.as_deref(), Some("Jane Doe"));
        assert_eq!(doc.keywords, vec!["package", "utilities"]);
    }

    #[test]
    fn examples_code_extraction() {
        let input = r#"\name{test}
\examples{
x <- 1 + 2
stopifnot(x == 3)
}
"#;
        let doc = RdDoc::parse(input).unwrap();
        let code = doc.examples_code().unwrap();
        assert!(code.contains("x <- 1 + 2"));
        assert!(code.contains("stopifnot(x == 3)"));
    }

    #[test]
    fn parse_r_command() {
        let input = r#"\name{test}
\description{This uses \R for statistics.}
"#;
        let doc = RdDoc::parse(input).unwrap();
        let desc = doc.description.as_deref().unwrap();
        assert!(desc.contains("R"));
        assert!(desc.contains("statistics"));
    }

    #[test]
    fn parse_sexpr_skipped() {
        let input = r#"\name{foo}
\alias{foo}
\title{Foo Title}
\description{
  Does nothing.  Here is pi:  \Sexpr{pi}.
}
"#;
        let doc = RdDoc::parse(input).unwrap();
        assert_eq!(doc.name.as_deref(), Some("foo"));
        assert_eq!(doc.title.as_deref(), Some("Foo Title"));
        // \Sexpr content is skipped since we can't evaluate R
        let desc = doc.description.as_deref().unwrap();
        assert!(desc.contains("Does nothing."));
    }

    #[test]
    fn parse_ver20_rd() {
        let input = r#"\name{ver20}
\Rdversion{1.1}
\title{Johnson & Johnson,  $ _  ### ^ ~}
\arguments{
    \item{foo}{item 1}
    \item{bar}{space, the item 2}
    \item{bah}{newline, then item 3}
}
\description{
  This is the description
}
\examples{
\\x
}
"#;
        let doc = RdDoc::parse(input).unwrap();
        assert_eq!(doc.name.as_deref(), Some("ver20"));
        assert!(doc.title.as_deref().unwrap().contains("Johnson & Johnson"));
        assert_eq!(doc.arguments.len(), 3);
        assert_eq!(doc.arguments[0].0, "foo");
        assert_eq!(doc.arguments[1].0, "bar");
        assert_eq!(doc.arguments[2].0, "bah");
        assert!(doc.description.is_some());
    }

    #[test]
    fn parse_empty_input() {
        let doc = RdDoc::parse("").unwrap();
        assert_eq!(doc.name, None);
        assert!(doc.aliases.is_empty());
    }

    #[test]
    fn parse_only_comments() {
        let doc = RdDoc::parse("% just a comment\n% another one\n").unwrap();
        assert_eq!(doc.name, None);
    }

    #[test]
    fn index_from_directory() {
        // Test indexing with a synthetic directory structure
        let dir = std::env::temp_dir().join(format!("minir-rd-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let man_dir = dir.join("man");
        std::fs::create_dir(&man_dir).unwrap();

        std::fs::write(
            man_dir.join("add.Rd"),
            r#"\name{add}
\alias{add}
\alias{plus}
\title{Add two numbers}
\description{Adds two numbers together.}
\usage{add(x, y)}
\arguments{
  \item{x}{first number}
  \item{y}{second number}
}
\value{The sum of x and y.}
\examples{
add(1, 2)
}
"#,
        )
        .unwrap();

        std::fs::write(
            man_dir.join("sub.Rd"),
            r#"\name{sub}
\alias{sub}
\alias{minus}
\title{Subtract two numbers}
\description{Subtracts y from x.}
"#,
        )
        .unwrap();

        let mut index = RdHelpIndex::new();
        index.index_package_dir("mathPkg", &man_dir);

        // Lookup by name
        assert_eq!(index.lookup("add").len(), 1);
        assert_eq!(index.lookup("sub").len(), 1);

        // Lookup by alias
        assert_eq!(index.lookup("plus").len(), 1);
        assert_eq!(index.lookup("minus").len(), 1);

        // Package topics
        let topics = index.package_topics("mathPkg");
        assert!(topics.contains(&"add"));
        assert!(topics.contains(&"sub"));
        assert!(topics.contains(&"plus"));
        assert!(topics.contains(&"minus"));

        // Verify doc content
        let add_doc = &index.lookup("add")[0].doc;
        assert_eq!(add_doc.title.as_deref(), Some("Add two numbers"));
        assert_eq!(add_doc.arguments.len(), 2);
        assert!(add_doc.examples.is_some());

        // Cleanup
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn parse_real_rd_files() {
        // Test against the real .Rd files in the repository
        let test_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");

        // tests/Pkgs/pkgD/man/f.Rd
        let f_rd = test_dir.join("Pkgs/pkgD/man/f.Rd");
        if f_rd.exists() {
            let doc = RdDoc::parse_file(&f_rd).unwrap();
            assert_eq!(doc.name.as_deref(), Some("f"));
            assert_eq!(doc.aliases, vec!["f"]);
            assert_eq!(doc.title.as_deref(), Some("Function f -- a Test"));
            assert!(doc.description.is_some());
            assert!(doc.usage.is_some());
            assert_eq!(doc.arguments.len(), 1);
            assert_eq!(doc.arguments[0].0, "a");
            assert!(doc.examples.is_some());
        }

        // tests/Pkgs/pkgC/man/PkgC-package.Rd
        let pkgc_rd = test_dir.join("Pkgs/pkgC/man/PkgC-package.Rd");
        if pkgc_rd.exists() {
            let doc = RdDoc::parse_file(&pkgc_rd).unwrap();
            assert_eq!(doc.name.as_deref(), Some("PkgC-package"));
            assert!(doc.aliases.contains(&"PkgC-package".to_string()));
            assert!(doc.aliases.contains(&"PkgC".to_string()));
            assert_eq!(doc.doc_type.as_deref(), Some("package"));
        }

        // tests/ver20.Rd
        let ver20_rd = test_dir.join("ver20.Rd");
        if ver20_rd.exists() {
            let doc = RdDoc::parse_file(&ver20_rd).unwrap();
            assert_eq!(doc.name.as_deref(), Some("ver20"));
            assert_eq!(doc.arguments.len(), 3);
        }

        // tests/testit.Rd
        let testit_rd = test_dir.join("testit.Rd");
        if testit_rd.exists() {
            let doc = RdDoc::parse_file(&testit_rd).unwrap();
            assert_eq!(doc.name.as_deref(), Some("testit"));
            assert!(doc.title.is_some());
            assert!(doc.arguments.len() >= 2);
        }
    }
}
