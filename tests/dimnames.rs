use r::Session;

#[test]
fn dimname_replacements_keep_data_frame_labels_in_sync() {
    let mut s = Session::new();
    s.eval_source(
        r#"m <- matrix(1:4, nrow = 2)
rownames(m) <- c("r1", "r2")
colnames(m) <- c("x", "y")
stopifnot(
  identical(rownames(m), c("r1", "r2")),
  identical(colnames(m), c("x", "y"))
)

rownames(m) <- NULL
colnames(m) <- NULL
stopifnot(is.null(rownames(m)), is.null(colnames(m)))

df <- data.frame(x = 1:2, y = 3:4)
names(df) <- c("u", "v")
stopifnot(
  identical(names(df), c("u", "v")),
  identical(colnames(df), c("u", "v")),
  identical(dimnames(df)[[2]], c("u", "v"))
)

rownames(df) <- c("a", "b")
stopifnot(
  identical(row.names(df), c("a", "b")),
  identical(dimnames(df)[[1]], c("a", "b"))
)

colnames(df) <- c("p", "q")
stopifnot(
  identical(names(df), c("p", "q")),
  identical(colnames(df), c("p", "q")),
  identical(dimnames(df)[[2]], c("p", "q"))
)

dimnames(df) <- list(c("m", "n"), c("g", "h"))
stopifnot(
  identical(row.names(df), c("m", "n")),
  identical(names(df), c("g", "h")),
  identical(dimnames(df)[[1]], c("m", "n")),
  identical(dimnames(df)[[2]], c("g", "h"))
)

rownames(df) <- NULL
stopifnot(identical(row.names(df), c("1", "2")))

colnames(df) <- NULL
stopifnot(is.null(names(df)), is.null(colnames(df)), is.null(dimnames(df)[[2]]))"#,
    )
    .expect("dimnames tests failed");
}

#[test]
fn data_frame_dimnames_set_rejects_invalid_shapes() {
    let mut s = Session::new();
    let err = s
        .eval_source(
            r#"df <- data.frame(x = 1:2, y = 3:4)
dimnames(df) <- list(c("a", "b"), NULL)"#,
        )
        .expect_err("dimnames<- with NULL colnames on data.frame should fail");

    assert!(
        err.to_string()
            .contains("invalid 'dimnames' given for data frame"),
        "unexpected error: {err}"
    );
}
