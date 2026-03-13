mod arguments;
pub mod builtins;
pub mod call;
mod call_eval;
pub mod coerce;
pub mod environment;
mod indexing;
mod s3;
pub mod value;

use std::cell::RefCell;

use ndarray::{Array2, ShapeBuilder};

use crate::parser::ast::*;
pub use call::BuiltinContext;
pub(crate) use call::{CallFrame, S3DispatchContext};
use coerce::f64_to_i64;
use environment::Environment;
use value::*;

thread_local! {
    static INTERPRETER: RefCell<Interpreter> = RefCell::new(Interpreter::new());
}

/// Access the thread-local interpreter. Safe for nested/re-entrant calls
/// because all methods take `&self` (shared borrows are re-entrant).
pub fn with_interpreter<F, R>(f: F) -> R
where
    F: FnOnce(&Interpreter) -> R,
{
    INTERPRETER.with(|cell| f(&cell.borrow()))
}

/// Temporarily install an explicit interpreter instance into thread-local state
/// while executing `f`. This keeps legacy builtin TLS access working, but lets
/// higher-level code own interpreter instances directly.
pub fn with_interpreter_state<F, R>(state: &mut Interpreter, f: F) -> R
where
    F: FnOnce(&Interpreter) -> R,
{
    INTERPRETER.with(|cell| {
        {
            let mut installed = cell.borrow_mut();
            std::mem::swap(&mut *installed, state);
        }

        struct Restore<'a> {
            cell: &'a RefCell<Interpreter>,
            state: &'a mut Interpreter,
        }

        impl Drop for Restore<'_> {
            fn drop(&mut self) {
                let mut installed = self.cell.borrow_mut();
                std::mem::swap(&mut *installed, self.state);
            }
        }

        let _restore = Restore { cell, state };
        let installed = cell.borrow();
        f(&installed)
    })
}

fn formula_value(expr: Expr, env: &Environment) -> RValue {
    let mut lang = Language::new(expr);
    lang.set_attr(
        "class".to_string(),
        RValue::vec(Vector::Character(vec![Some("formula".to_string())].into())),
    );
    lang.set_attr(".Environment".to_string(), RValue::Environment(env.clone()));
    RValue::Language(lang)
}

/// A handler registered by withCallingHandlers().
#[derive(Clone)]
pub(crate) struct ConditionHandler {
    pub class: String,
    pub handler: RValue,
    #[allow(dead_code)]
    pub env: Environment,
}

pub struct Interpreter {
    pub global_env: Environment,
    s3_dispatch_stack: RefCell<Vec<S3DispatchContext>>,
    call_stack: RefCell<Vec<CallFrame>>,
    /// Stack of handler sets from withCallingHandlers() calls.
    pub(crate) condition_handlers: RefCell<Vec<Vec<ConditionHandler>>>,
    #[cfg(feature = "random")]
    rng: RefCell<rand::rngs::StdRng>,
    /// Session-scoped temporary directory, auto-cleaned on drop.
    pub(crate) temp_dir: temp_dir::TempDir,
    /// Counter for unique tempfile names within the session.
    pub(crate) temp_counter: std::cell::Cell<u64>,
}

impl Default for Interpreter {
    fn default() -> Self {
        Self::new()
    }
}

impl Interpreter {
    fn ensure_builtin_max_arity(
        name: &str,
        max_args: Option<usize>,
        actual_args: usize,
    ) -> Result<(), RError> {
        let Some(max_args) = max_args else {
            return Ok(());
        };

        if actual_args <= max_args {
            return Ok(());
        }

        let expectation = match max_args {
            0 => "takes no arguments".to_string(),
            1 => "takes at most 1 argument".to_string(),
            n => format!("takes at most {n} arguments"),
        };
        let suffix = if actual_args == 1 { "" } else { "s" };

        Err(RError::new(
            RErrorKind::Argument,
            format!("{name}() {expectation}, got {actual_args} argument{suffix}"),
        ))
    }

    pub fn new() -> Self {
        let base_env = Environment::new_global();
        base_env.set_name("base".to_string());
        builtins::register_builtins(&base_env);
        let global_env = Environment::new_child(&base_env);
        global_env.set_name("R_GlobalEnv".to_string());
        Interpreter {
            global_env,
            s3_dispatch_stack: RefCell::new(Vec::new()),
            call_stack: RefCell::new(Vec::new()),
            condition_handlers: RefCell::new(Vec::new()),
            #[cfg(feature = "random")]
            rng: RefCell::new(<rand::rngs::StdRng as rand::SeedableRng>::from_os_rng()),
            temp_dir: temp_dir::TempDir::new().expect("failed to create session temp directory"),
            temp_counter: std::cell::Cell::new(0),
        }
    }

