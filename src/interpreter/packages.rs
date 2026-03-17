//! Package metadata parsing and runtime loading.
//!
//! This module provides:
//!
//! - **DESCRIPTION parser** (`description.rs`): Debian Control File format for
//!   package metadata (name, version, dependencies).
//!
//! - **NAMESPACE parser** (`namespace.rs`): directive-based DSL for exports,
//!   imports, and S3 method registrations.
//!
//! - **Rd parser** (`rd.rs`): LaTeX-like format used in package `man/`
//!   directories for `help()` lookup and example extraction.
//!
//! - **Package loader** (`loader.rs`): runtime package loading — discovers
//!   packages on `.libPaths()`, creates namespace/exports environments, sources
//!   R files, and manages the search path.

pub mod description;
pub mod loader;
pub mod namespace;
pub mod rd;

pub use description::PackageDescription;
pub use loader::{LoadedNamespace, SearchPathEntry};
pub use namespace::PackageNamespace;
pub use rd::{RdDoc, RdHelpIndex};
