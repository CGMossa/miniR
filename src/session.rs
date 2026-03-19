use std::cell::RefCell;
use std::fmt;
use std::fs;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use tracing::info;

use crate::interpreter::value::{RFlow, RValue};
use crate::interpreter::{with_interpreter_state, Interpreter};
use crate::parser::ast::Expr;
use crate::parser::{parse_program, ParseError};

#[derive(Debug)]
pub struct EvalOutput {
    pub value: RValue,
    pub visible: bool,
}

#[derive(Debug)]
pub enum SessionError {
    Parse(Box<ParseError>),
    Runtime(RFlow),
    CannotRead {
        path: String,
        source: std::io::Error,
    },
}

impl SessionError {
    /// Render the error as a string. When the `diagnostics` feature is enabled,
    /// parse errors are rendered using miette's graphical report handler with
    /// source spans, colors, and suggestions.
    pub fn render(&self) -> String {
        match self {
            SessionError::Parse(err) => err.render(),
            other => format!("{}", other),
        }
    }
}

impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SessionError::Parse(err) => write!(f, "{}", err),
            SessionError::Runtime(err) => write!(f, "{}", err),
            SessionError::CannotRead { path, source } => {
                write!(f, "Error reading file '{}': {}", path, source)
            }
        }
    }
}

impl std::error::Error for SessionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SessionError::CannotRead { source, .. } => Some(source),
            SessionError::Parse(_) | SessionError::Runtime(_) => None,
        }
    }
}

