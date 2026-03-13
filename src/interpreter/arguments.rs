//! Closure argument binding helpers used by the evaluator.

use std::collections::HashSet;

use crate::interpreter::call::CallFrame;
use crate::interpreter::environment::Environment;
use crate::interpreter::value::{RFlow, RList, RValue};
use crate::interpreter::Interpreter;
use crate::parser::ast::{Expr, Param};

pub(crate) struct BoundClosureCall {
    pub env: Environment,
    pub frame: CallFrame,
}

impl Interpreter {
    pub(crate) fn bind_closure_call(
        &self,
        params: &[Param],
        positional: &[RValue],
        named: &[(String, RValue)],
        closure_env: &Environment,
        function: &RValue,
        call: Option<Expr>,
    ) -> Result<BoundClosureCall, RFlow> {
        let call_env = Environment::new_child(closure_env);
        let mut pos_idx = 0usize;
        let mut dots_vals: Vec<(Option<String>, RValue)> = Vec::new();
        let mut has_dots = false;
        let param_names: Vec<&str> = params.iter().map(|param| param.name.as_str()).collect();
        let mut formal_args = HashSet::new();
        let mut supplied_args = HashSet::new();
        let mut supplied_arg_count = 0usize;

        for param in params {
            if param.is_dots {
                formal_args.insert("...".to_string());
            } else {
                formal_args.insert(param.name.clone());
            }
        }

        for param in params {
            if param.is_dots {
                has_dots = true;
                while pos_idx < positional.len() {
                    dots_vals.push((None, positional[pos_idx].clone()));
                    pos_idx += 1;
                }
                for (name, value) in named {
                    if !param_names.contains(&name.as_str()) {
                        dots_vals.push((Some(name.clone()), value.clone()));
                    }
                }
                supplied_arg_count += dots_vals.len();
                continue;
            }

            if let Some((_, value)) = named.iter().find(|(name, _)| *name == param.name) {
                call_env.set(param.name.clone(), value.clone());
                supplied_args.insert(param.name.clone());
                supplied_arg_count += 1;
            } else if pos_idx < positional.len() {
                call_env.set(param.name.clone(), positional[pos_idx].clone());
                supplied_args.insert(param.name.clone());
                supplied_arg_count += 1;
                pos_idx += 1;
            } else if let Some(default) = &param.default {
                let value = self.eval_in(default, &call_env)?;
                call_env.set(param.name.clone(), value);
            }
        }

        if has_dots {
            call_env.set("...".to_string(), RValue::List(RList::new(dots_vals)));
        }

        Ok(BoundClosureCall {
            env: call_env.clone(),
            frame: CallFrame {
                call,
                function: function.clone(),
                env: call_env,
                formal_args,
                supplied_args,
                supplied_positional: positional.to_vec(),
                supplied_named: named.to_vec(),
                supplied_arg_count,
            },
        })
    }
}
