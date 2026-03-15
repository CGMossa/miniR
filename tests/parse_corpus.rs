//! Equivalent of scripts/parse-test.sh — runs every .R file in tests/
//! through the Session API and asserts zero panics.

use std::path::Path;

#[test]
fn test_corpus_parses_without_crashes() {
    let test_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");

    let mut files: Vec<_> = std::fs::read_dir(test_dir)
        .expect("failed to read test dir")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("R"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no .R files found in tests/");

    let mut passed = 0usize;
    let mut parse_failures = 0usize;
    let mut runtime_errors = 0usize;

    for path in &files {
        let mut session = r::Session::new();
        match session.eval_file(path) {
            Ok(_) => passed += 1,
            Err(r::SessionError::Parse(_)) => parse_failures += 1,
            Err(r::SessionError::Runtime(_)) => {
                // Runtime errors are expected — missing builtins, etc.
                passed += 1;
                runtime_errors += 1;
            }
            Err(r::SessionError::CannotRead { .. }) => parse_failures += 1,
        }
    }

    let total = files.len();
    eprintln!(
        "\n=== Parse Corpus: {total} files, {passed} ran ({}%), {parse_failures} parse failures, {runtime_errors} runtime errors ===",
        passed * 100 / total
    );
}
