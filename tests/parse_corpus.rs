//! Recursive parse-corpus check for every `.R` file under `tests/`.
//!
//! Unlike the shell harness, this stays parse-only so it cannot hang on
//! runtime behavior while still catching grammar regressions.

use std::path::{Path, PathBuf};

use r::parser::parse_program;

/// Known parse failures in tests/ from GNU R's test suite.
/// These use syntax we haven't implemented yet (e.g. R 4.1+ features).
/// If this list shrinks, update the constant. If a new failure appears,
/// the test fails — that's a regression.
const KNOWN_PARSE_FAILURES: &[&str] = &[
    "reg-tests-1a.R",
    "reg-tests-1b.R",
    "reg-tests-1c.R",
    "reg-tests-1e.R",
    "reg-tests-2.R",
    "utf8-regex.R",
    "Pkgs/PR17859.1/R/f2.R",
    "Pkgs/PR17859.1/R/f3.R",
    "Pkgs/PR17859.2/R/f2.R",
    "Pkgs/PR17859.2/R/f3.R",
];

fn collect_r_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = std::fs::read_dir(dir).expect("failed to read test dir");
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_r_files(&path, out);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("R") {
            out.push(path);
        }
    }
}

fn read_source(path: &Path) -> Result<String, std::io::Error> {
    match std::fs::read_to_string(path) {
        Ok(source) => Ok(source),
        Err(err) if err.kind() == std::io::ErrorKind::InvalidData => {
            std::fs::read(path).map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        }
        Err(err) => Err(err),
    }
}

#[test]
fn test_corpus_parses_without_regressions() {
    let test_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");

    let mut files = Vec::new();
    collect_r_files(&test_dir, &mut files);
    files.sort();
    assert!(!files.is_empty(), "no .R files found in tests/");

    let mut passed = 0usize;
    let mut expected_failures = Vec::new();
    let mut unexpected_failures = Vec::new();

    for path in &files {
        let file = path
            .strip_prefix(&test_dir)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();
        match read_source(path).and_then(|source| {
            parse_program(&source)
                .map(|_| ())
                .map_err(std::io::Error::other)
        }) {
            Ok(()) => {
                passed += 1;
                // If this was a known failure that now passes, flag it
                if KNOWN_PARSE_FAILURES.contains(&file.as_str()) {
                    eprintln!("  FIXED: {file} now parses (remove from KNOWN_PARSE_FAILURES)");
                }
            }
            Err(_) => {
                if KNOWN_PARSE_FAILURES.contains(&file.as_str()) {
                    expected_failures.push(file);
                } else {
                    unexpected_failures.push(file);
                }
            }
        }
    }

    let total = files.len();
    eprintln!(
        "\n=== Parse Corpus: {total} files, {passed} passed, {} known failures, {} regressions ===",
        expected_failures.len(),
        unexpected_failures.len()
    );

    assert!(
        unexpected_failures.is_empty(),
        "parse regressions: {}",
        unexpected_failures.join(", ")
    );
}
