//! Connection object builtins — file connections, stdin/stdout/stderr, TCP sockets.
//!
//! R connections wrap file handles (or other I/O sources) behind integer IDs.
//! This module provides: `file()`, `open()`, `close()`, `isOpen()`,
//! `readLines()`/`writeLines()` connection dispatch, the three standard stream
//! constructors `stdin()`, `stdout()`, `stderr()`, and TCP client socket
//! builtins: `make.socket()`, `read.socket()`, `write.socket()`, `close.socket()`.
//!
//! Connection metadata lives on the `Interpreter` struct as a `Vec<ConnectionInfo>`.
//! Slots 0-2 are pre-allocated for stdin, stdout, and stderr.
//!
//! TCP streams (`std::net::TcpStream`) are not `Clone`, so they are stored
//! separately in a `HashMap<usize, TcpStream>` keyed by connection ID.

use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream};

use super::CallArgs;
use crate::interpreter::value::*;
use crate::interpreter::BuiltinContext;
use crate::interpreter::Interpreter;
use itertools::Itertools;
use minir_macros::interpreter_builtin;

// region: ConnectionKind + ConnectionInfo

/// Discriminates what kind of I/O backing a connection has.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionKind {
    /// Standard stream (stdin/stdout/stderr).
    StdStream,
    /// File connection — path is stored in `ConnectionInfo::path`.
    File,
    /// TCP client socket — the actual `TcpStream` handle lives in
    /// `Interpreter::tcp_streams`, keyed by connection ID.
    TcpClient,
}

/// Describes a single connection slot in the interpreter.
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    /// File path (empty for stdin/stdout/stderr and TCP sockets).
    pub path: String,
    /// Open mode string (e.g. "r", "w", "rt", "wt"). Empty means not yet opened.
    pub mode: String,
    /// Whether the connection is currently open.
    pub is_open: bool,
    /// Human-readable description (e.g. "stdin", "stdout", the file path, or "host:port").
    pub description: String,
    /// What kind of I/O this connection wraps.
    pub kind: ConnectionKind,
}

impl ConnectionInfo {
    /// Create a new file connection with the given path and description, initially closed.
    fn new(path: String, description: String) -> Self {
        Self {
            path,
            mode: String::new(),
            is_open: false,
            description,
            kind: ConnectionKind::File,
        }
    }

    /// Create a standard stream connection (pre-opened).
    fn std_stream(description: &str) -> Self {
        Self {
            path: String::new(),
            mode: String::new(),
            is_open: true,
            description: description.to_string(),
            kind: ConnectionKind::StdStream,
        }
    }

    /// Create a TCP client socket connection (pre-opened).
    fn tcp_client(description: String) -> Self {
        Self {
            path: String::new(),
            mode: "a+".to_string(),
            is_open: true,
            description,
            kind: ConnectionKind::TcpClient,
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

    /// Store a TCP stream for the given connection ID.
    pub(crate) fn store_tcp_stream(&self, id: usize, stream: TcpStream) {
        self.tcp_streams.borrow_mut().insert(id, stream);
    }

    /// Remove and return a TCP stream for the given connection ID, if present.
    pub(crate) fn take_tcp_stream(&self, id: usize) -> Option<TcpStream> {
        self.tcp_streams.borrow_mut().remove(&id)
    }

    /// Execute a closure with mutable access to the TCP stream for `id`.
    /// Returns an error if no TCP stream exists for that connection.
    pub(crate) fn with_tcp_stream<F, T>(&self, id: usize, f: F) -> Result<T, RError>
    where
        F: FnOnce(&mut TcpStream) -> Result<T, RError>,
    {
        let mut streams = self.tcp_streams.borrow_mut();
        let stream = streams.get_mut(&id).ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                format!("connection {id} does not have an active TCP stream"),
            )
        })?;
        f(stream)
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

// region: File/stream builtins

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
/// Marks the connection as closed. For TCP socket connections, also shuts
/// down and removes the underlying stream. Returns invisible NULL.
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

    // Check if this is a TCP connection and clean up the stream.
    let kind = interp
        .get_connection(id)
        .map(|c| c.kind.clone())
        .unwrap_or(ConnectionKind::File);
    if kind == ConnectionKind::TcpClient {
        if let Some(stream) = interp.take_tcp_stream(id) {
            let _ = stream.shutdown(Shutdown::Both);
        }
    }

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
/// connection's stored file path or TCP socket.
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

