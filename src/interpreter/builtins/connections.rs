//! Connection object builtins — file connections, stdin/stdout/stderr.
//!
//! R connections wrap file handles (or other I/O sources) behind integer IDs.
//! This module provides a minimal implementation: `file()`, `open()`, `close()`,
//! `isOpen()`, `readLines()`/`writeLines()` connection dispatch, and the three
//! standard stream constructors `stdin()`, `stdout()`, `stderr()`.
//!
//! Connection state lives on the `Interpreter` struct as a `Vec<ConnectionInfo>`.
//! Slots 0-2 are pre-allocated for stdin, stdout, and stderr.

use super::CallArgs;
use crate::interpreter::value::*;
use crate::interpreter::BuiltinContext;
use crate::interpreter::Interpreter;
use itertools::Itertools;
use minir_macros::interpreter_builtin;

// region: ConnectionInfo

/// Describes a single connection slot in the interpreter.
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    /// File path (empty for stdin/stdout/stderr).
    pub path: String,
    /// Open mode string (e.g. "r", "w", "rt", "wt"). Empty means not yet opened.
    pub mode: String,
    /// Whether the connection is currently open.
    pub is_open: bool,
    /// Human-readable description (e.g. "stdin", "stdout", or the file path).
    pub description: String,
}

impl ConnectionInfo {
    /// Create a new connection with the given path and description, initially closed.
    fn new(path: String, description: String) -> Self {
        Self {
            path,
            mode: String::new(),
            is_open: false,
            description,
        }
    }

    /// Create a standard stream connection (pre-opened).
    fn std_stream(description: &str) -> Self {
        Self {
            path: String::new(),
            mode: String::new(),
            is_open: true,
            description: description.to_string(),
        }
    }
}

// endregion

// region: Interpreter connection helpers

impl Interpreter {
    /// Ensure the connections table is initialised with the three standard streams.
    /// Called lazily on first access.
    pub(crate) fn ensure_connections(&self) {
        let mut conns = self.connections.borrow_mut();
        if conns.is_empty() {
            conns.push(ConnectionInfo::std_stream("stdin"));
            conns.push(ConnectionInfo::std_stream("stdout"));
            conns.push(ConnectionInfo::std_stream("stderr"));
        }
    }

    /// Allocate a new connection slot, returning its integer ID.
    pub(crate) fn add_connection(&self, info: ConnectionInfo) -> usize {
        self.ensure_connections();
        let mut conns = self.connections.borrow_mut();
        let id = conns.len();
        conns.push(info);
        id
    }

    /// Get a clone of the connection info at `id`, or None if out of range.
    pub(crate) fn get_connection(&self, id: usize) -> Option<ConnectionInfo> {
        self.ensure_connections();
        self.connections.borrow().get(id).cloned()
    }

    /// Mutate a connection in place. Returns an error if the ID is invalid.
    pub(crate) fn with_connection_mut<F>(&self, id: usize, f: F) -> Result<(), RError>
    where
        F: FnOnce(&mut ConnectionInfo),
    {
        self.ensure_connections();
        let mut conns = self.connections.borrow_mut();
        let conn = conns.get_mut(id).ok_or_else(|| {
            RError::new(RErrorKind::Argument, format!("invalid connection id {id}"))
        })?;
        f(conn);
        Ok(())
    }
}

// endregion

// region: Helper — build a connection RValue

/// Build an integer scalar with class `"connection"` representing connection `id`.
fn connection_value(id: usize) -> RValue {
    let mut rv = RVector::from(Vector::Integer(
        vec![Some(i64::try_from(id).unwrap_or(0))].into(),
    ));
    rv.set_attr(
        "class".to_string(),
        RValue::vec(Vector::Character(
            vec![Some("connection".to_string())].into(),
        )),
    );
    RValue::Vector(rv)
}

/// Extract a connection ID from an argument that is either an integer (possibly
/// with class "connection") or a double that can be losslessly converted.
fn connection_id(val: &RValue) -> Option<usize> {
    val.as_vector()
        .and_then(|v| v.as_integer_scalar())
        .and_then(|i| usize::try_from(i).ok())
}

/// Returns `true` if `val` carries the `"connection"` class attribute.
fn is_connection(val: &RValue) -> bool {
    match val {
        RValue::Vector(rv) => rv
            .class()
            .map(|cls| cls.iter().any(|c| c == "connection"))
            .unwrap_or(false),
        _ => false,
    }
}

