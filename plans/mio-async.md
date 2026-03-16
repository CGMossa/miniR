# mio async I/O plan

> Already vendored as transitive dep of crossterm/reedline.

## What it does

Low-level non-blocking I/O using OS-level event notification (epoll/kqueue/IOCP).
Foundation for building async networking without tokio's full runtime.

## Where it fits in miniR

### Non-blocking socket connections

Phase 2+ of the networking plan needs non-blocking accept/read for server
sockets. mio provides `Poll` + `Events` for multiplexing multiple sockets.

### Shiny/plumber support

These packages need an event loop that handles multiple client connections.
mio is the right level — lighter than tokio, heavier than raw std::net.

### Architecture

```rust
// Event loop on the Interpreter
let poll = mio::Poll::new()?;
let events = mio::Events::with_capacity(128);
poll.registry().register(&mut socket, token, Interest::READABLE)?;
poll.poll(&mut events, timeout)?;
```

## Priority: Medium — needed for server-side R (Shiny/plumber), but those
are complex packages that need many other features first.
