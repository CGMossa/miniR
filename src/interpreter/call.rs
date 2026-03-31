//! Call-dispatch state and helpers shared across evaluator and builtin code.

use std::collections::HashSet;

use smallvec::SmallVec;

use crate::interpreter::environment::Environment;
use crate::interpreter::value::RValue;
use crate::interpreter::{DiagnosticStyle, Interpreter};
use crate::parser::ast::Expr;

/// Convert a byte offset in source text to a 1-based line number.
fn byte_offset_to_line(source: &str, offset: usize) -> usize {
    source[..offset.min(source.len())]
        .bytes()
        .filter(|&b| b == b'\n')
        .count()
        + 1
}

/// Raw native backtrace captured from a C error (Rf_error).
/// Contains instruction pointer addresses from the C stack at the point
/// of the error, before longjmp destroyed the frames.
#[derive(Debug, Clone, Default)]
pub struct NativeBacktrace {
    /// Raw instruction pointer addresses from the C stack.
    pub frames: Vec<usize>,
}

/// A single entry in a stack trace — a snapshot of one call frame.
#[derive(Debug, Clone)]
pub struct TraceEntry {
    /// The call expression (e.g., `f(x, y)`). None for anonymous calls.
    pub call: Option<Expr>,
    /// Native backtrace if this frame was a .Call/.C that errored in C code.
    pub native_backtrace: Option<NativeBacktrace>,
    /// True if this is a C→R boundary (C code called Rf_eval back into R).
    pub is_native_boundary: bool,
    /// Source file and text for resolving span → file:line (captured at traceback time).
    pub source_context: Option<(String, String)>,
}

pub(crate) fn retarget_call_expr(call_expr: Option<Expr>, target: &str) -> Option<Expr> {
    match call_expr {
        Some(Expr::Call { args, .. }) => Some(Expr::Call {
            func: Box::new(Expr::Symbol(target.to_string())),
            args,
            span: None,
        }),
        _ => None,
    }
}

/// Context for S3 method dispatch — tracks which class was dispatched and the
/// remaining classes in the chain (for NextMethod).
#[derive(Debug, Clone)]
pub(crate) struct S3DispatchContext {
    pub generic: String,
    pub classes: Vec<String>,
    pub class_index: usize,
    pub object: RValue,
}

#[derive(Debug, Clone)]
pub(crate) struct CallFrame {
    pub call: Option<Expr>,
    pub function: RValue,
    pub env: Environment,
    pub formal_args: HashSet<String>,
    pub supplied_args: HashSet<String>,
    pub supplied_positional: SmallVec<[RValue; 4]>,
    pub supplied_named: SmallVec<[(String, RValue); 2]>,
    pub supplied_arg_count: usize,
    /// True if this frame is a synthetic boundary marker for a C→R callback
    /// (e.g., C code called Rf_eval which re-entered the R interpreter).
    pub is_native_boundary: bool,
}

#[derive(Clone, Copy)]
pub struct BuiltinContext<'a> {
    interpreter: &'a Interpreter,
    env: &'a Environment,
}

impl<'a> BuiltinContext<'a> {
    pub(crate) fn new(interpreter: &'a Interpreter, env: &'a Environment) -> Self {
        Self { interpreter, env }
    }

    pub fn env(&self) -> &'a Environment {
        self.env
    }

    pub fn interpreter(&self) -> &'a Interpreter {
        self.interpreter
    }

    pub fn with_interpreter<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&Interpreter) -> R,
    {
        f(self.interpreter)
    }

    /// Write a message to the interpreter's stdout writer.
    pub fn write(&self, msg: &str) {
        self.interpreter.write_stdout(msg);
    }

    /// Write a message to the interpreter's stderr writer.
    pub fn write_err(&self, msg: &str) {
        self.interpreter.write_stderr(msg);
    }

    /// Write a colored diagnostic message to the interpreter's stderr writer.
    ///
    /// When the `color` feature is enabled and stderr is a terminal, the
    /// message is written with the color corresponding to the given style.
    /// Otherwise, falls back to plain uncolored text.
    pub fn write_err_colored(&self, msg: &str, style: DiagnosticStyle) {
        self.interpreter.write_stderr_colored(msg, style);
    }
}

