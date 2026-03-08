# lexopt integration plan

> `lexopt` 0.3 — Minimal CLI argument parser.
> https://github.com/blyber/lexopt

## What it does

Zero-dependency argument parser. Iterates over arguments as `Short('v')`,
`Long("output")`, `Value(OsString)`. No derive macros, no help generation,
no magic — just raw argument parsing.

```rust
let mut parser = lexopt::Parser::from_env();
while let Some(arg) = parser.next()? {
    match arg {
        Short('v') | Long("verbose") => verbose = true,
        Short('o') | Long("output") => output = parser.value()?,
        Value(path) => files.push(path),
        _ => return Err(arg.unexpected()),
    }
}
```

## Where it fits in newr

### 1. CLI argument parsing — already the right fit

newr's CLI needs:
- `newr script.R` — run a file
- `newr -e "expr"` — evaluate expression
- `newr --vanilla` / `--no-init` — skip startup files
- `newr --args ...` — pass args to R script (→ `commandArgs()`)
- `newr --interactive` / `-i` — force interactive mode

lexopt is perfect for this: lightweight, no proc-macros, handles `--args` passthrough
cleanly with `parser.raw_args()`.

### 2. `commandArgs()` builtin

R's `commandArgs(trailingOnly=TRUE)` returns arguments after `--args`. lexopt's
`raw_args()` method captures remaining args after a `--` or `--args` separator.

### 3. `Rscript` compatibility flags

Some flags for compatibility: `--default-packages=`, `--max-ppsize=`, `--encoding=`.
lexopt handles `Long("encoding")` → `parser.value()?` cleanly.

## Relationship to builtins plan

| Phase | Builtins affected | Impact |
|---|---|---|
| Phase 6 (OS) | `commandArgs()` | CLI arg passthrough |
| Core | startup behavior | `--vanilla`, `--no-init` flags |

## Recommendation

**Add when we formalize the CLI.** Currently `src/main.rs` may use manual arg
parsing. lexopt is the right choice — tiny, no deps, fits the minimalist style.

**Effort:** 30 minutes to replace current arg parsing.
