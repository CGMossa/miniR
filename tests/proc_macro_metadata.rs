use r::{Session, SessionError};

fn runtime_error(source: &str) -> String {
    let err = Session::new()
        .eval_source(source)
        .expect_err("script unexpectedly succeeded");

    match err {
        SessionError::Runtime(flow) => flow.to_string(),
        other => panic!("unexpected error: {other}"),
    }
}

#[test]
fn builtin_aliases_are_registered_from_descriptor_metadata() {
    let mut session = Session::new();
    let value = session
        .eval_source(
            r#"
e <- new.env()
evalq({ beta <- 1L; alpha <- 2L }, e)
objects(envir = e)
"#,
        )
        .expect("failed to evaluate alias script")
        .value;

    assert_eq!(
        value.as_vector().map(|vector| vector.to_characters()),
        Some(vec![Some("alpha".to_string()), Some("beta".to_string())]),
    );
}

#[test]
fn builtin_max_args_metadata_rejects_extra_arguments() {
    let err = runtime_error("globalenv(1)");
    assert!(
        err.contains("globalenv() takes no arguments"),
        "unexpected error: {err}"
    );
}

#[test]
fn stub_builtin_reports_explicit_unimplemented_errors() {
    let err = runtime_error("arity(1)");
    assert!(
        err.contains("arity() is not implemented yet"),
        "unexpected error: {err}"
    );
}
