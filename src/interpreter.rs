mod arguments;
mod assignment;
pub mod builtins;
pub mod call;
mod call_eval;
pub mod coerce;
mod control_flow;
pub mod environment;
pub mod graphics;
pub(crate) mod indexing;
mod ops;
pub mod packages;
mod s3;
pub mod value;

use std::cell::RefCell;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tracing::{debug, info};

use crate::parser::ast::*;
pub use call::BuiltinContext;
pub(crate) use call::{CallFrame, S3DispatchContext};
use environment::Environment;
use value::*;

// region: InterpreterRng

/// Selectable RNG backend for the interpreter.
///
/// `Fast` (default) uses `SmallRng` (Xoshiro256++) — fast, non-cryptographic,
/// suitable for R's statistical RNG. `Deterministic` uses ChaCha20 — slower
/// but produces identical sequences across all platforms and Rust versions,
/// which is important for cross-platform reproducibility.
#[cfg(feature = "random")]
pub(crate) enum InterpreterRng {
    Fast(rand::rngs::SmallRng),
    Deterministic(Box<rand_chacha::ChaCha20Rng>),
}

#[cfg(feature = "random")]
impl rand::TryRng for InterpreterRng {
    type Error = std::convert::Infallible;

    fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
        match self {
            InterpreterRng::Fast(rng) => rng.try_next_u32(),
            InterpreterRng::Deterministic(rng) => rng.try_next_u32(),
        }
    }

    fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
        match self {
            InterpreterRng::Fast(rng) => rng.try_next_u64(),
            InterpreterRng::Deterministic(rng) => rng.try_next_u64(),
        }
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Self::Error> {
        match self {
            InterpreterRng::Fast(rng) => rng.try_fill_bytes(dest),
            InterpreterRng::Deterministic(rng) => rng.try_fill_bytes(dest),
        }
    }
}

/// The kind of RNG currently selected, for reporting via `RNGkind()`.
#[cfg(feature = "random")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RngKind {
    Xoshiro,
    ChaCha20,
}

#[cfg(feature = "random")]
impl std::fmt::Display for RngKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RngKind::Xoshiro => write!(f, "Xoshiro"),
            RngKind::ChaCha20 => write!(f, "ChaCha20"),
        }
    }
}

// endregion

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

/// Semantic styles for colored diagnostic output.
///
/// Used by `Interpreter::write_stderr_colored` to select the appropriate color
/// when the `color` feature is enabled and stderr is connected to a terminal.
/// When color is unavailable or disabled, the text is written uncolored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticStyle {
    /// Error messages — displayed in bold red.
    Error,
    /// Warning messages — displayed in bold yellow.
    Warning,
    /// Informational messages — displayed in cyan.
    Message,
}

