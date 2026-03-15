//! Closure argument binding helpers used by the evaluator.
//!
//! Implements R's three-pass argument matching algorithm:
//! 1. Exact name match
//! 2. Partial (prefix) name match (must be unambiguous)
//! 3. Positional match for remaining unmatched formals
//!
//! After matching, errors on unused arguments when no `...` is present.

use std::collections::{HashMap, HashSet};

use crate::interpreter::call::CallFrame;
use crate::interpreter::environment::Environment;
use crate::interpreter::value::{RError, RErrorKind, RFlow, RList, RValue};
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
        let has_dots = params.iter().any(|p| p.is_dots);

        // Collect formal parameter names (excluding ...)
        let formal_names: Vec<&str> = params
            .iter()
            .filter(|p| !p.is_dots)
            .map(|p| p.name.as_str())
            .collect();

        // Maps: named_arg_index → formal_name it matched to
        let mut named_to_formal: HashMap<usize, &str> = HashMap::new();
        let mut matched_formals: HashSet<&str> = HashSet::new();

        // Pass 1: Exact name matching
        for (i, (arg_name, _)) in named.iter().enumerate() {
            if let Some(&formal) = formal_names.iter().find(|&&f| f == arg_name) {
                if !matched_formals.contains(formal) {
                    matched_formals.insert(formal);
                    named_to_formal.insert(i, formal);
                }
            }
        }

        // Pass 2: Partial (prefix) matching for remaining named args
        for (i, (arg_name, _)) in named.iter().enumerate() {
            if named_to_formal.contains_key(&i) {
                continue;
            }
            let candidates: Vec<&str> = formal_names
                .iter()
                .filter(|&&f| !matched_formals.contains(f) && f.starts_with(arg_name.as_str()))
                .copied()
                .collect();
            match candidates.len() {
                1 => {
                    matched_formals.insert(candidates[0]);
                    named_to_formal.insert(i, candidates[0]);
                }
                n if n > 1 && !has_dots => {
                    return Err(RError::new(
                        RErrorKind::Argument,
                        format!(
                            "argument '{}' matches multiple formal arguments: {}",
                            arg_name,
                            candidates.join(", ")
                        ),
                    )
                    .into());
                }
                _ => {} // 0 or ambiguous with dots — handled later
            }
        }

        // Build reverse map: formal_name → named_arg_index
        let formal_to_named: HashMap<&str, usize> = named_to_formal
            .iter()
            .map(|(&idx, &formal)| (formal, idx))
            .collect();

        // Pass 3: Bind matched formals and positional fill
        let mut pos_idx = 0usize;
        let mut dots_vals: Vec<(Option<String>, RValue)> = Vec::new();
        let mut formal_args = HashSet::new();
        let mut supplied_args = HashSet::new();

        for param in params {
            if param.is_dots {
                formal_args.insert("...".to_string());
                // Collect remaining unmatched positional args
                while pos_idx < positional.len() {
                    dots_vals.push((None, positional[pos_idx].clone()));
                    pos_idx += 1;
                }
                // Collect unmatched named args
                for (i, (name, value)) in named.iter().enumerate() {
                    if !named_to_formal.contains_key(&i) {
                        dots_vals.push((Some(name.clone()), value.clone()));
                    }
                }
                continue;
            }

            formal_args.insert(param.name.clone());

            if let Some(&named_idx) = formal_to_named.get(param.name.as_str()) {
                // Matched by name (exact or partial)
                call_env.set(param.name.clone(), named[named_idx].1.clone());
                supplied_args.insert(param.name.clone());
            } else if pos_idx < positional.len() {
                // Positional fill
                call_env.set(param.name.clone(), positional[pos_idx].clone());
                supplied_args.insert(param.name.clone());
                pos_idx += 1;
            } else if let Some(default) = &param.default {
                let value = self.eval_in(default, &call_env)?;
                call_env.set(param.name.clone(), value);
            }
        }

        // Error on unused arguments when no ... is present
        if !has_dots {
            if pos_idx < positional.len() {
                return Err(RError::new(
                    RErrorKind::Argument,
                    format!(
                        "unused argument{}",
                        if positional.len() - pos_idx == 1 {
                            ""
                        } else {
                            "s"
                        }
                    ),
                )
                .into());
            }
            let unused_named: Vec<&str> = named
                .iter()
                .enumerate()
                .filter(|(i, _)| !named_to_formal.contains_key(i))
                .map(|(_, (name, _))| name.as_str())
                .collect();
            if !unused_named.is_empty() {
                return Err(RError::new(
                    RErrorKind::Argument,
                    format!(
                        "unused argument{} ({})",
                        if unused_named.len() == 1 { "" } else { "s" },
                        unused_named
                            .iter()
                            .map(|n| format!("{n} = "))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                )
                .into());
            }
        }

        if has_dots {
            call_env.set("...".to_string(), RValue::List(RList::new(dots_vals)));
        }

        let supplied_arg_count = supplied_args.len();
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