impl Interpreter {
    pub(crate) fn current_call_frame(&self) -> Option<CallFrame> {
        self.call_stack.borrow().last().cloned()
    }

    pub(crate) fn call_frame(&self, which: usize) -> Option<CallFrame> {
        self.call_stack
            .borrow()
            .get(which.saturating_sub(1))
            .cloned()
    }

    pub(crate) fn call_frames(&self) -> Vec<CallFrame> {
        self.call_stack.borrow().clone()
    }

    pub(crate) fn current_call_expr(&self) -> Option<Expr> {
        self.current_call_frame().and_then(|frame| frame.call)
    }

    /// Snapshot the current call stack into `last_traceback`.
    ///
    /// Within the same top-level `eval()` (same generation), only the deepest
    /// trace is kept (the first capture wins as the error bubbles up and frames
    /// are popped). Across different evals, a new error always replaces the old
    /// traceback, matching R's `traceback()` behavior.
    pub(crate) fn capture_traceback(&self) {
        let current_gen = self.traceback_generation.get();
        let captured_gen = self.traceback_captured_generation.get();
        let mut tb = self.last_traceback.borrow_mut();

        // Same eval: only keep the deepest (first) capture
        if current_gen == captured_gen && !tb.is_empty() {
            return;
        }
        self.traceback_captured_generation.set(current_gen);
        // Consume any pending native backtrace (set by dot_call/dot_c on C error).
        #[cfg(feature = "native")]
        let pending_native = self.pending_native_backtrace.borrow_mut().take();

        // Snapshot current source context for span resolution
        let source_ctx = self.source_stack.borrow().last().cloned();

        let frames = self.call_stack.borrow();
        let len = frames.len();
        *tb = frames
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let native_backtrace = {
                    #[cfg(feature = "native")]
                    {
                        // Attach to the innermost (last) frame
                        if i == len - 1 {
                            pending_native.clone()
                        } else {
                            None
                        }
                    }
                    #[cfg(not(feature = "native"))]
                    {
                        let _ = i;
                        None
                    }
                };
                TraceEntry {
                    call: f.call.clone(),
                    native_backtrace,
                    is_native_boundary: f.is_native_boundary,
                    source_context: source_ctx.clone(),
                }
            })
            .collect();
    }

    /// Format the last traceback for display, R-style (deepest frame first).
    /// Returns `None` if there is no traceback.
    pub fn format_traceback(&self) -> Option<String> {
        use crate::interpreter::value::deparse_expr;

        let tb = self.last_traceback.borrow();
        if tb.is_empty() {
            return None;
        }
        let mut lines = Vec::with_capacity(tb.len());
        for (i, entry) in tb.iter().enumerate().rev() {
            if entry.is_native_boundary {
                lines.push("   --- entered native code (Rf_eval callback) ---".to_string());
                continue;
            }
            let call_str = match &entry.call {
                Some(expr) => deparse_expr(expr),
                None => "<anonymous>".to_string(),
            };

            // Resolve span to file:line if available
            let location = entry
                .call
                .as_ref()
                .and_then(|expr| match expr {
                    Expr::Call {
                        span: Some(span), ..
                    } => Some(span),
                    _ => None,
                })
                .and_then(|span| {
                    let (filename, source_text) = entry.source_context.as_ref()?;
                    let line = byte_offset_to_line(source_text, span.start as usize);
                    Some(format!(" at {}:{}", filename, line))
                })
                .unwrap_or_default();

            lines.push(format!("{}: {}{}", i + 1, call_str, location));

            // Append resolved native frames if this entry has a C backtrace
            #[cfg(feature = "native")]
            if let Some(ref bt) = entry.native_backtrace {
                if !bt.frames.is_empty() {
                    let resolved = crate::interpreter::native::stacktrace::resolve_native_backtrace(
                        &bt.frames,
                    );
                    if !resolved.is_empty() {
                        lines.push(
                            crate::interpreter::native::stacktrace::format_native_frames(&resolved),
                        );
                    }
                }
            }
        }
        Some(lines.join("\n"))
    }
}
