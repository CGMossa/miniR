# sha2 / digest integration plan

> `sha2` 0.10 + `digest` 0.10 — Cryptographic hash functions.
> Already vendored as transitive dependencies of reedline.

## What it does

SHA-224, SHA-256, SHA-384, SHA-512 hash functions. The `digest` crate provides the `Digest` trait that sha2 implements.

## Where it fits in miniR

### R functions

| R function | What it does |
| ---------- | ------------ |
| `tools::md5sum(files)` | MD5 checksum of files (would need md5 crate, not sha2) |
| `digest::digest(x, algo)` | Hash any R object (popular package, 10M+ downloads) |
| `digest::sha256(x)` | SHA-256 of a string |
| `rlang::hash(x)` | Object hashing for caching |

### Integration points

```rust
use sha2::{Sha256, Digest};

fn r_sha256(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    format!("{:x}", hasher.finalize())
}
```

### Builtins to add

- `sha256(x)` — SHA-256 hash of character vector (non-standard but useful)
- `digest(x, algo="sha256")` — Hash R object as serialized bytes

## Priority

Low — this is a nice-to-have utility, not a core R feature. Implement when we need object hashing for caching or environments.
