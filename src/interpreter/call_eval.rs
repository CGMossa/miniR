//! Call evaluation and dispatch helpers used by the evaluator.

use smallvec::SmallVec;
use tracing::trace;

use crate::interpreter::environment::Environment;
use crate::interpreter::value::*;
use crate::interpreter::{BuiltinContext, Interpreter};
use crate::parser::ast::{Arg, Expr, Param};

type PositionalArgs = SmallVec<[RValue; 4]>;

type NamedArgs = SmallVec<[(String, RValue); 2]>;

type EvaluatedCallArgs = (PositionalArgs, NamedArgs);

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
    trace!(func = %expr_name(func), nargs = args.len(), "calling function");
    let function = resolve_callable_for_call(interp, func, env)?;
    let call_expr = Expr::Call {
        func: Box::new(func.clone()),
        args: args.to_vec(),
    };

    if let Some(result) = try_call_special_builtin(interp, &function, args, env)? {
        return Ok(result);
    }

    // For closures, create lazy promises instead of eagerly evaluating args.
    // This implements R's call-by-need semantics: arguments are only evaluated
    // when their value is first accessed.
    if let RValue::Function(RFunction::Closure {
        params,
        body,
        env: closure_env,
    }) = &function
    {
        let (positional, named) = create_promise_arguments(args, env);
        return call_closure_lazy(
            interp,
            params,
            body,
            closure_env,
            &function,
            &positional,
            &named,
            call_expr,
        );
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

#[tracing::instrument(level = "trace", name = "call_function", skip_all, fields(func_type))]
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
            min_args,
            max_args,
            formals,
        }) => {
            tracing::Span::current().record("func_type", "builtin");
            trace!(
                builtin = name.as_str(),
                positional = positional.len(),
                named = named.len(),
                "call builtin"
            );
            // Arity checks use the original arg counts (before reordering)
            // because reordering may duplicate named args into positional slots.
            let actual_args = positional.len() + named.len();
            Interpreter::ensure_builtin_min_arity(name, *min_args, actual_args)
                .map_err(RFlow::from)?;
            Interpreter::ensure_builtin_max_arity(name, *max_args, actual_args)
                .map_err(RFlow::from)?;

            // Force all promise arguments at the builtin boundary —
            // builtins expect concrete values, not promises.
            let (forced_pos, forced_named) = interp.force_args(positional, named)?;

            // Reorder args so positional slots match formal parameter order,
            // regardless of whether the user passed args by name or position.
            let (reordered_pos, remaining_named) =
                reorder_builtin_args(&forced_pos, &forced_named, formals);
            call_builtin(
                interp,
                name,
                implementation,
                None, // max already checked above
                &reordered_pos,
                &remaining_named,
                env,
            )
        }
        RValue::Function(RFunction::Closure {
            params,
            body,
            env: closure_env,
        }) => {
            tracing::Span::current().record("func_type", "closure");
            trace!(
                params = params.len(),
                positional = positional.len(),
                named = named.len(),
                "call closure"
            );
            call_closure(
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
            )
        }
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
        let ctx = BuiltinContext::new(interp, env);
        return handler(args, env, &ctx).map_err(Into::into).map(Some);
    }

    Ok(None)
}