pub struct Interpreter {
    pub global_env: Environment,
    /// Per-interpreter stdout writer. Defaults to `std::io::stdout()`.
    /// Use `Vec<u8>` for captured/embedded output.
    pub(crate) stdout: RefCell<Box<dyn Write>>,
    /// Per-interpreter stderr writer. Defaults to `std::io::stderr()`.
    pub(crate) stderr: RefCell<Box<dyn Write>>,
    /// Whether to emit colored diagnostics to stderr. True when stderr is a
    /// real terminal and the `color` feature is enabled; false for captured
    /// output sessions or when the feature is off.
    color_stderr: bool,
    s3_dispatch_stack: RefCell<Vec<S3DispatchContext>>,
    call_stack: RefCell<Vec<CallFrame>>,
    /// Stack of handler sets from withCallingHandlers() calls.
    pub(crate) condition_handlers: RefCell<Vec<Vec<ConditionHandler>>>,
    /// Per-interpreter RNG state. Defaults to `SmallRng` (Xoshiro256++) for
    /// speed; can be switched to `ChaCha20Rng` via `RNGkind("ChaCha20")` for
    /// cross-platform deterministic reproducibility.
    ///
    /// # Parallel RNG considerations
    ///
    /// The RNG is behind `RefCell` on the single-threaded `Interpreter`, so there
    /// are no data races. If we ever add rayon-based parallel operations, each
    /// worker thread must get its own RNG seeded deterministically from the parent
    /// to avoid contention and ensure reproducibility.
    #[cfg(feature = "random")]
    rng: RefCell<InterpreterRng>,
    /// Which RNG algorithm is currently selected.
    #[cfg(feature = "random")]
    pub(crate) rng_kind: std::cell::Cell<RngKind>,
    /// Session-scoped temporary directory, auto-cleaned on drop.
    pub(crate) temp_dir: temp_dir::TempDir,
    /// Counter for unique tempfile names within the session.
    pub(crate) temp_counter: std::cell::Cell<u64>,
    /// Per-interpreter environment variables, snapshotted at interpreter creation
    /// and then mutated locally via Sys.setenv().
    pub(crate) env_vars: RefCell<std::collections::HashMap<String, String>>,
    /// Per-interpreter working directory, snapshotted at interpreter creation
    /// and then mutated locally via setwd().
    pub(crate) working_dir: RefCell<std::path::PathBuf>,
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
    /// Buffered response bodies for URL connections, keyed by connection ID.
    /// URL connections eagerly fetch the entire HTTP response body, which is
    /// stored here for subsequent `readLines()` calls.
    #[cfg(feature = "tls")]
    pub(crate) url_bodies: RefCell<std::collections::HashMap<usize, Vec<u8>>>,
    /// Finalizers registered with reg.finalizer(onexit = TRUE), run when the
    /// interpreter is dropped.
    pub(crate) finalizers: RefCell<Vec<RValue>>,
    /// Flag set by the SIGINT handler; checked at loop boundaries to interrupt
    /// long-running computations without killing the process.
    interrupted: Arc<AtomicBool>,
    /// Per-interpreter R options (accessed via `options()` and `getOption()`).
    pub(crate) options: RefCell<std::collections::HashMap<String, value::RValue>>,
    /// Rd documentation help index for package `man/` directories.
    pub(crate) rd_help_index: RefCell<packages::rd::RdHelpIndex>,
    /// Visibility flag — set to `true` by `invisible()` to suppress auto-printing
    pub(crate) last_value_invisible: std::cell::Cell<bool>,
    /// Recursion depth counter — prevents stack overflow on deeply nested expressions.
    eval_depth: std::cell::Cell<usize>,
    /// S4 class registry: class name -> class definition.
    pub(crate) s4_classes: RefCell<std::collections::HashMap<String, S4ClassDef>>,
    /// S4 generic registry: generic name -> generic definition.
    pub(crate) s4_generics: RefCell<std::collections::HashMap<String, S4GenericDef>>,
    /// S4 method dispatch table: (generic, signature) -> method function.
    pub(crate) s4_methods: RefCell<std::collections::HashMap<S4MethodKey, value::RValue>>,
    /// Loaded package namespaces, keyed by package name.
    pub(crate) loaded_namespaces:
        RefCell<std::collections::HashMap<String, packages::LoadedNamespace>>,
    /// Search path entries (between .GlobalEnv and package:base).
    pub(crate) search_path: RefCell<Vec<packages::SearchPathEntry>>,
    /// S3 method registry for methods declared via S3method() in NAMESPACE files.
    /// Key is (generic, class), value is the method function.
    /// Checked by dispatch_s3 after the environment chain lookup fails.
    pub(crate) s3_method_registry:
        RefCell<std::collections::HashMap<(String, String), value::RValue>>,
    /// Active progress bars, keyed by integer ID.
    #[cfg(feature = "progress")]
    pub(crate) progress_bars: RefCell<Vec<Option<builtins::progress::ProgressBarState>>>,
    /// Graphics device manager — tracks open devices and the active device.
    pub(crate) device_manager: RefCell<graphics::device_manager::DeviceManager>,
    /// Graphics parameters (par state) — per-interpreter, not global.
    pub(crate) par_state: RefCell<graphics::par::ParState>,
    /// Color palette for indexed color access (e.g. col=1 means palette[0]).
    pub(crate) color_palette: RefCell<Vec<graphics::color::RColor>>,
}

