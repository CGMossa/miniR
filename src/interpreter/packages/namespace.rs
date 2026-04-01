//! Parser for R package NAMESPACE files.
//!
//! NAMESPACE files use a simple directive-based DSL with function-call syntax.
//! Each directive is one of:
//!
//! - `export(name1, name2, ...)` — export symbols
//! - `exportPattern("^[^.]")` — export symbols matching a regex
//! - `import(pkg1, pkg2, ...)` — import all exports from packages
//! - `importFrom(pkg, sym1, sym2, ...)` — import specific symbols from a package
//! - `S3method(generic, class)` or `S3method(generic, class, method)` — register S3 methods
//! - `useDynLib(pkg, ...)` — load a shared library
//! - `exportClasses(cls1, cls2, ...)` — export S4 classes
//! - `exportMethods(meth1, meth2, ...)` — export S4 methods
//! - `importClassesFrom(pkg, cls1, cls2, ...)` — import S4 classes from a package
//! - `importMethodsFrom(pkg, meth1, meth2, ...)` — import S4 methods from a package
//!
//! Lines starting with `#` are comments. Directives can span multiple lines
//! (the parser collects text until balanced parentheses).

/// A parsed R package NAMESPACE file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PackageNamespace {
    /// Symbols explicitly exported by name.
    pub exports: Vec<String>,
    /// Regex patterns for exported symbols.
    pub export_patterns: Vec<String>,
    /// Packages whose entire namespace is imported.
    pub imports: Vec<String>,
    /// Specific symbol imports: `(package, symbol)` pairs.
    pub imports_from: Vec<(String, String)>,
    /// S3 method registrations: `(generic, class, optional method_name)`.
    pub s3_methods: Vec<S3MethodRegistration>,
    /// Dynamic library loads: `(library_name, registrations)`.
    pub use_dyn_libs: Vec<DynLibDirective>,
    /// S4 classes exported.
    pub export_classes: Vec<String>,
    /// S4 methods exported.
    pub export_methods: Vec<String>,
    /// S4 classes imported from a package: `(package, class)`.
    pub import_classes_from: Vec<(String, String)>,
    /// S4 methods imported from a package: `(package, method)`.
    pub import_methods_from: Vec<(String, String)>,
}

impl PackageNamespace {
    /// Create a namespace that exports everything (used for built-in base packages).
    pub fn export_all() -> Self {
        PackageNamespace {
            exports: vec![],
            export_patterns: vec![".*".to_string()], // exportPattern(".*")
            imports: vec![],
            imports_from: vec![],
            s3_methods: vec![],
            use_dyn_libs: vec![],
            export_classes: vec![],
            export_methods: vec![],
            import_classes_from: vec![],
            import_methods_from: vec![],
        }
    }
}

/// An S3 method registration from the NAMESPACE file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct S3MethodRegistration {
    /// The generic function name (e.g. `print`).
    pub generic: String,
    /// The class name (e.g. `data.frame`).
    pub class: String,
    /// An optional explicit method function name. If absent, R assumes
    /// the method is named `generic.class`.
    pub method: Option<String>,
}

/// A `useDynLib` directive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynLibDirective {
    /// The shared library name.
    pub library: String,
    /// Additional registration entries (symbols, `.registration = TRUE`, etc.).
    pub registrations: Vec<String>,
}

/// Errors that can occur when parsing a NAMESPACE file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NamespaceError {
    /// Parentheses are not balanced (unclosed directive).
    UnbalancedParens { line_number: usize },
    /// An unknown directive was found.
    UnknownDirective {
        line_number: usize,
        directive: String,
    },
    /// A directive had too few arguments.
    TooFewArgs {
        line_number: usize,
        directive: String,
        expected: usize,
        got: usize,
    },
}

impl std::fmt::Display for NamespaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NamespaceError::UnbalancedParens { line_number } => {
                write!(
                    f,
                    "NAMESPACE line {line_number}: unbalanced parentheses (unclosed directive)"
                )
            }
            NamespaceError::UnknownDirective {
                line_number,
                directive,
            } => {
                write!(
                    f,
                    "NAMESPACE line {line_number}: unknown directive '{directive}'"
                )
            }
            NamespaceError::TooFewArgs {
                line_number,
                directive,
                expected,
                got,
            } => {
                write!(
                    f,
                    "NAMESPACE line {line_number}: {directive}() requires at least {expected} argument(s), got {got}"
                )
            }
        }
    }
}

impl std::error::Error for NamespaceError {}

