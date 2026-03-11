# openssl integration plan

> `openssl` +vendored — OpenSSL bindings for Rust.
> <https://github.com/sfackler/rust-openssl>

## What it does

Rust bindings to OpenSSL. With `+vendored` feature, statically links a bundled
OpenSSL, avoiding system dependency issues. Provides TLS, hashing, encryption,
X.509 certificates.

## Where it fits in miniR

### 1. HTTPS connections

R's `download.file()`, `url()`, `readLines(url)` need TLS for HTTPS URLs.
OpenSSL provides the TLS backend. Often used via higher-level HTTP crates
(reqwest, ureq) that use openssl or rustls as their TLS provider.

### 2. `digest::digest()` — cryptographic hashing

R's `digest` package provides MD5, SHA-1, SHA-256 etc. OpenSSL includes all of
these, though `sha2` / `md5` crates are lighter alternatives.

### 3. Not directly exposed to R

OpenSSL is infrastructure — R code doesn't call OpenSSL directly. It enables
other features (HTTPS, hashing) that R builtins need.

## Relationship to builtins plan

| Phase | Builtins affected | Impact |
|---|---|---|
| Phase 11 (I/O) | `download.file()`, `url()` | HTTPS support |
| Phase 12 (hash) | `digest()` equivalents | cryptographic hashing |

## Recommendation

**Add when implementing HTTP/HTTPS.** The `+vendored` feature is important for
reproducible builds. Consider `rustls` as a lighter, pure-Rust alternative if
we don't need OpenSSL-specific features.

**Effort:** Build dependency only — transparent to code.
