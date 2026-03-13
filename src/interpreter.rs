pub mod builtins;
pub mod coerce;
pub mod environment;
pub mod value;

use std::cell::RefCell;
use std::collections::HashSet;

use ndarray::{Array2, ShapeBuilder};

use crate::parser::ast::*;
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

pub(crate) fn retarget_call_expr(call_expr: Option<Expr>, target: &str) -> Option<Expr> {
    match call_expr {
        Some(Expr::Call { args, .. }) => Some(Expr::Call {
            func: Box::new(Expr::Symbol(target.to_string())),
            args,
        }),
        _ => None,
    }
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

/// Context for S3 method dispatch — tracks which class was dispatched and the
/// remaining classes in the chain (for NextMethod).
#[derive(Debug, Clone)]
pub(crate) struct S3DispatchContext {
    pub generic: String,
    pub classes: Vec<String>,
    pub class_index: usize, // index of the class that was dispatched
    pub object: RValue,
}

#[derive(Debug, Clone)]
pub(crate) struct CallFrame {
    pub call: Option<Expr>,
    pub function: RValue,
    pub env: Environment,
    pub formal_args: HashSet<String>,
    pub supplied_args: HashSet<String>,
    pub supplied_positional: Vec<RValue>,
    pub supplied_named: Vec<(String, RValue)>,
    pub supplied_arg_count: usize,
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

impl Interpreter {
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

    pub(crate) fn current_call_frame(&self) -> Option<CallFrame> {
        self.call_stack.borrow().last().cloned()
    }

    pub(crate) fn call_frame(&self, which: usize) -> Option<CallFrame> {
        self.call_stack
            .borrow()
            .get(which.saturating_sub(1))
            .cloned()
    }

    pub(crate) fn call_frames(&self) -> Vec<CallFrame> {
        self.call_stack.borrow().clone()
    }

    pub(crate) fn current_call_expr(&self) -> Option<Expr> {
        self.current_call_frame().and_then(|frame| frame.call)
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
        let f = self.eval_in(func, env)?;
        let call_expr = Expr::Call {
            func: Box::new(func.clone()),
            args: args.to_vec(),
        };

        // R behavior: if the symbol resolved to a non-function but we're in
        // call position, search up the env chain for a function with that name
        // (like R's findFun). This lets `c <- 1; c(1,2,3)` still work.
        let f = if !matches!(f, RValue::Function(_)) {
            if let Expr::Symbol(name) = func {
                env.get_function(name)
                    .ok_or_else(|| RError::other("attempt to apply non-function".to_string()))?
            } else {
                f
            }
        } else {
            f
        };

        // Pre-eval builtins intercept before argument evaluation
        if let RValue::Function(RFunction::Builtin { name, .. }) = &f {
            if name == "UseMethod" {
                return self.eval_use_method(args, env);
            }
            for &(pname, pfunc, _) in builtins::PRE_EVAL_BUILTIN_REGISTRY {
                if pname == name {
                    return pfunc(args, env).map_err(Into::into);
                }
            }
        }

        let mut positional = Vec::new();
        let mut named = Vec::new();

        for arg in args {
            if let Some(ref name) = arg.name {
                if let Some(ref val_expr) = arg.value {
                    named.push((name.clone(), self.eval_in(val_expr, env)?));
                } else {
                    // name= with no value (missing)
                    named.push((name.clone(), RValue::Null));
                }
            } else if let Some(ref val_expr) = arg.value {
                // Expand ... into individual positional/named args
                if matches!(val_expr, Expr::Dots) {
                    if let Some(RValue::List(list)) = env.get("...") {
                        for (opt_name, val) in &list.values {
                            if let Some(n) = opt_name {
                                named.push((n.clone(), val.clone()));
                            } else {
                                positional.push(val.clone());
                            }
                        }
                    }
                } else {
                    positional.push(self.eval_in(val_expr, env)?);
                }
            }
        }

        self.call_function_with_call(&f, &positional, &named, env, Some(call_expr))
    }

    fn eval_use_method(&self, args: &[Arg], env: &Environment) -> Result<RValue, RFlow> {
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

        let generic_value = self.eval_in(generic_expr, env)?;
        let generic = match generic_value {
            RValue::Vector(rv) => match &rv.inner {
                Vector::Character(values) if values.len() == 1 => {
                    values.first().cloned().flatten().ok_or_else(|| {
                        RError::other("'generic' argument must be a character string".to_string())
                    })?
                }
                _ => {
                    return Err(RError::other(
                        "'generic' argument must be a character string".to_string(),
                    )
                    .into());
                }
            },
            _ => {
                return Err(RError::other(
                    "'generic' argument must be a character string".to_string(),
                )
                .into());
            }
        };

        let object_expr = args
            .iter()
            .find(|arg| arg.name.as_deref() == Some("object"))
            .or_else(|| args.get(1))
            .and_then(|arg| arg.value.as_ref());

        let dispatch_object = match object_expr {
            Some(expr) => Some(self.eval_in(expr, env)?),
            None => match &frame.function {
                RValue::Function(RFunction::Closure { params, .. }) => match params.first() {
                    Some(param) if param.is_dots => {
                        frame.env.get("...").and_then(|value| match value {
                            RValue::List(list) => {
                                list.values.first().map(|(_, value)| value.clone())
                            }
                            _ => None,
                        })
                    }
                    Some(param) => frame.env.get(&param.name),
                    None => None,
                },
                _ => {
                    return Err(RError::other(
                        "'UseMethod' used in an inappropriate fashion".to_string(),
                    )
                    .into());
                }
            },
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

    pub fn call_function(
        &self,
        func: &RValue,
        positional: &[RValue],
        named: &[(String, RValue)],
        env: &Environment,
    ) -> Result<RValue, RFlow> {
        self.call_function_with_call(func, positional, named, env, None)
    }

    pub(crate) fn call_function_with_call(
        &self,
        func: &RValue,
        positional: &[RValue],
        named: &[(String, RValue)],
        env: &Environment,
        call_expr: Option<Expr>,
    ) -> Result<RValue, RFlow> {
        match func {
            RValue::Function(RFunction::Builtin { func, name, .. }) => {
                // Check interpreter-level builtins (access interp via thread-local)
                for &(iname, ifunc, _) in builtins::INTERPRETER_BUILTIN_REGISTRY {
                    if iname == name {
                        return ifunc(positional, named, env).map_err(Into::into);
                    }
                }
                func(positional, named).map_err(Into::into)
            }
            RValue::Function(RFunction::Closure {
                params,
                body,
                env: closure_env,
            }) => {
                let call_env = Environment::new_child(closure_env);

                // Bind parameters
                let mut pos_idx = 0;
                let mut dots_vals: Vec<(Option<String>, RValue)> = Vec::new();
                let mut has_dots = false;
                let param_names: Vec<&str> = params.iter().map(|p| p.name.as_str()).collect();
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
                        // Collect remaining positional args into ...
                        while pos_idx < positional.len() {
                            dots_vals.push((None, positional[pos_idx].clone()));
                            pos_idx += 1;
                        }
                        // Collect unmatched named args into ...
                        for (n, v) in named {
                            if !param_names.contains(&n.as_str()) {
                                dots_vals.push((Some(n.clone()), v.clone()));
                            }
                        }
                        supplied_arg_count += dots_vals.len();
                        continue;
                    }

                    // Try named argument first
                    if let Some((_, val)) = named.iter().find(|(n, _)| *n == param.name) {
                        call_env.set(param.name.clone(), val.clone());
                        supplied_args.insert(param.name.clone());
                        supplied_arg_count += 1;
                    } else if pos_idx < positional.len() {
                        call_env.set(param.name.clone(), positional[pos_idx].clone());
                        supplied_args.insert(param.name.clone());
                        supplied_arg_count += 1;
                        pos_idx += 1;
                    } else if let Some(ref default) = param.default {
                        let val = self.eval_in(default, &call_env)?;
                        call_env.set(param.name.clone(), val);
                    }
                    // else: missing argument, will error when accessed
                }

                // Bind ... as a list in the call environment
                if has_dots {
                    call_env.set("...".to_string(), RValue::List(RList::new(dots_vals)));
                }

                self.call_stack.borrow_mut().push(CallFrame {
                    call: call_expr,
                    function: func.clone(),
                    env: call_env.clone(),
                    formal_args,
                    supplied_args,
                    supplied_positional: positional.to_vec(),
                    supplied_named: named.to_vec(),
                    supplied_arg_count,
                });

                let result = match self.eval_in(body, &call_env) {
                    Ok(val) => Ok(val),
                    Err(RFlow::Signal(RSignal::Return(val))) => Ok(val),
                    Err(e) => Err(e),
                };

                // Run on.exit handlers regardless of success/failure
                let on_exit_exprs = call_env.take_on_exit();
                for expr in &on_exit_exprs {
                    // on.exit handlers run but don't alter the return value
                    let _ = self.eval_in(expr, &call_env);
                }
                self.call_stack.borrow_mut().pop();

                result
            }
            _ => Err(RError::new(
                RErrorKind::Type,
                "attempt to apply non-function".to_string(),
            )
            .into()),
        }
    }

    fn eval_index(
        &self,
        object: &Expr,
        indices: &[Arg],
        env: &Environment,
    ) -> Result<RValue, RFlow> {
        let obj = self.eval_in(object, env)?;

        if indices.is_empty() {
            return Ok(obj);
        }

        // 2D indexing: x[i, j] for matrices
        if indices.len() >= 2 {
            return self.eval_matrix_index(&obj, indices, env);
        }

        // Evaluate indices
        let idx_val = if let Some(ref val_expr) = indices[0].value {
            self.eval_in(val_expr, env)?
        } else {
            return Ok(obj);
        };

        match &obj {
            RValue::Vector(v) => {
                match &idx_val {
                    RValue::Vector(idx_vec) => {
                        // Logical indexing
                        if let Vector::Logical(mask) = &idx_vec.inner {
                            return self.index_by_logical(v, mask);
                        }
                        // Negative indexing (exclusion)
                        let indices = idx_vec.to_integers();
                        if indices.iter().all(|x| x.map(|i| i < 0).unwrap_or(false)) {
                            return self.index_by_negative(v, &indices);
                        }
                        // Positive integer indexing
                        self.index_by_integer(v, &indices)
                    }
                    RValue::Null => Ok(obj.clone()),
                    _ => {
                        Err(RError::new(RErrorKind::Index, "invalid index type".to_string()).into())
                    }
                }
            }
            RValue::List(list) => {
                match &idx_val {
                    RValue::Vector(idx_vec) => {
                        // String indexing
                        if let Vector::Character(names) = &idx_vec.inner {
                            let mut result = Vec::new();
                            for name in names.iter().flatten() {
                                let found = list
                                    .values
                                    .iter()
                                    .find(|(n, _)| n.as_ref() == Some(name))
                                    .map(|(n, v)| (n.clone(), v.clone()));
                                if let Some(item) = found {
                                    result.push(item);
                                }
                            }
                            return Ok(RValue::List(RList::new(result)));
                        }
                        // Integer indexing
                        let indices = idx_vec.to_integers();
                        let mut result = Vec::new();
                        for i in indices.iter().flatten() {
                            let i = usize::try_from(*i).unwrap_or(0);
                            if i > 0 && i <= list.values.len() {
                                result.push(list.values[i - 1].clone());
                            }
                        }
                        Ok(RValue::List(RList::new(result)))
                    }
                    _ => {
                        Err(RError::new(RErrorKind::Index, "invalid index type".to_string()).into())
                    }
                }
            }
            _ => {
                Err(RError::new(RErrorKind::Index, "object is not subsettable".to_string()).into())
            }
        }
    }

    /// 2D matrix indexing: x[i, j] where the vector has a dim attribute
    fn eval_matrix_index(
        &self,
        obj: &RValue,
        indices: &[Arg],
        env: &Environment,
    ) -> Result<RValue, RFlow> {
        // Get the dim attribute
        let (data, dim_attr) = match obj {
            RValue::Vector(rv) => (&rv.inner, rv.get_attr("dim")),
            RValue::List(l) => {
                // Data frame: x[rows, cols] or list with dim
                return self.eval_list_2d_index(l, indices, env);
            }
            _ => {
                return Err(RError::new(
                    RErrorKind::Index,
                    "incorrect number of dimensions".to_string(),
                )
                .into())
            }
        };

        let dims = match dim_attr {
            Some(RValue::Vector(rv)) => match &rv.inner {
                Vector::Integer(d) => d.0.clone(),
                _ => {
                    return Err(RError::new(
                        RErrorKind::Index,
                        "incorrect number of dimensions".to_string(),
                    )
                    .into())
                }
            },
            _ => {
                return Err(RError::new(
                    RErrorKind::Index,
                    "incorrect number of dimensions".to_string(),
                )
                .into())
            }
        };

        if dims.len() < 2 {
            return Err(RError::new(
                RErrorKind::Index,
                "incorrect number of dimensions".to_string(),
            )
            .into());
        }
        let nrow = usize::try_from(dims[0].unwrap_or(0)).unwrap_or(0);
        let ncol = usize::try_from(dims[1].unwrap_or(0)).unwrap_or(0);

        // Evaluate row indices (empty = all rows)
        let row_idx = if let Some(ref val_expr) = indices[0].value {
            let v = self.eval_in(val_expr, env)?;
            Some(v)
        } else {
            None // empty = all
        };

        // Evaluate col indices (empty = all cols)
        let col_idx = if let Some(ref val_expr) = indices[1].value {
            let v = self.eval_in(val_expr, env)?;
            Some(v)
        } else {
            None
        };

        let rows: Vec<usize> = match &row_idx {
            None => (0..nrow).collect(),
            Some(RValue::Vector(rv)) => rv
                .to_integers()
                .iter()
                .filter_map(|x| x.and_then(|i| usize::try_from(i - 1).ok()))
                .collect(),
            _ => {
                return Err(RError::new(RErrorKind::Index, "invalid row index".to_string()).into())
            }
        };

        let cols: Vec<usize> = match &col_idx {
            None => (0..ncol).collect(),
            Some(RValue::Vector(rv)) => rv
                .to_integers()
                .iter()
                .filter_map(|x| x.and_then(|i| usize::try_from(i - 1).ok()))
                .collect(),
            _ => {
                return Err(
                    RError::new(RErrorKind::Index, "invalid column index".to_string()).into(),
                )
            }
        };

        // Extract elements in column-major order
        let doubles = data.to_doubles();
        let mut result = Vec::new();
        for &j in &cols {
            for &i in &rows {
                let flat_idx = j * nrow + i;
                result.push(doubles.get(flat_idx).copied().unwrap_or(None));
            }
        }

        // If result is a single element, return scalar
        if rows.len() == 1 && cols.len() == 1 {
            return Ok(RValue::vec(Vector::Double(result.into())));
        }

        // If selecting a sub-matrix, add dim attribute
        let mut rv = RVector::from(Vector::Double(result.into()));
        if rows.len() > 1 || cols.len() > 1 {
            rv.set_attr(
                "dim".to_string(),
                RValue::vec(Vector::Integer(
                    vec![
                        Some(i64::try_from(rows.len())?),
                        Some(i64::try_from(cols.len())?),
                    ]
                    .into(),
                )),
            );
        }
        Ok(RValue::Vector(rv))
    }

    /// 2D indexing for lists/data frames: x[rows, cols]
    fn eval_list_2d_index(
        &self,
        list: &RList,
        indices: &[Arg],
        env: &Environment,
    ) -> Result<RValue, RFlow> {
        // For data frames: x[rows, cols]
        let is_df = if let Some(RValue::Vector(rv)) = list.get_attr("class") {
            if let Vector::Character(cls) = &rv.inner {
                cls.iter().any(|c| c.as_deref() == Some("data.frame"))
            } else {
                false
            }
        } else {
            false
        };
        if !is_df {
            // Non-data-frame list with 2D index — just use first index
            if let Some(ref val_expr) = indices[0].value {
                let idx_val = self.eval_in(val_expr, env)?;
                return match &idx_val {
                    RValue::Vector(iv) => {
                        let i = usize::try_from(iv.as_integer_scalar().unwrap_or(0)).unwrap_or(0);
                        if i > 0 && i <= list.values.len() {
                            Ok(list.values[i - 1].1.clone())
                        } else {
                            Ok(RValue::Null)
                        }
                    }
                    _ => Ok(RValue::Null),
                };
            }
            return Ok(RValue::Null);
        }

        // Data frame: columns from second index, then rows from first
        let col_idx = if let Some(ref val_expr) = indices[1].value {
            Some(self.eval_in(val_expr, env)?)
        } else {
            None
        };

        let selected_cols: Vec<(Option<String>, RValue)> = match &col_idx {
            None => list.values.clone(),
            Some(RValue::Vector(rv)) if matches!(rv.inner, Vector::Character(_)) => {
                let Vector::Character(names) = &rv.inner else {
                    unreachable!()
                };
                names
                    .iter()
                    .filter_map(|n| {
                        n.as_ref().and_then(|name| {
                            list.values
                                .iter()
                                .find(|(k, _)| k.as_ref() == Some(name))
                                .cloned()
                        })
                    })
                    .collect()
            }
            Some(RValue::Vector(rv)) => {
                let idxs = rv.to_integers();
                idxs.iter()
                    .filter_map(|i| {
                        i.and_then(|i| {
                            let i = usize::try_from(i - 1).ok()?;
                            list.values.get(i).cloned()
                        })
                    })
                    .collect()
            }
            _ => list.values.clone(),
        };

        // Now apply row subsetting
        let row_idx = if let Some(ref val_expr) = indices[0].value {
            Some(self.eval_in(val_expr, env)?)
        } else {
            None
        };

        if row_idx.is_none() {
            // Single column selection with drop=TRUE (R default) returns the vector
            if col_idx.is_some() && selected_cols.len() == 1 {
                return Ok(selected_cols.into_iter().next().unwrap().1);
            }
            // All rows — return data frame with selected columns
            let col_names: Vec<Option<String>> =
                selected_cols.iter().map(|(n, _)| n.clone()).collect();
            let nrows = selected_cols.first().map(|(_, v)| v.length()).unwrap_or(0);
            let mut result = RList::new(selected_cols);
            result.set_attr(
                "class".to_string(),
                RValue::vec(Vector::Character(
                    vec![Some("data.frame".to_string())].into(),
                )),
            );
            result.set_attr(
                "names".to_string(),
                RValue::vec(Vector::Character(col_names.into())),
            );
            let row_names: Vec<Option<i64>> =
                (1..=i64::try_from(nrows).unwrap_or(0)).map(Some).collect();
            result.set_attr(
                "row.names".to_string(),
                RValue::vec(Vector::Integer(row_names.into())),
            );
            return Ok(RValue::List(result));
        }

        // Convert logical row index to integer indices
        let int_rows: Vec<Option<i64>> = match &row_idx {
            Some(RValue::Vector(rv)) if matches!(rv.inner, Vector::Logical(_)) => {
                let Vector::Logical(lv) = &rv.inner else {
                    unreachable!()
                };
                lv.iter()
                    .enumerate()
                    .filter(|(_, v)| v.unwrap_or(false))
                    .filter_map(|(i, _)| i64::try_from(i).ok().map(|i| Some(i + 1)))
                    .collect()
            }
            Some(RValue::Vector(rv)) => rv.to_integers(),
            _ => vec![],
        };

        // Single column with row selection — return the column vector subsetted
        if selected_cols.len() == 1 {
            if let RValue::Vector(rv) = &selected_cols[0].1 {
                return self.index_by_integer(&rv.inner, &int_rows);
            }
            return Ok(selected_cols[0].1.clone());
        }

        // Multiple columns with row selection — subset each column
        let mut result_cols = Vec::new();
        for (name, col_val) in &selected_cols {
            if let RValue::Vector(rv) = col_val {
                let indexed = self.index_by_integer(&rv.inner, &int_rows)?;
                result_cols.push((name.clone(), indexed));
            } else {
                result_cols.push((name.clone(), col_val.clone()));
            }
        }
        let col_names: Vec<Option<String>> = result_cols.iter().map(|(n, _)| n.clone()).collect();
        let nrows = result_cols.first().map(|(_, v)| v.length()).unwrap_or(0);
        let mut result = RList::new(result_cols);
        result.set_attr(
            "class".to_string(),
            RValue::vec(Vector::Character(
                vec![Some("data.frame".to_string())].into(),
            )),
        );
        result.set_attr(
            "names".to_string(),
            RValue::vec(Vector::Character(col_names.into())),
        );
        let row_names: Vec<Option<i64>> =
            (1..=i64::try_from(nrows).unwrap_or(0)).map(Some).collect();
        result.set_attr(
            "row.names".to_string(),
            RValue::vec(Vector::Integer(row_names.into())),
        );
        Ok(RValue::List(result))
    }

    pub(crate) fn index_by_integer(
        &self,
        v: &Vector,
        indices: &[Option<i64>],
    ) -> Result<RValue, RFlow> {
        macro_rules! index_vec {
            ($vals:expr, $variant:ident) => {{
                let result: Vec<_> = indices
                    .iter()
                    .map(|idx| {
                        idx.and_then(|i| {
                            let i = usize::try_from(i).unwrap_or(0);
                            if i > 0 && i <= $vals.len() {
                                $vals[i - 1].clone().into()
                            } else {
                                None
                            }
                        })
                    })
                    .collect();
                Ok(RValue::vec(Vector::$variant(result.into())))
            }};
        }
        match v {
            Vector::Raw(vals) => {
                let result: Vec<u8> = indices
                    .iter()
                    .map(|idx| {
                        idx.and_then(|i| {
                            let i = usize::try_from(i).unwrap_or(0);
                            if i > 0 && i <= vals.len() {
                                Some(vals[i - 1])
                            } else {
                                Some(0u8)
                            }
                        })
                        .unwrap_or(0u8)
                    })
                    .collect();
                Ok(RValue::vec(Vector::Raw(result)))
            }
            Vector::Double(vals) => index_vec!(vals, Double),
            Vector::Integer(vals) => index_vec!(vals, Integer),
            Vector::Logical(vals) => index_vec!(vals, Logical),
            Vector::Complex(vals) => index_vec!(vals, Complex),
            Vector::Character(vals) => index_vec!(vals, Character),
        }
    }

    fn index_by_negative(&self, v: &Vector, indices: &[Option<i64>]) -> Result<RValue, RFlow> {
        let exclude: Vec<usize> = indices
            .iter()
            .filter_map(|x| x.and_then(|i| usize::try_from(-i).ok()))
            .collect();

        macro_rules! filter_vec {
            ($vals:expr, $variant:ident) => {{
                let result: Vec<_> = $vals
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| !exclude.contains(&(i + 1)))
                    .map(|(_, v)| v.clone())
                    .collect();
                Ok(RValue::vec(Vector::$variant(result.into())))
            }};
        }
        match v {
            Vector::Raw(vals) => {
                let result: Vec<u8> = vals
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| !exclude.contains(&(i + 1)))
                    .map(|(_, &v)| v)
                    .collect();
                Ok(RValue::vec(Vector::Raw(result)))
            }
            Vector::Double(vals) => filter_vec!(vals, Double),
            Vector::Integer(vals) => filter_vec!(vals, Integer),
            Vector::Logical(vals) => filter_vec!(vals, Logical),
            Vector::Complex(vals) => filter_vec!(vals, Complex),
            Vector::Character(vals) => filter_vec!(vals, Character),
        }
    }

    fn index_by_logical(&self, v: &Vector, mask: &[Option<bool>]) -> Result<RValue, RFlow> {
        macro_rules! mask_vec {
            ($vals:expr, $variant:ident) => {{
                let result: Vec<_> = $vals
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| mask.get(*i).copied().flatten().unwrap_or(false))
                    .map(|(_, v)| v.clone())
                    .collect();
                Ok(RValue::vec(Vector::$variant(result.into())))
            }};
        }
        match v {
            Vector::Raw(vals) => {
                let result: Vec<u8> = vals
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| mask.get(*i).copied().flatten().unwrap_or(false))
                    .map(|(_, &v)| v)
                    .collect();
                Ok(RValue::vec(Vector::Raw(result)))
            }
            Vector::Double(vals) => mask_vec!(vals, Double),
            Vector::Integer(vals) => mask_vec!(vals, Integer),
            Vector::Logical(vals) => mask_vec!(vals, Logical),
            Vector::Complex(vals) => mask_vec!(vals, Complex),
            Vector::Character(vals) => mask_vec!(vals, Character),
        }
    }

    fn eval_index_double(
        &self,
        object: &Expr,
        indices: &[Arg],
        env: &Environment,
    ) -> Result<RValue, RFlow> {
        let obj = self.eval_in(object, env)?;
        if indices.is_empty() {
            return Ok(obj);
        }

        let idx_val = if let Some(ref val_expr) = indices[0].value {
            self.eval_in(val_expr, env)?
        } else {
            return Ok(obj);
        };

        match &obj {
            RValue::List(list) => match &idx_val {
                RValue::Vector(rv) if matches!(rv.inner, Vector::Character(_)) => {
                    let Vector::Character(names) = &rv.inner else {
                        unreachable!()
                    };
                    if let Some(Some(name)) = names.first() {
                        for (n, v) in &list.values {
                            if n.as_ref() == Some(name) {
                                return Ok(v.clone());
                            }
                        }
                    }
                    Ok(RValue::Null)
                }
                RValue::Vector(v) => {
                    let i = usize::try_from(v.as_integer_scalar().unwrap_or(0)).unwrap_or(0);
                    if i > 0 && i <= list.values.len() {
                        Ok(list.values[i - 1].1.clone())
                    } else {
                        Ok(RValue::Null)
                    }
                }
                _ => Ok(RValue::Null),
            },
            RValue::Vector(v) => {
                let i = match &idx_val {
                    RValue::Vector(iv) => {
                        usize::try_from(iv.as_integer_scalar().unwrap_or(0)).unwrap_or(0)
                    }
                    _ => 0,
                };
                if i > 0 && i <= v.len() {
                    let idx = i - 1;
                    match &v.inner {
                        Vector::Raw(vals) => Ok(RValue::vec(Vector::Raw(vec![vals[idx]]))),
                        Vector::Double(vals) => {
                            Ok(RValue::vec(Vector::Double(vec![vals[idx]].into())))
                        }
                        Vector::Integer(vals) => {
                            Ok(RValue::vec(Vector::Integer(vec![vals[idx]].into())))
                        }
                        Vector::Logical(vals) => {
                            Ok(RValue::vec(Vector::Logical(vec![vals[idx]].into())))
                        }
                        Vector::Complex(vals) => {
                            Ok(RValue::vec(Vector::Complex(vec![vals[idx]].into())))
                        }
                        Vector::Character(vals) => Ok(RValue::vec(Vector::Character(
                            vec![vals[idx].clone()].into(),
                        ))),
                    }
                } else {
                    Ok(RValue::Null)
                }
            }
            _ => {
                Err(RError::new(RErrorKind::Index, "object is not subsettable".to_string()).into())
            }
        }
    }

    fn eval_dollar(&self, object: &Expr, member: &str, env: &Environment) -> Result<RValue, RFlow> {
        let obj = self.eval_in(object, env)?;
        match &obj {
            RValue::List(list) => {
                for (name, val) in &list.values {
                    if name.as_deref() == Some(member) {
                        return Ok(val.clone());
                    }
                }
                Ok(RValue::Null)
            }
            RValue::Environment(e) => e
                .get(member)
                .ok_or_else(|| RError::new(RErrorKind::Name, member.to_string()).into()),
            _ => Ok(RValue::Null),
        }
    }

    fn eval_index_assign(
        &self,
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

        let idx_val = if let Some(ref val_expr) = indices[0].value {
            self.eval_in(val_expr, env)?
        } else {
            return Ok(val);
        };

        match &mut obj {
            RValue::Vector(v) => {
                let idx_ints = match &idx_val {
                    RValue::Vector(iv) => iv.to_integers(),
                    _ => {
                        return Err(
                            RError::new(RErrorKind::Index, "invalid index".to_string()).into()
                        )
                    }
                };

                let new_vals = match &val {
                    RValue::Vector(vv) => vv.to_doubles(),
                    _ => {
                        return Err(RError::new(
                            RErrorKind::Type,
                            "replacement value error".to_string(),
                        )
                        .into())
                    }
                };

                let mut doubles = v.to_doubles();
                for (j, idx) in idx_ints.iter().enumerate() {
                    if let Some(i) = idx {
                        let i = usize::try_from(*i).unwrap_or(0);
                        if i > 0 {
                            // Extend if necessary
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
                // Create new vector/list
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
            _ => {
                Err(RError::new(RErrorKind::Index, "object is not subsettable".to_string()).into())
            }
        }
    }

    fn eval_index_double_assign(
        &self,
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
        let idx_val = if let Some(ref val_expr) = indices[0].value {
            self.eval_in(val_expr, env)?
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
            _ => self.eval_index_assign(object, indices, val, env),
        }
    }

    fn eval_dollar_assign(
        &self,
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
                // Convert to list
                let list = RList::new(vec![(Some(member.to_string()), val.clone())]);
                env.set(var_name, RValue::List(list));
                Ok(val)
            }
        }
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

    /// S3 method dispatch: look up generic.class in the environment chain
    fn dispatch_s3(
        &self,
        generic: &str,
        positional: &[RValue],
        named: &[(String, RValue)],
        dispatch_object: Option<RValue>,
        env: &Environment,
        call_expr: Option<Expr>,
    ) -> Result<RValue, RFlow> {
        let dispatch_object =
            dispatch_object.unwrap_or_else(|| positional.first().cloned().unwrap_or(RValue::Null));

        // Get class of first argument
        let classes = match &dispatch_object {
            RValue::List(l) => {
                if let Some(RValue::Vector(rv)) = l.get_attr("class") {
                    if let Vector::Character(cls) = &rv.inner {
                        cls.iter().filter_map(|s| s.clone()).collect::<Vec<_>>()
                    } else {
                        vec!["list".to_string()]
                    }
                } else {
                    vec!["list".to_string()]
                }
            }
            RValue::Vector(rv) => {
                if let Some(cls) = rv.class() {
                    cls
                } else {
                    match &rv.inner {
                        Vector::Raw(_) => vec!["raw".to_string()],
                        Vector::Logical(_) => vec!["logical".to_string()],
                        Vector::Integer(_) => vec!["integer".to_string()],
                        Vector::Double(_) => vec!["numeric".to_string()],
                        Vector::Complex(_) => vec!["complex".to_string()],
                        Vector::Character(_) => vec!["character".to_string()],
                    }
                }
            }
            RValue::Function(_) => vec!["function".to_string()],
            RValue::Null => vec!["NULL".to_string()],
            RValue::Language(lang) => lang.class().unwrap_or_default(),
            _ => vec![],
        };

        // Try generic.class for each class in the inheritance chain
        for (i, class) in classes.iter().enumerate() {
            let method_name = format!("{}.{}", generic, class);
            if let Some(method) = env.get(&method_name) {
                let ctx = S3DispatchContext {
                    generic: generic.to_string(),
                    classes: classes.clone(),
                    class_index: i,
                    object: dispatch_object.clone(),
                };
                self.s3_dispatch_stack.borrow_mut().push(ctx);
                let method_call = retarget_call_expr(call_expr.clone(), &method_name);
                let result =
                    self.call_function_with_call(&method, positional, named, env, method_call);
                self.s3_dispatch_stack.borrow_mut().pop();
                return result;
            }
        }

        // Try generic.default
        let default_name = format!("{}.default", generic);
        if let Some(method) = env.get(&default_name) {
            let ctx = S3DispatchContext {
                generic: generic.to_string(),
                classes: classes.clone(),
                class_index: classes.len(),
                object: dispatch_object.clone(),
            };
            self.s3_dispatch_stack.borrow_mut().push(ctx);
            let method_call = retarget_call_expr(call_expr, &default_name);
            let result = self.call_function_with_call(&method, positional, named, env, method_call);
            self.s3_dispatch_stack.borrow_mut().pop();
            return result;
        }

        Err(RError::other(format!(
            "no applicable method for '{}' applied to an object of class \"{}\"",
            generic,
            classes.first().unwrap_or(&"unknown".to_string())
        ))
        .into())
    }
}
