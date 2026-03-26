# serde integration plan

> `serde` 1.0+derive — Serialization/deserialization framework.
> <https://github.com/serde-rs/serde>

## What it does

Derive-based serialization framework. `#[derive(Serialize, Deserialize)]` on
structs/enums to enable conversion to/from any format (JSON, TOML, YAML, etc.).

```rust
#[derive(Serialize, Deserialize)]
struct Config {
    width: usize,
    prompt: String,
}
```

The `+derive` feature enables the proc-macro derives.

## Where it fits in miniR

### 1. Configuration files

Deserialize miniR configuration from JSON/TOML:

```rust
#[derive(Deserialize)]
struct NewrConfig {
    max_print: usize,
    digits: usize,
    warn: i32,
    prompt: String,
}
```

### 2. RDS / saveRDS() — R serialization format

R's `saveRDS()` / `readRDS()` saves R objects to binary format. A custom serde
format could implement this, or we define our own binary format using serde.

### 3. Session save/restore

`.RData` files save the global environment. With serde, `RValue` can be serialized
to disk and restored.

### 4. Package metadata

If we implement a package system, package `DESCRIPTION` files can be parsed with
serde (via serde_yaml or custom format).

## Relationship to builtins plan

| Phase | Builtins affected | Impact |
|---|---|---|
| Phase 11 (I/O) | `saveRDS()`, `readRDS()`, `save()`, `load()` | R object serialization |
| Core | configuration, `.RData` | session persistence |

## Recommendation

**Add when implementing saveRDS/readRDS or configuration files.** serde is the
standard Rust serialization framework — no alternatives worth considering.

**Effort:** 10 minutes to add, effort is in defining Serialize/Deserialize for RValue.
