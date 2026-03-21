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
    let function = resolve_callable_for_call(interp, func, env)?;
    let call_expr = Expr::Call {
        func: Box::new(func.clone()),
        args: args.to_vec(),
    };

    if let Some(result) = try_call_special_builtin(interp, &function, args, env)? {
        return Ok(result);
    }

    // For closures, store the original source expressions as promise exprs
    // so that substitute() can access the unevaluated expressions.
    // Use lenient evaluation so that `f(a+b)` where `a` is unbound doesn't
    // fail immediately — substitute() can still access the expression.
    if let RValue::Function(RFunction::Closure {
        params,
        body,
        env: closure_env,
    }) = &function
    {
        let (positional, named) = evaluate_call_arguments_lenient(interp, args, env);
        return call_closure_with_promises(
            interp,
            params,
            body,
            closure_env,
            &function,
            &positional,
            &named,
            args,
            env,
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

            // Reorder args so positional slots match formal parameter order,
            // regardless of whether the user passed args by name or position.
            let (reordered_pos, remaining_named) = reorder_builtin_args(positional, named, formals);
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

/// Evaluate call arguments leniently for closure calls.
///
/// Like `evaluate_call_arguments`, but when evaluation of an individual
/// argument fails (e.g., `f(a+b)` where `a` is unbound), stores the
/// unevaluated expression as a Language value instead of propagating the error.
/// This enables `substitute()` to work with expressions that reference
/// unbound symbols, matching R's lazy evaluation semantics.
fn evaluate_call_arguments_lenient(
    interp: &Interpreter,
    args: &[Arg],
    env: &Environment,
) -> EvaluatedCallArgs {
    let mut positional = PositionalArgs::new();
    let mut named = NamedArgs::new();

    for arg in args {
        if let Some(name) = &arg.name {
            if let Some(val_expr) = &arg.value {
                let val = interp
                    .eval_in(val_expr, env)
                    .unwrap_or_else(|_| RValue::Language(Language::new(val_expr.clone())));
                named.push((name.clone(), val));
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
            let val = interp
                .eval_in(val_expr, env)
                .unwrap_or_else(|_| RValue::Language(Language::new(val_expr.clone())));
            positional.push(val);
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
    interp.call_stack.borrow_mut().pop();

    result
}

/// Call a closure with eagerly evaluated arguments, but also store the original
/// source expressions as promise exprs on the call environment. This enables
/// `substitute()` to access the original unevaluated expressions while
/// keeping full backward compatibility with match.call, sys.call, UseMethod, etc.
#[allow(clippy::too_many_arguments)]
fn call_closure_with_promises(
    interp: &Interpreter,
    params: &[Param],
    body: &Expr,
    closure_env: &Environment,
    func: &RValue,
    positional: &[RValue],
    named: &[(String, RValue)],
    raw_args: &[Arg],
    _calling_env: &Environment,
    call_expr: Expr,
) -> Result<RValue, RFlow> {
    // Use the standard binding mechanism for argument matching
    let bound = interp.bind_closure_call(
        params,
        positional,
        named,
        closure_env,
        func,
        Some(call_expr),
    )?;
    let call_env = bound.env;

    // Store the original source expressions as promise exprs so that
    // substitute() can retrieve them. We match raw_args to formal params
    // using the same three-pass algorithm.
    store_promise_exprs_from_args(params, raw_args, &call_env);

    interp.call_stack.borrow_mut().push(bound.frame);
    let result = match interp.eval_in(body, &call_env) {
        Ok(value) => Ok(value),
        Err(RFlow::Signal(RSignal::Return(value))) => Ok(value),
        Err(err) => Err(err),
    };
    run_on_exit_handlers(interp, &call_env);
    interp.call_stack.borrow_mut().pop();

    result
}

/// Store the original source expressions from raw call arguments onto the call
/// environment as promise expressions. This uses the same three-pass matching
/// (exact name, partial prefix, positional fill) to map each Arg's expression
/// to the correct formal parameter name.
fn store_promise_exprs_from_args(params: &[Param], raw_args: &[Arg], call_env: &Environment) {
    use std::collections::{HashMap, HashSet};

    // Separate positional and named arg expressions
    let mut positional_exprs: Vec<Option<Expr>> = Vec::new();
    let mut named_arg_exprs: Vec<(String, Option<Expr>)> = Vec::new();

    for arg in raw_args {
        if let Some(name) = &arg.name {
            named_arg_exprs.push((name.clone(), arg.value.clone()));
            continue;
        }
        let Some(val_expr) = &arg.value else {
            continue;
        };
        if matches!(val_expr, Expr::Dots) {
            // Skip dots — they don't have source expressions
        } else {
            positional_exprs.push(Some(val_expr.clone()));
        }
    }

    let formal_names: Vec<&str> = params
        .iter()
        .filter(|p| !p.is_dots)
        .map(|p| p.name.as_str())
        .collect();

    let mut named_to_formal: HashMap<usize, &str> = HashMap::new();
    let mut matched_formals: HashSet<&str> = HashSet::new();

    // Pass 1: Exact name matching
    for (i, (arg_name, _)) in named_arg_exprs.iter().enumerate() {
        if let Some(&formal) = formal_names.iter().find(|&&f| f == arg_name) {
            if !matched_formals.contains(formal) {
                matched_formals.insert(formal);
                named_to_formal.insert(i, formal);
            }
        }
    }

    // Pass 2: Partial (prefix) matching
    for (i, (arg_name, _)) in named_arg_exprs.iter().enumerate() {
        if named_to_formal.contains_key(&i) {
            continue;
        }
        let candidates: Vec<&str> = formal_names
            .iter()
            .filter(|&&f| !matched_formals.contains(f) && f.starts_with(arg_name.as_str()))
            .copied()
            .collect();
        if candidates.len() == 1 {
            matched_formals.insert(candidates[0]);
            named_to_formal.insert(i, candidates[0]);
        }
    }

    // Build reverse map: formal_name -> named_arg_index
    let formal_to_named: HashMap<&str, usize> = named_to_formal
        .iter()
        .map(|(&idx, &formal)| (formal, idx))
        .collect();

    // Pass 3: Store promise expressions
    let mut pos_idx = 0usize;
    for param in params {
        if param.is_dots {
            pos_idx = positional_exprs.len();
            continue;
        }
        if let Some(&named_idx) = formal_to_named.get(param.name.as_str()) {
            if let Some(e) = &named_arg_exprs[named_idx].1 {
                call_env.set_promise_expr(param.name.clone(), e.clone());
            }
        } else if pos_idx < positional_exprs.len() {
            if let Some(Some(e)) = positional_exprs.get(pos_idx) {
                call_env.set_promise_expr(param.name.clone(), e.clone());
            }
            pos_idx += 1;
        }
    }
}

fn run_on_exit_handlers(interp: &Interpreter, call_env: &Environment) {
    let on_exit_exprs = call_env.take_on_exit();
    for expr in &on_exit_exprs {
        let _ = interp.eval_in(expr, call_env);
    }
}
