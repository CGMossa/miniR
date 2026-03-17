mod arguments;
mod assignment;
pub mod builtins;
pub mod call;
mod call_eval;
pub mod coerce;
mod control_flow;
pub mod environment;
pub(crate) mod indexing;
mod ops;
mod s3;
pub mod value;

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use log::{debug, info, trace};

use crate::parser::ast::*;
pub use call::BuiltinContext;
pub(crate) use call::{CallFrame, S3DispatchContext};
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
    /// Per-interpreter RNG state. Uses `SmallRng` (Xoshiro256PlusPlus on 64-bit)
    /// which is fast and non-cryptographic — appropriate for R's statistical RNG.
    ///
    /// # Parallel RNG considerations
    ///
    /// The RNG is behind `RefCell` on the single-threaded `Interpreter`, so there
    /// are no data races. If we ever add rayon-based parallel operations, each
    /// worker thread must get its own RNG seeded deterministically from the parent
    /// (e.g. `SmallRng::seed_from_u64(parent_seed + thread_id)`) to avoid
    /// contention and ensure reproducibility. The current single-threaded design
    /// is correct as-is.
    #[cfg(feature = "random")]
    rng: RefCell<rand::rngs::SmallRng>,
    /// Session-scoped temporary directory, auto-cleaned on drop.
    pub(crate) temp_dir: temp_dir::TempDir,
    /// Counter for unique tempfile names within the session.
    pub(crate) temp_counter: std::cell::Cell<u64>,
    /// Per-interpreter environment variable overrides.
    /// Keys present here shadow process env; absent keys fall through to process env.
    pub(crate) env_vars: RefCell<std::collections::HashMap<String, String>>,
    /// Per-interpreter working directory override.
    /// If None, falls through to the process working directory.
    pub(crate) working_dir: RefCell<Option<std::path::PathBuf>>,
    /// Instant when the interpreter was created, used by proc.time() for elapsed time.
    pub(crate) start_instant: std::time::Instant,
    /// Collection objects (HashMap, BTreeMap, HashSet, BinaryHeap, VecDeque).
    /// Each collection is addressed by its index in this Vec.
    #[cfg(feature = "collections")]
    pub(crate) collections: RefCell<Vec<builtins::collections::CollectionObject>>,
    /// Connection table — slots 0-2 are stdin/stdout/stderr, lazily initialised.
    pub(crate) connections: RefCell<Vec<builtins::connections::ConnectionInfo>>,
    /// TCP stream handles, keyed by connection ID. Stored separately from
    /// `ConnectionInfo` because `TcpStream` is not `Clone`.
    pub(crate) tcp_streams: RefCell<std::collections::HashMap<usize, std::net::TcpStream>>,
    /// Finalizers registered with reg.finalizer(onexit = TRUE), run when the
    /// interpreter is dropped.
    pub(crate) finalizers: RefCell<Vec<RValue>>,
    /// Flag set by the SIGINT handler; checked at loop boundaries to interrupt
    /// long-running computations without killing the process.
    interrupted: Arc<AtomicBool>,
    /// Per-interpreter R options (accessed via `options()` and `getOption()`).
    pub(crate) options: RefCell<std::collections::HashMap<String, value::RValue>>,
}

