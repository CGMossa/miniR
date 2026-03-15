//! Assignment and replacement semantics: `<-`, `<<-`, `->`, `x[i] <- v`,
//! `x[[i]] <- v`, `x$name <- v`, and replacement functions like `names<-`.

use crate::interpreter::environment::Environment;
use crate::interpreter::value::*;
use crate::interpreter::Interpreter;
use crate::parser::ast::{Arg, AssignOp, Expr};

// region: Interpreter delegation

impl Interpreter {
    pub(super) fn eval_assign(
        &self,
        op: &AssignOp,
        target: &Expr,
        val: RValue,
        env: &Environment,
    ) -> Result<RValue, RFlow> {
        eval_assign(self, op, target, val, env)
    }
}

// endregion

// region: eval_assign

fn eval_assign(
    interp: &Interpreter,
    op: &AssignOp,
    target: &Expr,
    val: RValue,
    env: &Environment,
) -> Result<RValue, RFlow> {
    match target {
        Expr::Symbol(name) => {
            match op {
                AssignOp::SuperAssign | AssignOp::RightSuperAssign => {
                    env.set_super(name.clone(), val.clone());
                }
                _ => {
                    env.set(name.clone(), val.clone());
                }
            }
            Ok(val)
        }
        // Assignment to index: x[i] <- val
        Expr::Index { object, indices } => eval_index_assign(interp, object, indices, val, env),
        Expr::IndexDouble { object, indices } => {
            eval_index_double_assign(interp, object, indices, val, env)
        }
        Expr::Dollar { object, member } => eval_dollar_assign(interp, object, member, val, env),
        // Handle function calls on left side like names(x) <- val, attr(x, "which") <- val
        Expr::Call {
            func,
            args: call_args,
        } => {
            if let Expr::Symbol(fname) = func.as_ref() {
                let replacement_fn = format!("{}<-", fname);
                if let Some(first_arg) = call_args.first() {
                    if let Some(ref val_expr) = first_arg.value {
                        let obj = interp.eval_in(val_expr, env)?;
                        if let Some(f) = env.get(&replacement_fn) {
                            // Evaluate extra args (e.g. "which" in attr(x, "which") <- val)
                            let mut positional = vec![obj];
                            for arg in &call_args[1..] {
                                if let Some(ref v) = arg.value {
                                    positional.push(interp.eval_in(v, env)?);
                                }
                            }
                            positional.push(val.clone());
                            let result = interp.call_function(&f, &positional, &[], env)?;
                            if let Expr::Symbol(var_name) = val_expr {
                                env.set(var_name.clone(), result);
                            }
                            return Ok(val);
                        }
                    }
                }
            }
            Err(RError::other("invalid assignment target".to_string()).into())
        }
        // In R, "name" <- value creates a binding named "name"
        Expr::String(name) => {
            match op {
                AssignOp::SuperAssign | AssignOp::RightSuperAssign => {
                    env.set_super(name.clone(), val.clone());
                }
                _ => {
                    env.set(name.clone(), val.clone());
                }
            }
            Ok(val)
        }
        _ => Err(RError::other("invalid assignment target".to_string()).into()),
    }
}

// endregion

// region: index assignment (x[i] <- val)

