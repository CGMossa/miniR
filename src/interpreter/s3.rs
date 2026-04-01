//! S3 method dispatch helpers for UseMethod/NextMethod and class-based lookup.

use tracing::debug;

use crate::interpreter::call::{retarget_call_expr, S3DispatchContext};
use crate::interpreter::environment::Environment;
use crate::interpreter::value::{RError, RFlow, RFunction, RSignal, RValue, Vector};
use crate::interpreter::Interpreter;
use crate::parser::ast::{Arg, Expr};

struct S3MethodCall<'a> {
    method: &'a RValue,
    method_name: &'a str,
    generic: &'a str,
    classes: &'a [String],
    class_index: usize,
    dispatch_object: RValue,
    positional: &'a [RValue],
    named: &'a [(String, RValue)],
    env: &'a Environment,
    call_expr: Option<Expr>,
}

impl Interpreter {
    pub(crate) fn dispatch_next_method(
        &self,
        positional: &[RValue],
        named: &[(String, RValue)],
        env: &Environment,
    ) -> Result<RValue, RFlow> {
        let ctx = self
            .s3_dispatch_stack
            .borrow()
            .last()
            .cloned()
            .ok_or_else(|| {
                RError::other("NextMethod called outside of a method dispatch".to_string())
            })?;

        let args: Vec<RValue> = if positional.is_empty() {
            vec![ctx.object.clone()]
        } else {
            positional.to_vec()
        };

        // Look up in environment chain first, then the S3 method registry
        for i in (ctx.class_index + 1)..ctx.classes.len() {
            let method_name = format!("{}.{}", ctx.generic, ctx.classes[i]);
            let method = env
                .get(&method_name)
                .or_else(|| self.lookup_s3_method(&ctx.generic, &ctx.classes[i]));
            if let Some(method) = method {
                return self.call_s3_method(S3MethodCall {
                    method: &method,
                    method_name: &method_name,
                    generic: &ctx.generic,
                    classes: &ctx.classes,
                    class_index: i,
                    dispatch_object: args.first().cloned().unwrap_or(RValue::Null),
                    positional: &args,
                    named,
                    env,
                    call_expr: self.current_call_expr(),
                });
            }
        }

        let default_name = format!("{}.default", ctx.generic);
        let default_method = env
            .get(&default_name)
            .or_else(|| self.lookup_s3_method(&ctx.generic, "default"));
        if let Some(method) = default_method {
            return self.call_s3_method(S3MethodCall {
                method: &method,
                method_name: &default_name,
                generic: &ctx.generic,
                classes: &ctx.classes,
                class_index: ctx.classes.len(),
                dispatch_object: args.first().cloned().unwrap_or(RValue::Null),
                positional: &args,
                named,
                env,
                call_expr: self.current_call_expr(),
            });
        }

        Err(RError::other(format!("no more methods to dispatch for '{}'", ctx.generic)).into())
    }

    pub(crate) fn eval_use_method(&self, args: &[Arg], env: &Environment) -> Result<RValue, RFlow> {
        let frame = self.current_call_frame().ok_or_else(|| {
            RError::other("'UseMethod' used in an inappropriate fashion".to_string())
        })?;

        let generic_expr = match args
            .iter()
            .find(|arg| arg.name.as_deref() == Some("generic"))
            .or_else(|| args.first())
            .and_then(|arg| arg.value.as_ref())
        {
            Some(expr) => expr,
            None => {
                return Err(RError::other("there must be a 'generic' argument".to_string()).into());
            }
        };

        let generic = self.eval_generic_name(generic_expr, env)?;

        let object_expr = args
            .iter()
            .find(|arg| arg.name.as_deref() == Some("object"))
            .or_else(|| args.get(1))
            .and_then(|arg| arg.value.as_ref());

        let dispatch_object = match object_expr {
            Some(expr) => Some(self.eval_in(expr, env)?),
            None => {
                // Force the default object (first param) — it may be a promise
                let obj = self.default_use_method_object(&frame)?;
                match obj {
                    Some(val) => Some(self.force_value(val)?),
                    None => None,
                }
            }
        };

        let value = self.dispatch_s3(
            &generic,
            &frame.supplied_positional,
            &frame.supplied_named,
            dispatch_object,
            env,
            frame.call.clone(),
        )?;

        Err(RSignal::Return(value).into())
    }

    fn eval_generic_name(&self, generic_expr: &Expr, env: &Environment) -> Result<String, RFlow> {
        let generic_value = self.eval_in(generic_expr, env)?;
        match generic_value {
            RValue::Vector(rv) => match &rv.inner {
                Vector::Character(values) if values.len() == 1 => {
                    values.first().cloned().flatten().ok_or_else(|| {
                        RError::other("'generic' argument must be a character string".to_string())
                            .into()
                    })
                }
                _ => Err(RError::other(
                    "'generic' argument must be a character string".to_string(),
                )
                .into()),
            },
            _ => Err(
                RError::other("'generic' argument must be a character string".to_string()).into(),
            ),
        }
    }

    fn default_use_method_object(
        &self,
        frame: &crate::interpreter::call::CallFrame,
    ) -> Result<Option<RValue>, RFlow> {
        match &frame.function {
            RValue::Function(RFunction::Closure { params, .. }) => match params.first() {
                Some(param) if param.is_dots => {
                    Ok(frame.env.get("...").and_then(|value| match value {
                        RValue::List(list) => list.values.first().map(|(_, value)| value.clone()),
                        _ => None,
                    }))
                }
                Some(param) => Ok(frame.env.get(&param.name)),
                None => Ok(None),
            },
            _ => Err(
                RError::other("'UseMethod' used in an inappropriate fashion".to_string()).into(),
            ),
        }
    }

