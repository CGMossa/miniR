//! Equivalent of scripts/parse-test.sh — runs every .R file in tests/
//! through the Session API and asserts no parse regressions.

use std::path::Path;

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
    "simple-true.R",
];

#[test]
fn test_corpus_parses_without_regressions() {
    let test_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");

    let mut files: Vec<_> = std::fs::read_dir(test_dir)
        .expect("failed to read test dir")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("R"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no .R files found in tests/");

    let mut passed = 0usize;
    let mut expected_failures = Vec::new();
    let mut unexpected_failures = Vec::new();

    for path in &files {
        let file = path.file_name().unwrap().to_string_lossy().to_string();
        let mut session = r::Session::new();
        match session.eval_file(path) {
            Ok(_) | Err(r::SessionError::Runtime(_)) => {
                passed += 1;
                // If this was a known failure that now passes, flag it
                if KNOWN_PARSE_FAILURES.contains(&file.as_str()) {
                    eprintln!("  FIXED: {file} now parses (remove from KNOWN_PARSE_FAILURES)");
                }
            }
            Err(r::SessionError::Parse(_) | r::SessionError::CannotRead { .. }) => {
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
