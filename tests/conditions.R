# Test R condition system: tryCatch, withCallingHandlers, suppressWarnings, etc.

# --- tryCatch with error ---
result <- tryCatch(
  stop("test error"),
  error = function(e) conditionMessage(e)
)
stopifnot(result == "test error")
cat("PASS: tryCatch catches error\n")

# --- tryCatch with warning handler ---
result <- tryCatch(
  {
    warning("test warning")
    42
  },
  warning = function(w) paste("caught:", conditionMessage(w))
)
stopifnot(result == "caught: test warning")
cat("PASS: tryCatch catches warning\n")

# --- tryCatch finally block ---
cleanup_ran <- FALSE
result <- tryCatch(
  42,
  finally = { cleanup_ran <- TRUE }
)
stopifnot(result == 42)
stopifnot(cleanup_ran)
cat("PASS: tryCatch finally block runs\n")

# --- tryCatch finally with error ---
cleanup2 <- FALSE
result2 <- tryCatch(
  stop("oops"),
  error = function(e) "recovered",
  finally = { cleanup2 <- TRUE }
)
stopifnot(result2 == "recovered")
stopifnot(cleanup2)
cat("PASS: tryCatch finally runs after error\n")

# --- simpleError constructor ---
e <- simpleError("my error")
stopifnot(conditionMessage(e) == "my error")
stopifnot(inherits(e, "error"))
stopifnot(inherits(e, "condition"))
cat("PASS: simpleError constructor\n")

# --- simpleWarning constructor ---
w <- simpleWarning("my warning")
stopifnot(conditionMessage(w) == "my warning")
stopifnot(inherits(w, "warning"))
stopifnot(inherits(w, "condition"))
cat("PASS: simpleWarning constructor\n")

# --- simpleMessage constructor ---
m <- simpleMessage("my message")
stopifnot(conditionMessage(m) == "my message")
stopifnot(inherits(m, "message"))
stopifnot(inherits(m, "condition"))
cat("PASS: simpleMessage constructor\n")

# --- withCallingHandlers (non-unwinding) ---
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
cat("PASS: withCallingHandlers collects warnings\n")

# --- suppressWarnings ---
result <- suppressWarnings({
  warning("ignored")
  42
})
stopifnot(result == 42)
cat("PASS: suppressWarnings\n")

# --- suppressMessages ---
result <- suppressMessages({
  message("ignored")
  42
})
stopifnot(result == 42)
cat("PASS: suppressMessages\n")

# --- stop() with condition object ---
cond <- simpleError("pre-made error")
result <- tryCatch(
  stop(cond),
  error = function(e) conditionMessage(e)
)
stopifnot(result == "pre-made error")
cat("PASS: stop() with condition object\n")

# --- invokeRestart for muffling messages ---
msgs <- character(0)
withCallingHandlers(
  message("hello"),
  message = function(m) {
    msgs <<- c(msgs, conditionMessage(m))
    invokeRestart("muffleMessage")
  }
)
stopifnot(length(msgs) == 1)
cat("PASS: invokeRestart muffleMessage\n")

# --- conditionCall returns NULL for our conditions ---
e <- simpleError("test")
stopifnot(is.null(conditionCall(e)))
cat("PASS: conditionCall\n")

# --- signalCondition ---
caught_signal <- FALSE
cond <- simpleWarning("sig test")
withCallingHandlers(
  signalCondition(cond),
  warning = function(w) {
    caught_signal <<- TRUE
    invokeRestart("muffleWarning")
  }
)
stopifnot(caught_signal)
cat("PASS: signalCondition triggers calling handlers\n")

# --- tryCatch with named expr argument ---
result <- tryCatch(expr = stop("boom"), error = function(e) "caught")
stopifnot(result == "caught")
cat("PASS: tryCatch with expr= named arg\n")

# --- warning() with condition object ---
w <- simpleWarning("pre-made warning")
caught_w <- NULL
withCallingHandlers(
  warning(w),
  warning = function(cond) {
    caught_w <<- conditionMessage(cond)
    invokeRestart("muffleWarning")
  }
)
stopifnot(caught_w == "pre-made warning")
cat("PASS: warning() with condition object\n")

# --- stop() with call.=FALSE ---
result <- tryCatch(
  stop("msg", call. = FALSE),
  error = function(e) conditionMessage(e)
)
stopifnot(result == "msg")
cat("PASS: stop() with call.=FALSE\n")

cat("\nAll condition system tests passed!\n")
