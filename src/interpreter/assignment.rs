//! Assignment and replacement semantics: `<-`, `<<-`, `->`, `x[i] <- v`,
//! `x[[i]] <- v`, `x$name <- v`, and replacement functions like `names<-`.

use derive_more::{Display, Error};

use crate::interpreter::environment::Environment;
use crate::interpreter::value::*;
use crate::interpreter::Interpreter;
use crate::parser::ast::{Arg, AssignOp, Expr};

// region: AssignmentError

/// Structured error type for assignment operations.
#[derive(Debug, Display, Error)]
pub enum AssignmentError {
    #[display("invalid assignment target")]
    InvalidTarget,

    #[display("invalid index")]
    InvalidIndex,

    #[display("replacement value must be a vector")]
    InvalidReplacementValue,

    #[display("object is not subsettable")]
    NotSubsettable,
}

impl From<AssignmentError> for RError {
    fn from(e: AssignmentError) -> Self {
        let kind = match &e {
            AssignmentError::InvalidTarget => RErrorKind::Other,
            AssignmentError::InvalidIndex | AssignmentError::NotSubsettable => RErrorKind::Index,
            AssignmentError::InvalidReplacementValue => RErrorKind::Type,
        };
        RError::from_source(kind, e)
    }
}

impl From<AssignmentError> for RFlow {
    fn from(e: AssignmentError) -> Self {
        RFlow::Error(RError::from(e))
    }
}

// endregion

// region: type-preserving replacement

/// Replace elements in a vector at 1-based indices, preserving type when possible.
/// If the replacement value's type differs, coerces both to the common type.
fn replace_elements(
    target: &Vector,
    indices: &[Option<i64>],
    replacement: &Vector,
    max_idx: usize,
) -> Vector {
    macro_rules! replace_typed {
        ($target_vals:expr, $repl_vals:expr, $variant:ident) => {{
            let mut result = $target_vals.to_vec();
            while result.len() < max_idx {
                result.push(Default::default());
            }
            for (j, idx) in indices.iter().enumerate() {
                if let Some(i) = idx {
                    let i = usize::try_from(*i).unwrap_or(0);
                    if i > 0 && i <= result.len() {
                        result[i - 1] = $repl_vals
                            .get(j % $repl_vals.len())
                            .cloned()
                            .unwrap_or_default();
                    }
                }
            }
            Vector::$variant(result.into())
        }};
    }

    // Same-type fast path
    match (target, replacement) {
        (Vector::Integer(tv), Vector::Integer(rv)) => replace_typed!(tv, rv, Integer),
        (Vector::Double(tv), Vector::Double(rv)) => replace_typed!(tv, rv, Double),
        (Vector::Character(tv), Vector::Character(rv)) => replace_typed!(tv, rv, Character),
        (Vector::Logical(tv), Vector::Logical(rv)) => replace_typed!(tv, rv, Logical),
        (Vector::Complex(tv), Vector::Complex(rv)) => replace_typed!(tv, rv, Complex),
        (Vector::Raw(tv), Vector::Raw(rv)) => {
            let mut result = tv.to_vec();
            while result.len() < max_idx {
                result.push(0);
            }
            for (j, idx) in indices.iter().enumerate() {
                if let Some(i) = idx {
                    let i = usize::try_from(*i).unwrap_or(0);
                    if i > 0 && i <= result.len() {
                        result[i - 1] = rv.get(j % rv.len()).copied().unwrap_or(0);
                    }
                }
            }
            Vector::Raw(result)
        }
        // Type mismatch — coerce both to doubles
        _ => {
            let mut result = target.to_doubles();
            let repl = replacement.to_doubles();
            while result.len() < max_idx {
                result.push(None);
            }
            for (j, idx) in indices.iter().enumerate() {
                if let Some(i) = idx {
                    let i = usize::try_from(*i).unwrap_or(0);
                    if i > 0 && i <= result.len() {
                        result[i - 1] = repl
                            .get(j % repl.len())
                            .copied()
                            .flatten()
                            .map(Some)
                            .unwrap_or(None);
                    }
                }
            }
            Vector::Double(result.into())
        }
    }
}

// endregion

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
            Err(AssignmentError::InvalidTarget.into())
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
        _ => Err(AssignmentError::InvalidTarget.into()),
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
        _ => return Err(AssignmentError::InvalidTarget.into()),
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
                _ => return Err(AssignmentError::InvalidIndex.into()),
            };

            let val_vec = match &val {
                RValue::Vector(vv) => vv,
                _ => {
                    return Err(AssignmentError::InvalidReplacementValue.into());
                }
            };

            // Determine max index to know if we need to extend
            let max_idx = idx_ints
                .iter()
                .filter_map(|x| x.and_then(|i| usize::try_from(i).ok()))
                .max()
                .unwrap_or(0);

            let new_vec = replace_elements(&v.inner, &idx_ints, &val_vec.inner, max_idx);

            // Preserve attributes (dim, dimnames, class, names, etc.)
            let mut rv = RVector::from(new_vec);
            if let Some(attrs) = &v.attrs {
                rv.attrs = Some(attrs.clone());
            }
            env.set(var_name, RValue::Vector(rv));
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
        _ => Err(AssignmentError::NotSubsettable.into()),
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
        _ => return Err(AssignmentError::InvalidTarget.into()),
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
        _ => return Err(AssignmentError::InvalidTarget.into()),
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
