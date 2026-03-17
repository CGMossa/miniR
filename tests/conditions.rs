use r::Session;

// region: tryCatch

#[test]
fn try_catch_catches_error() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- tryCatch(
  stop("test error"),
  error = function(e) conditionMessage(e)
)
stopifnot(result == "test error")
"#,
    )
    .unwrap();
}

#[test]
fn try_catch_catches_warning() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- tryCatch(
  { warning("test warning"); 42 },
  warning = function(w) paste("caught:", conditionMessage(w))
)
stopifnot(result == "caught: test warning")
"#,
    )
    .unwrap();
}

#[test]
fn try_catch_catches_message() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- tryCatch(
  { message("hello"); 42 },
  message = function(m) paste("got:", conditionMessage(m))
)
stopifnot(result == "got: hello")
"#,
    )
    .unwrap();
}

#[test]
fn try_catch_finally_runs_on_success() {
    let mut s = Session::new();
    s.eval_source(
        r#"
cleanup <- FALSE
result <- tryCatch(
  42,
  finally = { cleanup <- TRUE }
)
stopifnot(result == 42)
stopifnot(cleanup)
"#,
    )
    .unwrap();
}

#[test]
fn try_catch_finally_runs_on_error() {
    let mut s = Session::new();
    s.eval_source(
        r#"
cleanup <- FALSE
result <- tryCatch(
  stop("oops"),
  error = function(e) "recovered",
  finally = { cleanup <- TRUE }
)
stopifnot(result == "recovered")
stopifnot(cleanup)
"#,
    )
    .unwrap();
}

#[test]
fn try_catch_returns_expr_value_when_no_condition() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- tryCatch(42 + 1)
stopifnot(result == 43)
"#,
    )
    .unwrap();
}

#[test]
fn try_catch_multiple_handlers() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Error handler is chosen when stop() is called
r1 <- tryCatch(
  stop("boom"),
  warning = function(w) "warning",
  error = function(e) "error"
)
stopifnot(r1 == "error")

# Warning handler is chosen when warning() is called
r2 <- tryCatch(
  { warning("oops"); 42 },
  warning = function(w) "warning",
  error = function(e) "error"
)
stopifnot(r2 == "warning")
"#,
    )
    .unwrap();
}

#[test]
fn try_catch_with_named_expr_arg() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# tryCatch(expr = ..., error = ...) should work
result <- tryCatch(expr = stop("boom"), error = function(e) "caught")
stopifnot(result == "caught")
"#,
    )
    .unwrap();
}

#[test]
fn try_catch_nested() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- tryCatch(
  tryCatch(
    stop("inner"),
    warning = function(w) "wrong"
  ),
  error = function(e) paste("outer:", conditionMessage(e))
)
stopifnot(result == "outer: inner")
"#,
    )
    .unwrap();
}

#[test]
fn try_catch_stop_with_condition_object() {
    let mut s = Session::new();
    s.eval_source(
        r#"
cond <- simpleError("pre-made error")
result <- tryCatch(
  stop(cond),
  error = function(e) conditionMessage(e)
)
stopifnot(result == "pre-made error")
"#,
    )
    .unwrap();
}

#[test]
fn try_catch_custom_condition_class() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Create a custom condition and catch it by class
make_custom <- function(msg) {
  cond <- simpleCondition(msg, class = "myCustom")
  # Add extra classes to make it catchable
  cond
}

# Custom conditions signaled via stop() get caught by error handler
custom_cond <- simpleError("custom msg")
result <- tryCatch(
  stop(custom_cond),
  simpleError = function(e) paste("custom:", conditionMessage(e))
)
stopifnot(result == "custom: custom msg")
"#,
    )
    .unwrap();
}

// endregion

// region: withCallingHandlers

#[test]
fn with_calling_handlers_collects_warnings() {
    let mut s = Session::new();
    s.eval_source(
        r#"
log <- character(0)
result <- withCallingHandlers(
  {
    warning("w1")
    warning("w2")
    "done"
  },
  warning = function(w) {
    log <<- c(log, conditionMessage(w))
    invokeRestart("muffleWarning")
  }
)
stopifnot(result == "done")
stopifnot(length(log) == 2)
stopifnot(log[1] == "w1")
stopifnot(log[2] == "w2")
"#,
    )
    .unwrap();
}

#[test]
fn with_calling_handlers_does_not_unwind() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# withCallingHandlers doesn't unwind — code after warning() continues
counter <- 0
result <- withCallingHandlers(
  {
    warning("w1")
    counter <- counter + 1
    warning("w2")
    counter <- counter + 1
    counter
  },
  warning = function(w) {
    invokeRestart("muffleWarning")
  }
)
stopifnot(result == 2)
"#,
    )
    .unwrap();
}

#[test]
fn with_calling_handlers_with_named_expr() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- withCallingHandlers(
  expr = { warning("w"); "ok" },
  warning = function(w) invokeRestart("muffleWarning")
)
stopifnot(result == "ok")
"#,
    )
    .unwrap();
}

#[test]
fn with_calling_handlers_messages() {
    let mut s = Session::new();
    s.eval_source(
        r#"
msgs <- character(0)
withCallingHandlers(
  message("hello"),
  message = function(m) {
    msgs <<- c(msgs, conditionMessage(m))
    invokeRestart("muffleMessage")
  }
)
stopifnot(length(msgs) == 1)
stopifnot(msgs[1] == "hello")
"#,
    )
    .unwrap();
}

// endregion

// region: suppressWarnings / suppressMessages

#[test]
fn suppress_warnings_silences_warnings() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- suppressWarnings({
  warning("ignored")
  42
})
stopifnot(result == 42)
"#,
    )
    .unwrap();
}

