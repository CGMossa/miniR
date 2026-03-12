mod interpreter;
mod parser;
mod repl;

use std::env;
use std::fs;

use nu_ansi_term::{Color, Style};
use reedline::{
    default_emacs_keybindings, ColumnarMenu, DefaultHinter, EditCommand, Emacs, FileBackedHistory,
    KeyCode, KeyModifiers, MenuBuilder, Reedline, ReedlineEvent, ReedlineMenu, Signal,
};

use interpreter::with_interpreter;
use parser::ast::Expr;
use parser::parse_program;
use repl::{RCompleter, RHighlighter, RPrompt, RValidator};

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() > 1 {
        if args[1] == "-e" {
            if args.len() < 3 {
                eprintln!("Error: -e requires an expression argument");
                std::process::exit(1);
            }
            run_expr(&args[2]);
        } else {
            run_file(&args[1]);
        }
    } else {
        run_repl();
    }
}

fn run_expr(source: &str) {
    match parse_program(source) {
        Ok(ast) => match with_interpreter(|interp| interp.eval(&ast)) {
            Ok(val) => {
                if !val.is_null() && !is_invisible_result(&ast) {
                    println!("{}", val);
                }
            }
            Err(e) => {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        },
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
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
miniR version 0.1.0 -- "Fresh Start"
An R interpreter written in Rust.
Type 'q()' to quit.
"#
    );

    // Persistent history (~/.miniR_history, last 1000 entries)
    let history_path = env::var("MINIR_HISTFILE")
        .or_else(|_| env::var("HOME").map(|h| format!("{h}/.miniR_history")))
        .unwrap_or_else(|_| ".miniR_history".to_string())
        .into();
    let history = Box::new(
        FileBackedHistory::with_file(1000, history_path).expect("Error configuring history file"),
    );

    // Fish-style history hints (gray italic)
    let hinter =
        Box::new(DefaultHinter::default().with_style(Style::new().italic().fg(Color::DarkGray)));

    // Tab completion with columnar menu
    let completer = Box::new(RCompleter::new());
    let completion_menu = Box::new(
        ColumnarMenu::default()
            .with_name("completion_menu")
            .with_columns(4)
            .with_column_padding(2),
    );

    // Emacs keybindings + Tab for completion menu
    let mut keybindings = default_emacs_keybindings();
    keybindings.add_binding(
        KeyModifiers::NONE,
        KeyCode::Tab,
        ReedlineEvent::UntilFound(vec![
            ReedlineEvent::Menu("completion_menu".to_string()),
            ReedlineEvent::MenuNext,
        ]),
    );
    keybindings.add_binding(
        KeyModifiers::ALT,
        KeyCode::Enter,
        ReedlineEvent::Edit(vec![EditCommand::InsertNewline]),
    );
    let edit_mode = Box::new(Emacs::new(keybindings));

    let mut line_editor = Reedline::create()
        .with_history(history)
        .with_hinter(hinter)
        .with_highlighter(Box::new(RHighlighter))
        .with_validator(Box::new(RValidator))
        .with_completer(completer)
        .with_menu(ReedlineMenu::EngineCompleter(completion_menu))
        .with_edit_mode(edit_mode);

    let prompt = RPrompt;

    loop {
        match line_editor.read_line(&prompt) {
            Ok(Signal::Success(buffer)) => {
                // The validator already ensures we only get complete expressions
                match parse_program(&buffer) {
                    Ok(ast) => match with_interpreter(|interp| interp.eval(&ast)) {
                        Ok(val) => {
                            if !val.is_null() && !is_invisible_result(&ast) {
                                println!("{}", val);
                            }
                        }
                        Err(e) => {
                            eprintln!("{}", e);
                        }
                    },
                    Err(e) => {
                        eprintln!("{}", e);
                    }
                }
            }
            Ok(Signal::CtrlD) | Ok(Signal::CtrlC) => {
                println!();
                break;
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                break;
            }
        }
    }
}

fn is_invisible_result(ast: &Expr) -> bool {
    match ast {
        Expr::Assign { .. } => true,
        Expr::For { .. } => true,
        Expr::While { .. } => true,
        Expr::Repeat { .. } => true,
        Expr::Call { func, .. } => {
            matches!(func.as_ref(), Expr::Symbol(name) if name == "invisible")
        }
        Expr::Program(exprs) => exprs.last().is_some_and(is_invisible_result),
        Expr::Block(exprs) => exprs.last().is_some_and(is_invisible_result),
        _ => false,
    }
}