    /// S3 method dispatch: look up generic.class in the environment chain,
    /// then fall back to the per-interpreter S3 method registry (populated
    /// by S3method() directives in NAMESPACE files).
    #[tracing::instrument(
        level = "debug",
        skip(self, positional, named, dispatch_object, env, call_expr)
    )]
    fn dispatch_s3(
        &self,
        generic: &str,
        positional: &[RValue],
        named: &[(String, RValue)],
        dispatch_object: Option<RValue>,
        env: &Environment,
        call_expr: Option<Expr>,
    ) -> Result<RValue, RFlow> {
        let raw_dispatch =
            dispatch_object.unwrap_or_else(|| positional.first().cloned().unwrap_or(RValue::Null));
        // Force promises so we can inspect the object's class
        let dispatch_object = self.force_value(raw_dispatch)?;
        let classes = self.s3_classes_for(&dispatch_object);

        // First pass: look up generic.class in the environment chain
        for (i, class) in classes.iter().enumerate() {
            let method_name = format!("{}.{}", generic, class);
            if let Some(method) = env.get(&method_name) {
                debug!(
                    generic,
                    method = method_name.as_str(),
                    "S3 dispatch resolved"
                );
                return self.call_s3_method(S3MethodCall {
                    method: &method,
                    method_name: &method_name,
                    generic,
                    classes: &classes,
                    class_index: i,
                    dispatch_object: dispatch_object.clone(),
                    positional,
                    named,
                    env,
                    call_expr: call_expr.clone(),
                });
            }
        }

        // Second pass: check the per-interpreter S3 method registry
        for (i, class) in classes.iter().enumerate() {
            if let Some(method) = self.lookup_s3_method(generic, class) {
                let method_name = format!("{}.{}", generic, class);
                debug!(
                    generic,
                    method = method_name.as_str(),
                    "S3 dispatch resolved (registry)"
                );
                return self.call_s3_method(S3MethodCall {
                    method: &method,
                    method_name: &method_name,
                    generic,
                    classes: &classes,
                    class_index: i,
                    dispatch_object: dispatch_object.clone(),
                    positional,
                    named,
                    env,
                    call_expr: call_expr.clone(),
                });
            }
        }

        // Fall back to generic.default in the environment chain
        let default_name = format!("{}.default", generic);
        if let Some(method) = env.get(&default_name) {
            debug!(
                generic,
                method = default_name.as_str(),
                "S3 dispatch resolved (default)"
            );
            return self.call_s3_method(S3MethodCall {
                method: &method,
                method_name: &default_name,
                generic,
                classes: &classes,
                class_index: classes.len(),
                dispatch_object: dispatch_object.clone(),
                positional,
                named,
                env,
                call_expr: call_expr.clone(),
            });
        }

        // Fall back to generic.default in the registry
        if let Some(method) = self.lookup_s3_method(generic, "default") {
            debug!(
                generic,
                method = default_name.as_str(),
                "S3 dispatch resolved (registry default)"
            );
            return self.call_s3_method(S3MethodCall {
                method: &method,
                method_name: &default_name,
                generic,
                classes: &classes,
                class_index: classes.len(),
                dispatch_object: dispatch_object.clone(),
                positional,
                named,
                env,
                call_expr,
            });
        }

        debug!(generic, ?classes, "S3 dispatch failed: no method found");
        Err(RError::other(format!(
            "no applicable method for '{}' applied to an object of class \"{}\"",
            generic,
            classes.first().unwrap_or(&"unknown".to_string())
        ))
        .into())
    }

    pub(crate) fn s3_classes_for(&self, dispatch_object: &RValue) -> Vec<String> {
        match dispatch_object {
            RValue::List(list) => {
                if let Some(RValue::Vector(rv)) = list.get_attr("class") {
                    if let Vector::Character(classes) = &rv.inner {
                        classes
                            .iter()
                            .filter_map(|class| class.clone())
                            .collect::<Vec<_>>()
                    } else {
                        vec!["list".to_string()]
                    }
                } else {
                    vec!["list".to_string()]
                }
            }
            RValue::Vector(rv) => rv.class().unwrap_or_else(|| match &rv.inner {
                Vector::Raw(_) => vec!["raw".to_string()],
                Vector::Logical(_) => vec!["logical".to_string()],
                Vector::Integer(_) => vec!["integer".to_string()],
                Vector::Double(_) => vec!["numeric".to_string()],
                Vector::Complex(_) => vec!["complex".to_string()],
                Vector::Character(_) => vec!["character".to_string()],
            }),
            RValue::Function(_) => vec!["function".to_string()],
            RValue::Null => vec!["NULL".to_string()],
            RValue::Language(lang) => lang.class().unwrap_or_default(),
            _ => vec![],
        }
    }

    fn call_s3_method(&self, dispatch: S3MethodCall<'_>) -> Result<RValue, RFlow> {
        let ctx = S3DispatchContext {
            generic: dispatch.generic.to_string(),
            classes: dispatch.classes.to_vec(),
            class_index: dispatch.class_index,
            object: dispatch.dispatch_object,
        };
        self.s3_dispatch_stack.borrow_mut().push(ctx);
        let method_call = retarget_call_expr(dispatch.call_expr, dispatch.method_name);
        let result = self.call_function_with_call(
            dispatch.method,
            dispatch.positional,
            dispatch.named,
            dispatch.env,
            method_call,
        );
        self.s3_dispatch_stack.borrow_mut().pop();
        result
    }
}