// endregion

// region: Builtins

/// Create a file connection.
///
/// Returns an integer connection ID with class "connection". The connection
/// is not opened unless `open` is non-empty.
///
/// @param description character scalar: file path
/// @param open character scalar: open mode ("" means unopened, "r", "w", etc.)
/// @return integer scalar with class "connection"
#[interpreter_builtin(name = "file", min_args = 1)]
fn interp_file(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let path = call_args.string("description", 0)?;
    let open_mode = call_args.optional_string("open", 1).unwrap_or_default();

    let interp = context.interpreter();
    let mut info = ConnectionInfo::new(path.clone(), path);
    if !open_mode.is_empty() {
        info.mode = open_mode;
        info.is_open = true;
    }
    let id = interp.add_connection(info);
    Ok(connection_value(id))
}

/// Open a connection.
///
/// If the connection is already open this is a no-op. Otherwise the mode is
/// recorded and the connection is marked open.
///
/// @param con integer scalar: connection ID
/// @param open character scalar: open mode (default "r")
/// @return the connection (invisibly)
#[interpreter_builtin(name = "open", min_args = 1)]
fn interp_open(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let con_val = call_args.value("con", 0).ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "argument 'con' is missing".to_string(),
        )
    })?;
    let id = connection_id(con_val)
        .ok_or_else(|| RError::new(RErrorKind::Argument, "invalid connection".to_string()))?;
    let mode = call_args
        .optional_string("open", 1)
        .unwrap_or_else(|| "r".to_string());

    let interp = context.interpreter();
    interp.with_connection_mut(id, |conn| {
        if !conn.is_open {
            conn.mode = mode;
            conn.is_open = true;
        }
    })?;
    Ok(connection_value(id))
}

/// Close a connection.
///
/// Marks the connection as closed. Returns invisible NULL.
///
/// @param con integer scalar: connection ID
/// @return NULL (invisibly)
#[interpreter_builtin(name = "close", min_args = 1)]
fn interp_close(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let con_val = call_args.value("con", 0).ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "argument 'con' is missing".to_string(),
        )
    })?;
    let id = connection_id(con_val)
        .ok_or_else(|| RError::new(RErrorKind::Argument, "invalid connection".to_string()))?;

    let interp = context.interpreter();
    interp.with_connection_mut(id, |conn| {
        conn.is_open = false;
        conn.mode.clear();
    })?;
    Ok(RValue::Null)
}

/// Test whether a connection is open.
///
/// @param con integer scalar: connection ID
/// @return logical scalar: TRUE if open, FALSE if closed
#[interpreter_builtin(name = "isOpen", min_args = 1)]
fn interp_is_open(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let con_val = call_args.value("con", 0).ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "argument 'con' is missing".to_string(),
        )
    })?;
    let id = connection_id(con_val)
        .ok_or_else(|| RError::new(RErrorKind::Argument, "invalid connection".to_string()))?;

    let interp = context.interpreter();
    let conn = interp
        .get_connection(id)
        .ok_or_else(|| RError::new(RErrorKind::Argument, format!("invalid connection id {id}")))?;
    Ok(RValue::vec(Vector::Logical(
        vec![Some(conn.is_open)].into(),
    )))
}

/// Read text lines from a file path or a connection.
///
/// If `con` is a character string, reads directly from that file path.
/// If `con` is an integer with class "connection", reads from the
/// connection's stored file path.
///
/// @param con character scalar or connection integer: source to read from
/// @param n integer scalar: maximum number of lines to read (-1 for all)
/// @return character vector with one element per line
#[interpreter_builtin(name = "readLines", min_args = 1)]
fn interp_read_lines(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let con_val = call_args.value("con", 0).ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "argument 'con' is missing".to_string(),
        )
    })?;

    let n = call_args.integer_or("n", 1, -1);

    // Resolve the file path — either from a string argument or from a connection.
    let path = if is_connection(con_val) {
        let id = connection_id(con_val)
            .ok_or_else(|| RError::new(RErrorKind::Argument, "invalid connection".to_string()))?;
        let interp = context.interpreter();
        let conn = interp.get_connection(id).ok_or_else(|| {
            RError::new(RErrorKind::Argument, format!("invalid connection id {id}"))
        })?;
        if conn.path.is_empty() {
            return Err(RError::new(
                RErrorKind::Argument,
                format!(
                    "cannot read from '{}' — standard stream connections are not supported for readLines",
                    conn.description
                ),
            ));
        }
        conn.path.clone()
    } else {
        call_args.string("con", 0)?
    };

    let content = std::fs::read_to_string(&path).map_err(|e| {
        RError::new(
            RErrorKind::Other,
            format!("cannot open file '{}': {}", path, e),
        )
    })?;

    let lines: Vec<Option<String>> = if n < 0 {
        content.lines().map(|l| Some(l.to_string())).collect()
    } else {
        content
            .lines()
            .take(usize::try_from(n).unwrap_or(usize::MAX))
            .map(|l| Some(l.to_string()))
            .collect()
    };
    Ok(RValue::vec(Vector::Character(lines.into())))
}

