# System Dependencies Strategy

## Problem
CRAN packages with native code need system libraries: libcurl, OpenSSL,
libpng, zlib, libgit2, etc. These are available via Homebrew/apt but
requiring system installs is fragile.

## Approach: Rust -sys Crates

Rust's `-sys` crate ecosystem bundles C libraries and builds them from
source. By adding these as optional dependencies, CRAN packages can
link against them without system installs.

### Crates to integrate

| -sys crate | Provides | CRAN packages unblocked |
|-----------|----------|------------------------|
| `curl-sys` | libcurl | curl, covr, gert (via libgit2) |
| `openssl-sys` | OpenSSL/LibreSSL | openssl, gert |
| `libz-sys` | zlib | many (compression) |
| `libpng-sys` | libpng | png |
| `libgit2-sys` | libgit2 | gert |

### Fallback: pkg-config

The `pkg-config` crate (already vendored) can find system-installed
libraries as a fallback. In `compile.rs`, when PKG_LIBS has `-lcurl`:

1. Try `-sys` crate paths first (if feature enabled)
2. Fall back to `pkg-config::probe_library("libcurl")`
3. Last resort: pass `-lcurl` to linker and hope

### Implementation

Add features:
```toml
curl-sys = { version = "0.4", optional = true }
openssl-sys = { version = "0.9", optional = true }
libz-sys = { version = "1", optional = true }
```

In `compile.rs`, resolve system library flags:
```rust
fn resolve_system_lib(name: &str) -> Option<Vec<String>> {
    // Try -sys crate first
    #[cfg(feature = "curl-sys")]
    if name == "curl" {
        // curl-sys provides link paths via DEP_CURL_* env vars
    }
    // Fall back to pkg-config
    pkg_config::probe_library(name).ok().map(|lib| {
        lib.link_paths.iter().map(|p| format!("-L{}", p.display()))
            .chain(lib.libs.iter().map(|l| format!("-l{l}")))
            .collect()
    })
}
```

### vcpkg (Windows)

The `vcpkg` crate handles Windows package management. Lower priority
since miniR is primarily macOS/Linux for now.