/// A `Write` adapter backed by a shared `Arc<Mutex<Vec<u8>>>` so that both
/// the interpreter and the session can access the accumulated bytes.
struct SharedBuf(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

impl std::io::Write for SharedBuf {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut guard = self.0.lock().unwrap_or_else(|e| e.into_inner());
        guard.extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub struct Session {
    interpreter: Interpreter,
    /// Shared stdout capture buffer (only set for captured-output sessions).
    captured_stdout: Option<std::sync::Arc<std::sync::Mutex<Vec<u8>>>>,
    /// Shared stderr capture buffer (only set for captured-output sessions).
    captured_stderr: Option<std::sync::Arc<std::sync::Mutex<Vec<u8>>>>,
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

impl Session {
    pub fn new() -> Self {
        Session {
            interpreter: Interpreter::new(),
            captured_stdout: None,
            captured_stderr: None,
        }
    }

    /// Create a session that captures stdout and stderr into in-memory buffers
    /// instead of writing to the process streams. Use `captured_stdout()` and
    /// `captured_stderr()` to retrieve the accumulated output.
    pub fn new_with_captured_output() -> Self {
        let mut interp = Interpreter::new();
        let stdout_buf = std::sync::Arc::new(std::sync::Mutex::new(Vec::<u8>::new()));
        let stderr_buf = std::sync::Arc::new(std::sync::Mutex::new(Vec::<u8>::new()));
        interp.stdout = RefCell::new(Box::new(SharedBuf(stdout_buf.clone())));
        interp.stderr = RefCell::new(Box::new(SharedBuf(stderr_buf.clone())));
        Session {
            interpreter: interp,
            captured_stdout: Some(stdout_buf),
            captured_stderr: Some(stderr_buf),
        }
    }

    /// Return all output written to the interpreter's stdout writer so far.
    ///
    /// Only meaningful when the session was created with `new_with_captured_output()`.
    /// For sessions using real stdio this will return an empty string.
    pub fn captured_stdout(&self) -> String {
        match &self.captured_stdout {
            Some(buf) => {
                let guard = buf.lock().unwrap_or_else(|e| e.into_inner());
                String::from_utf8_lossy(&guard).into_owned()
            }
            None => String::new(),
        }
    }

    /// Return all output written to the interpreter's stderr writer so far.
    ///
    /// Only meaningful when the session was created with `new_with_captured_output()`.
    pub fn captured_stderr(&self) -> String {
        match &self.captured_stderr {
            Some(buf) => {
                let guard = buf.lock().unwrap_or_else(|e| e.into_inner());
                String::from_utf8_lossy(&guard).into_owned()
            }
            None => String::new(),
        }
    }

    pub fn eval_expr(&mut self, expr: &Expr) -> Result<EvalOutput, SessionError> {
        // Reset the invisible flag before evaluation so we can detect
        // whether invisible() was called during this eval.
        self.interpreter.last_value_invisible.set(false);
        let value = with_interpreter_state(&mut self.interpreter, |interp| interp.eval(expr))
            .map_err(SessionError::Runtime)?;
        // Check both the runtime flag (set by invisible()) and the syntactic test
        let runtime_invisible = self.interpreter.take_invisible();
        let syntactic_invisible = is_invisible_result(expr);
        Ok(EvalOutput {
            visible: !runtime_invisible && !syntactic_invisible,
            value,
        })
    }

    pub fn eval_source(&mut self, source: &str) -> Result<EvalOutput, SessionError> {
        let ast = parse_program(source).map_err(SessionError::Parse)?;
        self.eval_expr(&ast)
    }

    pub fn eval_file(&mut self, path: impl AsRef<Path>) -> Result<EvalOutput, SessionError> {
        let path = path.as_ref();
        info!(path = %path.display(), "loading source file");
        let source = read_source(path)?;
        let ast = match parse_program(&source) {
            Ok(ast) => ast,
            Err(mut err) => {
                err.filename = Some(path.display().to_string());
                return Err(SessionError::Parse(err));
            }
        };
        self.eval_expr(&ast)
    }

    pub fn interpreter(&self) -> &Interpreter {
        &self.interpreter
    }

    /// Set a per-interpreter R option (same effect as `options(name = value)` in R).
    pub fn set_option(&self, name: &str, value: RValue) {
        self.interpreter
            .options
            .borrow_mut()
            .insert(name.to_string(), value);
    }

    /// Update `getOption("width")` to match the current terminal width.
    /// Falls back to 80 columns if terminal size cannot be determined.
    pub fn sync_terminal_width(&self) {
        #[cfg(feature = "repl")]
        let cols = crossterm::terminal::size()
            .map(|(c, _)| i64::from(c).clamp(10, 10000))
            .unwrap_or(80);
        #[cfg(not(feature = "repl"))]
        let cols = 80i64;
        self.set_option(
            "width",
            RValue::vec(crate::interpreter::value::Vector::Integer(
                vec![Some(cols)].into(),
            )),
        );
    }

    /// Return a clone of the interpreter's interrupt flag.
    /// The caller (or a signal handler) can set it to `true` to interrupt
    /// the current computation.
    pub fn interrupt_flag(&self) -> Arc<AtomicBool> {
        self.interpreter.interrupt_flag()
    }

    /// Register a SIGINT handler that sets the interpreter's interrupt flag
    /// instead of killing the process. Returns `Ok(())` on success.
    ///
    /// This should be called once at startup (e.g. before entering the REPL).
    /// On platforms where SIGINT is not available this is a no-op.
    #[cfg(feature = "signal")]
    pub fn install_signal_handler(&self) -> std::io::Result<()> {
        #[cfg(unix)]
        {
            use signal_hook::consts::SIGINT;
            signal_hook::flag::register(SIGINT, self.interrupt_flag())?;
        }
        Ok(())
    }

    /// No-op stub when signal-hook is not available.
    #[cfg(not(feature = "signal"))]
    pub fn install_signal_handler(&self) -> std::io::Result<()> {
        Ok(())
    }
}

pub fn is_invisible_result(ast: &Expr) -> bool {
    match ast {
        Expr::Assign { .. } => true,
        Expr::For { .. } => true,
        Expr::While { .. } => true,
        Expr::Repeat { .. } => true,
        Expr::Call { func, .. } => {
            matches!(func.as_ref(), Expr::Symbol(name) if name == "invisible")
        }
        Expr::Program(exprs) => exprs.last().is_some_and(is_invisible_result),
        Expr::Block(exprs) => exprs.last().is_some_and(is_invisible_result),
        _ => false,
    }
}

fn read_source(path: &Path) -> Result<String, SessionError> {
    match fs::read_to_string(path) {
        Ok(source) => Ok(source),
        Err(source_err) if source_err.kind() == std::io::ErrorKind::InvalidData => fs::read(path)
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
            .map_err(|source| SessionError::CannotRead {
                path: path.display().to_string(),
                source,
            }),
        Err(source) => Err(SessionError::CannotRead {
            path: path.display().to_string(),
            source,
        }),
    }
}