fn eval_index_assign(
    interp: &Interpreter,
    object: &Expr,
    indices: &[Arg],
    val: RValue,
    env: &Environment,
) -> Result<RValue, RFlow> {
    let var_name = match object {
        Expr::Symbol(name) => name.clone(),
        _ => return Err(RError::other("invalid assignment target".to_string()).into()),
    };

    let mut obj = env.get(&var_name).unwrap_or(RValue::Null);

    if indices.is_empty() {
        env.set(var_name, val.clone());
        return Ok(val);
    }

    let idx_val = if let Some(val_expr) = &indices[0].value {
        interp.eval_in(val_expr, env)?
    } else {
        return Ok(val);
    };

    match &mut obj {
        RValue::Vector(v) => {
            let idx_ints = match &idx_val {
                RValue::Vector(iv) => iv.to_integers(),
                _ => return Err(RError::new(RErrorKind::Index, "invalid index".to_string()).into()),
            };

            let new_vals = match &val {
                RValue::Vector(vv) => vv.to_doubles(),
                _ => {
                    return Err(RError::new(
                        RErrorKind::Type,
                        "replacement value error".to_string(),
                    )
                    .into());
                }
            };

            let mut doubles = v.to_doubles();
            for (j, idx) in idx_ints.iter().enumerate() {
                if let Some(i) = idx {
                    let i = usize::try_from(*i).unwrap_or(0);
                    if i > 0 {
                        while doubles.len() < i {
                            doubles.push(None);
                        }
                        doubles[i - 1] = new_vals
                            .get(j % new_vals.len())
                            .copied()
                            .flatten()
                            .map(Some)
                            .unwrap_or(None);
                    }
                }
            }
            let new_obj = RValue::vec(Vector::Double(doubles.into()));
            env.set(var_name, new_obj.clone());
            Ok(val)
        }
        RValue::List(list) => {
            match &idx_val {
                RValue::Vector(rv) if matches!(rv.inner, Vector::Character(_)) => {
                    let Vector::Character(names) = &rv.inner else {
                        unreachable!()
                    };
                    if let Some(Some(name)) = names.first() {
                        if let Some(entry) = list
                            .values
                            .iter_mut()
                            .find(|(n, _)| n.as_ref() == Some(name))
                        {
                            entry.1 = val.clone();
                        } else {
                            list.values.push((Some(name.clone()), val.clone()));
                        }
                    }
                }
                RValue::Vector(iv) => {
                    let i = usize::try_from(iv.as_integer_scalar().unwrap_or(0)).unwrap_or(0);
                    if i > 0 && i <= list.values.len() {
                        list.values[i - 1].1 = val.clone();
                    }
                }
                _ => {}
            }
            env.set(var_name, obj);
            Ok(val)
        }
        RValue::Null => {
            match &idx_val {
                RValue::Vector(rv) if matches!(rv.inner, Vector::Character(_)) => {
                    let Vector::Character(names) = &rv.inner else {
                        unreachable!()
                    };
                    let mut list = RList::new(vec![]);
                    if let Some(Some(name)) = names.first() {
                        list.values.push((Some(name.clone()), val.clone()));
                    }
                    env.set(var_name, RValue::List(list));
                }
                _ => {
                    let idx = match &idx_val {
                        RValue::Vector(iv) => {
                            usize::try_from(iv.as_integer_scalar().unwrap_or(0)).unwrap_or(0)
                        }
                        _ => 0,
                    };
                    let mut doubles = vec![None; idx];
                    if idx > 0 {
                        if let RValue::Vector(vv) = &val {
                            doubles[idx - 1] = vv.to_doubles().into_iter().next().flatten();
                        }
                    }
                    env.set(var_name, RValue::vec(Vector::Double(doubles.into())));
                }
            }
            Ok(val)
        }
        _ => Err(RError::new(RErrorKind::Index, "object is not subsettable".to_string()).into()),
    }
}

// endregion

// region: double-bracket assignment (x[[i]] <- val)

fn eval_index_double_assign(
    interp: &Interpreter,
    object: &Expr,
    indices: &[Arg],
    val: RValue,
    env: &Environment,
) -> Result<RValue, RFlow> {
    let var_name = match object {
        Expr::Symbol(name) => name.clone(),
        _ => return Err(RError::other("invalid assignment target".to_string()).into()),
    };

    let mut obj = env
        .get(&var_name)
        .unwrap_or(RValue::List(RList::new(vec![])));
    let idx_val = if let Some(val_expr) = &indices[0].value {
        interp.eval_in(val_expr, env)?
    } else {
        return Ok(val);
    };

    match &mut obj {
        RValue::List(list) => {
            match &idx_val {
                RValue::Vector(rv) if matches!(rv.inner, Vector::Character(_)) => {
                    let Vector::Character(names) = &rv.inner else {
                        unreachable!()
                    };
                    if let Some(Some(name)) = names.first() {
                        if let Some(entry) = list
                            .values
                            .iter_mut()
                            .find(|(n, _)| n.as_ref() == Some(name))
                        {
                            entry.1 = val.clone();
                        } else {
                            list.values.push((Some(name.clone()), val.clone()));
                        }
                    }
                }
                RValue::Vector(iv) => {
                    let i = usize::try_from(iv.as_integer_scalar().unwrap_or(0)).unwrap_or(0);
                    if i > 0 {
                        while list.values.len() < i {
                            list.values.push((None, RValue::Null));
                        }
                        list.values[i - 1].1 = val.clone();
                    }
                }
                _ => {}
            }
            env.set(var_name, obj);
            Ok(val)
        }
        _ => eval_index_assign(interp, object, indices, val, env),
    }
}

// endregion

// region: dollar assignment (x$name <- val)

fn eval_dollar_assign(
    _interp: &Interpreter,
    object: &Expr,
    member: &str,
    val: RValue,
    env: &Environment,
) -> Result<RValue, RFlow> {
    let var_name = match object {
        Expr::Symbol(name) => name.clone(),
        _ => return Err(RError::other("invalid assignment target".to_string()).into()),
    };

    let mut obj = env
        .get(&var_name)
        .unwrap_or(RValue::List(RList::new(vec![])));
    match &mut obj {
        RValue::List(list) => {
            if let Some(entry) = list
                .values
                .iter_mut()
                .find(|(n, _)| n.as_deref() == Some(member))
            {
                entry.1 = val.clone();
            } else {
                list.values.push((Some(member.to_string()), val.clone()));
            }
            env.set(var_name, obj);
            Ok(val)
        }
        RValue::Null => {
            let list = RList::new(vec![(Some(member.to_string()), val.clone())]);
            env.set(var_name, RValue::List(list));
            Ok(val)
        }
        _ => {
            let list = RList::new(vec![(Some(member.to_string()), val.clone())]);
            env.set(var_name, RValue::List(list));
            Ok(val)
        }
    }
}

// endregion
