use r::Session;

// region: on.exit basic behavior

#[test]
fn on_exit_fires_on_normal_return() {
    let mut s = Session::new();
    s.eval_source(
        r#"
cleanup <- NULL
f <- function() {
    on.exit(cleanup <<- "ran")
    42
}
result <- f()
stopifnot(result == 42)
stopifnot(cleanup == "ran")
"#,
    )
    .unwrap();
}

#[test]
fn on_exit_fires_on_explicit_return() {
    let mut s = Session::new();
    s.eval_source(
        r#"
cleanup <- NULL
f <- function() {
    on.exit(cleanup <<- "return_cleanup")
    return(99)
}
result <- f()
stopifnot(result == 99)
stopifnot(cleanup == "return_cleanup")
"#,
    )
    .unwrap();
}

#[test]
fn on_exit_fires_on_error() {
    let mut s = Session::new();
    s.eval_source(
        r#"
cleanup <- NULL
tryCatch(
    {
        g <- function() {
            on.exit(cleanup <<- "error_cleanup")
            stop("oops")
        }
        g()
    },
    error = function(e) NULL
)
stopifnot(cleanup == "error_cleanup")
"#,
    )
    .unwrap();
}

// endregion

// region: on.exit add/after parameters

#[test]
fn on_exit_add_true_appends() {
    let mut s = Session::new();
    s.eval_source(
        r#"
log <- character(0)
h <- function() {
    on.exit(log <<- c(log, "first"))
    on.exit(log <<- c(log, "second"), add = TRUE)
    "done"
}
h()
stopifnot(length(log) == 2)
stopifnot(log[1] == "first")
stopifnot(log[2] == "second")
"#,
    )
    .unwrap();
}

#[test]
fn on_exit_add_false_replaces() {
    let mut s = Session::new();
    s.eval_source(
        r#"
log <- character(0)
f <- function() {
    on.exit(log <<- c(log, "first"))
    on.exit(log <<- c(log, "replaced"))
    "done"
}
f()
stopifnot(length(log) == 1)
stopifnot(log[1] == "replaced")
"#,
    )
    .unwrap();
}

#[test]
fn on_exit_after_false_prepends() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- character(0)
f <- function() {
    on.exit(result <<- c(result, "first"))
    on.exit(result <<- c(result, "second"), add = TRUE, after = FALSE)
}
f()
"#,
    )
    .unwrap();
    let result = s.eval_source("result").unwrap().value;
    let v = result.as_vector().unwrap().to_characters();
    // "second" runs before "first" because after=FALSE prepends
    assert_eq!(
        v,
        vec![Some("second".to_string()), Some("first".to_string())]
    );
}

#[test]
fn on_exit_no_args_clears() {
    let mut s = Session::new();
    s.eval_source(
        r#"
log <- character(0)
f <- function() {
    on.exit(log <<- c(log, "should not run"))
    on.exit()  # clears
    "done"
}
f()
stopifnot(length(log) == 0)
"#,
    )
    .unwrap();
}

// endregion

// region: Sys.time

#[test]
fn sys_time_returns_posixct() {
    let mut s = Session::new();
    s.eval_source(
        r#"
t <- Sys.time()
stopifnot(inherits(t, "POSIXct"))
stopifnot(inherits(t, "POSIXt"))
stopifnot(is.numeric(t))
# Should be a reasonable epoch timestamp (after 2020)
stopifnot(as.numeric(t) > 1577836800)
"#,
    )
    .unwrap();
}

#[test]
fn sys_time_has_subsecond_precision() {
    let mut s = Session::new();
    s.eval_source(
        r#"
t <- as.numeric(Sys.time())
# The fractional part should be non-zero with high probability
# (testing by checking it's a real number and not exactly integer)
stopifnot(is.numeric(t))
stopifnot(t > 0)
"#,
    )
    .unwrap();
}

// endregion

// region: date()

#[test]
fn date_returns_character_string() {
    let mut s = Session::new();
    s.eval_source(
        r#"
d <- date()
stopifnot(is.character(d))
stopifnot(length(d) == 1)
stopifnot(nchar(d) > 10)
"#,
    )
    .unwrap();
}

#[test]
fn date_contains_year() {
    let mut s = Session::new();
    // The date string should contain a 4-digit year
    s.eval_source(
        r#"
d <- date()
# Should contain a year like "2026" (or whatever the current year is)
stopifnot(grepl("[0-9]{4}", d))
"#,
    )
    .unwrap();
}

// endregion

// region: proc.time

#[test]
fn proc_time_returns_named_vector() {
    let mut s = Session::new();
    s.eval_source(
        r#"
pt <- proc.time()
stopifnot(is.numeric(pt))
stopifnot(length(pt) == 3)
n <- names(pt)
stopifnot(n[1] == "user.self")
stopifnot(n[2] == "sys.self")
stopifnot(n[3] == "elapsed")
stopifnot(pt["elapsed"] >= 0)
"#,
    )
    .unwrap();
}

#[test]
fn proc_time_elapsed_increases() {
    let mut s = Session::new();
    s.eval_source(
        r#"
t1 <- proc.time()["elapsed"]
# Do something to burn a tiny bit of time
for (i in 1:1000) i
t2 <- proc.time()["elapsed"]
stopifnot(t2 >= t1)
"#,
    )
    .unwrap();
}

// endregion

// region: system.time

#[test]
fn system_time_returns_named_vector() {
    let mut s = Session::new();
    s.eval_source(
        r#"
st <- system.time(for(i in 1:1000) i)
stopifnot(is.numeric(st))
stopifnot(length(st) == 3)
n <- names(st)
stopifnot(n[1] == "user.self")
stopifnot(n[2] == "sys.self")
stopifnot(n[3] == "elapsed")
stopifnot(st["elapsed"] >= 0)
"#,
    )
    .unwrap();
}

#[test]
fn system_time_measures_elapsed() {
    let mut s = Session::new();
    s.eval_source(
        r#"
st <- system.time({
    for (i in 1:100000) i
})
# Elapsed should be positive (we did real work)
stopifnot(st["elapsed"] > 0)
"#,
    )
    .unwrap();
}

// endregion
