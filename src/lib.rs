pub mod interpreter;
pub mod parser;
pub mod repl;
pub mod session;

pub use session::{is_invisible_result, EvalOutput, Session, SessionError};

/// Initialize logging from the `MINIR_LOG` environment variable.
///
/// When the `logging` feature is enabled, this configures `env_logger` to read
/// the log level from `MINIR_LOG` (e.g. `MINIR_LOG=debug`). When the feature
/// is disabled, this is a no-op — `log` macros compile to nothing.
///
/// # Examples
///
/// ```bash
/// MINIR_LOG=debug cargo run -- -e '1+1'   # shows evaluation trace
/// MINIR_LOG=trace cargo run -- -e '1+1'   # shows all trace-level detail
/// ```
pub fn init_logging() {
    #[cfg(feature = "logging")]
    {
        use std::env;
        use std::sync::Once;

        static INIT_LOGGER: Once = Once::new();

        if env::var("MINIR_LOG").is_ok() {
            INIT_LOGGER.call_once(|| {
                let _ = env_logger::Builder::new()
                    .parse_env("MINIR_LOG")
                    .format_timestamp(None)
                    .format_target(false)
                    .try_init();
            });
        }
    }
}

#[cfg(all(test, feature = "logging"))]
mod tests {
    use super::init_logging;

    #[test]
    fn init_logging_is_idempotent() {
        std::env::set_var("MINIR_LOG", "debug");
        init_logging();
        init_logging();
    }
}
