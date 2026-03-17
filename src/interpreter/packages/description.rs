//! Parser for R package DESCRIPTION files (Debian Control File format).
//!
//! The DCF format is simple:
//! - Lines of the form `Field: Value` introduce a new field
//! - Continuation lines start with whitespace and append to the current field
//! - Blank lines separate stanzas (DESCRIPTION files have exactly one stanza)
//! - `#` comments are NOT supported in DCF (unlike NAMESPACE)
//!
//! Dependency fields (`Depends`, `Imports`, `Suggests`, `LinkingTo`) contain
//! comma-separated package names with optional version constraints like
//! `Matrix (>= 1.2-0)`.

use std::collections::HashMap;

/// A parsed R package DESCRIPTION file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageDescription {
    /// The package name (`Package` field). Always present in valid DESCRIPTION files.
    pub package: String,
    /// The package version (`Version` field).
    pub version: String,
    /// The package title (`Title` field).
    pub title: Option<String>,
    /// Packages listed in `Depends` (attached when this package loads).
    pub depends: Vec<Dependency>,
    /// Packages listed in `Imports` (loaded but not attached).
    pub imports: Vec<Dependency>,
    /// Packages listed in `Suggests` (optional, for tests/examples).
    pub suggests: Vec<Dependency>,
    /// Packages listed in `LinkingTo` (C/C++ headers at compile time).
    pub linking_to: Vec<Dependency>,
    /// All raw fields from the DCF file, for access to fields we don't
    /// explicitly model (e.g. `License`, `Author`, `Description`).
    pub fields: HashMap<String, String>,
}

/// A single dependency entry, e.g. `Matrix (>= 1.2-0)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dependency {
    /// Package name.
    pub package: String,
    /// Optional version constraint, e.g. `>= 1.2-0`.
    pub version_constraint: Option<String>,
}

/// Errors that can occur when parsing a DESCRIPTION file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DescriptionError {
    /// The `Package` field is missing.
    MissingPackage,
    /// The `Version` field is missing.
    MissingVersion,
    /// A line could not be parsed (not a field and not a continuation).
    MalformedLine { line_number: usize, line: String },
}

impl std::fmt::Display for DescriptionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DescriptionError::MissingPackage => {
                write!(f, "DESCRIPTION is missing required 'Package' field")
            }
            DescriptionError::MissingVersion => {
                write!(f, "DESCRIPTION is missing required 'Version' field")
            }
            DescriptionError::MalformedLine { line_number, line } => {
                write!(
                    f,
                    "DESCRIPTION line {line_number}: malformed line (not a field or continuation): {line:?}"
                )
            }
        }
    }
}

impl std::error::Error for DescriptionError {}

impl PackageDescription {
    /// Parse a DESCRIPTION file from its text content.
    pub fn parse(input: &str) -> Result<Self, DescriptionError> {
        let fields = parse_dcf(input)?;

        let package = fields
            .get("Package")
            .cloned()
            .ok_or(DescriptionError::MissingPackage)?;
        let version = fields
            .get("Version")
            .cloned()
            .ok_or(DescriptionError::MissingVersion)?;
        let title = fields.get("Title").cloned();

        let depends = fields
            .get("Depends")
            .map(|s| parse_dependency_list(s))
            .unwrap_or_default();
        let imports = fields
            .get("Imports")
            .map(|s| parse_dependency_list(s))
            .unwrap_or_default();
        let suggests = fields
            .get("Suggests")
            .map(|s| parse_dependency_list(s))
            .unwrap_or_default();
        let linking_to = fields
            .get("LinkingTo")
            .map(|s| parse_dependency_list(s))
            .unwrap_or_default();

        Ok(PackageDescription {
            package,
            version,
            title,
            depends,
            imports,
            suggests,
            linking_to,
            fields,
        })
    }
}