impl PackageNamespace {
    /// Parse a NAMESPACE file from its text content.
    pub fn parse(input: &str) -> Result<Self, NamespaceError> {
        let mut ns = PackageNamespace::default();
        let directives = collect_directives(input)?;

        for (line_number, name, args_str) in directives {
            let args = parse_args(&args_str);

            match name.as_str() {
                "export" => {
                    ns.exports.extend(args);
                }
                "exportPattern" => {
                    ns.export_patterns.extend(args);
                }
                "import" => {
                    ns.imports.extend(args);
                }
                "importFrom" => {
                    if args.len() < 2 {
                        return Err(NamespaceError::TooFewArgs {
                            line_number,
                            directive: name,
                            expected: 2,
                            got: args.len(),
                        });
                    }
                    let pkg = &args[0];
                    for sym in &args[1..] {
                        ns.imports_from.push((pkg.clone(), sym.clone()));
                    }
                }
                "S3method" => {
                    if args.len() < 2 {
                        return Err(NamespaceError::TooFewArgs {
                            line_number,
                            directive: name,
                            expected: 2,
                            got: args.len(),
                        });
                    }
                    ns.s3_methods.push(S3MethodRegistration {
                        generic: args[0].clone(),
                        class: args[1].clone(),
                        method: args.get(2).cloned(),
                    });
                }
                "useDynLib" => {
                    if args.is_empty() {
                        return Err(NamespaceError::TooFewArgs {
                            line_number,
                            directive: name,
                            expected: 1,
                            got: 0,
                        });
                    }
                    ns.use_dyn_libs.push(DynLibDirective {
                        library: args[0].clone(),
                        registrations: args[1..].to_vec(),
                    });
                }
                "exportClasses" | "exportClass" => {
                    ns.export_classes.extend(args);
                }
                "exportMethods" => {
                    ns.export_methods.extend(args);
                }
                "importClassesFrom" => {
                    if args.len() < 2 {
                        return Err(NamespaceError::TooFewArgs {
                            line_number,
                            directive: name,
                            expected: 2,
                            got: args.len(),
                        });
                    }
                    let pkg = &args[0];
                    for cls in &args[1..] {
                        ns.import_classes_from.push((pkg.clone(), cls.clone()));
                    }
                }
                "importMethodsFrom" => {
                    if args.len() < 2 {
                        return Err(NamespaceError::TooFewArgs {
                            line_number,
                            directive: name,
                            expected: 2,
                            got: args.len(),
                        });
                    }
                    let pkg = &args[0];
                    for meth in &args[1..] {
                        ns.import_methods_from.push((pkg.clone(), meth.clone()));
                    }
                }
                _ => {
                    // Unknown directives are warnings, not errors — new/uncommon
                    // directives shouldn't block package loading.
                    tracing::warn!(
                        "NAMESPACE line {}: unknown directive '{}' (ignored)",
                        line_number,
                        name
                    );
                }
            }
        }

        Ok(ns)
    }
}

