pub mod interpreter;
pub mod parser;
#[cfg(feature = "repl")]
pub mod repl;
pub mod session;

pub use session::{is_invisible_result, EvalOutput, Session, SessionError};

/// Initialize logging and tracing from the `MINIR_LOG` environment variable.
///
/// When the `tracing-output` feature is enabled (default), this configures
/// `tracing-subscriber` with an env filter read from `MINIR_LOG` (e.g.
/// `MINIR_LOG=debug`). The tracing subscriber also bridges `log` records, so
/// existing `log::*` calls are captured too.
///
/// When only the `logging` feature is enabled, this configures `env_logger`
/// as a fallback. When neither feature is enabled, this is a no-op.
///
/// # Examples
///
/// ```bash
/// MINIR_LOG=debug cargo run -- -e '1+1'   # shows evaluation trace
/// MINIR_LOG=trace cargo run -- -e '1+1'   # shows all trace-level detail
/// MINIR_LOG=r=debug cargo run -- -e '1+1' # only miniR debug output
/// ```
pub fn init_logging() {
    #[cfg(feature = "tracing-output")]
    {
        use std::sync::Once;
        static INIT: Once = Once::new();
        use std::env;
        if env::var("MINIR_LOG").is_ok() {
            INIT.call_once(|| {
                use tracing_subscriber::EnvFilter;
                let filter =
                    EnvFilter::try_from_env("MINIR_LOG").unwrap_or_else(|_| EnvFilter::new("info"));
                tracing_subscriber::fmt()
                    .with_env_filter(filter)
                    .without_time()
                    .with_target(false)
                    .init();
            });
        }
    }

    #[cfg(all(feature = "logging", not(feature = "tracing-output")))]
    {
        use std::sync::Once;
        static INIT: Once = Once::new();
        use std::env;
        if env::var("MINIR_LOG").is_ok() {
            INIT.call_once(|| {
                let _ = env_logger::Builder::new()
                    .parse_env("MINIR_LOG")
                    .format_timestamp(None)
                    .format_target(false)
                    .try_init();
            });
        }
    }
}

#[cfg(all(test, any(feature = "logging", feature = "tracing-output")))]
mod tests {
    use super::init_logging;

    #[test]
    fn init_logging_is_idempotent() {
        std::env::set_var("MINIR_LOG", "debug");
        init_logging();
        init_logging();
    }
}