/// Parse DCF (Debian Control File) format into a field map.
///
/// Returns fields in their original casing. Continuation lines are joined
/// with a single space (leading whitespace on continuation lines is stripped,
/// but the continuation itself is space-separated from the previous line).
fn parse_dcf(input: &str) -> Result<HashMap<String, String>, DescriptionError> {
    let mut fields = HashMap::new();
    let mut current_field: Option<String> = None;
    let mut current_value = String::new();

    for (line_number, line) in input.lines().enumerate() {
        let line_number = line_number + 1; // 1-indexed for error messages

        // Blank lines end the current stanza. DESCRIPTION has one stanza,
        // so we stop collecting after the first blank line.
        if line.trim().is_empty() {
            if let Some(field) = current_field.take() {
                fields.insert(field, current_value.trim().to_string());
                current_value.clear();
            }
            continue;
        }

        // Continuation line: starts with whitespace
        if line.starts_with(' ') || line.starts_with('\t') {
            if current_field.is_some() {
                // Preserve the continuation with a newline for multiline fields
                // like Description, but for dependency fields the newlines
                // will be handled by the dependency parser.
                current_value.push('\n');
                current_value.push_str(line.trim());
            } else {
                return Err(DescriptionError::MalformedLine {
                    line_number,
                    line: line.to_string(),
                });
            }
            continue;
        }

        // New field: `Field: Value`
        if let Some(colon_pos) = line.find(':') {
            let field_name = line[..colon_pos].trim();
            // Field names must not contain whitespace
            if field_name.contains(' ') || field_name.contains('\t') {
                return Err(DescriptionError::MalformedLine {
                    line_number,
                    line: line.to_string(),
                });
            }

            // Save the previous field
            if let Some(prev_field) = current_field.take() {
                fields.insert(prev_field, current_value.trim().to_string());
                current_value.clear();
            }

            current_field = Some(field_name.to_string());
            let value_part = &line[colon_pos + 1..];
            current_value.push_str(value_part.trim());
            continue;
        }

        // Line is not blank, not a continuation, not a field — error
        return Err(DescriptionError::MalformedLine {
            line_number,
            line: line.to_string(),
        });
    }

    // Don't forget the last field
    if let Some(field) = current_field.take() {
        fields.insert(field, current_value.trim().to_string());
    }

    Ok(fields)
}

/// Parse a comma-separated dependency list like `R (>= 3.5.0), dplyr, Matrix (>= 1.2-0)`.
fn parse_dependency_list(input: &str) -> Vec<Dependency> {
    // Dependencies are comma-separated, possibly spanning multiple lines
    // (already joined by parse_dcf with newlines, which we treat as whitespace).
    input
        .split(',')
        .filter_map(|entry| {
            let entry = entry.replace(['\n', '\r'], " ");
            let entry = entry.trim().to_string();
            if entry.is_empty() {
                return None;
            }
            Some(parse_single_dependency(&entry))
        })
        .collect()
}

