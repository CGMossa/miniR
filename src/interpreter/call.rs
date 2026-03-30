//! Call-dispatch state and helpers shared across evaluator and builtin code.

use std::collections::HashSet;

use smallvec::SmallVec;

use crate::interpreter::environment::Environment;
use crate::interpreter::value::RValue;
use crate::interpreter::{DiagnosticStyle, Interpreter};
use crate::parser::ast::Expr;

/// A single entry in a stack trace — a snapshot of one call frame.
#[derive(Debug, Clone)]
pub struct TraceEntry {
    /// The call expression (e.g., `f(x, y)`). None for anonymous calls.
    pub call: Option<Expr>,
}

pub(crate) fn retarget_call_expr(call_expr: Option<Expr>, target: &str) -> Option<Expr> {
    match call_expr {
        Some(Expr::Call { args, .. }) => Some(Expr::Call {
            func: Box::new(Expr::Symbol(target.to_string())),
            args,
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
    /// Only captures if `last_traceback` is currently empty (preserves the
    /// deepest trace as the error bubbles up through multiple frames).
    pub(crate) fn capture_traceback(&self) {
        let mut tb = self.last_traceback.borrow_mut();
        if !tb.is_empty() {
            return;
        }
        let frames = self.call_stack.borrow();
        *tb = frames
            .iter()
            .map(|f| TraceEntry {
                call: f.call.clone(),
            })
            .collect();
    }

    /// Clear the last traceback (called at top-level eval entry).
    pub(crate) fn clear_traceback(&self) {
        self.last_traceback.borrow_mut().clear();
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
            let call_str = match &entry.call {
                Some(expr) => deparse_expr(expr),
                None => "<anonymous>".to_string(),
            };
            lines.push(format!("{}: {}", i + 1, call_str));
        }
        Some(lines.join("\n"))
    }
}
