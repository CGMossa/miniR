# Networking Plan

R's networking is used by Shiny, plumber, httr2, curl, and 163 packages in the
corpus that reference connections. `std::net` gives us TCP/UDP for free.

## What R has

- `socketConnection(host, port, server, open, blocking)` — create TCP socket
- `make.socket(host, port, server)` — simpler TCP socket
- `read.socket(socket, maxlen)` — read bytes from socket
- `write.socket(socket, string)` — write string to socket
- `close.socket(socket)` — close socket
- `url(description, open)` — URL connection (HTTP/HTTPS)
- `download.file(url, destfile, method)` — download a file
- `readLines(con)` — already works on file connections, needs socket support

## Implementation using std::net

### TCP Client

```rust
// make.socket("httpbin.org", 80) → TcpStream
let stream = TcpStream::connect((host, port))?;
// Store in connection table like file connections
```

### TCP Server

```rust
// socketConnection(port=8080, server=TRUE) → TcpListener
let listener = TcpListener::bind(("0.0.0.0", port))?;
// accept() returns TcpStream for each client
```

### UDP

```rust
// Not commonly used in R, but easy to add
let socket = UdpSocket::bind("0.0.0.0:0")?;
socket.send_to(data, addr)?;
```

## Phases

### Phase 1: TCP client sockets

- `make.socket(host, port)` — connect to a TCP server
- `read.socket(socket, maxlen)` — read up to maxlen bytes as character
- `write.socket(socket, string)` — write string to socket
- `close.socket(socket)` — close the connection
- Store socket state in the existing connection table on Interpreter
- Add `ConnectionObject::TcpStream(TcpStream)` variant

### Phase 2: TCP server sockets

- `socketConnection(port, server=TRUE)` — bind and listen
- Accept connections via the connection read interface
- `socketAccept(socket)` — accept one incoming connection, return new socket
- Non-blocking accept via `TcpListener::set_nonblocking(true)`

### Phase 3: Integration with connections

- Make `readLines()` and `writeLines()` work on socket connections
- Make `readBin()` / `writeBin()` work on sockets for binary data
- `isOpen()`, `close()` work on socket connections

### Phase 4: HTTP client (basic) -- DONE

- `url(description)` — open HTTP/HTTPS connection
- Parse URL, connect via TCP, send HTTP/1.1 request
- Read response headers and body
- `download.file(url, destfile)` — GET + write to file
- Implemented in `src/interpreter/builtins/net.rs`

### Phase 5: HTTPS and TLS -- DONE

- Using `rustls` (pure Rust TLS) + `rustls-native-certs` (system certs) + `webpki-roots` (Mozilla roots fallback)
- `url("https://...")` works — eagerly fetches response, supports `readLines()`
- `download.file()` handles both HTTP and HTTPS
- Feature-gated behind `tls` feature (NOT in default — heavy dep)

## Architecture

Socket connections reuse the existing `ConnectionInfo` / connection table
pattern from `connections.rs`. Add new variants:

```rust
pub enum ConnectionObject {
    File { path: String, mode: String },
    TcpClient(std::net::TcpStream),
    TcpServer(std::net::TcpListener),
    // future: UdpSocket, TlsStream
}
```

All socket state is per-interpreter (no global sockets).

## Dependencies

- Phase 1-3: `std::net` only — no new crates needed
- Phase 4-5: `rustls` 0.23 + `rustls-native-certs` 0.8 + `webpki-roots` 0.26 (16 transitive deps total)

## First deliverable

`make.socket("example.com", 80)` + `write.socket` + `read.socket` works —
enough to do raw HTTP requests. This unblocks basic web scraping and API
access patterns used by httr2 and curl packages.