/// Collect complete directives from a NAMESPACE file.
///
/// A directive is `name(args)` which may span multiple lines. Comments (`#`)
/// are stripped. Returns `(start_line, directive_name, args_content)` triples.
fn collect_directives(input: &str) -> Result<Vec<(usize, String, String)>, NamespaceError> {
    let mut directives = Vec::new();
    let mut current_name: Option<String> = None;
    let mut current_args = String::new();
    let mut paren_depth: usize = 0;
    let mut start_line: usize = 0;

    for (line_idx, raw_line) in input.lines().enumerate() {
        let line_number = line_idx + 1;

        // Strip comments (but only outside of quoted strings in the directive)
        let line = strip_comment(raw_line);
        let line = line.trim();

        if line.is_empty() {
            continue;
        }

        if current_name.is_some() {
            // We're inside a multi-line directive — accumulate
            let mut in_quotes = false;
            for ch in line.chars() {
                if ch == '"' {
                    in_quotes = !in_quotes;
                    current_args.push(ch);
                    continue;
                }
                if in_quotes {
                    current_args.push(ch);
                    continue;
                }
                if ch == '(' {
                    paren_depth += 1;
                    current_args.push(ch);
                } else if ch == ')' {
                    if paren_depth == 0 {
                        continue;
                    }
                    paren_depth -= 1;
                    if paren_depth == 0 {
                        // Directive complete
                        directives.push((
                            start_line,
                            current_name
                                .take()
                                .expect("current_name is Some (checked above)"),
                            current_args.clone(),
                        ));
                        current_args.clear();
                    } else {
                        current_args.push(ch);
                    }
                } else {
                    current_args.push(ch);
                }
            }
        } else {
            // Handle conditional directives: `if (getRversion() < "X.Y.Z") directive(args)`
            // miniR is a modern R — treat all conditions as true, extract the directive.
            let line = if line.starts_with("if ") || line.starts_with("if(") {
                // Find the closing `)` of the condition, then the directive after it
                if let Some(directive_start) = find_directive_after_if(line) {
                    &line[directive_start..]
                } else {
                    continue; // malformed if — skip
                }
            } else {
                line
            };

            // Look for a new directive: `name(`
            if let Some(paren_pos) = line.find('(') {
                let name = line[..paren_pos].trim().to_string();
                if name.is_empty() {
                    continue;
                }
                start_line = line_number;
                current_name = Some(name);
                paren_depth = 0;

                // Process the rest of the line after the directive name
                let mut in_quotes = false;
                for ch in line[paren_pos..].chars() {
                    if ch == '"' {
                        in_quotes = !in_quotes;
                        if paren_depth > 0 {
                            current_args.push(ch);
                        }
                        continue;
                    }
                    if in_quotes {
                        if paren_depth > 0 {
                            current_args.push(ch);
                        }
                        continue;
                    }
                    if ch == '(' {
                        paren_depth += 1;
                        if paren_depth > 1 {
                            current_args.push(ch);
                        }
                    } else if ch == ')' {
                        paren_depth -= 1;
                        if paren_depth == 0 {
                            directives.push((
                                start_line,
                                current_name
                                    .take()
                                    .expect("current_name is Some (just set above)"),
                                current_args.clone(),
                            ));
                            current_args.clear();
                        } else {
                            current_args.push(ch);
                        }
                    } else if paren_depth > 0 {
                        current_args.push(ch);
                    }
                }
            }
            // Lines without `(` that aren't continuations are ignored
            // (could be stray text or formatting)
        }
    }

    if current_name.is_some() {
        return Err(NamespaceError::UnbalancedParens {
            line_number: start_line,
        });
    }

    Ok(directives)
}

/// Strip a `#` comment from a line, respecting quoted strings.
fn strip_comment(line: &str) -> &str {
    let mut in_double_quote = false;
    let mut in_single_quote = false;
    let mut prev_was_backslash = false;

    for (i, ch) in line.char_indices() {
        if prev_was_backslash {
            prev_was_backslash = false;
            continue;
        }
        match ch {
            '\\' => {
                prev_was_backslash = true;
            }
            '"' if !in_single_quote => {
                in_double_quote = !in_double_quote;
            }
            '\'' if !in_double_quote => {
                in_single_quote = !in_single_quote;
            }
            '#' if !in_double_quote && !in_single_quote => {
                return &line[..i];
            }
            _ => {}
        }
    }
    line
}

/// Parse the argument content of a directive into individual string tokens.
///
/// Given a line like `if (getRversion() < "3.2.0") export(anyNA)`,
/// find the byte offset where the actual directive starts (after the if condition).
/// Returns None if the line is malformed.
fn find_directive_after_if(line: &str) -> Option<usize> {
    // Find the opening `(` of the if condition
    let cond_start = line.find('(')?;
    // Walk forward counting parens to find the matching `)`
    let mut depth = 0;
    let mut cond_end = None;
    for (i, ch) in line[cond_start..].char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    cond_end = Some(cond_start + i + 1);
                    break;
                }
            }
            _ => {}
        }
    }
    let after_cond = cond_end?;
    // Skip whitespace after the condition
    let rest = line[after_cond..].trim_start();
    if rest.is_empty() {
        return None;
    }
    // Return offset into original line
    Some(line.len() - rest.len())
}

/// Arguments are comma-separated. Surrounding quotes (single or double) are
/// stripped. Named arguments like `.registration = TRUE` are preserved as
/// single tokens.
fn parse_args(args_str: &str) -> Vec<String> {
    args_str
        .split(',')
        .filter_map(|arg| {
            let arg = arg.trim();
            if arg.is_empty() {
                return None;
            }
            Some(unquote(arg).to_string())
        })
        .collect()
}

