//! Recursive parse-corpus check for `.R` files under `tests/` plus `cran/**/R/`.
//!
//! Unlike the shell harness, this stays parse-only so it cannot hang on
//! runtime behavior while still catching grammar regressions.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use r::parser::parse_program;

/// Known parse failures, stored as repo-relative paths one per line.
const KNOWN_PARSE_FAILURES: &str = include_str!("parse_corpus_known_failures.txt");

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

fn collect_cran_r_files(cran_dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = std::fs::read_dir(cran_dir).expect("failed to read cran dir");
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().and_then(|name| name.to_str()) == Some("R") {
                collect_r_files(&path, out);
            } else {
                collect_cran_r_files(&path, out);
            }
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
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let test_dir = repo_root.join("tests");
    let cran_dir = repo_root.join("cran");

    let mut files = Vec::new();
    collect_r_files(&test_dir, &mut files);
    if cran_dir.is_dir() {
        collect_cran_r_files(&cran_dir, &mut files);
    }
    files.sort();
    assert!(
        !files.is_empty(),
        "no .R files found in tests/ or cran/**/R/"
    );

    let mut passed = 0usize;
    let mut expected_failures = Vec::new();
    let mut unexpected_failures = Vec::new();
    let known_failures: HashSet<&str> = KNOWN_PARSE_FAILURES
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();

    for path in &files {
        let file = path
            .strip_prefix(repo_root)
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
                if known_failures.contains(file.as_str()) {
                    eprintln!("  FIXED: {file} now parses (remove from KNOWN_PARSE_FAILURES)");
                }
            }
            Err(_) => {
                if known_failures.contains(file.as_str()) {
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