/// Write text lines to a file path, connection, or stdout.
///
/// If `con` is a character string, writes to that file path.
/// If `con` is an integer with class "connection", writes to the
/// connection's stored file path (or stdout for connection 1).
/// If `con` is omitted, writes to stdout.
///
/// @param text character vector of lines to write
/// @param con character scalar or connection integer: destination
/// @param sep character scalar: line separator (default "\n")
/// @return NULL (invisibly)
#[interpreter_builtin(name = "writeLines", min_args = 1)]
fn interp_write_lines(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let text = args
        .first()
        .and_then(|v| v.as_vector())
        .map(|v| v.to_characters())
        .unwrap_or_default();
    let sep = call_args
        .named_string("sep")
        .unwrap_or_else(|| "\n".to_string());

    let output: String = text
        .iter()
        .map(|s| s.clone().unwrap_or_else(|| "NA".to_string()))
        .join(&sep);

    // Determine destination from `con` argument (position 1).
    let con_val = call_args.value("con", 1);

    enum Dest {
        Stdout,
        File(String),
    }

    let dest = match con_val {
        Some(val) if is_connection(val) => {
            let id = connection_id(val).ok_or_else(|| {
                RError::new(RErrorKind::Argument, "invalid connection".to_string())
            })?;
            let interp = context.interpreter();
            let conn = interp.get_connection(id).ok_or_else(|| {
                RError::new(RErrorKind::Argument, format!("invalid connection id {id}"))
            })?;
            if conn.path.is_empty() {
                // Standard stream — connection 1 is stdout, others not supported for writing
                if id == 1 {
                    Dest::Stdout
                } else {
                    return Err(RError::new(
                        RErrorKind::Argument,
                        format!(
                            "cannot write to '{}' — only stdout() is supported for writeLines",
                            conn.description
                        ),
                    ));
                }
            } else {
                Dest::File(conn.path.clone())
            }
        }
        Some(val) => {
            let path = val
                .as_vector()
                .and_then(|v| v.as_character_scalar())
                .ok_or_else(|| {
                    RError::new(RErrorKind::Argument, "invalid 'con' argument".to_string())
                })?;
            Dest::File(path)
        }
        None => Dest::Stdout,
    };

    match dest {
        Dest::Stdout => {
            println!("{}", output);
        }
        Dest::File(path) => {
            std::fs::write(&path, format!("{}{}", output, sep)).map_err(|e| {
                RError::new(
                    RErrorKind::Other,
                    format!("cannot write to file '{}': {}", path, e),
                )
            })?;
        }
    }
    Ok(RValue::Null)
}

/// Return connection 0 (standard input).
///
/// @return integer scalar with class "connection" (value 0)
#[interpreter_builtin(name = "stdin", min_args = 0, max_args = 0)]
fn interp_stdin(
    _args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    context.interpreter().ensure_connections();
    Ok(connection_value(0))
}

/// Return connection 1 (standard output).
///
/// @return integer scalar with class "connection" (value 1)
#[interpreter_builtin(name = "stdout", min_args = 0, max_args = 0)]
fn interp_stdout(
    _args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    context.interpreter().ensure_connections();
    Ok(connection_value(1))
}

/// Return connection 2 (standard error).
///
/// @return integer scalar with class "connection" (value 2)
#[interpreter_builtin(name = "stderr", min_args = 0, max_args = 0)]
fn interp_stderr(
    _args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    context.interpreter().ensure_connections();
    Ok(connection_value(2))
}

// endregion
