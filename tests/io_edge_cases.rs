use r::Session;

#[test]
fn read_csv_header_true() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        tf <- tempfile()
        writeLines("name,value\nalice,1\nbob,2", tf)
        df <- read.csv(tf, header = TRUE)
        stopifnot(nrow(df) == 2)
        stopifnot(identical(df$name, c("alice", "bob")))
        stopifnot(identical(df$value, c(1L, 2L)))
    "#,
    )
    .expect("read.csv with header=TRUE failed");
}

#[test]
fn read_csv_header_false() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        tf <- tempfile()
        writeLines("alice,1\nbob,2\ncarol,3", tf)
        df <- read.csv(tf, header = FALSE)
        stopifnot(identical(names(df), c("V1", "V2")))
        stopifnot(nrow(df) >= 2)
    "#,
    )
    .expect("read.csv with header=FALSE failed");
}

#[test]
fn write_csv_then_read_csv_roundtrip() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        tf <- tempfile()
        df <- data.frame(x = c(1L, 2L, 3L), y = c("a", "b", "c"))
        write.csv(df, tf, row.names = FALSE)
        df2 <- read.csv(tf)
        stopifnot(identical(df2$x, c(1L, 2L, 3L)))
        stopifnot(identical(df2$y, c("a", "b", "c")))
    "#,
    )
    .expect("write.csv/read.csv roundtrip failed");
}

#[test]
fn read_lines_multiline() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        tf <- tempfile()
        writeLines(c("line one", "line two", "line three"), tf)
        lines <- readLines(tf)
        stopifnot(length(lines) == 3)
        stopifnot(lines[1] == "line one")
        stopifnot(lines[2] == "line two")
        stopifnot(lines[3] == "line three")
    "#,
    )
    .expect("readLines on multi-line file failed");
}

#[test]
fn write_lines_then_read_lines_roundtrip() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        tf <- tempfile()
        original <- c("hello", "world", "test")
        writeLines(original, tf)
        result <- readLines(tf)
        stopifnot(identical(result, original))
    "#,
    )
    .expect("writeLines/readLines roundtrip failed");
}

#[test]
fn read_csv_missing_values() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        tf <- tempfile()
        writeLines("a,b,c\n1,,3\nNA,5,", tf)
        df <- read.csv(tf)
        stopifnot(is.na(df$a[2]))
        stopifnot(is.na(df$b[1]))
        stopifnot(is.na(df$c[2]))
        stopifnot(df$a[1] == 1L)
        stopifnot(df$b[2] == 5L)
        stopifnot(df$c[1] == 3L)
    "#,
    )
    .expect("read.csv with missing values failed");
}

#[test]
fn file_connection_lifecycle() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        tf <- tempfile()
        writeLines("connection test", tf)
        con <- file(tf, open = "r")
        stopifnot(isOpen(con))
        lines <- readLines(con)
        stopifnot(lines[1] == "connection test")
        close(con)
        stopifnot(!isOpen(con))
    "#,
    )
    .expect("file/readLines/close lifecycle failed");
}

#[test]
fn cat_to_file_then_read_lines() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        tf <- tempfile()
        cat("hello from cat\n", file = tf)
        result <- readLines(tf)
        stopifnot(result[1] == "hello from cat")
    "#,
    )
    .expect("cat to file then readLines failed");
}

#[test]
fn tempfile_returns_unique_paths() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        t1 <- tempfile()
        t2 <- tempfile()
        t3 <- tempfile()
        stopifnot(t1 != t2)
        stopifnot(t2 != t3)
        stopifnot(t1 != t3)
    "#,
    )
    .expect("tempfile uniqueness failed");
}

#[test]
fn file_exists_before_and_after_writing() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        tf <- tempfile()
        stopifnot(!file.exists(tf))
        writeLines("now it exists", tf)
        stopifnot(file.exists(tf))
    "#,
    )
    .expect("file.exists before/after write failed");
}
