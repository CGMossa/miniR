+++
title = "Parser And Diagnostics"
weight = 7
description = "How miniR parses R source with pest, builds the AST, and turns parse failures into better diagnostics than raw grammar errors"
+++

miniR's parser is not only a grammar file. It is a pipeline:

1. `pest` parses source text into grammar pairs
2. builder code turns those pairs into the AST used by the interpreter
3. diagnostics code turns raw parser failures into R-facing error messages with source context and suggestions

This split is important because "the parser" often gets blamed for bugs that are actually in evaluation. The parser should answer syntax questions. The interpreter should answer semantic ones.

## The Entry Point

`src/parser.rs` is the public parser boundary.

The core function is:

```rust
pub fn parse_program(input: &str) -> Result<ast::Expr, Box<ParseError>>
```

Its job is deliberately small:

- invoke `RParser::parse()` against the top-level `program` rule
- take the successful parse tree
- pass it to `build_program()`
- return a single AST expression for the whole program

That means the parser boundary is stable and easy to call from `Session`, tests, and tooling.

## Grammar

The grammar lives in `src/parser/r.pest`.

This is where operator precedence and syntax shape are defined. It decides what counts as:

- expressions
- function definitions
- calls and indexing
- control-flow forms
- assignment operators
- special operators and pipes

When a bug is genuinely about precedence or tokenization, this is where it starts.

## AST Construction

The parser does not hand the interpreter raw pest pairs.

`src/parser/builder.rs` converts grammar pairs into AST nodes under `src/parser/ast.rs`. That builder layer is where miniR turns syntax into a runtime-oriented tree that the evaluator can walk directly.

This is also where a few syntax rewrites happen. For example, help syntax such as:

- `?foo`
- `methods?show`

is lowered into a call to `help()` rather than being treated as a special runtime form forever.

That is a good example of the parser doing syntax work while leaving runtime behavior to the ordinary evaluator.

## Diagnostics

Raw pest errors are not good user-facing error messages.

`src/parser/diagnostics.rs` converts them into `ParseError`, which carries:

- the formatted message
- line and column
- the source line
- an optional filename
- an optional suggestion
- byte offsets and span length for richer rendering

With the `diagnostics` feature enabled, the same structured error can render through `miette` for a more graphical report. Without that feature, miniR still has a plain-text fallback that keeps the useful context.

## Why The Parse Errors Matter

miniR is intentionally trying to have better parse diagnostics than GNU R, not just equivalent ones.

A parse error should answer:

- what token or construct was unexpected
- where it happened
- what the user probably meant

That is why the diagnostics layer tries to classify tokens and generate suggestions instead of dumping the raw grammar failure.

## Parse-Only Versus Runtime Work

The parser should only answer syntax questions.

If the failure is about:

- lazy evaluation
- environments
- S3 dispatch
- replacement semantics
- package loading

the fix almost certainly does **not** belong in the parser.

That distinction matters because CRAN compatibility work can otherwise drift into syntax hacks for bugs that are really semantic.

## Parser Divergences

Some deliberate language divergences do start in the parser.

Examples include:

- newline continuation in postfix chains such as `x\n(y)`
- accepting `if (...) ...\nelse ...`
- treating `**` as a synonym for `^`

Those are documented in the divergences docs because they are language choices, not parser accidents.

## Where To Debug Parser Problems

Start in the parser layer when the symptom looks like:

- precedence is wrong
- a valid file fails before evaluation even begins
- a token is classified or highlighted badly
- a parse error points to the wrong span
- `?topic` or similar syntax is rewritten incorrectly

The main files are:

- `src/parser/r.pest`
- `src/parser.rs`
- `src/parser/builder.rs`
- `src/parser/diagnostics.rs`

That combination is the real parser, not only the grammar file.
