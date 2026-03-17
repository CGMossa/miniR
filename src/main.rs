use std::env;

use nu_ansi_term::{Color, Style};
use reedline::{
    default_emacs_keybindings, ColumnarMenu, DefaultHinter, EditCommand, Emacs, FileBackedHistory,
    KeyCode, KeyModifiers, MenuBuilder, Reedline, ReedlineEvent, ReedlineMenu, Signal,
};

use r::repl::{RCompleter, RHighlighter, RPrompt, RValidator};
use r::Session;

fn main() {
    r::init_logging();

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
    let mut session = Session::new();
    let _ = session.install_signal_handler();
    match session.eval_source(source) {
        Ok(result) => {
            if result.visible {
                println!("{}", result.value);
            }
        }
        Err(e) => {
            eprint!("{}", e.render());
            std::process::exit(1);
        }
    }
}

fn run_file(filename: &str) {
    let mut session = Session::new();
    let _ = session.install_signal_handler();
    match session.eval_file(filename) {
        Ok(_) => {}
        Err(e) => {
            eprint!("{}", e.render());
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
    let history: Box<dyn reedline::History> = match FileBackedHistory::with_file(1000, history_path)
    {
        Ok(h) => Box::new(h),
        Err(e) => {
            eprintln!("Warning: could not open history file: {e}");
            Box::new(FileBackedHistory::new(1000).expect("in-memory history"))
        }
    };

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
    let mut session = Session::new();
    let _ = session.install_signal_handler();

    loop {
        match line_editor.read_line(&prompt) {
            Ok(Signal::Success(buffer)) => match session.eval_source(&buffer) {
                Ok(result) => {
                    if result.visible {
                        println!("{}", result.value);
                    }
                }
                Err(e) => {
                    // Just print the error — interrupt errors display as
                    // "Interrupted" via their Display impl, no special case needed.
                    // Parse errors use miette rendering when `diagnostics` is on.
                    eprint!("{}", e.render());
                }
            },
            Ok(Signal::CtrlC) => {
                // Ctrl+C while waiting for input — print a blank line and
                // show a new prompt (like R does).
                println!();
            }
            Ok(Signal::CtrlD) => {
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
