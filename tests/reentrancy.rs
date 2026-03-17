//! Tests that the interpreter is reentrant: multiple sessions can coexist,
//! nested eval works (TLS shared borrows are re-entrant), and parallel
//! threads get isolated state.

use r::Session;

#[test]
fn same_thread_sessions_are_isolated() {
    let mut s1 = Session::new();
    let mut s2 = Session::new();

    s1.eval_source("x <- 111L").unwrap();
    s2.eval_source("x <- 222L").unwrap();

    let v1 = s1.eval_source("x").unwrap().value;
    let v2 = s2.eval_source("x").unwrap().value;

    assert_eq!(
        v1.as_vector().and_then(|v| v.as_integer_scalar()),
        Some(111)
    );
    assert_eq!(
        v2.as_vector().and_then(|v| v.as_integer_scalar()),
        Some(222)
    );
}

#[test]
fn nested_eval_is_reentrant() {
    // eval(parse(text=...)) triggers with_interpreter inside with_interpreter
    let mut s = Session::new();
    let result = s
        .eval_source(r#"eval(parse(text = "1 + 2"))"#)
        .unwrap()
        .value;
    assert_eq!(
        result.as_vector().and_then(|v| v.as_double_scalar()),
        Some(3.0)
    );
}

#[test]
fn deeply_nested_eval() {
    // Three levels of eval nesting
    let mut s = Session::new();
    let result = s
        .eval_source(r#"eval(parse(text = 'eval(parse(text = "10 * 5"))'))"#)
        .unwrap()
        .value;
    assert_eq!(
        result.as_vector().and_then(|v| v.as_double_scalar()),
        Some(50.0)
    );
}

#[test]
fn trycatch_reentrancy() {
    // tryCatch with eval inside — exercises condition handling + nested eval
    let mut s = Session::new();
    let result = s
        .eval_source(r#"tryCatch(eval(parse(text = "42L")), error = function(e) -1L)"#)
        .unwrap()
        .value;
    assert_eq!(
        result.as_vector().and_then(|v| v.as_integer_scalar()),
        Some(42)
    );
}

#[test]
fn parallel_threads_are_isolated() {
    // RValue is !Send (Rc-based environments), so extract scalars inside each thread.
    let t1 = std::thread::spawn(|| {
        let mut s = Session::new();
        s.eval_source("x <- 1L").unwrap();
        for i in 2..=100 {
            s.eval_source(&format!("x <- x + {i}L")).unwrap();
        }
        s.eval_source("x")
            .unwrap()
            .value
            .as_vector()
            .and_then(|v| v.as_integer_scalar())
    });

    let t2 = std::thread::spawn(|| {
        let mut s = Session::new();
        s.eval_source("x <- 1000L").unwrap();
        for i in 1..=100 {
            s.eval_source(&format!("x <- x - {i}L")).unwrap();
        }
        s.eval_source("x")
            .unwrap()
            .value
            .as_vector()
            .and_then(|v| v.as_integer_scalar())
    });

    // Thread 1: 1 + 2 + 3 + ... + 100 = 5050
    assert_eq!(t1.join().unwrap(), Some(5050));
    // Thread 2: 1000 - (1 + 2 + ... + 100) = 1000 - 5050 = -4050
    assert_eq!(t2.join().unwrap(), Some(-4050));
}

#[test]
fn same_thread_sessions_isolate_env_and_cwd_state() {
    let mut s1 = Session::new();
    let mut s2 = Session::new();

    s1.eval_source(
        r#"
        base <- tempfile()
        dir.create(base)
        Sys.setenv(TZ = "Pacific/Honolulu", HOME = "/tmp/minir-home-one", USER = "user-one", LANG = "en_DK.UTF-8")
        setwd(base)
        writeLines("one", "marker.txt")
    "#,
    )
    .unwrap();

    s2.eval_source(
        r#"
        base <- tempfile()
        dir.create(base)
        Sys.setenv(TZ = "Europe/Copenhagen", HOME = "/tmp/minir-home-two", USER = "user-two", LANG = "da_DK.UTF-8")
        setwd(base)
        writeLines("two", "marker.txt")
    "#,
    )
    .unwrap();

    s1.eval_source(
        r#"
        stopifnot(Sys.timezone() == "Pacific/Honolulu")
        stopifnot(path.expand("~") == "/tmp/minir-home-one")
        stopifnot(Sys.info()[["user"]] == "user-one")
        stopifnot(sessionInfo()[["locale"]] == "en_DK.UTF-8")
        stopifnot(file.exists("marker.txt"))
        stopifnot(readLines("marker.txt")[1] == "one")
    "#,
    )
    .unwrap();

    s2.eval_source(
        r#"
        stopifnot(Sys.timezone() == "Europe/Copenhagen")
        stopifnot(path.expand("~") == "/tmp/minir-home-two")
        stopifnot(Sys.info()[["user"]] == "user-two")
        stopifnot(sessionInfo()[["locale"]] == "da_DK.UTF-8")
        stopifnot(file.exists("marker.txt"))
        stopifnot(readLines("marker.txt")[1] == "two")
    "#,
    )
    .unwrap();
}
