//! Tests for text progress bar builtins (indicatif-backed).

#[cfg(feature = "progress")]
mod progress_tests {
    use r::Session;

    #[test]
    fn create_and_close_progress_bar() {
        let mut s = Session::new();
        s.eval_source(
            r#"
pb <- txtProgressBar(min = 0, max = 100, style = 3)
stopifnot(inherits(pb, "txtProgressBar"))
close(pb)
"#,
        )
        .unwrap();
    }

    #[test]
    fn set_and_get_progress_bar_value() {
        let mut s = Session::new();
        s.eval_source(
            r#"
pb <- txtProgressBar(min = 0, max = 100, style = 3)
setTxtProgressBar(pb, 50)
val <- getTxtProgressBar(pb)
stopifnot(val == 50)
close(pb)
"#,
        )
        .unwrap();
    }

    #[test]
    fn progress_bar_default_min_max() {
        let mut s = Session::new();
        s.eval_source(
            r#"
pb <- txtProgressBar()
setTxtProgressBar(pb, 0.5)
val <- getTxtProgressBar(pb)
stopifnot(val == 0.5)
close(pb)
"#,
        )
        .unwrap();
    }

    #[test]
    fn progress_bar_full_sweep() {
        let mut s = Session::new();
        s.eval_source(
            r#"
pb <- txtProgressBar(min = 0, max = 10, style = 1)
for (i in 0:10) {
    setTxtProgressBar(pb, i)
}
val <- getTxtProgressBar(pb)
stopifnot(val == 10)
close(pb)
"#,
        )
        .unwrap();
    }

    #[test]
    fn progress_bar_invalid_max_le_min() {
        let mut s = Session::new();
        let result = s.eval_source(
            r#"
pb <- txtProgressBar(min = 10, max = 5)
"#,
        );
        assert!(result.is_err(), "expected error when max <= min");
    }

    #[test]
    fn multiple_progress_bars() {
        let mut s = Session::new();
        s.eval_source(
            r#"
pb1 <- txtProgressBar(min = 0, max = 100, style = 1)
pb2 <- txtProgressBar(min = 0, max = 50, style = 2)
setTxtProgressBar(pb1, 75)
setTxtProgressBar(pb2, 25)
stopifnot(getTxtProgressBar(pb1) == 75)
stopifnot(getTxtProgressBar(pb2) == 25)
close(pb1)
close(pb2)
"#,
        )
        .unwrap();
    }

    #[test]
    fn close_progress_bar_twice_errors() {
        let mut s = Session::new();
        let result = s.eval_source(
            r#"
pb <- txtProgressBar(min = 0, max = 100)
close(pb)
close(pb)
"#,
        );
        assert!(result.is_err(), "expected error on double close");
    }

    #[test]
    fn get_after_close_errors() {
        let mut s = Session::new();
        let result = s.eval_source(
            r#"
pb <- txtProgressBar(min = 0, max = 100)
close(pb)
getTxtProgressBar(pb)
"#,
        );
        assert!(result.is_err(), "expected error when getting closed bar");
    }

    #[test]
    fn set_after_close_errors() {
        let mut s = Session::new();
        let result = s.eval_source(
            r#"
pb <- txtProgressBar(min = 0, max = 100)
close(pb)
setTxtProgressBar(pb, 50)
"#,
        );
        assert!(result.is_err(), "expected error when setting closed bar");
    }

    #[test]
    fn progress_bar_all_styles() {
        let mut s = Session::new();
        s.eval_source(
            r#"
for (sty in 1:3) {
    pb <- txtProgressBar(min = 0, max = 1, style = sty)
    setTxtProgressBar(pb, 0.5)
    close(pb)
}
"#,
        )
        .unwrap();
    }
}
