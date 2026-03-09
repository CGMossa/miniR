//! Pre-eval builtins — functions that intercept before argument evaluation.
//! Each is auto-registered via `#[pre_eval_builtin]`.
//! The interpreter is accessed via the thread-local `with_interpreter()`.

use crate::interpreter::environment::Environment;
use crate::interpreter::value::*;
use crate::interpreter::with_interpreter;
use crate::parser::ast::{Arg, Expr};
use newr_macros::pre_eval_builtin;

#[pre_eval_builtin(name = "tryCatch", min_args = 1)]
fn pre_eval_try_catch(args: &[Arg], env: &Environment) -> Result<RValue, RError> {
    // First unnamed arg is the expression to evaluate
    let expr = args
        .iter()
        .find(|a| a.name.is_none())
        .and_then(|a| a.value.as_ref());

    // Collect named handlers (error=, warning=, finally=)
    let mut error_handler = None;
    let mut finally_expr = None;
    with_interpreter(|interp| {
        for arg in args {
            match arg.name.as_deref() {
                Some("error") => {
                    if let Some(ref val_expr) = arg.value {
                        error_handler = Some(interp.eval_in(val_expr, env)?);
                    }
                }
                Some("finally") => {
                    finally_expr = arg.value.clone();
                }
                _ => {}
            }
        }
        Ok(())
    })?;

    let result = with_interpreter(|interp| match expr {
        Some(e) => match interp.eval_in(e, env) {
            Ok(val) => Ok(val),
            Err(err) => {
                if let Some(handler) = error_handler.clone() {
                    let err_msg = format!("{}", err);
                    let err_val = RValue::vec(Vector::Character(vec![Some(err_msg)].into()));
                    interp.call_function(&handler, &[err_val], &[], env)
                } else {
                    Err(err)
                }
            }
        },
        None => Ok(RValue::Null),
    });

    // Run finally block if present
    if let Some(ref fin) = finally_expr {
        with_interpreter(|interp| interp.eval_in(fin, env))?;
    }

    result
}

#[pre_eval_builtin(name = "try", min_args = 1)]
fn pre_eval_try(args: &[Arg], env: &Environment) -> Result<RValue, RError> {
    let expr = args
        .iter()
        .find(|a| a.name.is_none())
        .and_then(|a| a.value.as_ref());
    with_interpreter(|interp| match expr {
        Some(e) => match interp.eval_in(e, env) {
            Ok(val) => Ok(val),
            Err(err) => {
                let msg = format!("{}", err);
                eprintln!("Error in try : {}", msg);
                Ok(RValue::vec(Vector::Character(vec![Some(msg)].into())))
            }
        },
        None => Ok(RValue::Null),
    })
}

#[pre_eval_builtin(name = "quote", min_args = 1)]
fn pre_eval_quote(args: &[Arg], _env: &Environment) -> Result<RValue, RError> {
    match args.first().and_then(|a| a.value.as_ref()) {
        Some(expr) => Ok(RValue::Language(Box::new(expr.clone()))),
        None => Ok(RValue::Null),
    }
}

#[pre_eval_builtin(name = "substitute", min_args = 1)]
fn pre_eval_substitute(args: &[Arg], env: &Environment) -> Result<RValue, RError> {
    let expr = match args.first().and_then(|a| a.value.as_ref()) {
        Some(e) => e.clone(),
        None => return Ok(RValue::Null),
    };
    // Walk the AST and replace symbols with their values from the environment
    let substituted = substitute_expr(&expr, env);
    Ok(RValue::Language(Box::new(substituted)))
}

/// Walk an AST, replacing symbols with their values from the environment.
/// If a symbol is bound to an RValue::Language, splice in the inner Expr.
/// If bound to a literal value, convert to the appropriate Expr literal.
fn substitute_expr(expr: &Expr, env: &Environment) -> Expr {
    match expr {
        Expr::Symbol(name) => {
            if let Some(val) = env.get(name) {
                rvalue_to_expr(&val)
            } else {
                expr.clone()
            }
        }
        Expr::Call { func, args } => Expr::Call {
            func: Box::new(substitute_expr(func, env)),
            args: args
                .iter()
                .map(|a| Arg {
                    name: a.name.clone(),
                    value: a.value.as_ref().map(|v| substitute_expr(v, env)),
                })
                .collect(),
        },
        Expr::BinaryOp { op, lhs, rhs } => Expr::BinaryOp {
            op: *op,
            lhs: Box::new(substitute_expr(lhs, env)),
            rhs: Box::new(substitute_expr(rhs, env)),
        },
        Expr::UnaryOp { op, operand } => Expr::UnaryOp {
            op: *op,
            operand: Box::new(substitute_expr(operand, env)),
        },
        Expr::If {
            condition,
            then_body,
            else_body,
        } => Expr::If {
            condition: Box::new(substitute_expr(condition, env)),
            then_body: Box::new(substitute_expr(then_body, env)),
            else_body: else_body
                .as_ref()
                .map(|e| Box::new(substitute_expr(e, env))),
        },
        Expr::Block(exprs) => Expr::Block(exprs.iter().map(|e| substitute_expr(e, env)).collect()),
        // For other AST nodes, return as-is (can expand later)
        _ => expr.clone(),
    }
}

/// Convert an RValue back to an AST expression (for substitute).
fn rvalue_to_expr(val: &RValue) -> Expr {
    match val {
        RValue::Language(expr) => *expr.clone(),
        RValue::Null => Expr::Null,
        RValue::Vector(rv) => match &rv.inner {
            Vector::Double(d) if d.len() == 1 => match d[0] {
                Some(v) => Expr::Double(v),
                None => Expr::Na(crate::parser::ast::NaType::Real),
            },
            Vector::Integer(i) if i.len() == 1 => match i[0] {
                Some(v) => Expr::Integer(v),
                None => Expr::Na(crate::parser::ast::NaType::Integer),
            },
            Vector::Logical(l) if l.len() == 1 => match l[0] {
                Some(v) => Expr::Bool(v),
                None => Expr::Na(crate::parser::ast::NaType::Logical),
            },
            Vector::Character(c) if c.len() == 1 => match &c[0] {
                Some(v) => Expr::String(v.clone()),
                None => Expr::Na(crate::parser::ast::NaType::Character),
            },
            _ => Expr::Symbol(format!("{}", val)),
        },
        _ => Expr::Symbol(format!("{}", val)),
    }
}