    /// Signal a condition to withCallingHandlers handlers (non-unwinding).
    /// Returns Ok(true) if muffled, Ok(false) if not handled, or Err if a handler
    /// raised an unwinding condition (e.g. tryCatch's unwind handler).
    pub(crate) fn signal_condition(
        &self,
        condition: &RValue,
        env: &Environment,
    ) -> Result<bool, RError> {
        let classes = value::get_class(condition);
        // Clone handlers to release the borrow — handlers may trigger nested conditions
        let handler_stack: Vec<Vec<ConditionHandler>> = self.condition_handlers.borrow().clone();
        // Walk handlers top-down (most recently established first)
        for handler_set in handler_stack.iter().rev() {
            for handler in handler_set {
                if classes.iter().any(|c| c == &handler.class) {
                    // Call the handler — if it returns normally, continue signaling
                    let result = self.call_function(
                        &handler.handler,
                        std::slice::from_ref(condition),
                        &[],
                        env,
                    );
                    match &result {
                        Err(RFlow::Error(RError::Standard { message: msg, .. }))
                            if msg == "muffleWarning" || msg == "muffleMessage" =>
                        {
                            return Ok(true);
                        }
                        Err(e) => return Err(RError::from(e.clone())),
                        Ok(_) => {} // handler returned normally, continue signaling
                    }
                }
            }
        }
        Ok(false)
    }

    #[cfg(feature = "random")]
    pub fn rng(&self) -> &RefCell<rand::rngs::StdRng> {
        &self.rng
    }

    pub fn eval(&self, expr: &Expr) -> Result<RValue, RFlow> {
        self.eval_in(expr, &self.global_env)
    }

