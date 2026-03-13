use r::interpreter::value::{RValue, Vector};
use r::interpreter::{with_interpreter_state, Interpreter};
use r::parser::parse_program;

#[test]
fn interpreter_builtins_use_the_dispatched_interpreter_context() {
    let expr =
        parse_program("exists('marker', envir = globalenv())").expect("failed to parse test");
    let eval_interp = Interpreter::new();
    eval_interp.global_env.set(
        "marker".to_string(),
        RValue::vec(Vector::Logical(vec![Some(true)].into())),
    );

    let mut foreign_tls_interp = Interpreter::new();
    assert!(foreign_tls_interp.global_env.get("marker").is_none());

    let value = with_interpreter_state(&mut foreign_tls_interp, |_| eval_interp.eval(&expr))
        .expect("failed to evaluate under foreign TLS state");

    assert_eq!(
        value
            .as_vector()
            .and_then(|vector| vector.as_logical_scalar()),
        Some(true)
    );
}
