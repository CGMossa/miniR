use r::parser::parse_program;

#[test]
fn parser_accepts_postfix_suffixes_on_following_lines() {
    parse_program(
        r#"
oexpr <- if (!is.null(stdout)) substitute({
  assign(
    ".__stdout__",
    as.environment("tools:callr")$`__callr_data__`$pxlib$
                                 set_stdout_file(`__fn__`),
    envir = as.environment("tools:callr")$`__callr_data__`)
}, list(`__fn__` = stdout))
"#,
    )
    .expect("parser should accept line breaks before postfix call chains");
}

#[test]
fn parser_accepts_line_break_after_unary_not() {
    parse_program(
        r#"
if(!
  identical(
    h$tar.rng.sub,
    h$cur.rng.sub - h$cur.rng.sub[1L] + h$tar.rng.sub[1L]
) )
  stop("Logic Error")
"#,
    )
    .expect("parser should accept line breaks after unary '!'");
}

#[test]
fn parser_accepts_empty_statements() {
    parse_program(
        r#"
if (is.na(curname)) {
  ;## do nothing
}
"#,
    )
    .expect("parser should allow empty statements separated by semicolons");
}

#[test]
fn parser_accepts_reserved_named_arguments_without_values() {
    parse_program("switch(typeof(x), NULL = , environment = FALSE, TRUE)")
        .expect("parser should accept reserved-word argument names with missing values");
}

#[test]
fn parser_accepts_pipe_placeholder_underscore() {
    parse_program("lapply(test, function(.test) anovax(test = .test)) |> do.call(rbind, args = _)")
        .expect("parser should accept '_' as a pipe placeholder symbol");
}

#[test]
fn parser_accepts_escaped_backticks_in_backtick_identifiers() {
    parse_program("`\\``").expect("parser should accept escaped backticks inside backtick names");
}
