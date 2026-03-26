use std::env;
use std::path::Path;

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
                eprint_colored("Error: -e requires an expression argument\n");
                std::process::exit(1);
            }
            run_expr(&args[2]);
        } else if args[1] == "--generate-docs" {
            if args.len() < 3 {
                eprintln!("Error: --generate-docs requires a directory argument");
                std::process::exit(1);
            }
            generate_docs(&args[2]);
        } else {
            run_file(&args[1]);
        }
    } else {
        run_repl();
    }
}

/// Print an error message to stderr with red color when available.
fn eprint_colored(msg: &str) {
    #[cfg(feature = "repl")]
    {
        use crossterm::style::{Attribute, Color, Stylize};
        use std::io::{IsTerminal, Write};
        if std::io::stderr().is_terminal() {
            let styled = msg.with(Color::Red).attribute(Attribute::Bold);
            write!(std::io::stderr(), "{styled}").ok();
            return;
        }
    }
    eprint!("{}", msg);
}

fn generate_docs(dir: &str) {
    let path = Path::new(dir);
    match Session::generate_rd_docs(path) {
        Ok(count) => {
            println!("Generated {count} .Rd files in {dir}");
        }
        Err(e) => {
            eprintln!("Error generating docs: {e}");
            std::process::exit(1);
        }
    }
}

fn run_expr(source: &str) {
    let mut session = Session::new();
    session.install_signal_handler().ok(); // best-effort: REPL works without signals
    match session.eval_source(source) {
        Ok(result) => {
            if result.visible {
                session.auto_print(&result.value);
            }
        }
        Err(e) => {
            eprint_colored(&e.render());
            std::process::exit(1);
        }
    }
}

fn run_file(filename: &str) {
    let mut session = Session::new();
    session.install_signal_handler().ok();
    match session.eval_file(filename) {
        Ok(_) => {}
        Err(e) => {
            eprint_colored(&e.render());
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

    // When the `plot` feature is enabled, the REPL runs on a background thread
    // and the main thread runs the egui event loop (macOS requires GUI on main).
    // When `plot` is disabled, the REPL runs directly on the main thread.
    #[cfg(feature = "plot")]
    {
        let (tx, rx) = r::interpreter::graphics::egui_device::plot_channel();
        // Spawn REPL on background thread
        let repl_thread = std::thread::Builder::new()
            .name("repl".into())
            .spawn(move || {
                repl_loop(Some(tx));
            })
            .expect("failed to spawn REPL thread");

        // Run egui event loop on main thread (blocks until closed).
        // When the REPL thread exits (q()), the sender drops, and the
        // event loop will eventually exit too.
        r::interpreter::graphics::egui_device::run_plot_event_loop(rx).ok();

        // Wait for REPL thread to finish
        repl_thread.join().ok();
    }

    #[cfg(not(feature = "plot"))]
    {
        repl_loop(None::<()>);
    }
}

/// The actual REPL loop. `plot_tx` is the channel sender for sending plots
/// to the GUI thread (None when `plot` feature is off).
#[cfg(feature = "plot")]
fn repl_loop(plot_tx: Option<r::interpreter::graphics::egui_device::PlotSender>) {
    repl_loop_inner(plot_tx);
}

#[cfg(not(feature = "plot"))]
fn repl_loop<T>(_plot_tx: Option<T>) {
    repl_loop_inner(());
}

#[cfg(feature = "plot")]
fn repl_loop_inner(plot_tx: Option<r::interpreter::graphics::egui_device::PlotSender>) {
    let mut session = Session::new();
    if let Some(tx) = plot_tx {
        session.set_plot_sender(tx);
    }
    repl_main(&mut session);
}

#[cfg(not(feature = "plot"))]
fn repl_loop_inner(_: ()) {
    let mut session = Session::new();
    repl_main(&mut session);
}

fn repl_main(session: &mut Session) {
    session.install_signal_handler().ok();

    // Persistent history
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

    let hinter =
        Box::new(DefaultHinter::default().with_style(Style::new().italic().fg(Color::DarkGray)));

    let completer = Box::new(RCompleter::new());
    let completion_menu = Box::new(
        ColumnarMenu::default()
            .with_name("completion_menu")
            .with_columns(4)
            .with_column_padding(2),
    );

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
    session.sync_terminal_width();

    loop {
        session.sync_terminal_width();

        match line_editor.read_line(&prompt) {
            Ok(Signal::Success(buffer)) => {
                match session.eval_source(&buffer) {
                    Ok(result) => {
                        if result.visible {
                            session.auto_print(&result.value);
                        }
                    }
                    Err(e) => {
                        eprint_colored(&e.render());
                    }
                }
                // Auto-flush any accumulated grid graphics to the GUI window.
                r::interpreter::builtins::grid::flush_grid(session.interpreter());
                // Auto-flush any accumulated base plot to the GUI window.
                r::interpreter::builtins::graphics::flush_plot(session.interpreter());
            }
            Ok(Signal::CtrlC) => {
                println!();
            }
            Ok(Signal::CtrlD) => {
                println!();
                break;
            }
            Err(e) => {
                eprint_colored(&format!("Error: {}\n", e));
                break;
            }
        }
    }
}
