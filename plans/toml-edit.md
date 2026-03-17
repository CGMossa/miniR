# toml_edit integration plan

> `toml_edit` — TOML parser that preserves formatting and comments.
> <https://github.com/toml-rs/toml/tree/main/crates/toml_edit>

## What it does

Full TOML parser and serializer. Unlike the simpler `toml` crate,
`toml_edit` preserves comments, whitespace, and formatting — useful
for reading and writing config files without destroying user formatting.

Key types: `DocumentMut` (parsed TOML doc), `Item` (value/table/array),
`Value` (scalar), `Table`, `Array`, `InlineTable`.

## Where it fits in miniR

### `read.toml(file)` — parse TOML file to R list

TOML maps naturally to R:
- TOML table → named list
- TOML array → vector (if homogeneous) or list (if mixed)
- TOML string → character
- TOML integer → integer
- TOML float → double
- TOML boolean → logical
- TOML datetime → POSIXct (if datetime feature enabled)
- TOML array of tables → data.frame (if all tables have same keys)

### `write.toml(x, file)` — write R list as TOML

Reverse mapping:
- Named list → TOML table
- Unnamed list → TOML array
- Character → TOML string
- Integer/double → TOML integer/float
- Logical → TOML boolean
- Data.frame → array of tables

### `toml::parse(text)` — parse TOML string (like fromJSON)

### DESCRIPTION/NAMESPACE parsing

R package DESCRIPTION files are similar to TOML (key: value format).
While not exactly TOML, having a TOML parser available is useful
infrastructure for the package-runtime plan.

## Implementation

1. Add `toml_edit = { version = "0.22", optional = true }` with feature `toml`
2. Create `src/interpreter/builtins/toml.rs`
3. Implement `read.toml(file)` / `write.toml(x, file)` / `toml::parse(text)`
4. Register module behind `#[cfg(feature = "toml")]`
5. Namespace: `"utils"` (analogous to read.csv/write.csv)

## Priority: Medium — useful for config file handling and R package
DESCRIPTION parsing. Also commonly used in Rust ecosystem projects.