    if is_connection(con_val) {
        let id = connection_id(con_val)
            .ok_or_else(|| RError::new(RErrorKind::Argument, "invalid connection".to_string()))?;
        let interp = context.interpreter();
        let conn = interp.get_connection(id).ok_or_else(|| {
            RError::new(RErrorKind::Argument, format!("invalid connection id {id}"))
        })?;

        match conn.kind {
            ConnectionKind::TcpClient => {
                // Read from TCP socket — read available data, split into lines.
                let data = interp.with_tcp_stream(id, |stream| {
                    let mut buf = vec![0u8; 65536];
                    let bytes_read = stream.read(&mut buf).map_err(|e| {
                        RError::new(
                            RErrorKind::Other,
                            format!("error reading from socket '{}': {}", conn.description, e),
                        )
                    })?;
                    Ok(String::from_utf8_lossy(&buf[..bytes_read]).into_owned())
                })?;

                let lines: Vec<Option<String>> = if n < 0 {
                    data.lines().map(|l| Some(l.to_string())).collect()
                } else {
                    data.lines()
                        .take(usize::try_from(n).unwrap_or(usize::MAX))
                        .map(|l| Some(l.to_string()))
                        .collect()
                };
                return Ok(RValue::vec(Vector::Character(lines.into())));
            }
            ConnectionKind::StdStream => {
                return Err(RError::new(
                    RErrorKind::Argument,
                    format!(
                        "cannot read from '{}' — standard stream connections are not supported for readLines",
                        conn.description
                    ),
                ));
            }
            ConnectionKind::File => {
                if conn.path.is_empty() {
                    return Err(RError::new(
                        RErrorKind::Argument,
                        "connection has no file path".to_string(),
                    ));
                }
                // Fall through to file reading below.
            }
        }
    }

    // File path reading — either from string argument or file connection.
    let path = if is_connection(con_val) {
        let id = connection_id(con_val).unwrap();
        let interp = context.interpreter();
        let conn = interp.get_connection(id).unwrap();
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
/// connection's stored file path, stdout for connection 1, or TCP socket.
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
        TcpSocket(usize),
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
            match conn.kind {
                ConnectionKind::TcpClient => Dest::TcpSocket(id),
                ConnectionKind::StdStream => {
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
                }
                ConnectionKind::File => {
                    if conn.path.is_empty() {
                        return Err(RError::new(
                            RErrorKind::Argument,
                            "connection has no file path".to_string(),
                        ));
                    }
                    Dest::File(conn.path.clone())
                }
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
        Dest::TcpSocket(id) => {
            let interp = context.interpreter();
            let payload = format!("{}{}", output, sep);
            interp.with_tcp_stream(id, |stream| {
                stream.write_all(payload.as_bytes()).map_err(|e| {
                    RError::new(RErrorKind::Other, format!("error writing to socket: {}", e))
                })
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

// region: TCP socket builtins

/// Create a TCP client socket connection.
///
/// Connects to the specified host and port via TCP. Only client mode
/// (`server = FALSE`) is currently supported. Returns a connection ID
/// with class "connection".
///
/// @param host character scalar: hostname or IP address to connect to
/// @param port integer scalar: port number
/// @param server logical scalar: whether to create a server socket (only FALSE supported)
/// @return integer scalar with class "connection"
#[interpreter_builtin(name = "make.socket", min_args = 2)]
fn interp_make_socket(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let host = call_args.string("host", 0)?;
    let port = call_args.integer_or("port", 1, -1);
    let server = call_args.logical_flag("server", 2, false);

    if server {
        return Err(RError::new(
            RErrorKind::Argument,
            "make.socket() with server = TRUE is not yet supported — \
             only client sockets are implemented. Use server = FALSE (the default)."
                .to_string(),
        ));
    }

    if !(0..=65535).contains(&port) {
        return Err(RError::new(
            RErrorKind::Argument,
            format!("invalid port number {port} — must be between 0 and 65535"),
        ));
    }
    let port_u16 = u16::try_from(port)
        .map_err(|_| RError::new(RErrorKind::Argument, format!("invalid port number {port}")))?;

    let stream = TcpStream::connect((host.as_str(), port_u16)).map_err(|e| {
        RError::new(
            RErrorKind::Other,
            format!(
                "cannot connect to {}:{} — {}. \
                 Check that the host is reachable and the port is open.",
                host, port_u16, e
            ),
        )
    })?;

    let interp = context.interpreter();
    let description = format!("{}:{}", host, port_u16);
    let info = ConnectionInfo::tcp_client(description);
    let id = interp.add_connection(info);
    interp.store_tcp_stream(id, stream);

    Ok(connection_value(id))
}

/// Read up to `maxlen` bytes from a TCP socket connection.
///
/// Returns the data as a character string. If no data is available, blocks
/// until data arrives or the connection is closed.
///
/// @param socket integer scalar: connection ID of a TCP socket
/// @param maxlen integer scalar: maximum number of bytes to read (default 256)
/// @return character scalar containing the data read
#[interpreter_builtin(name = "read.socket", min_args = 1)]
fn interp_read_socket(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let con_val = call_args.value("socket", 0).ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "argument 'socket' is missing".to_string(),
        )
    })?;
    let id = connection_id(con_val).ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "invalid socket connection".to_string(),
        )
    })?;
    let maxlen = call_args.integer_or("maxlen", 1, 256);

