# Rd Parser Plan

Plan for turning package `man/*.Rd` files into something miniR can use for `help()`, `?topic`, package help pages, and example extraction.

## Goal

Build a package-help pipeline that works on installed package docs without requiring a full GNU-R-compatible Rd renderer on day one.

## Current State

- miniR already parses `?topic` syntax and lowers it to `help("topic")`.
- `help()` currently only knows builtin docs and prints "No documentation" for package topics.
- The checked-in `cran/` tree contains 234 package directories and 10,738 `.Rd` files.
- Package help has to be wired into the package/runtime work: `DESCRIPTION`, `NAMESPACE`, package install layout, and asset staging.
- GNU R's Rd parser is substantial and macro-heavy; copying it directly is not an option for this codebase.

## Non-Goals

- Do not start by cloning GNU R's full Rd rendering stack.
- Do not block package execution on perfect documentation rendering.
- Do not treat Rd as plain markdown or free-form text; we need structural parsing, not regex scraping.

## Why This Matters

- `help()` and `?topic` are already part of the user language.
- Package installation is not complete if `man/` is ignored.
- Examples and aliases in Rd are useful for tests, help lookup, and package inspection.
- A metadata-first parser lets us get useful package docs much earlier than a full HTML/text/LaTeX converter.

## Recommended Architecture

### 1. Separate lexer/indexing from rendering

Treat Rd as a package asset format with three layers:

- tokenization / parsing
- metadata and alias indexing
- rendering or extraction for specific consumers

This keeps `help()` unblocked even if full rendering is incomplete.

### 2. Build a metadata-first parser first

The first parser only needs to reliably extract:

- `\name`
- `\alias`
- `\title`
- `\description`
- `\usage`
- `\examples`
- `\keyword`
- `\docType`

This is enough for:

- topic lookup
- package indexes
- package help pages
- basic text output
- example extraction for future tooling

### 3. Use a stateful lexer

Rd is a macro language with nested braces, escaped text, sections, mode changes, and later-stage features such as conditionals and `\Sexpr`. A stateful lexer is the right foundation whether the parser above it is hand-written or generated.

### 4. Prefer a hand-written parser or LR-style parser over defaulting to Pest

Pest is workable for a small metadata-first parser, but it is not the default best fit for full Rd because:

- Rd has lexer modes and macro semantics that are awkward to model cleanly in a pure PEG grammar
- GNU R's own implementation is lexer/parser based
- miniR will likely need explicit recovery and partial parsing for damaged docs

The best candidates for the full parser layer are:

- hand-written recursive descent over a custom token stream
- `lrlex` + `lrpar`
- `LALRPOP` with a custom lexer

Pest remains acceptable for a narrow metadata extractor if it keeps momentum high, but it should not lock the design.

## Implementation Order

1. Package doc staging
   - Preserve `man/`, `inst/doc/`, and vignette assets in the package install layout.
   - Add an installed-package doc path model to package metadata.

2. Rd help index
   - Scan installed `man/*.Rd`.
   - Build an alias index keyed by topic and package.
   - Record doc metadata (`name`, aliases, title, type, keywords, file path).

3. Metadata-first parser
   - Implement a lexer for Rd commands, escapes, text, braces, and section heads.
   - Parse the core metadata sections listed above.
   - Store a lightweight AST or section map per file.

4. `help()` integration
   - Resolve `help(topic)` through installed package alias indexes.
   - Show package/topic/title/usage/description in a basic text format.
   - Support package-qualified lookups once package namespaces exist.

5. Example extraction
   - Parse and expose `\examples`.
   - Support future `example()` or test tooling built on Rd examples.

6. Richer Rd semantics
   - Add support for more sections and structural markup.
   - Decide how much of conditionals, user macros, and `\Sexpr` to support.
   - Add better text rendering once lookup/indexing is stable.

## Data Model

Expected minimum installed-doc structures:

```rust
struct RdDocMeta {
    package: String,
    file_name: String,
    name: Option<String>,
    aliases: Vec<String>,
    title: Option<String>,
    doc_type: Option<String>,
    keywords: Vec<String>,
}

struct RdDocSections {
    description: Option<String>,
    usage: Option<String>,
    examples: Option<String>,
}
```

The initial implementation does not need a complete lossless AST if metadata and core sections are available. If richer rendering lands later, we can add a token-preserving AST then.

## Parser Options

### Best fit for phase 1

- hand-written lexer + hand-written metadata parser

This gives the most control for the least machinery and makes it easier to evolve into a fuller parser later.

### Best fit for a fuller parser

- hand-written lexer + `lrlex` / `lrpar`
- hand-written lexer + `LALRPOP`
- hand-written lexer + recursive descent

### Acceptable but not preferred

- Pest
- `peg`
- `chumsky`
- `winnow`

These can work, but they are less obviously aligned with Rd's mode-heavy macro structure than a lexer-first design.

### Probably not worth it here

- `tree-sitter`

Useful for incremental editor parsing, not the main value for package help ingestion.

## Integration Points

- package install/load pipeline
- `help()` builtin
- package alias/topic indexes
- future `example()` implementation
- future package inspection or `help(package=...)` output

## Risks

- Rd has more macro semantics than it first appears to.
- `\Sexpr`, conditionals, and user-defined macros can expand the scope quickly.
- Rendering quality can consume a lot of time without improving package execution.

## Recommendation

Start with:

1. installed-doc staging
2. alias/index extraction
3. metadata-first lexer/parser
4. basic `help()` output

Do not start by building a full GNU R Rd renderer. The first milestone should be "package help works and aliases resolve", not "all Rd formatting is reproduced."