/// Parse a single dependency entry like `Matrix (>= 1.2-0)` or just `dplyr`.
fn parse_single_dependency(entry: &str) -> Dependency {
    let entry = entry.trim();
    if let Some(paren_start) = entry.find('(') {
        let package = entry[..paren_start].trim().to_string();
        let constraint = if let Some(paren_end) = entry.find(')') {
            entry[paren_start + 1..paren_end].trim().to_string()
        } else {
            // Unclosed paren — take everything after '(' as the constraint
            entry[paren_start + 1..].trim().to_string()
        };
        Dependency {
            package,
            version_constraint: if constraint.is_empty() {
                None
            } else {
                Some(constraint)
            },
        }
    } else {
        Dependency {
            package: entry.to_string(),
            version_constraint: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_description() {
        let input = "\
Package: myPkg
Version: 1.0.0
Title: A Test Package
Depends: R (>= 3.5.0), methods
Imports: dplyr, Matrix (>= 1.2-0)
License: MIT
";
        let desc = PackageDescription::parse(input).unwrap();
        assert_eq!(desc.package, "myPkg");
        assert_eq!(desc.version, "1.0.0");
        assert_eq!(desc.title.as_deref(), Some("A Test Package"));
        assert_eq!(desc.depends.len(), 2);
        assert_eq!(desc.depends[0].package, "R");
        assert_eq!(
            desc.depends[0].version_constraint.as_deref(),
            Some(">= 3.5.0")
        );
        assert_eq!(desc.depends[1].package, "methods");
        assert_eq!(desc.depends[1].version_constraint, None);
        assert_eq!(desc.imports.len(), 2);
        assert_eq!(desc.imports[0].package, "dplyr");
        assert_eq!(desc.imports[1].package, "Matrix");
        assert_eq!(
            desc.imports[1].version_constraint.as_deref(),
            Some(">= 1.2-0")
        );
        assert!(desc.suggests.is_empty());
        assert!(desc.linking_to.is_empty());
    }

    #[test]
    fn parse_continuation_lines() {
        let input = "\
Package: bigPkg
Version: 2.3.1
Title: A Package with
    a Multi-Line Title
Description: This is a long description
    that spans multiple lines and explains
    what the package does.
Depends: R (>= 4.0.0),
    rlang (>= 0.4.0),
    vctrs
Imports: lifecycle,
    pillar (>= 1.5.0)
";
        let desc = PackageDescription::parse(input).unwrap();
        assert_eq!(desc.package, "bigPkg");
        assert_eq!(desc.version, "2.3.1");
        // Title continuation is joined
        assert!(desc.title.as_deref().unwrap().contains("Multi-Line Title"));
        assert_eq!(desc.depends.len(), 3);
        assert_eq!(desc.depends[0].package, "R");
        assert_eq!(desc.depends[1].package, "rlang");
        assert_eq!(
            desc.depends[1].version_constraint.as_deref(),
            Some(">= 0.4.0")
        );
        assert_eq!(desc.depends[2].package, "vctrs");
        assert_eq!(desc.imports.len(), 2);
        assert_eq!(desc.imports[0].package, "lifecycle");
        assert_eq!(desc.imports[1].package, "pillar");
        assert_eq!(
            desc.imports[1].version_constraint.as_deref(),
            Some(">= 1.5.0")
        );
    }

    #[test]
    fn parse_missing_package_field() {
        let input = "\
Version: 1.0
Title: No package name
";
        let err = PackageDescription::parse(input).unwrap_err();
        assert_eq!(err, DescriptionError::MissingPackage);
    }

    #[test]
    fn parse_missing_version_field() {
        let input = "\
Package: oops
Title: No version
";
        let err = PackageDescription::parse(input).unwrap_err();
        assert_eq!(err, DescriptionError::MissingVersion);
    }

    #[test]
    fn parse_pkgb_description() {
        // Real DESCRIPTION from tests/Pkgs/pkgB
        let input = "\
Package: pkgB
Title: Simple Package with NameSpace and S4 Methods and Classes
Type: Package
Imports: methods, graphics, pkgA
Version: 1.0
Date: 2019-01-21
Author: Yohan Chalabi and R-core
Maintainer: R Core <R-core@almost.r-project.org>
Description: Example package with a namespace and imports of S4, but empty R/ ....
 used for regression testing the correct working of tools::codoc(), undoc()
 etc, but also S4 in connection with other packages.
License: GPL (>= 2)
";
        let desc = PackageDescription::parse(input).unwrap();
        assert_eq!(desc.package, "pkgB");
        assert_eq!(desc.version, "1.0");
        assert_eq!(desc.imports.len(), 3);
        assert_eq!(desc.imports[0].package, "methods");
        assert_eq!(desc.imports[1].package, "graphics");
        assert_eq!(desc.imports[2].package, "pkgA");
        assert!(desc.depends.is_empty());
    }

    #[test]
    fn parse_pkgd_description() {
        // Real DESCRIPTION from tests/Pkgs/pkgD — has versioned Depends and Imports
        let input = "\
Package: pkgD
Title: Simple Package with NameSpace and S4 Methods and Classes
Version: 1.2.0
Date: 2015-10-10
Type: Package
Depends: R (>= 2.14.0), R (>= r56550), methods
Imports: Matrix (>= 1.2-0), Matrix (<= 99.9-9)
LazyData: true
Author: Yohan Chalabi and R-core
Maintainer: R Core <R-core@almost.r-project.org>
Description: Example package with a namespace, and S4 method for \"plot\".
 used for regression testing the correct working of tools::codoc(), undoc()
 etc, but also S4 in connection with other packages.
License: GPL (>= 2)
";
        let desc = PackageDescription::parse(input).unwrap();
        assert_eq!(desc.package, "pkgD");
        assert_eq!(desc.version, "1.2.0");
        assert_eq!(desc.depends.len(), 3);
        assert_eq!(desc.depends[0].package, "R");
        assert_eq!(
            desc.depends[0].version_constraint.as_deref(),
            Some(">= 2.14.0")
        );
        assert_eq!(desc.depends[1].package, "R");
        assert_eq!(
            desc.depends[1].version_constraint.as_deref(),
            Some(">= r56550")
        );
        assert_eq!(desc.depends[2].package, "methods");
        assert_eq!(desc.imports.len(), 2);
        assert_eq!(desc.imports[0].package, "Matrix");
        assert_eq!(
            desc.imports[0].version_constraint.as_deref(),
            Some(">= 1.2-0")
        );
        assert_eq!(desc.imports[1].package, "Matrix");
        assert_eq!(
            desc.imports[1].version_constraint.as_deref(),
            Some("<= 99.9-9")
        );
    }

    #[test]
    fn parse_suggests_and_linking_to() {
        let input = "\
Package: testPkg
Version: 0.1.0
Suggests: testthat (>= 3.0.0), knitr
LinkingTo: Rcpp, RcppArmadillo (>= 0.9)
";
        let desc = PackageDescription::parse(input).unwrap();
        assert_eq!(desc.suggests.len(), 2);
        assert_eq!(desc.suggests[0].package, "testthat");
        assert_eq!(
            desc.suggests[0].version_constraint.as_deref(),
            Some(">= 3.0.0")
        );
        assert_eq!(desc.suggests[1].package, "knitr");
        assert_eq!(desc.linking_to.len(), 2);
        assert_eq!(desc.linking_to[0].package, "Rcpp");
        assert_eq!(desc.linking_to[1].package, "RcppArmadillo");
        assert_eq!(
            desc.linking_to[1].version_constraint.as_deref(),
            Some(">= 0.9")
        );
    }

    #[test]
    fn raw_fields_accessible() {
        let input = "\
Package: myPkg
Version: 1.0
License: MIT
NeedsCompilation: no
";
        let desc = PackageDescription::parse(input).unwrap();
        assert_eq!(desc.fields.get("License").unwrap(), "MIT");
        assert_eq!(desc.fields.get("NeedsCompilation").unwrap(), "no");
    }

    #[test]
    fn empty_dependency_fields() {
        let input = "\
Package: minimal
Version: 0.0.1
";
        let desc = PackageDescription::parse(input).unwrap();
        assert!(desc.depends.is_empty());
        assert!(desc.imports.is_empty());
        assert!(desc.suggests.is_empty());
        assert!(desc.linking_to.is_empty());
    }

    #[test]
    fn trailing_comma_in_deps() {
        // Some packages have trailing commas
        let input = "\
Package: messy
Version: 1.0
Imports: dplyr, tidyr,
";
        let desc = PackageDescription::parse(input).unwrap();
        assert_eq!(desc.imports.len(), 2);
        assert_eq!(desc.imports[0].package, "dplyr");
        assert_eq!(desc.imports[1].package, "tidyr");
    }
}
