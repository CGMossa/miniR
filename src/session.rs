use std::fmt;
use std::fs;
use std::path::Path;

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
    Parse(ParseError),
    Runtime(RFlow),
    CannotRead {
        path: String,
        source: std::io::Error,
    },
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

pub struct Session {
    interpreter: Interpreter,
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
        }
    }

    pub fn eval_expr(&mut self, expr: &Expr) -> Result<EvalOutput, SessionError> {
        let value = with_interpreter_state(&mut self.interpreter, |interp| interp.eval(expr))
            .map_err(SessionError::Runtime)?;
        Ok(EvalOutput {
            visible: !is_invisible_result(expr) && !value.is_null(),
            value,
        })
    }

    pub fn eval_source(&mut self, source: &str) -> Result<EvalOutput, SessionError> {
        let ast = parse_program(source).map_err(SessionError::Parse)?;
        self.eval_expr(&ast)
    }

    pub fn eval_file(&mut self, path: impl AsRef<Path>) -> Result<EvalOutput, SessionError> {
        let path = path.as_ref();
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
