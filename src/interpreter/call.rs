//! Call-dispatch state and helpers shared across evaluator and builtin code.

use std::collections::HashSet;

use crate::interpreter::environment::Environment;
use crate::interpreter::value::RValue;
use crate::interpreter::Interpreter;
use crate::parser::ast::Expr;

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
    pub supplied_positional: Vec<RValue>,
    pub supplied_named: Vec<(String, RValue)>,
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
}
