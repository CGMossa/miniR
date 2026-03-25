# Feature-Gate the IO Module

Make the `io` builtin module optional via a Cargo feature flag, enabled by default. This allows embedding miniR in sandboxed environments where filesystem access is prohibited.

## Motivation

For security-sensitive embeddings (WASM, sandboxed plugins, educational tools), the interpreter should be usable without any filesystem access. Feature-gating IO means:

- `miniR` with `default-features = false` gives a pure computation engine
- No `read.table`, `write.table`, `readLines`, `writeLines`, `scan`, `file.info`, `file.exists`, `file.remove`, `file.copy`, `dir.create`, `Sys.glob`, `tempfile`, `tempdir`, `system`, `system2`
- Math, string manipulation, data structures, apply-family, conditions all still work

## Implementation

1. Create a new Cargo feature `"io"` that is part of `default`:
   ```toml
   [features]
   default = ["random", "io"]
   io = ["dep:csv", "dep:glob", "dep:temp-dir"]
   random = ["dep:rand", "dep:rand_distr"]
   ```

2. Gate the IO module in `builtins.rs`:
   ```rust
   #[cfg(feature = "io")]
   pub mod io;
   ```

3. Gate the system module similarly (or split it — `Sys.time()` and `proc.time()` don't need filesystem, but `Sys.glob()` and `system()` do):
   ```rust
   #[cfg(feature = "io")]
   pub mod system;  // or split into system_time.rs (always) + system_fs.rs (gated)
   ```

4. Gate `csv`, `glob`, `temp-dir` dependencies as optional:
   ```toml
   csv = { version = "1.4", optional = true }
   glob = { version = "0.3", optional = true }
   temp-dir = { version = "0.1", optional = true }
   ```

5. Functions that are gated should produce clear errors when called without the feature:
   - Register stubs that return `RError::Other("read.table requires the 'io' feature — recompile with default features enabled")`
   - Or simply don't register them (they become "object not found" errors)

## Split candidates for system.rs

**Always available (no filesystem):**
- `Sys.time()`, `proc.time()`, `Sys.sleep()`
- `Sys.getenv()`, `Sys.setenv()`
- `.Platform`, `.Machine`

**IO-gated:**
- `Sys.glob()`, `system()`, `system2()`
- `file.info()`, `file.exists()`, `file.remove()`, `file.copy()`
- `dir.create()`, `dir.exists()`, `list.files()`, `list.dirs()`
- `getwd()`, `setwd()`
- `tempfile()`, `tempdir()`

## Testing

- Add a CI job that builds with `--no-default-features` to ensure the interpreter compiles without IO
- Add a test that runs basic R expressions without the IO feature