#[test]
fn suppress_messages_silences_messages() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- suppressMessages({
  message("ignored")
  42
})
stopifnot(result == 42)
"#,
    )
    .unwrap();
}

#[test]
fn suppress_warnings_does_not_suppress_errors() {
    let mut s = Session::new();
    let result = s.eval_source(
        r#"
suppressWarnings(stop("this should propagate"))
"#,
    );
    assert!(result.is_err());
}

#[test]
fn suppress_messages_does_not_suppress_errors() {
    let mut s = Session::new();
    let result = s.eval_source(
        r#"
suppressMessages(stop("this should propagate"))
"#,
    );
    assert!(result.is_err());
}

// endregion

// region: Condition constructors and accessors

#[test]
fn simple_error_constructor() {
    let mut s = Session::new();
    s.eval_source(
        r#"
e <- simpleError("my error")
stopifnot(conditionMessage(e) == "my error")
stopifnot(inherits(e, "error"))
stopifnot(inherits(e, "condition"))
stopifnot(inherits(e, "simpleError"))
"#,
    )
    .unwrap();
}

#[test]
fn simple_warning_constructor() {
    let mut s = Session::new();
    s.eval_source(
        r#"
w <- simpleWarning("my warning")
stopifnot(conditionMessage(w) == "my warning")
stopifnot(inherits(w, "warning"))
stopifnot(inherits(w, "condition"))
stopifnot(inherits(w, "simpleWarning"))
"#,
    )
    .unwrap();
}

#[test]
fn simple_message_constructor() {
    let mut s = Session::new();
    s.eval_source(
        r#"
m <- simpleMessage("my message")
stopifnot(conditionMessage(m) == "my message")
stopifnot(inherits(m, "message"))
stopifnot(inherits(m, "condition"))
stopifnot(inherits(m, "simpleMessage"))
"#,
    )
    .unwrap();
}

#[test]
fn simple_condition_constructor() {
    let mut s = Session::new();
    s.eval_source(
        r#"
c <- simpleCondition("test", class = "myClass")
stopifnot(conditionMessage(c) == "test")
stopifnot(inherits(c, "condition"))
stopifnot(inherits(c, "myClass"))
"#,
    )
    .unwrap();
}

#[test]
fn condition_call_returns_null_for_simple() {
    let mut s = Session::new();
    s.eval_source(
        r#"
e <- simpleError("test")
stopifnot(is.null(conditionCall(e)))
"#,
    )
    .unwrap();
}

// endregion

// region: signalCondition

#[test]
fn signal_condition_triggers_calling_handlers() {
    let mut s = Session::new();
    s.eval_source(
        r#"
caught <- FALSE
cond <- simpleWarning("sig test")
withCallingHandlers(
  signalCondition(cond),
  warning = function(w) {
    caught <<- TRUE
    invokeRestart("muffleWarning")
  }
)
stopifnot(caught)
"#,
    )
    .unwrap();
}

// endregion

// region: stop / warning / message edge cases

#[test]
fn stop_with_call_false() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# stop(call. = FALSE) should not error on the named argument
result <- tryCatch(
  stop("msg", call. = FALSE),
  error = function(e) conditionMessage(e)
)
stopifnot(result == "msg")
"#,
    )
    .unwrap();
}

#[test]
fn warning_with_condition_object() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# warning() with a condition object should signal that condition
w <- simpleWarning("pre-made warning")
caught <- NULL
withCallingHandlers(
  warning(w),
  warning = function(cond) {
    caught <<- conditionMessage(cond)
    invokeRestart("muffleWarning")
  }
)
stopifnot(caught == "pre-made warning")
"#,
    )
    .unwrap();
}

#[test]
fn message_concatenates_args() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# message() should concatenate multiple arguments
result <- tryCatch(
  { message("hello", " world"); 42 },
  message = function(m) conditionMessage(m)
)
stopifnot(result == "hello world")
"#,
    )
    .unwrap();
}

#[test]
fn try_idiomatic_pattern() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# try() should catch errors and return the error message
result <- try(42 + 1)
stopifnot(result == 43)
"#,
    )
    .unwrap();
}

// endregion

// region: Integration patterns common in CRAN

#[test]
fn cran_pattern_try_catch_msg_helper() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# tryCatch with conditionMessage as the handler function directly
# (Note: in GNU R, tryCatch(stop("e1"), error=conditionMessage) works
# because the argument to the wrapper is lazily evaluated. Without
# promises, we use the expression directly in tryCatch.)
result <- tryCatch(stop("e1"), error = conditionMessage)
stopifnot(result == "e1")

result2 <- tryCatch({ warning("w1"); 42 }, warning = conditionMessage)
stopifnot(result2 == "w1")
"#,
    )
    .unwrap();
}

#[test]
fn cran_pattern_with_calling_handlers_in_try_catch() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Pattern: collect warnings while catching errors
warnings_log <- character(0)
result <- tryCatch(
  withCallingHandlers(
    {
      warning("w1")
      warning("w2")
      42
    },
    warning = function(w) {
      warnings_log <<- c(warnings_log, conditionMessage(w))
      invokeRestart("muffleWarning")
    }
  ),
  error = function(e) conditionMessage(e)
)
stopifnot(result == 42)
stopifnot(length(warnings_log) == 2)
"#,
    )
    .unwrap();
}

#[test]
fn cran_pattern_nested_try_catch_with_warnings() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Inner tryCatch catches the warning, outer tryCatch is not triggered
result <- tryCatch(
  tryCatch(
    { warning("caught inner"); 99 },
    warning = function(w) paste("inner:", conditionMessage(w))
  ),
  error = function(e) "outer error"
)
stopifnot(result == "inner: caught inner")
"#,
    )
    .unwrap();
}

// endregion
