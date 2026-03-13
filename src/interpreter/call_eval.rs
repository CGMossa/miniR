//! Call evaluation and dispatch helpers used by the evaluator.

use crate::interpreter::environment::Environment;
use crate::interpreter::value::*;
use crate::interpreter::{BuiltinContext, Interpreter};
use crate::parser::ast::{Arg, Expr, Param};

type NamedArgs = Vec<(String, RValue)>;
type EvaluatedCallArgs = (Vec<RValue>, NamedArgs);

struct ClosureCall<'a> {
    func: &'a RValue,
    params: &'a [Param],
    body: &'a Expr,
    closure_env: &'a Environment,
    positional: &'a [RValue],
    named: &'a [(String, RValue)],
    call_expr: Option<Expr>,
}

impl Interpreter {
    pub(super) fn eval_call(
        &self,
        func: &Expr,
        args: &[Arg],
        env: &Environment,
    ) -> Result<RValue, RFlow> {
        eval_call(self, func, args, env)
    }

    pub fn call_function(
        &self,
        func: &RValue,
        positional: &[RValue],
        named: &[(String, RValue)],
        env: &Environment,
    ) -> Result<RValue, RFlow> {
        call_function(self, func, positional, named, env)
    }

    pub(crate) fn call_function_with_call(
        &self,
        func: &RValue,
        positional: &[RValue],
        named: &[(String, RValue)],
        env: &Environment,
        call_expr: Option<Expr>,
    ) -> Result<RValue, RFlow> {
        call_function_with_call(self, func, positional, named, env, call_expr)
    }
}

pub(super) fn eval_call(
    interp: &Interpreter,
    func: &Expr,
    args: &[Arg],
    env: &Environment,
) -> Result<RValue, RFlow> {
    let function = resolve_callable_for_call(interp, func, env)?;
    let call_expr = Expr::Call {
        func: Box::new(func.clone()),
        args: args.to_vec(),
    };

    if let Some(result) = try_call_special_builtin(interp, &function, args, env)? {
        return Ok(result);
    }

    let (positional, named) = evaluate_call_arguments(interp, args, env)?;
    call_function_with_call(interp, &function, &positional, &named, env, Some(call_expr))
}

pub(super) fn call_function(
    interp: &Interpreter,
    func: &RValue,
    positional: &[RValue],
    named: &[(String, RValue)],
    env: &Environment,
) -> Result<RValue, RFlow> {
    call_function_with_call(interp, func, positional, named, env, None)
}

pub(crate) fn call_function_with_call(
    interp: &Interpreter,
    func: &RValue,
    positional: &[RValue],
    named: &[(String, RValue)],
    env: &Environment,
    call_expr: Option<Expr>,
) -> Result<RValue, RFlow> {
    match func {
        RValue::Function(RFunction::Builtin {
            name,
            implementation,
            max_args,
            ..
        }) => call_builtin(
            interp,
            name,
            implementation,
            *max_args,
            positional,
            named,
            env,
        ),
        RValue::Function(RFunction::Closure {
            params,
            body,
            env: closure_env,
        }) => call_closure(
            interp,
            ClosureCall {
                func,
                params,
                body,
                closure_env,
                positional,
                named,
                call_expr,
            },
        ),
        _ => Err(RError::new(
            RErrorKind::Type,
            "attempt to apply non-function".to_string(),
        )
        .into()),
    }
}

fn resolve_callable_for_call(
    interp: &Interpreter,
    func: &Expr,
    env: &Environment,
) -> Result<RValue, RFlow> {
    let value = interp.eval_in(func, env)?;
    if matches!(value, RValue::Function(_)) {
        return Ok(value);
    }

    if let Expr::Symbol(name) = func {
        return env
            .get_function(name)
            .ok_or_else(|| RError::other("attempt to apply non-function".to_string()).into());
    }

    Ok(value)
}

fn try_call_special_builtin(
    interp: &Interpreter,
    function: &RValue,
    args: &[Arg],
    env: &Environment,
) -> Result<Option<RValue>, RFlow> {
    let RValue::Function(RFunction::Builtin {
        name,
        implementation,
        max_args,
        ..
    }) = function
    else {
        return Ok(None);
    };

    if name == "UseMethod" {
        return interp.eval_use_method(args, env).map(Some);
    }

    if let BuiltinImplementation::PreEval(handler) = implementation {
        Interpreter::ensure_builtin_max_arity(name, *max_args, args.len()).map_err(RFlow::from)?;
        return handler(args, env).map_err(Into::into).map(Some);
    }

    Ok(None)
}

fn evaluate_call_arguments(
    interp: &Interpreter,
    args: &[Arg],
    env: &Environment,
) -> Result<EvaluatedCallArgs, RFlow> {
    let mut positional = Vec::new();
    let mut named = Vec::new();

    for arg in args {
        if let Some(name) = &arg.name {
            if let Some(val_expr) = &arg.value {
                named.push((name.clone(), interp.eval_in(val_expr, env)?));
            } else {
                named.push((name.clone(), RValue::Null));
            }
            continue;
        }

        let Some(val_expr) = &arg.value else {
            continue;
        };

        if matches!(val_expr, Expr::Dots) {
            expand_dots_arguments(env, &mut positional, &mut named);
        } else {
            positional.push(interp.eval_in(val_expr, env)?);
        }
    }

    Ok((positional, named))
}

fn expand_dots_arguments(
    env: &Environment,
    positional: &mut Vec<RValue>,
    named: &mut Vec<(String, RValue)>,
) {
    if let Some(RValue::List(list)) = env.get("...") {
        for (opt_name, value) in &list.values {
            if let Some(name) = opt_name {
                named.push((name.clone(), value.clone()));
            } else {
                positional.push(value.clone());
            }
        }
    }
}

fn call_builtin(
    interp: &Interpreter,
    name: &str,
    implementation: &BuiltinImplementation,
    max_args: Option<usize>,
    positional: &[RValue],
    named: &[(String, RValue)],
    env: &Environment,
) -> Result<RValue, RFlow> {
    let actual_args = positional.len() + named.len();
    Interpreter::ensure_builtin_max_arity(name, max_args, actual_args).map_err(RFlow::from)?;

    match implementation {
        BuiltinImplementation::Eager(func) => func(positional, named).map_err(Into::into),
        BuiltinImplementation::Interpreter(handler) => {
            handler(positional, named, &BuiltinContext::new(interp, env)).map_err(Into::into)
        }
        BuiltinImplementation::PreEval(_) => Err(RError::other(
            "internal error: pre-eval builtin reached eager dispatch".to_string(),
        )
        .into()),
    }
}

fn call_closure(interp: &Interpreter, closure_call: ClosureCall<'_>) -> Result<RValue, RFlow> {
    let bound = interp.bind_closure_call(
        closure_call.params,
        closure_call.positional,
        closure_call.named,
        closure_call.closure_env,
        closure_call.func,
        closure_call.call_expr,
    )?;
    let call_env = bound.env;

    interp.call_stack.borrow_mut().push(bound.frame);
    let result = match interp.eval_in(closure_call.body, &call_env) {
        Ok(value) => Ok(value),
        Err(RFlow::Signal(RSignal::Return(value))) => Ok(value),
        Err(err) => Err(err),
    };
    run_on_exit_handlers(interp, &call_env);
    interp.call_stack.borrow_mut().pop();

    result
}

fn run_on_exit_handlers(interp: &Interpreter, call_env: &Environment) {
    let on_exit_exprs = call_env.take_on_exit();
    for expr in &on_exit_exprs {
        let _ = interp.eval_in(expr, call_env);
    }
}