    pub fn eval_in(&self, expr: &Expr, env: &Environment) -> Result<RValue, RFlow> {
        match expr {
            Expr::Null => Ok(RValue::Null),
            Expr::Na(na_type) => Ok(match na_type {
                NaType::Logical => RValue::vec(Vector::Logical(vec![None].into())),
                NaType::Integer => RValue::vec(Vector::Integer(vec![None].into())),
                NaType::Real => RValue::vec(Vector::Double(vec![None].into())),
                NaType::Character => RValue::vec(Vector::Character(vec![None].into())),
                NaType::Complex => RValue::vec(Vector::Double(vec![None].into())),
            }),
            Expr::Inf => Ok(RValue::vec(Vector::Double(
                vec![Some(f64::INFINITY)].into(),
            ))),
            Expr::NaN => Ok(RValue::vec(Vector::Double(vec![Some(f64::NAN)].into()))),
            Expr::Bool(b) => Ok(RValue::vec(Vector::Logical(vec![Some(*b)].into()))),
            Expr::Integer(i) => Ok(RValue::vec(Vector::Integer(vec![Some(*i)].into()))),
            Expr::Double(f) => Ok(RValue::vec(Vector::Double(vec![Some(*f)].into()))),
            Expr::String(s) => Ok(RValue::vec(Vector::Character(vec![Some(s.clone())].into()))),
            Expr::Complex(f) => Ok(RValue::vec(Vector::Complex(
                vec![Some(num_complex::Complex64::new(0.0, *f))].into(),
            ))),
            Expr::Symbol(name) => env
                .get(name)
                .ok_or_else(|| RError::new(RErrorKind::Name, name.clone()).into()),
            Expr::Dots => {
                // Return the ... list from the current environment
                env.get("...").ok_or_else(|| {
                    RError::other("'...' used in incorrect context".to_string()).into()
                })
            }
            Expr::DotDot(n) => {
                if *n == 0 {
                    return Err(RError::other(
                        "..0 is not valid — R uses 1-based indexing for ... arguments.\n  \
                         Did you mean ..1? (..1 is the first element, ..2 is the second, etc.)",
                    )
                    .into());
                }
                // ..1, ..2 etc. — 1-indexed access into ...
                let dots = env
                    .get("...")
                    .ok_or_else(|| RError::other(format!("'..{}' used in incorrect context", n)))?;
                match dots {
                    RValue::List(list) => {
                        let idx = usize::try_from(i64::from(*n))?.saturating_sub(1);
                        list.values.get(idx).map(|(_, v)| v.clone()).ok_or_else(|| {
                            RError::other(format!("the ... list does not contain {} elements", n))
                                .into()
                        })
                    }
                    _ => Err(RError::other(format!("'..{}' used in incorrect context", n)).into()),
                }
            }

            Expr::UnaryOp { op, operand } => {
                let val = self.eval_in(operand, env)?;
                self.eval_unary(*op, &val)
            }
            Expr::BinaryOp { op, lhs, rhs } => {
                // Special handling for pipe
                if matches!(op, BinaryOp::Pipe) {
                    return self.eval_pipe(lhs, rhs, env);
                }
                let left = self.eval_in(lhs, env)?;
                let right = self.eval_in(rhs, env)?;
                self.eval_binary(*op, &left, &right)
            }
            Expr::Assign { op, target, value } => {
                let val = self.eval_in(value, env)?;
                self.eval_assign(op, target, val, env)
            }

            Expr::Call { func, args } => self.eval_call(func, args, env),
            Expr::Index { object, indices } => self.eval_index(object, indices, env),
            Expr::IndexDouble { object, indices } => self.eval_index_double(object, indices, env),
            Expr::Dollar { object, member } => self.eval_dollar(object, member, env),
            Expr::Slot { object, member } => self.eval_dollar(object, member, env), // treat like $
            Expr::NsGet { namespace, name } => self.eval_ns_get(namespace, name, env),
            Expr::NsGetInt { namespace, name } => self.eval_ns_get(namespace, name, env),

            Expr::Formula { .. } => Ok(formula_value(expr.clone(), env)),

            Expr::If {
                condition,
                then_body,
                else_body,
            } => {
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

            Expr::For { var, iter, body } => {
                let iter_val = self.eval_in(iter, env)?;
                self.eval_for(var, &iter_val, body, env)
            }

            Expr::While { condition, body } => {
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
                        Err(e) => return Err(e),
                        _ => {}
                    }
                }
                Ok(RValue::Null)
            }

            Expr::Repeat { body } => {
                loop {
                    match self.eval_in(body, env) {
                        Err(RFlow::Signal(RSignal::Break)) => break,
                        Err(RFlow::Signal(RSignal::Next)) => continue,
                        Err(e) => return Err(e),
                        _ => {}
                    }
                }
                Ok(RValue::Null)
            }

            Expr::Break => Err(RFlow::Signal(RSignal::Break)),
            Expr::Next => Err(RFlow::Signal(RSignal::Next)),
            Expr::Return(val) => {
                let ret_val = match val {
                    Some(expr) => self.eval_in(expr, env)?,
                    None => RValue::Null,
                };
                Err(RFlow::Signal(RSignal::Return(ret_val)))
            }

            Expr::Block(exprs) => {
                let mut result = RValue::Null;
                for expr in exprs {
                    result = self.eval_in(expr, env)?;
                }
                Ok(result)
            }

            Expr::Function { params, body } => Ok(RValue::Function(RFunction::Closure {
                params: params.clone(),
                body: (**body).clone(),
                env: env.clone(),
            })),

            Expr::Program(exprs) => {
                let mut result = RValue::Null;
                for expr in exprs {
                    result = self.eval_in(expr, env)?;
                }
                Ok(result)
            }
        }
    }

    pub fn eval_unary(&self, op: UnaryOp, val: &RValue) -> Result<RValue, RFlow> {
        match op {
            UnaryOp::Neg => match val {
                RValue::Vector(v) => {
                    let result = match &v.inner {
                        Vector::Double(vals) => Vector::Double(
                            vals.iter()
                                .map(|x| x.map(|f| -f))
                                .collect::<Vec<_>>()
                                .into(),
                        ),
                        Vector::Integer(vals) => Vector::Integer(
                            vals.iter()
                                .map(|x| x.map(|i| -i))
                                .collect::<Vec<_>>()
                                .into(),
                        ),
                        Vector::Logical(vals) => Vector::Integer(
                            vals.iter()
                                .map(|x| x.map(|b| if b { -1 } else { 0 }))
                                .collect::<Vec<_>>()
                                .into(),
                        ),
                        _ => {
                            return Err(RError::new(
                                RErrorKind::Type,
                                "invalid argument to unary operator",
                            )
                            .into())
                        }
                    };
                    Ok(RValue::vec(result))
                }
                _ => {
                    Err(RError::new(RErrorKind::Type, "invalid argument to unary operator").into())
                }
            },
            UnaryOp::Pos => match val {
                RValue::Vector(v) if matches!(v.inner, Vector::Raw(_)) => Err(RError::new(
                    RErrorKind::Type,
                    "non-numeric argument to unary operator",
                )
                .into()),
                _ => Ok(val.clone()),
            },
            UnaryOp::Not => match val {
                // Bitwise NOT for raw vectors
                RValue::Vector(v) if matches!(v.inner, Vector::Raw(_)) => {
                    let bytes = v.inner.to_raw();
                    let result: Vec<u8> = bytes.iter().map(|b| !b).collect();
                    Ok(RValue::vec(Vector::Raw(result)))
                }
                RValue::Vector(v) => {
                    let logicals = v.to_logicals();
                    let result: Vec<Option<bool>> =
                        logicals.iter().map(|x| x.map(|b| !b)).collect();
                    Ok(RValue::vec(Vector::Logical(result.into())))
                }
                _ => Err(RError::new(RErrorKind::Type, "invalid argument type").into()),
            },
            UnaryOp::Formula => Ok(RValue::Null), // stub for unary ~
        }
    }

    pub fn eval_binary(
        &self,
        op: BinaryOp,
        left: &RValue,
        right: &RValue,
    ) -> Result<RValue, RFlow> {
        match op {
            BinaryOp::Range => return self.eval_range(left, right),
            BinaryOp::Special(SpecialOp::In) => return self.eval_in_op(left, right),
            BinaryOp::Special(SpecialOp::MatMul) => return self.eval_matmul(left, right),
            _ => {}
        };

        // Get vectors for element-wise operations
        let lv = match left {
            RValue::Vector(v) => v,
            RValue::Null => return Ok(RValue::Null),
            _ => {
                return Err(RError::new(
                    RErrorKind::Type,
                    "non-numeric argument to binary operator".to_string(),
                )
                .into())
            }
        };
        let rv = match right {
            RValue::Vector(v) => v,
            RValue::Null => return Ok(RValue::Null),
            _ => {
                return Err(RError::new(
                    RErrorKind::Type,
                    "non-numeric argument to binary operator".to_string(),
                )
                .into())
            }
        };

        match op {
            BinaryOp::Range => self.eval_range(left, right),
            BinaryOp::Special(SpecialOp::In) => self.eval_in_op(left, right),
            BinaryOp::Special(SpecialOp::MatMul) => self.eval_matmul(left, right),
            BinaryOp::Special(_) => Ok(RValue::Null),

            // Arithmetic (vectorized with recycling) — raw vectors cannot participate
            BinaryOp::Add
            | BinaryOp::Sub
            | BinaryOp::Mul
            | BinaryOp::Div
            | BinaryOp::Pow
            | BinaryOp::Mod
            | BinaryOp::IntDiv => {
                if matches!(lv.inner, Vector::Raw(_)) || matches!(rv.inner, Vector::Raw(_)) {
                    return Err(RError::new(
                        RErrorKind::Type,
                        "non-numeric argument to binary operator",
                    )
                    .into());
                }
                self.eval_arith(op, &lv.inner, &rv.inner)
            }

            // Comparison (vectorized)
            BinaryOp::Eq
            | BinaryOp::Ne
            | BinaryOp::Lt
            | BinaryOp::Gt
            | BinaryOp::Le
            | BinaryOp::Ge => self.eval_compare(op, lv, rv),

            // Logical — for raw vectors, & and | are bitwise
            BinaryOp::And | BinaryOp::Or => {
                if matches!(lv.inner, Vector::Raw(_)) || matches!(rv.inner, Vector::Raw(_)) {
                    return self.eval_raw_bitwise(op, &lv.inner, &rv.inner);
                }
                self.eval_logical_vec(op, &lv.inner, &rv.inner)
            }

            // Scalar logical
            BinaryOp::AndScalar => {
                let a = lv.as_logical_scalar();
                let b = rv.as_logical_scalar();
                match (a, b) {
                    (Some(false), _) | (_, Some(false)) => {
                        Ok(RValue::vec(Vector::Logical(vec![Some(false)].into())))
                    }
                    (Some(true), Some(true)) => {
                        Ok(RValue::vec(Vector::Logical(vec![Some(true)].into())))
                    }
                    _ => Ok(RValue::vec(Vector::Logical(vec![None].into()))),
                }
            }
            BinaryOp::OrScalar => {
                let a = lv.as_logical_scalar();
                let b = rv.as_logical_scalar();
                match (a, b) {
                    (Some(true), _) | (_, Some(true)) => {
                        Ok(RValue::vec(Vector::Logical(vec![Some(true)].into())))
                    }
                    (Some(false), Some(false)) => {
                        Ok(RValue::vec(Vector::Logical(vec![Some(false)].into())))
                    }
                    _ => Ok(RValue::vec(Vector::Logical(vec![None].into()))),
                }
            }

            BinaryOp::Pipe => unreachable!("pipe handled separately"),
            BinaryOp::Tilde => Ok(RValue::Null), // stub for binary ~
        }
    }

    fn eval_arith(&self, op: BinaryOp, lv: &Vector, rv: &Vector) -> Result<RValue, RFlow> {
        // Check if both are integer and op preserves integer type
        let use_integer = matches!(
            (&lv, &rv, &op),
            (
                Vector::Integer(_),
                Vector::Integer(_),
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::IntDiv | BinaryOp::Mod
            )
        );

        if use_integer {
            let li = lv.to_integers();
            let ri = rv.to_integers();
            let len = li.len().max(ri.len());
            if len == 0 {
                return Ok(RValue::vec(Vector::Integer(vec![].into())));
            }
            let result: Vec<Option<i64>> = (0..len)
                .map(|i| {
                    let a = li[i % li.len()];
                    let b = ri[i % ri.len()];
                    match (a, b) {
                        (Some(a), Some(b)) => match op {
                            BinaryOp::Add => Some(a.wrapping_add(b)),
                            BinaryOp::Sub => Some(a.wrapping_sub(b)),
                            BinaryOp::Mul => Some(a.wrapping_mul(b)),
                            BinaryOp::IntDiv => {
                                if b != 0 {
                                    Some(a / b)
                                } else {
                                    None
                                }
                            }
                            BinaryOp::Mod => {
                                if b != 0 {
                                    Some(a % b)
                                } else {
                                    None
                                }
                            }
                            _ => None,
                        },
                        _ => None,
                    }
                })
                .collect();
            return Ok(RValue::vec(Vector::Integer(result.into())));
        }

        // If either operand is complex, operate in complex space
        let use_complex = matches!(lv, Vector::Complex(_)) || matches!(rv, Vector::Complex(_));

        if use_complex {
            let lc = lv.to_complex();
            let rc = rv.to_complex();
            let len = lc.len().max(rc.len());
            if len == 0 {
                return Ok(RValue::vec(Vector::Complex(vec![].into())));
            }
            if matches!(op, BinaryOp::Mod | BinaryOp::IntDiv) {
                return Err(RError::new(
                    RErrorKind::Type,
                    "unimplemented complex operation".to_string(),
                )
                .into());
            }
            let result: Vec<Option<num_complex::Complex64>> = (0..len)
                .map(|i| {
                    let a = lc[i % lc.len()];
                    let b = rc[i % rc.len()];
                    match (a, b) {
                        (Some(a), Some(b)) => Some(match op {
                            BinaryOp::Add => a + b,
                            BinaryOp::Sub => a - b,
                            BinaryOp::Mul => a * b,
                            BinaryOp::Div => a / b,
                            BinaryOp::Pow => a.powc(b),
                            _ => unreachable!(),
                        }),
                        _ => None,
                    }
                })
                .collect();
            return Ok(RValue::vec(Vector::Complex(result.into())));
        }

        let ld = lv.to_doubles();
        let rd = rv.to_doubles();
        let len = ld.len().max(rd.len());
        if len == 0 {
            return Ok(RValue::vec(Vector::Double(vec![].into())));
        }

        let result: Vec<Option<f64>> = (0..len)
            .map(|i| {
                let a = ld[i % ld.len()];
                let b = rd[i % rd.len()];
                match (a, b) {
                    (Some(a), Some(b)) => Some(match op {
                        BinaryOp::Add => a + b,
                        BinaryOp::Sub => a - b,
                        BinaryOp::Mul => a * b,
                        BinaryOp::Div => a / b,
                        BinaryOp::Pow => a.powf(b),
                        BinaryOp::Mod => a % b,
                        BinaryOp::IntDiv => (a / b).floor(),
                        _ => unreachable!(),
                    }),
                    _ => None,
                }
            })
            .collect();
        Ok(RValue::vec(Vector::Double(result.into())))
    }

    fn eval_compare(&self, op: BinaryOp, lv: &Vector, rv: &Vector) -> Result<RValue, RFlow> {
        // Raw comparison: compares byte values
        if matches!(lv, Vector::Raw(_)) || matches!(rv, Vector::Raw(_)) {
            let lb = lv.to_raw();
            let rb = rv.to_raw();
            let len = lb.len().max(rb.len());
            if len == 0 {
                return Ok(RValue::vec(Vector::Logical(vec![].into())));
            }
            let result: Vec<Option<bool>> = (0..len)
                .map(|i| {
                    let a = lb[i % lb.len()];
                    let b = rb[i % rb.len()];
                    Some(match op {
                        BinaryOp::Eq => a == b,
                        BinaryOp::Ne => a != b,
                        BinaryOp::Lt => a < b,
                        BinaryOp::Gt => a > b,
                        BinaryOp::Le => a <= b,
                        BinaryOp::Ge => a >= b,
                        _ => unreachable!(),
                    })
                })
                .collect();
            return Ok(RValue::vec(Vector::Logical(result.into())));
        }

        // Complex comparison: only == and != are defined
        if matches!(lv, Vector::Complex(_)) || matches!(rv, Vector::Complex(_)) {
            if !matches!(op, BinaryOp::Eq | BinaryOp::Ne) {
                return Err(RError::new(
                    RErrorKind::Type,
                    "invalid comparison with complex values".to_string(),
                )
                .into());
            }
            let lc = lv.to_complex();
            let rc = rv.to_complex();
            let len = lc.len().max(rc.len());
            let result: Vec<Option<bool>> = (0..len)
                .map(|i| {
                    let a = lc[i % lc.len()];
                    let b = rc[i % rc.len()];
                    match (a, b) {
                        (Some(a), Some(b)) => Some(match op {
                            BinaryOp::Eq => a == b,
                            BinaryOp::Ne => a != b,
                            _ => unreachable!(),
                        }),
                        _ => None,
                    }
                })
                .collect();
            return Ok(RValue::vec(Vector::Logical(result.into())));
        }

        // If either is character, compare as strings
        if matches!(lv, Vector::Character(_)) || matches!(rv, Vector::Character(_)) {
            let lc = lv.to_characters();
            let rc = rv.to_characters();
            let len = lc.len().max(rc.len());
            let result: Vec<Option<bool>> = (0..len)
                .map(|i| {
                    let a = &lc[i % lc.len()];
                    let b = &rc[i % rc.len()];
                    match (a, b) {
                        (Some(a), Some(b)) => Some(match op {
                            BinaryOp::Eq => a == b,
                            BinaryOp::Ne => a != b,
                            BinaryOp::Lt => a < b,
                            BinaryOp::Gt => a > b,
                            BinaryOp::Le => a <= b,
                            BinaryOp::Ge => a >= b,
                            _ => unreachable!(),
                        }),
                        _ => None,
                    }
                })
                .collect();
            return Ok(RValue::vec(Vector::Logical(result.into())));
        }

        let ld = lv.to_doubles();
        let rd = rv.to_doubles();
        let len = ld.len().max(rd.len());
        if len == 0 {
            return Ok(RValue::vec(Vector::Logical(vec![].into())));
        }

        let result: Vec<Option<bool>> = (0..len)
            .map(|i| {
                let a = ld[i % ld.len()];
                let b = rd[i % rd.len()];
                match (a, b) {
                    (Some(a), Some(b)) => Some(match op {
                        BinaryOp::Eq => a == b,
                        BinaryOp::Ne => a != b,
                        BinaryOp::Lt => a < b,
                        BinaryOp::Gt => a > b,
                        BinaryOp::Le => a <= b,
                        BinaryOp::Ge => a >= b,
                        _ => unreachable!(),
                    }),
                    _ => None,
                }
            })
            .collect();
        Ok(RValue::vec(Vector::Logical(result.into())))
    }

    fn eval_logical_vec(&self, op: BinaryOp, lv: &Vector, rv: &Vector) -> Result<RValue, RFlow> {
        let ll = lv.to_logicals();
        let rl = rv.to_logicals();
        let len = ll.len().max(rl.len());

        let result: Vec<Option<bool>> = (0..len)
            .map(|i| {
                let a = ll[i % ll.len()];
                let b = rl[i % rl.len()];
                match op {
                    BinaryOp::And => match (a, b) {
                        (Some(false), _) | (_, Some(false)) => Some(false),
                        (Some(true), Some(true)) => Some(true),
                        _ => None,
                    },
                    BinaryOp::Or => match (a, b) {
                        (Some(true), _) | (_, Some(true)) => Some(true),
                        (Some(false), Some(false)) => Some(false),
                        _ => None,
                    },
                    _ => unreachable!(),
                }
            })
            .collect();
        Ok(RValue::vec(Vector::Logical(result.into())))
    }

    /// Bitwise &/| for raw vectors — returns raw
    fn eval_raw_bitwise(&self, op: BinaryOp, lv: &Vector, rv: &Vector) -> Result<RValue, RFlow> {
        let lb = lv.to_raw();
        let rb = rv.to_raw();
        let len = lb.len().max(rb.len());
        if len == 0 {
            return Ok(RValue::vec(Vector::Raw(vec![])));
        }
        let result: Vec<u8> = (0..len)
            .map(|i| {
                let a = lb[i % lb.len()];
                let b = rb[i % rb.len()];
                match op {
                    BinaryOp::And => a & b,
                    BinaryOp::Or => a | b,
                    _ => unreachable!(),
                }
            })
            .collect();
        Ok(RValue::vec(Vector::Raw(result)))
    }

    fn eval_range(&self, left: &RValue, right: &RValue) -> Result<RValue, RFlow> {
        let from = match left {
            RValue::Vector(v) => f64_to_i64(v.as_double_scalar().unwrap_or(0.0))?,
            _ => 0,
        };
        let to = match right {
            RValue::Vector(v) => f64_to_i64(v.as_double_scalar().unwrap_or(0.0))?,
            _ => 0,
        };

        let result: Vec<Option<i64>> = if from <= to {
            (from..=to).map(Some).collect()
        } else {
            (to..=from).rev().map(Some).collect()
        };
        Ok(RValue::vec(Vector::Integer(result.into())))
    }

    fn eval_in_op(&self, left: &RValue, right: &RValue) -> Result<RValue, RFlow> {
        match (left, right) {
            (RValue::Vector(lv), RValue::Vector(rv)) => {
                // If either side is character, compare as strings
                if matches!(lv.inner, Vector::Character(_))
                    || matches!(rv.inner, Vector::Character(_))
                {
                    let table = rv.to_characters();
                    let vals = lv.to_characters();
                    let result: Vec<Option<bool>> =
                        vals.iter().map(|x| Some(table.contains(x))).collect();
                    return Ok(RValue::vec(Vector::Logical(result.into())));
                }
                // Otherwise compare as doubles (handles int/double/logical correctly)
                let table = rv.to_doubles();
                let vals = lv.to_doubles();
                let result: Vec<Option<bool>> = vals
                    .iter()
                    .map(|x| match x {
                        Some(v) => Some(table.iter().any(|t| match t {
                            Some(t) => (*t == *v) || (t.is_nan() && v.is_nan()),
                            None => false,
                        })),
                        None => Some(table.iter().any(|t| t.is_none())),
                    })
                    .collect();
                Ok(RValue::vec(Vector::Logical(result.into())))
            }
            _ => Ok(RValue::vec(Vector::Logical(vec![Some(false)].into()))),
        }
    }

    /// Matrix multiplication using ndarray
    fn eval_matmul(&self, left: &RValue, right: &RValue) -> Result<RValue, RFlow> {
        // Helper to extract matrix dims and data
        fn to_matrix(val: &RValue) -> Result<(Array2<f64>, usize, usize), RError> {
            let (data, dim_attr) = match val {
                RValue::Vector(rv) => (rv.to_doubles(), rv.get_attr("dim")),
                _ => {
                    return Err(RError::new(
                        RErrorKind::Type,
                        "requires numeric/complex matrix/vector arguments".to_string(),
                    ))
                }
            };
            let (nrow, ncol) = match dim_attr {
                Some(RValue::Vector(rv)) => match &rv.inner {
                    Vector::Integer(d) if d.len() >= 2 => (
                        usize::try_from(d[0].unwrap_or(0))?,
                        usize::try_from(d[1].unwrap_or(0))?,
                    ),
                    _ => (data.len(), 1), // treat as column vector
                },
                _ => (data.len(), 1), // treat as column vector
            };
            let flat: Vec<f64> = data.iter().map(|x| x.unwrap_or(f64::NAN)).collect();
            // ndarray uses row-major by default, R uses column-major
            // Array2::from_shape_vec with column-major (Fortran) order
            let arr =
                Array2::from_shape_vec((nrow, ncol).f(), flat).map_err(|source| -> RError {
                    builtins::math::MathError::Shape { source }.into()
                })?;
            Ok((arr, nrow, ncol))
        }

        let (a, _arows, acols) = to_matrix(left)?;
        let (b, brows, bcols) = to_matrix(right)?;

        if acols != brows {
            return Err(RError::other(format!(
                "non-conformable arguments: {}x{} vs {}x{}",
                a.nrows(),
                acols,
                brows,
                bcols
            ))
            .into());
        }

        let c = a.dot(&b);
        let (rrows, rcols) = (c.nrows(), c.ncols());

        // Convert back to column-major R vector
        let mut result = Vec::with_capacity(rrows * rcols);
        for j in 0..rcols {
            for i in 0..rrows {
                result.push(Some(c[[i, j]]));
            }
        }

        let mut rv = RVector::from(Vector::Double(result.into()));
        rv.set_attr(
            "dim".to_string(),
            RValue::vec(Vector::Integer(
                vec![Some(i64::try_from(rrows)?), Some(i64::try_from(rcols)?)].into(),
            )),
        );
        Ok(RValue::Vector(rv))
    }

    fn eval_pipe(&self, lhs: &Expr, rhs: &Expr, env: &Environment) -> Result<RValue, RFlow> {
        let left_val = self.eval_in(lhs, env)?;
        // rhs should be a function call; inject left_val as first argument
        match rhs {
            Expr::Call { func, args } => {
                let f = self.eval_in(func, env)?;
                let mut eval_args = vec![left_val];
                let mut named_args = Vec::new();
                for arg in args {
                    if let Some(ref name) = arg.name {
                        if let Some(ref val_expr) = arg.value {
                            named_args.push((name.clone(), self.eval_in(val_expr, env)?));
                        }
                    } else if let Some(ref val_expr) = arg.value {
                        eval_args.push(self.eval_in(val_expr, env)?);
                    }
                }
                self.call_function(&f, &eval_args, &named_args, env)
            }
            Expr::Symbol(name) => {
                // x |> f  is equivalent to f(x)
                let f = env
                    .get(name)
                    .ok_or_else(|| RError::new(RErrorKind::Name, name.clone()))?;
                self.call_function(&f, &[left_val], &[], env)
            }
            _ => Err(RError::other("invalid use of pipe".to_string()).into()),
        }
    }

    fn eval_assign(
        &self,
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
            Expr::Index { object, indices } => self.eval_index_assign(object, indices, val, env),
            Expr::IndexDouble { object, indices } => {
                self.eval_index_double_assign(object, indices, val, env)
            }
            Expr::Dollar { object, member } => self.eval_dollar_assign(object, member, val, env),
            // Handle function calls on left side like names(x) <- val, attr(x, "which") <- val
            Expr::Call {
                func,
                args: call_args,
            } => {
                if let Expr::Symbol(fname) = func.as_ref() {
                    let replacement_fn = format!("{}<-", fname);
                    if let Some(first_arg) = call_args.first() {
                        if let Some(ref val_expr) = first_arg.value {
                            let obj = self.eval_in(val_expr, env)?;
                            if let Some(f) = env.get(&replacement_fn) {
                                // Evaluate extra args (e.g. "which" in attr(x, "which") <- val)
                                let mut positional = vec![obj];
                                for arg in &call_args[1..] {
                                    if let Some(ref v) = arg.value {
                                        positional.push(self.eval_in(v, env)?);
                                    }
                                }
                                positional.push(val.clone());
                                let result = self.call_function(&f, &positional, &[], env)?;
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

    fn eval_call(&self, func: &Expr, args: &[Arg], env: &Environment) -> Result<RValue, RFlow> {
        call_eval::eval_call(self, func, args, env)
    }

    pub fn call_function(
        &self,
        func: &RValue,
        positional: &[RValue],
        named: &[(String, RValue)],
        env: &Environment,
    ) -> Result<RValue, RFlow> {
        call_eval::call_function(self, func, positional, named, env)
    }

    pub(crate) fn call_function_with_call(
        &self,
        func: &RValue,
        positional: &[RValue],
        named: &[(String, RValue)],
        env: &Environment,
        call_expr: Option<Expr>,
    ) -> Result<RValue, RFlow> {
        call_eval::call_function_with_call(self, func, positional, named, env, call_expr)
    }

    fn eval_index(
        &self,
        object: &Expr,
        indices: &[Arg],
        env: &Environment,
    ) -> Result<RValue, RFlow> {
        indexing::eval_index(self, object, indices, env)
    }

    pub(crate) fn index_by_integer(
        &self,
        v: &Vector,
        indices: &[Option<i64>],
    ) -> Result<RValue, RFlow> {
        indexing::index_by_integer(self, v, indices)
    }

    fn eval_index_double(
        &self,
        object: &Expr,
        indices: &[Arg],
        env: &Environment,
    ) -> Result<RValue, RFlow> {
        indexing::eval_index_double(self, object, indices, env)
    }

    fn eval_dollar(&self, object: &Expr, member: &str, env: &Environment) -> Result<RValue, RFlow> {
        indexing::eval_dollar(self, object, member, env)
    }

    fn eval_index_assign(
        &self,
        object: &Expr,
        indices: &[Arg],
        val: RValue,
        env: &Environment,
    ) -> Result<RValue, RFlow> {
        indexing::eval_index_assign(self, object, indices, val, env)
    }

    fn eval_index_double_assign(
        &self,
        object: &Expr,
        indices: &[Arg],
        val: RValue,
        env: &Environment,
    ) -> Result<RValue, RFlow> {
        indexing::eval_index_double_assign(self, object, indices, val, env)
    }

    fn eval_dollar_assign(
        &self,
        object: &Expr,
        member: &str,
        val: RValue,
        env: &Environment,
    ) -> Result<RValue, RFlow> {
        indexing::eval_dollar_assign(self, object, member, val, env)
    }

    fn eval_ns_get(
        &self,
        namespace: &Expr,
        name: &str,
        env: &Environment,
    ) -> Result<RValue, RFlow> {
        // For now, just look up the name in the global environment
        // A real implementation would use R's namespace/package system
        let _ns = self.eval_in(namespace, env)?;
        env.get(name)
            .or_else(|| self.global_env.get(name))
            .ok_or_else(|| RError::new(RErrorKind::Name, format!("{}::{}", "pkg", name)).into())
    }

    fn eval_for(
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
                        Err(e) => return Err(e),
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
                        Err(e) => return Err(e),
                    }
                }
            }
            RValue::Null => {}
            _ => {
                return Err(RError::new(
                    RErrorKind::Type,
                    "invalid for() loop sequence".to_string(),
                )
                .into())
            }
        }
        Ok(RValue::Null)
    }
}