impl Default for Interpreter {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Interpreter {
    fn drop(&mut self) {
        let finalizers: Vec<RValue> = self.finalizers.borrow_mut().drain(..).collect();
        if finalizers.is_empty() {
            return;
        }
        let env = self.global_env.clone();
        for f in &finalizers {
            // Best-effort: errors during finalizer execution are silently ignored,
            // matching R's behavior for on-exit finalizers.
            let _ = self.call_function(f, &[RValue::Environment(env.clone())], &[], &env);
        }
    }
}

impl Interpreter {
    fn ensure_builtin_min_arity(
        name: &str,
        min_args: usize,
        actual_args: usize,
    ) -> Result<(), RError> {
        if min_args == 0 || actual_args >= min_args {
            return Ok(());
        }

        let expectation = match min_args {
            1 => "requires at least 1 argument".to_string(),
            n => format!("requires at least {n} arguments"),
        };
        let suffix = if actual_args == 1 { "" } else { "s" };

        Err(RError::new(
            RErrorKind::Argument,
            format!("{name}() {expectation}, got {actual_args} argument{suffix}"),
        ))
    }

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
        info!("creating new interpreter");
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
            rng: RefCell::new({
                use rand::SeedableRng;
                let mut thread_rng = rand::rng();
                rand::rngs::SmallRng::from_rng(&mut thread_rng)
            }),
            temp_dir: temp_dir::TempDir::new().expect("failed to create session temp directory"),
            temp_counter: std::cell::Cell::new(0),
            env_vars: RefCell::new(std::collections::HashMap::new()),
            working_dir: RefCell::new(None),
            start_instant: std::time::Instant::now(),
            #[cfg(feature = "collections")]
            collections: RefCell::new(Vec::new()),
            connections: RefCell::new(Vec::new()),
            tcp_streams: RefCell::new(std::collections::HashMap::new()),
            finalizers: RefCell::new(Vec::new()),
            interrupted: Arc::new(AtomicBool::new(false)),
            options: RefCell::new(Self::default_options()),
        }
    }

    /// Return a clone of the interrupt flag so the SIGINT handler can set it.
    pub fn interrupt_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.interrupted)
    }

    /// Check the interrupt flag; if set, clear it and return an interrupt error.
    pub(crate) fn check_interrupt(&self) -> Result<(), RFlow> {
        if self.interrupted.load(Ordering::Relaxed) {
            self.interrupted.store(false, Ordering::Relaxed);
            debug!("interrupt detected");
            Err(RFlow::Error(RError::interrupt()))
        } else {
            Ok(())
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

    /// Default R options, matching GNU R defaults where sensible.
    fn default_options() -> std::collections::HashMap<String, value::RValue> {
        use value::{RValue, Vector};
        let mut opts = std::collections::HashMap::new();
        opts.insert(
            "digits".to_string(),
            RValue::vec(Vector::Integer(vec![Some(7)].into())),
        );
        opts.insert(
            "warn".to_string(),
            RValue::vec(Vector::Integer(vec![Some(0)].into())),
        );
        opts.insert(
            "OutDec".to_string(),
            RValue::vec(Vector::Character(vec![Some(".".to_string())].into())),
        );
        opts.insert(
            "scipen".to_string(),
            RValue::vec(Vector::Integer(vec![Some(0)].into())),
        );
        opts.insert(
            "max.print".to_string(),
            RValue::vec(Vector::Integer(vec![Some(99999)].into())),
        );
        opts.insert(
            "width".to_string(),
            RValue::vec(Vector::Integer(vec![Some(80)].into())),
        );
        opts.insert(
            "warning.length".to_string(),
            RValue::vec(Vector::Integer(vec![Some(1000)].into())),
        );
        opts.insert(
            "prompt".to_string(),
            RValue::vec(Vector::Character(vec![Some("> ".to_string())].into())),
        );
        opts.insert(
            "continue".to_string(),
            RValue::vec(Vector::Character(vec![Some("+ ".to_string())].into())),
        );
        opts.insert(
            "encoding".to_string(),
            RValue::vec(Vector::Character(
                vec![Some("native.enc".to_string())].into(),
            )),
        );
        opts.insert(
            "stringsAsFactors".to_string(),
            RValue::vec(Vector::Logical(vec![Some(false)].into())),
        );
        opts
    }

    #[cfg(feature = "random")]
    pub fn rng(&self) -> &RefCell<rand::rngs::SmallRng> {
        &self.rng
    }

    /// Get an environment variable — checks interpreter-local overrides first,
    /// then falls through to process env.
    pub(crate) fn get_env_var(&self, name: &str) -> Option<String> {
        if let Some(val) = self.env_vars.borrow().get(name) {
            return Some(val.clone());
        }
        std::env::var(name).ok()
    }

    /// Set a per-interpreter environment variable (does not mutate process state).
    pub(crate) fn set_env_var(&self, name: String, value: String) {
        self.env_vars.borrow_mut().insert(name, value);
    }

    /// Get the working directory — uses interpreter-local override if set,
    /// otherwise the process working directory.
    pub(crate) fn get_working_dir(&self) -> std::path::PathBuf {
        self.working_dir
            .borrow()
            .clone()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
    }

    /// Set the per-interpreter working directory (does not mutate process state).
    pub(crate) fn set_working_dir(&self, path: std::path::PathBuf) {
        *self.working_dir.borrow_mut() = Some(path);
    }

    pub fn eval(&self, expr: &Expr) -> Result<RValue, RFlow> {
        self.eval_in(expr, &self.global_env)
    }

    pub fn eval_in(&self, expr: &Expr, env: &Environment) -> Result<RValue, RFlow> {
        trace!("eval: {:?}", expr);
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
            Expr::Symbol(name) => env.get(name).ok_or_else(|| {
                debug!("symbol not found: {}", name);
                RError::new(RErrorKind::Name, name.clone()).into()
            }),
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
            } => self.eval_if(condition, then_body, else_body.as_deref(), env),

            Expr::For { var, iter, body } => {
                let iter_val = self.eval_in(iter, env)?;
                self.eval_for(var, &iter_val, body, env)
            }

            Expr::While { condition, body } => self.eval_while(condition, body, env),

            Expr::Repeat { body } => self.eval_repeat(body, env),

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
                    self.check_interrupt()?;
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
                    self.check_interrupt()?;
                    result = self.eval_in(expr, env)?;
                }
                Ok(result)
            }
        }
    }
}
