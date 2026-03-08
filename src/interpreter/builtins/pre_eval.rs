//! Pre-eval builtins — functions that intercept before argument evaluation.
//! Each is auto-registered via `#[pre_eval_builtin]`.

use crate::interpreter::environment::Environment;
use crate::interpreter::value::*;
use crate::interpreter::Interpreter;
use crate::parser::ast::Arg;
use newr_macros::pre_eval_builtin;

#[pre_eval_builtin(name = "tryCatch", min_args = 1)]
fn pre_eval_try_catch(
    interp: &mut Interpreter,
    args: &[Arg],
    env: &Environment,
) -> Result<RValue, RError> {
    // First unnamed arg is the expression to evaluate
    let expr = args
        .iter()
        .find(|a| a.name.is_none())
        .and_then(|a| a.value.as_ref());

    // Collect named handlers (error=, warning=, finally=)
    let mut error_handler = None;
    let mut finally_expr = None;
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

    let result = match expr {
        Some(e) => match interp.eval_in(e, env) {
            Ok(val) => Ok(val),
            Err(err) => {
                if let Some(handler) = error_handler {
                    let err_msg = format!("{}", err);
                    let err_val = RValue::Vector(Vector::Character(vec![Some(err_msg)]));
                    interp.call_function(&handler, &[err_val], &[], env)
                } else {
                    Err(err)
                }
            }
        },
        None => Ok(RValue::Null),
    };

    // Run finally block if present
    if let Some(ref fin) = finally_expr {
        interp.eval_in(fin, env)?;
    }

    result
}

#[pre_eval_builtin(name = "try", min_args = 1)]
fn pre_eval_try(
    interp: &mut Interpreter,
    args: &[Arg],
    env: &Environment,
) -> Result<RValue, RError> {
    let expr = args
        .iter()
        .find(|a| a.name.is_none())
        .and_then(|a| a.value.as_ref());
    match expr {
        Some(e) => match interp.eval_in(e, env) {
            Ok(val) => Ok(val),
            Err(err) => {
                let msg = format!("{}", err);
                eprintln!("Error in try : {}", msg);
                Ok(RValue::Vector(Vector::Character(vec![Some(msg)])))
            }
        },
        None => Ok(RValue::Null),
    }
}
