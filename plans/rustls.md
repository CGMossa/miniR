# rustls integration plan

> `rustls` 0.23 -- Modern TLS library in pure Rust.
> <https://github.com/rustls/rustls>

## What it does

TLS 1.2/1.3 implementation in pure Rust. No dependency on OpenSSL or other
C libraries. Uses `ring` for cryptographic primitives.

Related vendored crates:
- `rustls-native-certs` 0.8 -- loads system TLS root certificates
- `webpki-roots` 0.26 -- bundled Mozilla root certificates as fallback
- `rustls-pki-types` -- PKI type definitions
- `rustls-webpki` -- WebPKI certificate validation
- `ring` 0.17 -- cryptographic primitives (AES, SHA, RSA, ECDSA)

## Where it fits in miniR

### HTTPS connections -- `url("https://...")`, `download.file()`

Already implemented in `src/interpreter/builtins/net.rs` behind the `tls`
feature gate. Provides:

```r
con <- url("https://httpbin.org/get")
lines <- readLines(con)
close(con)

download.file("https://example.com/data.csv", "data.csv")
```

### Future: `httr2`-style HTTP client

When implementing higher-level HTTP builtins (POST, headers, auth),
rustls provides the TLS layer underneath.

## Status

Already integrated behind `tls` feature (not in default features due to
heavy dependency footprint). See `plans/networking.md` for full details.

## Priority

Already done -- no further work needed for basic HTTPS. The `tls` feature
is opt-in and working.
