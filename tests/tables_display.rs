use r::session::Session;

// region: View()

#[test]
fn view_displays_data_frame() {
    let mut s = Session::new();
    // View() should not error and should return the data.frame invisibly
    s.eval_source(
        r#"
        df <- data.frame(x = 1:3, y = c("a", "b", "c"))
        result <- View(df)
        stopifnot(is.data.frame(result))
        stopifnot(identical(result, df))
        "#,
    )
    .unwrap();
}

#[test]
fn view_rejects_non_data_frame() {
    let mut s = Session::new();
    let result = s.eval_source("View(1:10)");
    assert!(result.is_err(), "View() should reject non-data.frame input");
}

#[test]
fn view_handles_empty_data_frame() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        df <- data.frame()
        result <- View(df)
        stopifnot(is.data.frame(result))
        "#,
    )
    .unwrap();
}

#[test]
fn view_handles_single_column() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        df <- data.frame(x = 1:5)
        result <- View(df)
        stopifnot(is.data.frame(result))
        "#,
    )
    .unwrap();
}

#[test]
fn view_truncates_long_data_frames() {
    let mut s = Session::new();
    // Create a data frame with more than 20 rows — View should still work
    s.eval_source(
        r#"
        df <- data.frame(x = 1:50, y = rep("hello", 50))
        result <- View(df)
        stopifnot(is.data.frame(result))
        stopifnot(nrow(result) == 50L)
        "#,
    )
    .unwrap();
}

// endregion

// region: kable()

#[test]
fn kable_returns_markdown_string() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        df <- data.frame(x = 1:3, y = c("a", "b", "c"))
        result <- kable(df)
        stopifnot(is.character(result))
        stopifnot(length(result) == 1L)
        # Markdown tables have pipe characters
        stopifnot(grepl("|", result, fixed = TRUE))
        "#,
    )
    .unwrap();
}

#[test]
fn kable_simple_format() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        df <- data.frame(a = 1:2, b = c("x", "y"))
        result <- kable(df, format = "simple")
        stopifnot(is.character(result))
        stopifnot(length(result) == 1L)
        "#,
    )
    .unwrap();
}

#[test]
fn kable_rejects_non_data_frame() {
    let mut s = Session::new();
    let result = s.eval_source("kable(1:10)");
    assert!(
        result.is_err(),
        "kable() should reject non-data.frame input"
    );
}

#[test]
fn kable_empty_data_frame() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        df <- data.frame()
        result <- kable(df)
        stopifnot(is.character(result))
        "#,
    )
    .unwrap();
}

// endregion

// region: str() for data.frames

#[test]
fn str_data_frame_runs_without_error() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        df <- data.frame(x = 1:3, y = c("a", "b", "c"), z = c(TRUE, FALSE, TRUE))
        str(df)
        "#,
    )
    .unwrap();
}

#[test]
fn str_data_frame_returns_null() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        df <- data.frame(x = 1:3)
        result <- str(df)
        stopifnot(is.null(result))
        "#,
    )
    .unwrap();
}

#[test]
fn str_still_works_for_vectors() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        str(1:10)
        str(c("a", "b", "c"))
        str(TRUE)
        "#,
    )
    .unwrap();
}

#[test]
fn str_data_frame_mixed_types() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        df <- data.frame(
            ints = 1:5,
            doubles = c(1.1, 2.2, 3.3, 4.4, 5.5),
            chars = c("a", "b", "c", "d", "e"),
            logicals = c(TRUE, FALSE, TRUE, FALSE, TRUE)
        )
        str(df)
        "#,
    )
    .unwrap();
}

// endregion
