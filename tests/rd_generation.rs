//! Tests for .Rd documentation generation from builtin descriptors.

use r::interpreter::packages::rd::RdDoc;
use r::Session;

#[test]
fn generate_rd_docs_creates_files() {
    let tmp = temp_dir::TempDir::new().unwrap();
    let dir = tmp.path().join("man");

    let count = Session::generate_rd_docs(&dir).expect("generate_rd_docs should succeed");
    assert!(count > 0, "should generate at least one .Rd file");

    // Verify the directory exists and has .Rd files.
    let entries: Vec<_> = std::fs::read_dir(&dir)
        .expect("dir should exist")
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext == "Rd")
                .unwrap_or(false)
        })
        .collect();
    assert_eq!(entries.len(), count);
}

#[test]
fn generated_rd_contains_expected_sections() {
    let tmp = temp_dir::TempDir::new().unwrap();
    let dir = tmp.path().join("man");

    Session::generate_rd_docs(&dir).expect("generate_rd_docs should succeed");

    // paste is a well-documented builtin — check its Rd file.
    let paste_rd = dir.join("paste.Rd");
    if paste_rd.exists() {
        let content = std::fs::read_to_string(&paste_rd).expect("should read paste.Rd");
        assert!(
            content.contains("\\name{paste}"),
            "should have \\name section"
        );
        assert!(
            content.contains("\\alias{paste}"),
            "should have \\alias section"
        );
        assert!(content.contains("\\title{"), "should have \\title section");
        assert!(content.contains("\\usage{"), "should have \\usage section");
    }

    // Check that sum.Rd exists and has correct structure.
    let sum_rd = dir.join("sum.Rd");
    if sum_rd.exists() {
        let content = std::fs::read_to_string(&sum_rd).expect("should read sum.Rd");
        assert!(content.contains("\\name{sum}"));
        assert!(content.contains("\\alias{sum}"));
    }
}

#[test]
fn to_rd_escapes_special_characters() {
    let doc = RdDoc {
        name: Some("test_func".to_string()),
        aliases: vec!["test_func".to_string()],
        title: Some("Test 100% of {edge} cases".to_string()),
        description: Some("Handles \\backslash and {braces} and 50% signs.".to_string()),
        usage: Some("test_func(x, y = 10 %% 3)".to_string()),
        arguments: vec![
            ("x".to_string(), "a value with 100% coverage".to_string()),
            ("y".to_string(), "default uses {modulo}".to_string()),
        ],
        value: Some("A result with \\special chars".to_string()),
        ..Default::default()
    };

    let rd = doc.to_rd();

    // Title should have special chars escaped.
    assert!(
        rd.contains("\\title{Test 100\\% of \\{edge\\} cases}"),
        "title should escape %, {{, }}: got:\n{}",
        rd
    );

    // Description should have all special chars escaped.
    assert!(
        rd.contains("Handles \\\\backslash and \\{braces\\} and 50\\% signs."),
        "description should escape special chars: got:\n{}",
        rd
    );

    // Usage is R code — only % should be escaped.
    assert!(
        rd.contains("test_func(x, y = 10 \\%\\% 3)"),
        "usage should only escape %: got:\n{}",
        rd
    );

    // Argument descriptions should be escaped.
    assert!(
        rd.contains("a value with 100\\% coverage"),
        "arg desc should escape %: got:\n{}",
        rd
    );
    assert!(
        rd.contains("default uses \\{modulo\\}"),
        "arg desc should escape braces: got:\n{}",
        rd
    );

    // Value should escape backslash.
    assert!(
        rd.contains("A result with \\\\special chars"),
        "value should escape backslash: got:\n{}",
        rd
    );
}

#[test]
fn to_rd_roundtrips_through_parser() {
    // Build an RdDoc, serialize to Rd, then parse it back and verify fields match.
    let original = RdDoc {
        name: Some("roundtrip".to_string()),
        aliases: vec!["roundtrip".to_string(), "rt".to_string()],
        title: Some("Roundtrip Test".to_string()),
        description: Some("Tests serialization and parsing.".to_string()),
        usage: Some("roundtrip(x, y)".to_string()),
        arguments: vec![
            ("x".to_string(), "first argument".to_string()),
            ("y".to_string(), "second argument".to_string()),
        ],
        value: Some("A result value.".to_string()),
        ..Default::default()
    };

    let rd_text = original.to_rd();
    let parsed = RdDoc::parse(&rd_text).expect("generated Rd should parse successfully");

    assert_eq!(parsed.name, original.name);
    assert_eq!(parsed.title, original.title);
    assert_eq!(parsed.description, original.description);
    // The aliases should be present.
    assert!(parsed.aliases.contains(&"roundtrip".to_string()));
    assert!(parsed.aliases.contains(&"rt".to_string()));
    // Arguments.
    assert_eq!(parsed.arguments.len(), 2);
    assert_eq!(parsed.arguments[0].0, "x");
    assert_eq!(parsed.arguments[0].1, "first argument");
    assert_eq!(parsed.arguments[1].0, "y");
    assert_eq!(parsed.arguments[1].1, "second argument");
    // Value.
    assert_eq!(parsed.value, original.value);
}

#[test]
fn to_rd_empty_doc() {
    let doc = RdDoc::default();
    let rd = doc.to_rd();
    // An empty doc should produce an empty (or near-empty) string.
    assert!(
        rd.trim().is_empty(),
        "empty doc should produce empty Rd: got:\n{}",
        rd
    );
}

#[test]
fn to_rd_includes_keywords_and_seealso() {
    let doc = RdDoc {
        name: Some("myfunc".to_string()),
        aliases: vec!["myfunc".to_string()],
        title: Some("My Function".to_string()),
        keywords: vec!["math".to_string(), "utilities".to_string()],
        seealso: Some("otherfunc, anotherfunc".to_string()),
        ..Default::default()
    };

    let rd = doc.to_rd();
    assert!(rd.contains("\\keyword{math}"), "should include keywords");
    assert!(
        rd.contains("\\keyword{utilities}"),
        "should include all keywords"
    );
    assert!(rd.contains("\\seealso{"), "should include seealso section");
    assert!(
        rd.contains("otherfunc, anotherfunc"),
        "seealso content should be present"
    );
}

#[test]
fn generate_rd_docs_idempotent() {
    // Running twice to the same directory should produce the same count.
    let tmp = temp_dir::TempDir::new().unwrap();
    let dir = tmp.path().join("man");

    let count1 = Session::generate_rd_docs(&dir).expect("first run");
    let count2 = Session::generate_rd_docs(&dir).expect("second run");
    assert_eq!(count1, count2, "should be idempotent");
}

#[test]
fn generated_rd_files_parse_without_errors() {
    let tmp = temp_dir::TempDir::new().unwrap();
    let dir = tmp.path().join("man");

    Session::generate_rd_docs(&dir).expect("generate_rd_docs should succeed");

    let mut failures = Vec::new();
    for entry in std::fs::read_dir(&dir).expect("dir should exist").flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("Rd") {
            continue;
        }
        let content = std::fs::read_to_string(&path).expect("should read file");
        if let Err(e) = RdDoc::parse(&content) {
            failures.push(format!("{}: {e}", path.display()));
        }
    }

    assert!(
        failures.is_empty(),
        "Some generated .Rd files failed to parse:\n{}",
        failures.join("\n")
    );
}