/// S4 class definition stored in the per-interpreter class registry.
#[derive(Debug, Clone)]
pub struct S4ClassDef {
    pub name: String,
    pub slots: Vec<(String, String)>,
    pub contains: Vec<String>,
    pub prototype: Vec<(String, value::RValue)>,
    pub is_virtual: bool,
    pub validity: Option<value::RValue>,
}

/// Key for S4 method dispatch table: (generic_name, signature).
pub(crate) type S4MethodKey = (String, Vec<String>);

/// S4 generic function definition.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct S4GenericDef {
    pub name: String,
    pub default: Option<value::RValue>,
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
        info!("creating new R interpreter");
        let base_env = Environment::new_global();
        base_env.set_name("base".to_string());
        builtins::register_builtins(&base_env);
        let global_env = Environment::new_child(&base_env);
        global_env.set_name("R_GlobalEnv".to_string());
        let color_stderr = {
            use std::io::IsTerminal;
            std::io::stderr().is_terminal()
        };
        let interp = Interpreter {
            global_env,
            stdout: RefCell::new(Box::new(std::io::stdout())),
            stderr: RefCell::new(Box::new(std::io::stderr())),
            color_stderr,
            s3_dispatch_stack: RefCell::new(Vec::new()),
            call_stack: RefCell::new(Vec::new()),
            condition_handlers: RefCell::new(Vec::new()),
            #[cfg(feature = "random")]
            rng: RefCell::new({
                use rand::SeedableRng;
                let mut thread_rng = rand::rng();
                InterpreterRng::Fast(rand::rngs::SmallRng::from_rng(&mut thread_rng))
            }),
            #[cfg(feature = "random")]
            rng_kind: std::cell::Cell::new(RngKind::Xoshiro),
            temp_dir: temp_dir::TempDir::new().expect("failed to create session temp directory"),
            temp_counter: std::cell::Cell::new(0),
            env_vars: RefCell::new(std::env::vars().collect()),
            working_dir: RefCell::new(
                std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
            ),
            start_instant: std::time::Instant::now(),
            #[cfg(feature = "collections")]
            collections: RefCell::new(Vec::new()),
            connections: RefCell::new(Vec::new()),
            tcp_streams: RefCell::new(std::collections::HashMap::new()),
            #[cfg(feature = "tls")]
            url_bodies: RefCell::new(std::collections::HashMap::new()),
            finalizers: RefCell::new(Vec::new()),
            interrupted: Arc::new(AtomicBool::new(false)),
            options: RefCell::new(Self::default_options()),
            rd_help_index: RefCell::new(packages::rd::RdHelpIndex::new()),
            last_value_invisible: std::cell::Cell::new(false),
            eval_depth: std::cell::Cell::new(0),
            s4_classes: RefCell::new(std::collections::HashMap::new()),
            s4_generics: RefCell::new(std::collections::HashMap::new()),
            s4_methods: RefCell::new(std::collections::HashMap::new()),
            loaded_namespaces: RefCell::new(std::collections::HashMap::new()),
            search_path: RefCell::new(Vec::new()),
            s3_method_registry: RefCell::new(std::collections::HashMap::new()),
            #[cfg(feature = "progress")]
            progress_bars: RefCell::new(Vec::new()),
            device_manager: RefCell::new(graphics::device_manager::DeviceManager::new()),
            par_state: RefCell::new(graphics::par::ParState::default()),
            color_palette: RefCell::new(graphics::color::default_palette()),
        };

        // Synthesize Rd help pages from builtin rustdoc comments so every
        // builtin has a rich help page via ?name from the start.
        builtins::synthesize_builtin_help(&mut interp.rd_help_index.borrow_mut());

        interp
    }

    /// Index all `.Rd` files in a package's `man/` directory for `help()` lookup.
    pub fn index_package_help(&self, package_name: &str, man_dir: &std::path::Path) {
        self.rd_help_index
            .borrow_mut()
            .index_package_dir(package_name, man_dir);
    }

    /// Mark the current result as invisible (suppresses auto-printing).
    pub(crate) fn set_invisible(&self) {
        self.last_value_invisible.set(true);
    }

    /// Check and consume the invisible flag. Returns `true` if the last
    /// value was marked invisible. The flag is reset after reading.
    pub(crate) fn take_invisible(&self) -> bool {
        self.last_value_invisible.replace(false)
    }

    /// Return a clone of the interrupt flag so the SIGINT handler can set it.
    pub fn interrupt_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.interrupted)
    }

    /// Check the interrupt flag; if set, clear it and return an interrupt error.
    pub(crate) fn check_interrupt(&self) -> Result<(), RFlow> {
        if self.interrupted.load(Ordering::Relaxed) {
            self.interrupted.store(false, Ordering::Relaxed);
            debug!("SIGINT interrupt detected");
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
    pub(crate) fn rng(&self) -> &RefCell<InterpreterRng> {
        &self.rng
    }

    /// Get an environment variable from the interpreter-local snapshot.
    pub(crate) fn get_env_var(&self, name: &str) -> Option<String> {
        self.env_vars.borrow().get(name).cloned()
    }

    /// Return a clone of the interpreter-local environment snapshot.
    pub(crate) fn env_vars_snapshot(&self) -> std::collections::HashMap<String, String> {
        self.env_vars.borrow().clone()
    }

    /// Set a per-interpreter environment variable (does not mutate process state).
    pub(crate) fn set_env_var(&self, name: String, value: String) {
        self.env_vars.borrow_mut().insert(name, value);
    }

    /// Remove a per-interpreter environment variable (does not mutate process state).
    pub(crate) fn remove_env_var(&self, name: &str) {
        self.env_vars.borrow_mut().remove(name);
    }

    /// Register an S3 method in the per-interpreter registry.
    /// This is used by NAMESPACE S3method() directives to make methods
    /// discoverable by S3 dispatch without polluting the base environment.
    pub(crate) fn register_s3_method(&self, generic: String, class: String, method: RValue) {
        self.s3_method_registry
            .borrow_mut()
            .insert((generic, class), method);
    }

    /// Look up an S3 method from the per-interpreter registry.
    /// Returns the method function if one was registered for the given
    /// generic/class combination.
    pub(crate) fn lookup_s3_method(&self, generic: &str, class: &str) -> Option<RValue> {
        self.s3_method_registry
            .borrow()
            .get(&(generic.to_string(), class.to_string()))
            .cloned()
    }

    /// Get the interpreter-local working directory.
    pub(crate) fn get_working_dir(&self) -> std::path::PathBuf {
        self.working_dir.borrow().clone()
    }

    /// Set the per-interpreter working directory (does not mutate process state).
    pub(crate) fn set_working_dir(&self, path: std::path::PathBuf) {
        *self.working_dir.borrow_mut() = path;
    }

    /// Resolve a user-supplied path against the interpreter-local working directory.
    pub(crate) fn resolve_path(&self, path: impl AsRef<std::path::Path>) -> std::path::PathBuf {
        let path = path.as_ref();
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.get_working_dir().join(path)
        }
    }

    /// Write a message to the interpreter's stdout writer.
    pub(crate) fn write_stdout(&self, msg: &str) {
        let _ = self.stdout.borrow_mut().write_all(msg.as_bytes());
    }

    /// Write a message to the interpreter's stderr writer.
    pub(crate) fn write_stderr(&self, msg: &str) {
        let _ = self.stderr.borrow_mut().write_all(msg.as_bytes());
    }

    /// Whether colored stderr output is enabled for this interpreter.
    pub fn color_stderr(&self) -> bool {
        self.color_stderr
    }

    /// Enable or disable colored stderr output.
    pub fn set_color_stderr(&mut self, enabled: bool) {
        self.color_stderr = enabled;
    }

    /// Write a colored diagnostic message to the interpreter's stderr writer.
    ///
    /// When the `color` feature is enabled and `color_stderr` is true, the
    /// message is written with the appropriate ANSI color. Otherwise, the
    /// message is written as plain text.
    pub(crate) fn write_stderr_colored(&self, msg: &str, style: DiagnosticStyle) {
        if self.try_write_colored(msg, style) {
            return;
        }
        // Fallback: no color
        self.write_stderr(msg);
    }

    /// Attempt to write colored text. Returns `true` if color was applied,
    /// `false` if the caller should fall back to plain text.
    #[cfg(feature = "color")]
    fn try_write_colored(&self, msg: &str, style: DiagnosticStyle) -> bool {
        if !self.color_stderr {
            return false;
        }
        use std::io::Write as _;
        use termcolor::{Color, ColorSpec, WriteColor};

        // Write colored text to a termcolor buffer, then copy the bytes
        // into the interpreter's stderr writer.
        let bufwtr = termcolor::BufferWriter::stderr(termcolor::ColorChoice::Auto);
        let mut buf = bufwtr.buffer();
        let color_spec = match style {
            DiagnosticStyle::Error => {
                let mut s = ColorSpec::new();
                s.set_fg(Some(Color::Red)).set_bold(true);
                s
            }
            DiagnosticStyle::Warning => {
                let mut s = ColorSpec::new();
                s.set_fg(Some(Color::Yellow)).set_bold(true);
                s
            }
            DiagnosticStyle::Message => {
                let mut s = ColorSpec::new();
                s.set_fg(Some(Color::Cyan));
                s
            }
        };
        if buf.set_color(&color_spec).is_err() {
            return false;
        }
        let _ = buf.write_all(msg.as_bytes());
        let _ = buf.reset();

        // Copy the buffer contents (with ANSI escapes) into the
        // interpreter's session-scoped stderr writer.
        let bytes = buf.as_slice();
        let _ = self.stderr.borrow_mut().write_all(bytes);
        true
    }

    /// Stub when the `color` feature is not enabled.
    #[cfg(not(feature = "color"))]
    fn try_write_colored(&self, _msg: &str, _style: DiagnosticStyle) -> bool {
        false
    }

    pub fn eval(&self, expr: &Expr) -> Result<RValue, RFlow> {
        self.eval_in(expr, &self.global_env)
    }

    /// Maximum evaluation recursion depth before returning an error.
    const MAX_EVAL_DEPTH: usize = 256;

    #[tracing::instrument(level = "trace", skip(self, env))]
    pub fn eval_in(&self, expr: &Expr, env: &Environment) -> Result<RValue, RFlow> {
        let depth = self.eval_depth.get();
        if depth >= Self::MAX_EVAL_DEPTH {
            return Err(RFlow::Error(value::RError::other(
                "evaluation nested too deeply: infinite recursion / options(expressions=) ?"
                    .to_string(),
            )));
        }
        self.eval_depth.set(depth + 1);
        let result = self.eval_in_inner(expr, env);
        self.eval_depth.set(depth);
        result
    }

    fn eval_in_inner(&self, expr: &Expr, env: &Environment) -> Result<RValue, RFlow> {
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
            Expr::Symbol(name) => {
                // Check for active bindings first — these re-evaluate a function on every access
                if let Some(fun) = env.get_active_binding(name) {
                    return self.call_function(&fun, &[], &[], env);
                }
                env.get(name).ok_or_else(|| {
                    debug!(symbol = name.as_str(), "symbol not found");
                    RError::new(RErrorKind::Name, name.clone()).into()
                })
            }
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
