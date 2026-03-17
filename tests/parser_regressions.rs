use r::parser::parse_program;

#[test]
fn parser_accepts_multiline_named_parameters_and_arguments() {
    parse_program(
        r#"
f <- function(limit
              = 10, eps = 1/
              50) {
  plot(x, type
       = "l")
}
"#,
    )
    .expect("parser should accept named parameters and arguments split before '='");
}

#[test]
fn parser_accepts_line_break_before_infix_operator() {
    parse_program(
        r#"
if ((self$geom$check_constant_aes %||% TRUE)
    && length(aes_n) > 0 && n > 1) {
  NULL
}
"#,
    )
    .expect("parser should accept line continuations before infix operators");
}

#[test]
fn parser_accepts_data_table_and_rlang_walrus_forms() {
    parse_program(r#"mutate(data, "{name}" := !!dot, .keep = "none")"#)
        .expect("parser should accept walrus expressions used by tidyverse code");
}

#[test]
fn parser_distinguishes_dotdot_from_longer_identifiers() {
    parse_program("..2dge <- function(from) from")
        .expect("parser should not tokenize '..2dge' as '..2' followed by 'dge'");
}

#[test]
fn parser_accepts_plotmath_double_tilde() {
    parse_program(r#"substitute(k ~~ "clusters" ~~ C[j], list(k = k))"#)
        .expect("parser should accept plotmath double-tilde expressions");
}