fn evaluate_call_arguments(
    interp: &Interpreter,
    args: &[Arg],
    env: &Environment,
) -> Result<EvaluatedCallArgs, RFlow> {
    let mut positional = PositionalArgs::new();
    let mut named = NamedArgs::new();

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

/// Create lazy promise arguments for closure calls.
///
/// Instead of evaluating arguments eagerly, wraps each argument expression
/// in an `RValue::Promise` that captures the calling environment. The promise
/// is only forced (evaluated) when its value is actually needed.
///
/// `...` (dots) entries are forwarded as-is — if they already contain promises,
/// those promises are passed through without forcing.
fn create_promise_arguments(args: &[Arg], env: &Environment) -> EvaluatedCallArgs {
    let mut positional = PositionalArgs::new();
    let mut named = NamedArgs::new();

    for arg in args {
        if let Some(name) = &arg.name {
            if let Some(val_expr) = &arg.value {
                named.push((name.clone(), RValue::promise(val_expr.clone(), env.clone())));
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
            positional.push(RValue::promise(val_expr.clone(), env.clone()));
        }
    }

    (positional, named)
}

fn expand_dots_arguments(
    env: &Environment,
    positional: &mut PositionalArgs,
    named: &mut NamedArgs,
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

/// Reorder positional and named args to match the builtin's formal parameter order.
///
/// When `formals` is non-empty, named args that match a formal are placed at
/// the corresponding slot, and remaining positional args fill the unmatched
/// slots left-to-right.  This is the same matching R does for closures, applied
/// at the builtin dispatch boundary so every builtin benefits automatically.
///
/// When `formals` is empty (variadic / dots builtins), args pass through unchanged.
fn reorder_builtin_args(
    positional: &[RValue],
    named: &[(String, RValue)],
    formals: &[&str],
) -> (PositionalArgs, NamedArgs) {
    if formals.is_empty() || named.is_empty() {
        // No formals declared (variadic) or no named args — nothing to reorder.
        return (
            positional.iter().cloned().collect(),
            named.iter().cloned().collect(),
        );
    }

    let mut matched: Vec<Option<RValue>> = vec![None; formals.len()];

    // Phase 1: match named args to formals (exact match, then unique partial match).
    for (arg_name, value) in named {
        // Exact match
        if let Some(idx) = formals.iter().position(|f| *f == arg_name.as_str()) {
            matched[idx] = Some(value.clone());
            continue;
        }
        // Partial prefix match — only if unambiguous
        let candidates: Vec<usize> = formals
            .iter()
            .enumerate()
            .filter(|(_, f)| f.starts_with(arg_name.as_str()))
            .map(|(i, _)| i)
            .collect();
        if candidates.len() == 1 && matched[candidates[0]].is_none() {
            matched[candidates[0]] = Some(value.clone());
        }
    }

    // Phase 2: fill unmatched formal slots with positional args, left-to-right.
    let mut pos_iter = positional.iter();
    for slot in &mut matched {
        if slot.is_none() {
            if let Some(val) = pos_iter.next() {
                *slot = Some(val.clone());
            }
        }
    }

    // Build result: consecutive matched values from the start.
    // Stop at the first gap — builtins with gaps rely on named-arg lookup
    // (which still works because `named` is passed through unchanged).
    let mut result: SmallVec<[RValue; 4]> = SmallVec::new();
    for slot in &matched {
        match slot {
            Some(val) => result.push(val.clone()),
            None => break,
        }
    }

    // Append any overflow positional args (more positional args than formals).
    result.extend(pos_iter.cloned());

    // Keep the original `named` slice unchanged so builtins that look up
    // optional params by name (e.g. `named.find("tolerance")`) still work.
    (result, named.iter().cloned().collect())
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
    if result.is_err() {
        interp.capture_traceback();
    }
    interp.call_stack.borrow_mut().pop();

    result
}

/// Call a closure with lazy (promise) arguments. Arguments are bound as
/// `RValue::Promise` values in the call environment, only forced when accessed.
#[allow(clippy::too_many_arguments)]
fn call_closure_lazy(
    interp: &Interpreter,
    params: &[Param],
    body: &Expr,
    closure_env: &Environment,
    func: &RValue,
    positional: &[RValue],
    named: &[(String, RValue)],
    call_expr: Expr,
) -> Result<RValue, RFlow> {
    let bound = interp.bind_closure_call(
        params,
        positional,
        named,
        closure_env,
        func,
        Some(call_expr),
    )?;
    let call_env = bound.env;

    interp.call_stack.borrow_mut().push(bound.frame);
    let result = match interp.eval_in(body, &call_env) {
        Ok(value) => Ok(value),
        Err(RFlow::Signal(RSignal::Return(value))) => Ok(value),
        Err(err) => Err(err),
    };
    run_on_exit_handlers(interp, &call_env);
    if result.is_err() {
        interp.capture_traceback();
    }
    interp.call_stack.borrow_mut().pop();

    result
}

/// Extract a short display name from a call-position expression for tracing.
fn expr_name(expr: &Expr) -> &str {
    match expr {
        Expr::Symbol(name) => name.as_str(),
        Expr::NsGet { name, .. } | Expr::NsGetInt { name, .. } => name.as_str(),
        _ => "<expr>",
    }
}

fn run_on_exit_handlers(interp: &Interpreter, call_env: &Environment) {
    let on_exit_exprs = call_env.take_on_exit();
    for expr in &on_exit_exprs {
        // on.exit handlers run for side effects; errors are silently ignored (R semantics).
        if interp.eval_in(expr, call_env).is_err() {}
    }
}
