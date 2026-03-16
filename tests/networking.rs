use r::Session;

/// Test that make.socket with a bad host produces a proper R error.
#[test]
fn make_socket_bad_host_error() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        err <- tryCatch(
            make.socket("this.host.does.not.exist.example.invalid", 9999),
            error = function(e) conditionMessage(e)
        )
        stopifnot(is.character(err))
        stopifnot(grepl("cannot connect", err))
    "#,
    )
    .expect("tryCatch around make.socket with bad host should not fail");
}

/// Test that make.socket with server=TRUE produces a proper error (not yet implemented).
#[test]
fn make_socket_server_not_supported() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        err <- tryCatch(
            make.socket("localhost", 8080, server = TRUE),
            error = function(e) conditionMessage(e)
        )
        stopifnot(is.character(err))
        stopifnot(grepl("server = TRUE is not yet supported", err))
    "#,
    )
    .expect("make.socket server=TRUE should produce informative error");
}

/// Test that make.socket with an invalid port produces a proper error.
#[test]
fn make_socket_invalid_port() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        err <- tryCatch(
            make.socket("localhost", -1),
            error = function(e) conditionMessage(e)
        )
        stopifnot(is.character(err))
        stopifnot(grepl("invalid port", err))
    "#,
    )
    .expect("make.socket with negative port should error");
}

/// Test that read.socket on a non-socket connection produces a proper error.
#[test]
fn read_socket_on_file_connection_errors() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        con <- file(tempfile(), open = "w")
        err <- tryCatch(
            read.socket(con),
            error = function(e) conditionMessage(e)
        )
        close(con)
        stopifnot(is.character(err))
        stopifnot(grepl("not a socket", err))
    "#,
    )
    .expect("read.socket on file connection should error");
}

/// Test that write.socket on a non-socket connection produces a proper error.
#[test]
fn write_socket_on_file_connection_errors() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        con <- file(tempfile(), open = "w")
        err <- tryCatch(
            write.socket(con, "hello"),
            error = function(e) conditionMessage(e)
        )
        close(con)
        stopifnot(is.character(err))
        stopifnot(grepl("not a socket", err))
    "#,
    )
    .expect("write.socket on file connection should error");
}

/// Test that close.socket on a non-socket connection produces a proper error.
#[test]
fn close_socket_on_file_connection_errors() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        con <- file(tempfile(), open = "w")
        err <- tryCatch(
            close.socket(con),
            error = function(e) conditionMessage(e)
        )
        close(con)
        stopifnot(is.character(err))
        stopifnot(grepl("not a socket", err))
    "#,
    )
    .expect("close.socket on file connection should error");
}

/// TCP round-trip test using a local echo-like server.
///
/// Spins up a local TCP listener, connects via make.socket, writes data,
/// reads it back, and closes. This tests the full socket lifecycle without
/// depending on external network infrastructure.
#[test]
fn tcp_loopback_roundtrip() {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    // Bind to an ephemeral port on localhost.
    let listener = TcpListener::bind("127.0.0.1:0").expect("failed to bind listener");
    let port = listener.local_addr().unwrap().port();

    // Spawn a thread that accepts one connection, reads data, and echoes it back.
    let handle = std::thread::spawn(move || {
        let (mut stream, _addr) = listener.accept().expect("accept failed");
        let mut buf = [0u8; 1024];
        let n = stream.read(&mut buf).expect("read failed");
        stream.write_all(&buf[..n]).expect("write failed");
    });

    let mut s = Session::new();
    let result = s.eval_source(&format!(
        r#"
        sock <- make.socket("127.0.0.1", {port})
        write.socket(sock, "hello from R")
        response <- read.socket(sock, maxlen = 256)
        close.socket(sock)
        response
    "#
    ));

    handle.join().expect("echo server thread panicked");

    let output = result.expect("TCP round-trip eval failed");
    let val = output.value;
    let response = val
        .as_vector()
        .and_then(|v| v.as_character_scalar())
        .expect("response should be a character scalar");
    assert_eq!(response, "hello from R");
}

/// Test that close() (the generic close) also works on TCP socket connections.
#[test]
fn generic_close_on_tcp_socket() {
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").expect("failed to bind listener");
    let port = listener.local_addr().unwrap().port();

    let handle = std::thread::spawn(move || {
        let (_stream, _addr) = listener.accept().expect("accept failed");
        // Just accept and drop — the client will close.
    });

    let mut s = Session::new();
    s.eval_source(&format!(
        r#"
        sock <- make.socket("127.0.0.1", {port})
        stopifnot(isOpen(sock))
        close(sock)
        stopifnot(!isOpen(sock))
    "#
    ))
    .expect("generic close() on TCP socket should work");

    handle.join().expect("server thread panicked");
}

/// Test that writeLines works on a TCP socket connection.
#[test]
fn write_lines_to_tcp_socket() {
    use std::io::Read;
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").expect("failed to bind listener");
    let port = listener.local_addr().unwrap().port();

    let handle = std::thread::spawn(move || {
        let (mut stream, _addr) = listener.accept().expect("accept failed");
        let mut buf = String::new();
        stream.read_to_string(&mut buf).expect("read failed");
        buf
    });

    let mut s = Session::new();
    s.eval_source(&format!(
        r#"
        sock <- make.socket("127.0.0.1", {port})
        writeLines(c("line1", "line2"), sock)
        close.socket(sock)
    "#
    ))
    .expect("writeLines to TCP socket should work");

    let received = handle.join().expect("server thread panicked");
    assert_eq!(received, "line1\nline2\n");
}

/// Test that readLines works on a TCP socket connection.
#[test]
fn read_lines_from_tcp_socket() {
    use std::io::Write;
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").expect("failed to bind listener");
    let port = listener.local_addr().unwrap().port();

    let handle = std::thread::spawn(move || {
        let (mut stream, _addr) = listener.accept().expect("accept failed");
        stream
            .write_all(b"alpha\nbeta\ngamma\n")
            .expect("write failed");
        // Shut down the write side so the client's read returns.
        stream
            .shutdown(std::net::Shutdown::Write)
            .expect("shutdown failed");
    });

    let mut s = Session::new();
    let result = s.eval_source(&format!(
        r#"
        sock <- make.socket("127.0.0.1", {port})
        lines <- readLines(sock)
        close.socket(sock)
        lines
    "#
    ));

    handle.join().expect("server thread panicked");

    let output = result.expect("readLines from TCP socket failed");
    let val = output.value;
    let lines = val
        .as_vector()
        .expect("result should be a vector")
        .to_characters();
    let lines: Vec<&str> = lines.iter().filter_map(|s| s.as_deref()).collect();
    assert_eq!(lines, vec!["alpha", "beta", "gamma"]);
}
