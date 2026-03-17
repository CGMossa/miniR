//! Package metadata parsing for DESCRIPTION, NAMESPACE, and Rd files.
//!
//! This module provides parsers for R's package metadata formats:
//!
//! - **DESCRIPTION** uses Debian Control File (DCF) format: `Field: Value` with
//!   continuation lines (indented with whitespace). We extract the fields needed
//!   for dependency resolution and package identity.
//!
//! - **NAMESPACE** uses a simple directive-based DSL with function-call syntax:
//!   `export(foo)`, `importFrom(pkg, sym)`, `S3method(generic, class)`, etc.
//!
//! - **Rd** (R documentation) is a LaTeX-like format used in package `man/`
//!   directories. The parser extracts metadata and section content for `help()`
//!   lookup and example extraction.
//!
//! This is pure parsing infrastructure — no package loading or environment
//! creation happens here.

pub mod description;
pub mod namespace;
pub mod rd;

pub use description::PackageDescription;
pub use namespace::PackageNamespace;
pub use rd::{RdDoc, RdHelpIndex};
