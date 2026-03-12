//! REPL support: syntax highlighting, tab completion, input validation, and history hints.

mod completer;
mod highlighter;
mod prompt;
mod validator;

pub use completer::RCompleter;
pub use highlighter::RHighlighter;
pub use prompt::RPrompt;
pub use validator::RValidator;
