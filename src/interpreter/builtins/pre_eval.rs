//! Pre-eval builtins — functions that intercept before argument evaluation.
//! Each is auto-registered via `#[pre_eval_builtin]`.
//! The interpreter is accessed via the thread-local `with_interpreter()`.

use crate::interpreter::environment::Environment;
use crate::interpreter::value::*;
use crate::interpreter::with_interpreter;
use crate::parser::ast::{Arg, Expr};
use minir_macros::pre_eval_builtin;

#[pre_eval_builtin(name = "tryCatch", min_args = 1)]
fn pre_eval_try_catch(args: &[Arg], env: &Environment) -> Result<RValue, RError> {
    use crate::interpreter::ConditionHandler;

    // First unnamed arg is the expression to evaluate
    let expr = args
        .iter()
        .find(|a| a.name.is_none())
        .and_then(|a| a.value.as_ref());

    // Collect named handlers and finally expression
    let mut handlers: Vec<(String, RValue)> = Vec::new();
    let mut finally_expr = None;
    with_interpreter(|interp| {
        for arg in args {
            match arg.name.as_deref() {
                Some("finally") => {
                    finally_expr = arg.value.clone();
                }
                Some(class) => {
                    if let Some(ref val_expr) = arg.value {
                        let handler = interp.eval_in(val_expr, env)?;
                        handlers.push((class.to_string(), handler));
                    }
                }
                None => {} // the expression itself
            }
        }
        Ok::<(), RError>(())
    })?;

    // For non-error classes (warning, message, etc.), install withCallingHandlers-style
    // handlers that convert them to unwinding RError::Condition so tryCatch can catch them.
    let non_error_classes: Vec<String> = handlers
        .iter()
        .filter(|(c, _)| c != "error")
        .map(|(c, _)| c.clone())
        .collect();

    let unwind_handlers: Vec<ConditionHandler> = non_error_classes
        .iter()
        .map(|class| ConditionHandler {
            class: class.clone(),
            handler: RValue::Function(RFunction::Builtin {
                name: "tryCatch_unwinder".to_string(),
                func: |args, _named| {
                    // Re-raise the condition to unwind past tryCatch
                    let condition = args.first().cloned().unwrap_or(RValue::Null);
                    let cond_classes = get_class(&condition);
                    let kind = if cond_classes.iter().any(|c| c == "warning") {
                        ConditionKind::Warning
                    } else if cond_classes.iter().any(|c| c == "message") {
                        ConditionKind::Message
                    } else {
                        ConditionKind::Error
                    };
                    Err(RError::Condition { condition, kind })
                },
            }),
            env: env.clone(),
        })
        .collect();

    // Install non-error handlers if any, then evaluate
    let result = with_interpreter(|interp| {
        if !unwind_handlers.is_empty() {
            interp.condition_handlers.borrow_mut().push(unwind_handlers);
        }
        let eval_result = match expr {
            Some(e) => interp.eval_in(e, env).map_err(RError::from),
            None => Ok(RValue::Null),
        };
        if !non_error_classes.is_empty() {
            interp.condition_handlers.borrow_mut().pop();
        }

        match eval_result {
            Ok(val) => Ok(val),
            Err(RError::Condition { condition, kind }) => {
                // Match against handler classes
                let cond_classes = get_class(&condition);
                for (handler_class, handler) in &handlers {
                    if cond_classes.iter().any(|c| c == handler_class) {
                        return interp
                            .call_function(handler, std::slice::from_ref(&condition), &[], env)
                            .map_err(RError::from);
                    }
                }
                // No matching handler — re-raise
                Err(RError::Condition { condition, kind })
            }
            Err(other) => {
                // Non-condition errors: check for "error" handler
                if let Some((_, handler)) = handlers.iter().find(|(c, _)| c == "error") {
                    let err_msg = format!("{}", other);
                    let condition =
                        make_condition(&err_msg, &["simpleError", "error", "condition"]);
                    interp
                        .call_function(handler, &[condition], &[], env)
                        .map_err(RError::from)
                } else {
                    Err(other)
                }
            }
        }
    });

    // Run finally block if present
    if let Some(ref fin) = finally_expr {
        with_interpreter(|interp| interp.eval_in(fin, env).map_err(RError::from))?;
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
        Some(e) => match interp.eval_in(e, env).map_err(RError::from) {
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

#[pre_eval_builtin(name = "withCallingHandlers", min_args = 1)]
fn pre_eval_with_calling_handlers(args: &[Arg], env: &Environment) -> Result<RValue, RError> {
    use crate::interpreter::ConditionHandler;

    let expr = args
        .iter()
        .find(|a| a.name.is_none())
        .and_then(|a| a.value.as_ref());

    // Collect named handlers (class = handler_function)
    let mut handler_set: Vec<ConditionHandler> = Vec::new();
    with_interpreter(|interp| {
        for arg in args {
            if let Some(class) = &arg.name {
                if let Some(ref val_expr) = arg.value {
                    let handler = interp.eval_in(val_expr, env).map_err(RError::from)?;
                    handler_set.push(ConditionHandler {
                        class: class.clone(),
                        handler,
                        env: env.clone(),
                    });
                }
            }
        }
        Ok::<(), RError>(())
    })?;

    // Push handler set onto the stack, evaluate, then pop
    with_interpreter(|interp| {
        interp.condition_handlers.borrow_mut().push(handler_set);
        let result = match expr {
            Some(e) => interp.eval_in(e, env).map_err(RError::from),
            None => Ok(RValue::Null),
        };
        interp.condition_handlers.borrow_mut().pop();
        result
    })
}

#[pre_eval_builtin(name = "suppressWarnings", min_args = 1)]
fn pre_eval_suppress_warnings(args: &[Arg], env: &Environment) -> Result<RValue, RError> {
    use crate::interpreter::ConditionHandler;

    let expr = args
        .first()
        .and_then(|a| a.value.as_ref())
        .ok_or_else(|| RError::new(RErrorKind::Argument, "argument is missing".to_string()))?;

    // Create a handler that muffles warnings by signaling muffleWarning
    let muffle_handler = RValue::Function(RFunction::Builtin {
        name: "suppressWarnings_handler".to_string(),
        func: |_args, _named| Err(RError::other("muffleWarning".to_string())),
    });

    let handler_set = vec![ConditionHandler {
        class: "warning".to_string(),
        handler: muffle_handler,
        env: env.clone(),
    }];

    with_interpreter(|interp| {
        interp.condition_handlers.borrow_mut().push(handler_set);
        let result = interp.eval_in(expr, env).map_err(RError::from);
        interp.condition_handlers.borrow_mut().pop();
        result
    })
}

#[pre_eval_builtin(name = "suppressMessages", min_args = 1)]
fn pre_eval_suppress_messages(args: &[Arg], env: &Environment) -> Result<RValue, RError> {
    use crate::interpreter::ConditionHandler;

    let expr = args
        .first()
        .and_then(|a| a.value.as_ref())
        .ok_or_else(|| RError::new(RErrorKind::Argument, "argument is missing".to_string()))?;

    let muffle_handler = RValue::Function(RFunction::Builtin {
        name: "suppressMessages_handler".to_string(),
        func: |_args, _named| Err(RError::other("muffleMessage".to_string())),
    });

    let handler_set = vec![ConditionHandler {
        class: "message".to_string(),
        handler: muffle_handler,
        env: env.clone(),
    }];

    with_interpreter(|interp| {
        interp.condition_handlers.borrow_mut().push(handler_set);
        let result = interp.eval_in(expr, env).map_err(RError::from);
        interp.condition_handlers.borrow_mut().pop();
        result
    })
}

#[pre_eval_builtin(name = "on.exit")]
fn pre_eval_on_exit(args: &[Arg], env: &Environment) -> Result<RValue, RError> {
    let expr = args.first().and_then(|a| a.value.as_ref()).cloned();

    // Check add= argument (default FALSE — replace existing on.exit)
    let add = with_interpreter(|interp| -> Result<bool, RError> {
        // Check named add= first
        for arg in args.iter().skip(1) {
            if arg.name.as_deref() == Some("add") {
                if let Some(ref val_expr) = arg.value {
                    let val = interp.eval_in(val_expr, env)?;
                    return Ok(val
                        .as_vector()
                        .and_then(|v| v.as_logical_scalar())
                        .unwrap_or(false));
                }
            }
        }
        // Check second positional arg
        if let Some(arg) = args.get(1) {
            if arg.name.is_none() {
                if let Some(ref val_expr) = arg.value {
                    let val = interp.eval_in(val_expr, env)?;
                    return Ok(val
                        .as_vector()
                        .and_then(|v| v.as_logical_scalar())
                        .unwrap_or(false));
                }
            }
        }
        Ok(false)
    })?;

    match expr {
        Some(e) => env.push_on_exit(e, add),
        None => {
            // on.exit() with no args clears on.exit handlers
            env.take_on_exit();
        }
    }

    Ok(RValue::Null)
}

#[pre_eval_builtin(name = "missing", min_args = 1)]
fn pre_eval_missing(args: &[Arg], _env: &Environment) -> Result<RValue, RError> {
    let expr = args
        .first()
        .and_then(|a| a.value.as_ref())
        .ok_or_else(|| RError::other("'missing(x)' did not find an argument".to_string()))?;

    let is_missing = with_interpreter(|interp| {
        let frame = interp
            .current_call_frame()
            .ok_or_else(|| RError::other("'missing(x)' did not find an argument".to_string()))?;

        match expr {
            Expr::Symbol(name) => {
                if !frame.formal_args.contains(name) {
                    return Err(RError::other(format!(
                        "'missing({})' did not find an argument",
                        name
                    )));
                }
                Ok(!frame.supplied_args.contains(name))
            }
            Expr::Dots => {
                if !frame.formal_args.contains("...") {
                    return Err(RError::other("'missing(...)' did not find an argument"));
                }
                let dots_len = match frame.env.get("...") {
                    Some(RValue::List(list)) => list.values.len(),
                    _ => 0,
                };
                Ok(dots_len == 0)
            }
            Expr::DotDot(n) => {
                if !frame.formal_args.contains("...") {
                    return Err(RError::other("'missing(...)' did not find an argument"));
                }
                let dots_len = match frame.env.get("...") {
                    Some(RValue::List(list)) => list.values.len(),
                    _ => 0,
                };
                Ok(dots_len < usize::try_from(*n).unwrap_or(0))
            }
            _ => Err(RError::other("invalid use of 'missing'".to_string())),
        }
    })?;

    Ok(RValue::vec(Vector::Logical(vec![Some(is_missing)].into())))
}

#[pre_eval_builtin(name = "quote", min_args = 1)]
fn pre_eval_quote(args: &[Arg], _env: &Environment) -> Result<RValue, RError> {
    match args.first().and_then(|a| a.value.as_ref()) {
        Some(expr) => Ok(RValue::Language(Language::new(expr.clone()))),
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
    Ok(RValue::Language(Language::new(substituted)))
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

#[pre_eval_builtin(name = "evalq", min_args = 1)]
fn pre_eval_evalq(args: &[Arg], env: &Environment) -> Result<RValue, RError> {
    // evalq(expr, envir) is equivalent to eval(quote(expr), envir)
    // First arg is the expression to quote-then-eval
    let expr = match args.first().and_then(|a| a.value.as_ref()) {
        Some(e) => e,
        None => return Ok(RValue::Null),
    };

    // Determine evaluation environment from second arg or named envir=
    let eval_env = with_interpreter(|interp| -> Result<Option<Environment>, RError> {
        // Check named envir= first
        for arg in args.iter().skip(1) {
            if arg.name.as_deref() == Some("envir") {
                if let Some(ref val_expr) = arg.value {
                    let val = interp.eval_in(val_expr, env)?;
                    if let RValue::Environment(e) = val {
                        return Ok(Some(e));
                    }
                }
            }
        }
        // Check second positional arg
        if let Some(arg) = args.get(1) {
            if arg.name.is_none() {
                if let Some(ref val_expr) = arg.value {
                    let val = interp.eval_in(val_expr, env)?;
                    if let RValue::Environment(e) = val {
                        return Ok(Some(e));
                    }
                }
            }
        }
        Ok(None)
    })?;

    let target_env = eval_env.unwrap_or_else(|| env.clone());
    with_interpreter(|interp| interp.eval_in(expr, &target_env)).map_err(RError::from)
}

#[pre_eval_builtin(name = "bquote", min_args = 1)]
fn pre_eval_bquote(args: &[Arg], env: &Environment) -> Result<RValue, RError> {
    // bquote(expr) is like quote() but evaluates anything wrapped in .()
    let expr = match args.first().and_then(|a| a.value.as_ref()) {
        Some(e) => e.clone(),
        None => return Ok(RValue::Null),
    };
    let result = bquote_expr(&expr, env)?;
    Ok(RValue::Language(Language::new(result)))
}

/// Walk an AST for bquote: evaluate .() splice expressions, leave everything else quoted.
fn bquote_expr(expr: &Expr, env: &Environment) -> Result<Expr, RError> {
    match expr {
        // Check for .(expr) — a call to `.` with one argument
        Expr::Call { func, args } => {
            if let Expr::Symbol(name) = func.as_ref() {
                if name == "." && args.len() == 1 {
                    // Evaluate the inner expression
                    if let Some(ref inner) = args[0].value {
                        let val = with_interpreter(|interp| interp.eval_in(inner, env))?;
                        return Ok(rvalue_to_expr(&val));
                    }
                }
            }
            // Not a .() call — recurse into func and args
            let new_func = Box::new(bquote_expr(func, env)?);
            let new_args: Result<Vec<Arg>, RError> = args
                .iter()
                .map(|a| {
                    Ok(Arg {
                        name: a.name.clone(),
                        value: match &a.value {
                            Some(v) => Some(bquote_expr(v, env)?),
                            None => None,
                        },
                    })
                })
                .collect();
            Ok(Expr::Call {
                func: new_func,
                args: new_args?,
            })
        }
        Expr::BinaryOp { op, lhs, rhs } => Ok(Expr::BinaryOp {
            op: *op,
            lhs: Box::new(bquote_expr(lhs, env)?),
            rhs: Box::new(bquote_expr(rhs, env)?),
        }),
        Expr::UnaryOp { op, operand } => Ok(Expr::UnaryOp {
            op: *op,
            operand: Box::new(bquote_expr(operand, env)?),
        }),
        Expr::If {
            condition,
            then_body,
            else_body,
        } => Ok(Expr::If {
            condition: Box::new(bquote_expr(condition, env)?),
            then_body: Box::new(bquote_expr(then_body, env)?),
            else_body: match else_body {
                Some(e) => Some(Box::new(bquote_expr(e, env)?)),
                None => None,
            },
        }),
        Expr::Block(exprs) => {
            let new_exprs: Result<Vec<Expr>, RError> =
                exprs.iter().map(|e| bquote_expr(e, env)).collect();
            Ok(Expr::Block(new_exprs?))
        }
        // Everything else stays as-is
        _ => Ok(expr.clone()),
    }
}

#[pre_eval_builtin(name = "withVisible", min_args = 1)]
fn pre_eval_with_visible(args: &[Arg], env: &Environment) -> Result<RValue, RError> {
    let expr = args
        .first()
        .and_then(|a| a.value.as_ref())
        .ok_or_else(|| RError::new(RErrorKind::Argument, "argument 'x' is missing".to_string()))?;

    let value = with_interpreter(|interp| interp.eval_in(expr, env))?;

    // We don't track visibility yet, so always TRUE
    Ok(RValue::List(RList::new(vec![
        (Some("value".to_string()), value),
        (
            Some("visible".to_string()),
            RValue::vec(Vector::Logical(vec![Some(true)].into())),
        ),
    ])))
}

/// `expression(...)` — construct an expression object from unevaluated arguments.
/// Returns a list of Language objects, each wrapping the unevaluated expression.
#[pre_eval_builtin(name = "expression")]
fn pre_eval_expression(args: &[Arg], _env: &Environment) -> Result<RValue, RError> {
    let entries: Vec<(Option<String>, RValue)> = args
        .iter()
        .filter_map(|a| {
            a.value
                .as_ref()
                .map(|expr| (None, RValue::Language(Language::new(expr.clone()))))
        })
        .collect();
    Ok(RValue::List(RList::new(entries)))
}

/// Convert an RValue back to an AST expression (for substitute).
fn rvalue_to_expr(val: &RValue) -> Expr {
    match val {
        RValue::Language(expr) => *expr.0.clone(),
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