/// Remove surrounding quotes from a string.
fn unquote(s: &str) -> &str {
    let s = s.trim();
    if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        &s[1..s.len() - 1]
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_exports() {
        let input = "\
export(foo)
export(bar, baz)
";
        let ns = PackageNamespace::parse(input).unwrap();
        assert_eq!(ns.exports, vec!["foo", "bar", "baz"]);
    }

    #[test]
    fn parse_export_pattern() {
        let input = r#"exportPattern("^[^.]")"#;
        let ns = PackageNamespace::parse(input).unwrap();
        assert_eq!(ns.export_patterns, vec!["^[^.]"]);
    }

    #[test]
    fn parse_import_and_import_from() {
        let input = "\
import(methods)
import(graphics)
importFrom(Matrix, t, mean, colMeans, colSums)
";
        let ns = PackageNamespace::parse(input).unwrap();
        assert_eq!(ns.imports, vec!["methods", "graphics"]);
        assert_eq!(
            ns.imports_from,
            vec![
                ("Matrix".to_string(), "t".to_string()),
                ("Matrix".to_string(), "mean".to_string()),
                ("Matrix".to_string(), "colMeans".to_string()),
                ("Matrix".to_string(), "colSums".to_string()),
            ]
        );
    }

    #[test]
    fn parse_s3method() {
        let input = "\
S3method(print, myClass)
S3method(within, myList, within.list)
";
        let ns = PackageNamespace::parse(input).unwrap();
        assert_eq!(ns.s3_methods.len(), 2);
        assert_eq!(ns.s3_methods[0].generic, "print");
        assert_eq!(ns.s3_methods[0].class, "myClass");
        assert_eq!(ns.s3_methods[0].method, None);
        assert_eq!(ns.s3_methods[1].generic, "within");
        assert_eq!(ns.s3_methods[1].class, "myList");
        assert_eq!(ns.s3_methods[1].method.as_deref(), Some("within.list"));
    }

    #[test]
    fn parse_use_dyn_lib() {
        let input = "useDynLib(myPkg, .registration = TRUE)\n";
        let ns = PackageNamespace::parse(input).unwrap();
        assert_eq!(ns.use_dyn_libs.len(), 1);
        assert_eq!(ns.use_dyn_libs[0].library, "myPkg");
        assert_eq!(
            ns.use_dyn_libs[0].registrations,
            vec![".registration = TRUE"]
        );
    }

    #[test]
    fn parse_with_comments() {
        let input = "\
# This is a comment
export(myList)

##                       within.list is in base
S3method(within, myList, within.list)
";
        let ns = PackageNamespace::parse(input).unwrap();
        assert_eq!(ns.exports, vec!["myList"]);
        assert_eq!(ns.s3_methods.len(), 1);
        assert_eq!(ns.s3_methods[0].generic, "within");
    }

    #[test]
    fn parse_inline_comments() {
        let input = "\
export(nil)
export(search)# --> \"conflict message\"
importClassesFrom(pkgA, \"classA\")# but not \"classApp\"
";
        let ns = PackageNamespace::parse(input).unwrap();
        assert_eq!(ns.exports, vec!["nil", "search"]);
        assert_eq!(
            ns.import_classes_from,
            vec![("pkgA".to_string(), "classA".to_string())]
        );
    }

    #[test]
    fn parse_multiline_directive() {
        let input = "\
exportMethods(
 pubGenf, pubfn,
 plot, show
)
";
        let ns = PackageNamespace::parse(input).unwrap();
        assert_eq!(ns.export_methods, vec!["pubGenf", "pubfn", "plot", "show"]);
    }

    #[test]
    fn parse_quoted_args() {
        let input = r#"
importFrom("graphics", plot)
exportClasses("classA", "classApp")
"#;
        let ns = PackageNamespace::parse(input).unwrap();
        assert_eq!(
            ns.imports_from,
            vec![("graphics".to_string(), "plot".to_string())]
        );
        assert_eq!(ns.export_classes, vec!["classA", "classApp"]);
    }

    #[test]
    fn parse_s3export_namespace() {
        // Real NAMESPACE from tests/Pkgs/S3export
        let input = "\
export(myList)

##                       within.list is in base
S3method(within, myList, within.list)
";
        let ns = PackageNamespace::parse(input).unwrap();
        assert_eq!(ns.exports, vec!["myList"]);
        assert_eq!(ns.s3_methods.len(), 1);
        assert_eq!(ns.s3_methods[0].generic, "within");
        assert_eq!(ns.s3_methods[0].class, "myList");
        assert_eq!(ns.s3_methods[0].method.as_deref(), Some("within.list"));
    }

    #[test]
    fn parse_pkgb_namespace() {
        // Real NAMESPACE from tests/Pkgs/pkgB
        let input = "\
import(methods)

import(graphics)

importMethodsFrom(pkgA, \"plot\")

importClassesFrom(pkgA, \"classA\")# but not \"classApp\"
";
        let ns = PackageNamespace::parse(input).unwrap();
        assert_eq!(ns.imports, vec!["methods", "graphics"]);
        assert_eq!(
            ns.import_methods_from,
            vec![("pkgA".to_string(), "plot".to_string())]
        );
        assert_eq!(
            ns.import_classes_from,
            vec![("pkgA".to_string(), "classA".to_string())]
        );
    }

    #[test]
    fn parse_exnss4_namespace() {
        // Real NAMESPACE from tests/Pkgs/exNSS4
        let input = r#"
importFrom("graphics", plot) # because we want to define methods on it

## Generics and functions defined in this package
export(pubGenf, pubfn, # generic functions
       assertError)# and a simple one

## own classes
exportClasses(pubClass, subClass)# both classes

exportMethods(
 ## for own generics:
 pubGenf, pubfn,
 ## for other generics:
 plot, show
)

## The "Matrix-like"
exportClasses("atomicVector", "array_or_vector")
exportClasses("M", "dM", "diagM", "ddiM") ## but *not* "mM" !
"#;
        let ns = PackageNamespace::parse(input).unwrap();
        assert_eq!(
            ns.imports_from,
            vec![("graphics".to_string(), "plot".to_string())]
        );
        assert_eq!(ns.exports, vec!["pubGenf", "pubfn", "assertError"]);
        assert_eq!(
            ns.export_classes,
            vec![
                "pubClass",
                "subClass",
                "atomicVector",
                "array_or_vector",
                "M",
                "dM",
                "diagM",
                "ddiM",
            ]
        );
        assert_eq!(ns.export_methods, vec!["pubGenf", "pubfn", "plot", "show"]);
    }

    #[test]
    fn parse_pkgd_namespace() {
        // Real NAMESPACE from tests/Pkgs/pkgD
        let input = "\
import(methods)

import(graphics)
## instead of just
## importFrom(\"graphics\", plot) # because we want to define methods on it
## *Still* do not want warning from this

## as \"mgcv\": this loads Matrix, but does not attach it  ==> Matrix methods \"semi-visible\"
importFrom(Matrix, t,mean,colMeans,colSums)

exportClasses(\"classA\", \"classApp\") # mother and sub-class R/pkgA.R

exportMethods(\"plot\")

export(nil)

export(search)# --> \"conflict message\"
";
        let ns = PackageNamespace::parse(input).unwrap();
        assert_eq!(ns.imports, vec!["methods", "graphics"]);
        assert_eq!(
            ns.imports_from,
            vec![
                ("Matrix".to_string(), "t".to_string()),
                ("Matrix".to_string(), "mean".to_string()),
                ("Matrix".to_string(), "colMeans".to_string()),
                ("Matrix".to_string(), "colSums".to_string()),
            ]
        );
        assert_eq!(ns.export_classes, vec!["classA", "classApp"]);
        assert_eq!(ns.export_methods, vec!["plot"]);
        assert_eq!(ns.exports, vec!["nil", "search"]);
    }

    #[test]
    fn unbalanced_parens_error() {
        let input = "export(foo, bar\n";
        let err = PackageNamespace::parse(input).unwrap_err();
        assert!(matches!(err, NamespaceError::UnbalancedParens { .. }));
    }

    #[test]
    fn unknown_directive_is_ignored() {
        // Unknown directives are now warnings (not errors) so packages
        // with new/uncommon directives can still load.
        let input = "frobnicate(foo)\nexport(bar)\n";
        let ns = PackageNamespace::parse(input).expect("should parse with unknown directive");
        assert_eq!(ns.exports, vec!["bar"]);
    }

    #[test]
    fn import_from_too_few_args() {
        let input = "importFrom(onlypkg)\n";
        let err = PackageNamespace::parse(input).unwrap_err();
        match err {
            NamespaceError::TooFewArgs {
                directive,
                expected,
                got,
                ..
            } => {
                assert_eq!(directive, "importFrom");
                assert_eq!(expected, 2);
                assert_eq!(got, 1);
            }
            _ => panic!("expected TooFewArgs error"),
        }
    }

    #[test]
    fn empty_namespace() {
        let input = "\n# just comments\n\n";
        let ns = PackageNamespace::parse(input).unwrap();
        assert_eq!(ns, PackageNamespace::default());
    }
}
