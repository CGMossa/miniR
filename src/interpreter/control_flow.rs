//! Control-flow and namespace evaluation helpers.

use crate::interpreter::environment::Environment;
use crate::interpreter::value::*;
use crate::interpreter::Interpreter;
use crate::parser::ast::Expr;

impl Interpreter {
    pub(super) fn eval_pipe(
        &self,
        lhs: &Expr,
        rhs: &Expr,
        env: &Environment,
    ) -> Result<RValue, RFlow> {
        let left_val = self.eval_in(lhs, env)?;
        match rhs {
            Expr::Call { func, args } => {
                let f = self.eval_in(func, env)?;
                let mut eval_args = vec![left_val];
                let mut named_args = Vec::new();
                for arg in args {
                    if let Some(name) = &arg.name {
                        if let Some(val_expr) = &arg.value {
                            named_args.push((name.clone(), self.eval_in(val_expr, env)?));
                        }
                    } else if let Some(val_expr) = &arg.value {
                        eval_args.push(self.eval_in(val_expr, env)?);
                    }
                }
                self.call_function(&f, &eval_args, &named_args, env)
            }
            Expr::Symbol(name) => {
                let f = env
                    .get(name)
                    .ok_or_else(|| RError::new(RErrorKind::Name, name.clone()))?;
                self.call_function(&f, &[left_val], &[], env)
            }
            _ => Err(RError::other("invalid use of pipe".to_string()).into()),
        }
    }

    pub(super) fn eval_if(
        &self,
        condition: &Expr,
        then_body: &Expr,
        else_body: Option<&Expr>,
        env: &Environment,
    ) -> Result<RValue, RFlow> {
        let cond = self.eval_in(condition, env)?;
        let test = match &cond {
            RValue::Vector(v) => v.as_logical_scalar(),
            _ => None,
        };
        match test {
            Some(true) => self.eval_in(then_body, env),
            Some(false) | None => {
                if let Some(else_expr) = else_body {
                    self.eval_in(else_expr, env)
                } else {
                    Ok(RValue::Null)
                }
            }
        }
    }

    pub(super) fn eval_while(
        &self,
        condition: &Expr,
        body: &Expr,
        env: &Environment,
    ) -> Result<RValue, RFlow> {
        loop {
            let cond = self.eval_in(condition, env)?;
            let test = match &cond {
                RValue::Vector(v) => v.as_logical_scalar().unwrap_or(false),
                _ => false,
            };
            if !test {
                break;
            }
            match self.eval_in(body, env) {
                Err(RFlow::Signal(RSignal::Break)) => break,
                Err(RFlow::Signal(RSignal::Next)) => continue,
                Err(err) => return Err(err),
                _ => {}
            }
        }
        Ok(RValue::Null)
    }

    pub(super) fn eval_repeat(&self, body: &Expr, env: &Environment) -> Result<RValue, RFlow> {
        loop {
            match self.eval_in(body, env) {
                Err(RFlow::Signal(RSignal::Break)) => break,
                Err(RFlow::Signal(RSignal::Next)) => continue,
                Err(err) => return Err(err),
                _ => {}
            }
        }
        Ok(RValue::Null)
    }

    pub(super) fn eval_ns_get(
        &self,
        namespace: &Expr,
        name: &str,
        env: &Environment,
    ) -> Result<RValue, RFlow> {
        let _ns = self.eval_in(namespace, env)?;
        env.get(name)
            .or_else(|| self.global_env.get(name))
            .ok_or_else(|| RError::new(RErrorKind::Name, format!("{}::{}", "pkg", name)).into())
    }

    pub(super) fn eval_for(
        &self,
        var: &str,
        iter_val: &RValue,
        body: &Expr,
        env: &Environment,
    ) -> Result<RValue, RFlow> {
        match iter_val {
            RValue::Vector(v) => {
                let len = v.len();
                for i in 0..len {
                    let elem = match &v.inner {
                        Vector::Raw(vals) => RValue::vec(Vector::Raw(vec![vals[i]])),
                        Vector::Double(vals) => RValue::vec(Vector::Double(vec![vals[i]].into())),
                        Vector::Integer(vals) => RValue::vec(Vector::Integer(vec![vals[i]].into())),
                        Vector::Logical(vals) => RValue::vec(Vector::Logical(vec![vals[i]].into())),
                        Vector::Complex(vals) => RValue::vec(Vector::Complex(vec![vals[i]].into())),
                        Vector::Character(vals) => {
                            RValue::vec(Vector::Character(vec![vals[i].clone()].into()))
                        }
                    };
                    env.set(var.to_string(), elem);
                    match self.eval_in(body, env) {
                        Ok(_) => {}
                        Err(RFlow::Signal(RSignal::Next)) => continue,
                        Err(RFlow::Signal(RSignal::Break)) => break,
                        Err(err) => return Err(err),
                    }
                }
            }
            RValue::List(list) => {
                for (_, val) in &list.values {
                    env.set(var.to_string(), val.clone());
                    match self.eval_in(body, env) {
                        Ok(_) => {}
                        Err(RFlow::Signal(RSignal::Next)) => continue,
                        Err(RFlow::Signal(RSignal::Break)) => break,
                        Err(err) => return Err(err),
                    }
                }
            }
            RValue::Null => {}
            _ => {
                return Err(RError::new(
                    RErrorKind::Type,
                    "invalid for() loop sequence".to_string(),
                )
                .into());
            }
        }
        Ok(RValue::Null)
    }
}
