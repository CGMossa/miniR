# Rustdoc-to-Rd: auto-generate help pages from proc-macro metadata

## Problem

Every builtin has roxygen-style doc tags (`@param`, `@return`, `@namespace`)
extracted by the `#[builtin]` proc-macro into `BuiltinDescriptor.doc`. But
the help system has two tiers:

1. **Rd pages** (rich) — parsed from package `man/*.Rd` files, shown with
   full formatting (title, description, usage, arguments table, examples)
2. **Rustdoc strings** (basic) — raw `@param`/`@return` lines, shown as-is

Builtins only get the basic tier. There are 650+ builtins with no Rd pages.

## Solution

Generate `RdDoc` structs directly from the `BuiltinDescriptor` metadata at
interpreter startup, and register them in the `RdHelpIndex`. No `.Rd` files
on disk needed — the Rd docs are synthesized in memory.

## Design

### Option A: Build-time generation (in the proc-macro)

The `#[builtin]` macro already parses doc comments. It could emit a
`static RD_DOC: &str = r#"\name{sum}..."#` alongside each descriptor.

**Pros:** Zero runtime cost, docs available as string literals.
**Cons:** Proc-macros can't easily emit files, bloats binary with string data.

### Option B: Runtime synthesis (at interpreter init) — **recommended**

After `register_builtins()`, iterate `BUILTIN_REGISTRY` and synthesize
`RdDoc` for each builtin from its descriptor fields:

```rust
fn synthesize_rd_docs(index: &mut RdHelpIndex) {
    for desc in BUILTIN_REGISTRY {
        let doc = RdDoc {
            name: Some(desc.name.to_string()),
            aliases: desc.aliases.iter().map(|a| a.to_string()).collect(),
            title: extract_title(desc.doc),
            description: extract_description(desc.doc),
            usage: synthesize_usage(desc),
            arguments: extract_params(desc.doc),
            value: extract_return(desc.doc),
            examples: None, // no examples from rustdoc
            ..Default::default()
        };
        index.register("base", &doc);
    }
}
```

The doc string parsing is straightforward since the proc-macro already
enforces a consistent format:

```rust
/// Short title on the first line.
///
/// Longer description paragraph(s).
///
/// @param x first argument description
/// @param y second argument description
/// @return description of return value
/// @namespace stats
```

Maps to:

```
\name{functionName}
\alias{functionName}
\title{Short title on the first line.}
\description{Longer description paragraph(s).}
\usage{functionName(x, y)}
\arguments{
  \item{x}{first argument description}
  \item{y}{second argument description}
}
\value{description of return value}
```

### Usage synthesis

The `usage` section can be synthesized from the descriptor's param names
(already extracted by `extract_param_names_from_doc`) plus the function name:

```rust
fn synthesize_usage(desc: &BuiltinDescriptor) -> Option<String> {
    let params = extract_param_names_from_doc(desc.doc);
    if params.is_empty() {
        Some(format!("{}(...)", desc.name))
    } else {
        Some(format!("{}({})", desc.name, params.join(", ")))
    }
}
```

### Where to hook in

In `Interpreter::new()`, after `register_builtins(&base_env)`:

```rust
synthesize_rd_docs(&mut self.rd_help_index.borrow_mut());
```

This means `?sum` immediately shows a rich help page even without any
package loaded — the builtin's rustdoc becomes an Rd page.

### Fallback chain (updated)

1. **Package Rd pages** — from loaded packages' `man/` directories
2. **Synthesized Rd pages** — from builtin rustdoc (generated at init)
3. Never "No documentation" for a registered builtin

## Implementation steps

1. Add `fn synthesize_rd_from_descriptor(desc: &BuiltinDescriptor) -> RdDoc`
   in `src/interpreter/builtins.rs` (or a new `src/interpreter/help.rs`)
2. Add `fn synthesize_all_builtin_docs(index: &mut RdHelpIndex)` that
   iterates BUILTIN_REGISTRY
3. Call it in `Interpreter::new()` after `register_builtins()`
4. Remove the rustdoc-based fallback from `help()` — Rd index handles everything
5. Add `register()` method to `RdHelpIndex` for programmatic doc insertion

## Benefits

- Every builtin gets a proper help page with structured sections
- `?sum` shows the same rich format as `?pkg::function`
- Doc quality improves by improving rustdoc comments — no separate Rd files
- Package Rd pages still override synthesized ones (they're registered later)
- Tab completion could show titles from the Rd index

## Priority

Medium-high — improves the user experience significantly for all 650+ builtins.
The implementation is ~100 lines since the doc string format is already standardized.
