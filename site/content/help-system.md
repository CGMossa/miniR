+++
title = "Help And Documentation"
weight = 13
description = "How `?topic`, package `man/` indexes, builtin rustdoc, and generated `.Rd` files fit together in miniR's help pipeline"
+++

miniR's help system is more layered than it first appears.

It combines:

- parser support for `?topic` syntax
- package `man/` indexing through parsed `.Rd` files
- synthesized builtin help from Rust doc comments
- optional `.Rd` generation for builtin docs

That matters because miniR is trying to treat packages as packages, not just as executable `R/` directories.

## Syntax: `?topic` Is Normalized Early

The parser rewrites help syntax into ordinary calls.

Examples:

- `?foo` becomes a call to `help("foo")`
- `methods?show` becomes a call to `help("show", package = "methods")`

That rewrite happens in the parser builder layer, which means the runtime can implement help as an ordinary builtin instead of carrying a special top-level syntax case forever.

## The `help()` Builtin

The runtime entry point is the `help()` builtin in `src/interpreter/builtins.rs`.

Its lookup flow is:

1. check whether the topic refers to a builtin namespace summary
2. check the Rd help index for package `man/` docs
3. fall back to builtin help synthesized from Rust doc comments
4. print a "No documentation" message if neither path matches

That fallback order is important. Package docs should win when they exist. Builtin help should still exist from day one even if no external `.Rd` file was written by hand.

## Package Help: `RdHelpIndex`

Package documentation is indexed through `RdHelpIndex` in `src/interpreter/packages/rd.rs`.

The index maps topic aliases to parsed docs and is built by scanning package `man/` directories.

Each indexed entry records:

- package name
- source `.Rd` path
- parsed `RdDoc`

Lookups can happen:

- across all packages for a topic
- within a specific package when the user qualifies the query

That keeps help behavior closer to how package users actually expect `help()` to work.

## What The `.Rd` Parser Tries To Do

miniR's `.Rd` support is metadata-first.

It is designed to reliably extract the sections needed for help lookup and terminal display, such as:

- `\name{}`
- `\alias{}`
- `\title{}`
- `\description{}`
- `\usage{}`
- `\arguments{}`
- `\examples{}`

The goal is not to perfectly reproduce every quirk of GNU R's Rd rendering. The goal is to make package help lookup and readable terminal output work well.

## Builtin Help From Rust Docs

miniR also synthesizes builtin help from Rust doc comments.

At interpreter startup:

- the builtin registry is already populated
- `synthesize_builtin_help()` walks documented builtins
- those docs are inserted into the same help index machinery

This means every documented builtin can participate in help lookup without requiring a second hand-written documentation format.

That is a strong architectural choice: the implementation docs are close to the code, but they still show up as runtime help.

## Generated `.Rd` Output For Builtins

miniR can also write `.Rd` files for documented builtins.

`Session::generate_rd_docs()` delegates to builtin doc generation so the project can export runtime help into package-style documentation files.

That is useful for:

- package-like docs workflows
- documentation auditing
- keeping builtin help portable outside the running interpreter

It also reinforces the idea that builtin docs are real documentation assets, not only terminal strings.

## Why This Matters For Package Compatibility

Package compatibility is not only about evaluating `R/` code.

Packages also expect:

- topics in `man/`
- aliases resolving correctly
- `?topic` and `help()` to work against package docs
- docs for builtins and package objects to be discoverable

That is why miniR treats help and Rd indexing as runtime subsystems instead of optional polish.

## Where To Debug Help Problems

Start in the help pipeline when the symptom looks like:

- `?topic` parses oddly
- package docs exist but are not found
- aliases resolve to the wrong topic
- builtin help is missing even though the implementation has doc comments
- generated `.Rd` files are incomplete or grouped incorrectly

The key files are:

- `src/parser/builder.rs`
- `src/interpreter/builtins.rs`
- `src/interpreter/packages/rd.rs`
- `src/session.rs`

Together, those files are the real help system.