    let interp = context.interpreter();

    // Verify this is actually a TCP connection.
    let conn = interp
        .get_connection(id)
        .ok_or_else(|| RError::new(RErrorKind::Argument, format!("invalid connection id {id}")))?;
    if conn.kind != ConnectionKind::TcpClient {
        return Err(RError::new(
            RErrorKind::Argument,
            format!(
                "connection {} ('{}') is not a socket — read.socket() requires a TCP socket connection",
                id, conn.description
            ),
        ));
    }
    if !conn.is_open {
        return Err(RError::new(
            RErrorKind::Argument,
            format!(
                "socket connection {} ('{}') is not open",
                id, conn.description
            ),
        ));
    }

    let buf_size = usize::try_from(maxlen).unwrap_or(256);
    let data = interp.with_tcp_stream(id, |stream| {
        let mut buf = vec![0u8; buf_size];
        let bytes_read = stream.read(&mut buf).map_err(|e| {
            RError::new(
                RErrorKind::Other,
                format!("error reading from socket: {}", e),
            )
        })?;
        Ok(String::from_utf8_lossy(&buf[..bytes_read]).into_owned())
    })?;

    Ok(RValue::vec(Vector::Character(vec![Some(data)].into())))
}

/// Write a string to a TCP socket connection.
///
/// Writes all bytes of the string to the socket. Returns invisible NULL.
///
/// @param socket integer scalar: connection ID of a TCP socket
/// @param string character scalar: data to write
/// @return NULL (invisibly)
#[interpreter_builtin(name = "write.socket", min_args = 2)]
fn interp_write_socket(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let con_val = call_args.value("socket", 0).ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "argument 'socket' is missing".to_string(),
        )
    })?;
    let id = connection_id(con_val).ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "invalid socket connection".to_string(),
        )
    })?;
    let data = call_args.string("string", 1)?;

    let interp = context.interpreter();

    // Verify this is actually a TCP connection.
    let conn = interp
        .get_connection(id)
        .ok_or_else(|| RError::new(RErrorKind::Argument, format!("invalid connection id {id}")))?;
    if conn.kind != ConnectionKind::TcpClient {
        return Err(RError::new(
            RErrorKind::Argument,
            format!(
                "connection {} ('{}') is not a socket — write.socket() requires a TCP socket connection",
                id, conn.description
            ),
        ));
    }
    if !conn.is_open {
        return Err(RError::new(
            RErrorKind::Argument,
            format!(
                "socket connection {} ('{}') is not open",
                id, conn.description
            ),
        ));
    }

    interp.with_tcp_stream(id, |stream| {
        stream
            .write_all(data.as_bytes())
            .map_err(|e| RError::new(RErrorKind::Other, format!("error writing to socket: {}", e)))
    })?;

    Ok(RValue::Null)
}

/// Close a TCP socket connection.
///
/// Shuts down the TCP stream and removes it from the connection table.
/// Returns invisible NULL. This is the socket-specific close — the generic
/// `close()` also handles TCP sockets.
///
/// @param socket integer scalar: connection ID of a TCP socket
/// @return NULL (invisibly)
#[interpreter_builtin(name = "close.socket", min_args = 1, max_args = 1)]
fn interp_close_socket(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let con_val = call_args.value("socket", 0).ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "argument 'socket' is missing".to_string(),
        )
    })?;
    let id = connection_id(con_val).ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "invalid socket connection".to_string(),
        )
    })?;

    let interp = context.interpreter();

    // Verify this is actually a TCP connection.
    let conn = interp
        .get_connection(id)
        .ok_or_else(|| RError::new(RErrorKind::Argument, format!("invalid connection id {id}")))?;
    if conn.kind != ConnectionKind::TcpClient {
        return Err(RError::new(
            RErrorKind::Argument,
            format!(
                "connection {} ('{}') is not a socket — close.socket() requires a TCP socket connection",
                id, conn.description
            ),
        ));
    }

    // Shut down and remove the TCP stream.
    if let Some(stream) = interp.take_tcp_stream(id) {
        let _ = stream.shutdown(Shutdown::Both);
    }

    interp.with_connection_mut(id, |conn| {
        conn.is_open = false;
        conn.mode.clear();
    })?;

    Ok(RValue::Null)
}

// endregion
