mod interpreter;
mod parser;

use std::env;
use std::fs;

use reedline::{DefaultPrompt, DefaultPromptSegment, Reedline, Signal};

use interpreter::with_interpreter;
use parser::{parse_program, ParseError};

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() > 1 {
        let filename = &args[1];
        run_file(filename);
    } else {
        run_repl();
    }
}

fn run_file(filename: &str) {
    // Try UTF-8 first, fall back to lossy conversion for Latin-1/other encodings
    let source = match fs::read_to_string(filename) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::InvalidData => match fs::read(filename) {
            Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
            Err(e2) => {
                eprintln!("Error reading file '{}': {}", filename, e2);
                std::process::exit(1);
            }
        },
        Err(e) => {
            eprintln!("Error reading file '{}': {}", filename, e);
            std::process::exit(1);
        }
    };

    match parse_program(&source) {
        Ok(ast) => match with_interpreter(|interp| interp.eval(&ast)) {
            Ok(_) => {}
            Err(e) => {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        },
        Err(mut e) => {
            e.filename = Some(filename.to_string());
            eprintln!("{}", e);
            std::process::exit(1);
        }
    }
}

fn run_repl() {
    println!(
        r#"
newr version 0.1.0 -- "Fresh Start"
An R interpreter written in Rust.
Type 'q()' to quit.
"#
    );

    let mut line_editor = Reedline::create();
    let mut buffer = String::new();

    loop {
        let current_prompt = if buffer.is_empty() {
            DefaultPrompt::new(
                DefaultPromptSegment::Basic("> ".to_string()),
                DefaultPromptSegment::Empty,
            )
        } else {
            DefaultPrompt::new(
                DefaultPromptSegment::Basic("+ ".to_string()),
                DefaultPromptSegment::Empty,
            )
        };

        match line_editor.read_line(&current_prompt) {
            Ok(Signal::Success(line)) => {
                if buffer.is_empty() {
                    buffer = line;
                } else {
                    buffer.push('\n');
                    buffer.push_str(&line);
                }

                match parse_program(&buffer) {
                    Ok(ast) => {
                        match with_interpreter(|interp| interp.eval(&ast)) {
                            Ok(val) => {
                                if !val.is_null() && !is_assignment_or_invisible(&buffer) {
                                    println!("{}", val);
                                }
                            }
                            Err(e) => {
                                eprintln!("{}", e);
                            }
                        }
                        buffer.clear();
                    }
                    Err(e) => {
                        if is_likely_incomplete(&buffer, &e) {
                            continue;
                        }
                        eprintln!("{}", e);
                        buffer.clear();
                    }
                }
            }
            Ok(Signal::CtrlD) | Ok(Signal::CtrlC) => {
                if !buffer.is_empty() {
                    buffer.clear();
                    println!();
                } else {
                    println!();
                    break;
                }
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                break;
            }
        }
    }
}

fn is_assignment_or_invisible(input: &str) -> bool {
    let trimmed = input.trim();
    trimmed.contains("<-")
        || trimmed.contains("<<-")
        || (trimmed.contains('=') && !trimmed.contains("==") && !trimmed.contains("!="))
        || trimmed.starts_with("for")
        || trimmed.starts_with("while")
        || trimmed.starts_with("if")
        || trimmed.starts_with("invisible")
}

fn is_likely_incomplete(input: &str, _error: &ParseError) -> bool {
    let mut open_parens = 0i32;
    let mut open_braces = 0i32;
    let mut open_brackets = 0i32;
    let mut in_string = false;
    let mut string_char = ' ';
    let mut prev_char = ' ';
    let mut in_comment = false;

    for c in input.chars() {
        if in_comment {
            if c == '\n' {
                in_comment = false;
            }
            prev_char = c;
            continue;
        }
        if in_string {
            if c == string_char && prev_char != '\\' {
                in_string = false;
            }
        } else {
            match c {
                '#' => in_comment = true,
                '"' | '\'' => {
                    in_string = true;
                    string_char = c;
                }
                '(' => open_parens += 1,
                ')' => open_parens -= 1,
                '{' => open_braces += 1,
                '}' => open_braces -= 1,
                '[' => open_brackets += 1,
                ']' => open_brackets -= 1,
                _ => {}
            }
        }
        prev_char = c;
    }

    if open_parens > 0 || open_braces > 0 || open_brackets > 0 || in_string {
        return true;
    }

    // Trailing binary operator means the expression continues
    let trimmed = input.trim_end();
    let trailing = trimmed
        .rfind(|c: char| !c.is_whitespace())
        .map(|i| &trimmed[i..])
        .unwrap_or("");
    if trailing.ends_with('+')
        || trailing.ends_with('*')
        || trailing.ends_with('/')
        || trailing.ends_with(',')
        || trailing.ends_with('|')
        || trailing.ends_with('&')
        || trailing.ends_with('~')
        || trailing.ends_with("<-")
        || trailing.ends_with("<<-")
        || trailing.ends_with("|>")
        || trailing.ends_with("||")
        || trailing.ends_with("&&")
    {
        return true;
    }

    // Trailing '-' that isn't part of '->' or '->>'
    if trailing.ends_with('-') && !trailing.ends_with("->") {
        return true;
    }

    false
}
