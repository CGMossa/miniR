use r::Session;

// region: substr / substring vectorization

#[test]
fn substr_vectorized_over_x() {
    let mut s = Session::new();
    s.eval_source(
        r#"
stopifnot(identical(substr(c("abcdef", "ghijkl"), 2, 4), c("bcd", "hij")))
"#,
    )
    .unwrap();
}

#[test]
fn substr_vectorized_over_start_stop() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# R's substring recycles start and stop
stopifnot(identical(substring("abcdef", 1:3, 4:6), c("abcd", "bcde", "cdef")))
"#,
    )
    .unwrap();
}

#[test]
fn substr_vectorized_split_chars() {
    let mut s = Session::new();
    // Classic R idiom: split a string into individual characters
    s.eval_source(
        r#"
ss <- substring("abcdef", 1:6, 1:6)
stopifnot(identical(ss, c("a", "b", "c", "d", "e", "f")))
"#,
    )
    .unwrap();
}

#[test]
fn substr_na_propagation() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- substr(c("abc", NA, "def"), 1, 2)
stopifnot(identical(result[1], "ab"))
stopifnot(is.na(result[2]))
stopifnot(identical(result[3], "de"))
"#,
    )
    .unwrap();
}

#[test]
fn substr_empty_input() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- substr(character(0), 1, 2)
stopifnot(identical(length(result), 0L))
"#,
    )
    .unwrap();
}

// endregion

// region: strsplit vectorization

#[test]
fn strsplit_vectorized_over_x() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- strsplit(c("a-b", "c-d-e"), "-")
stopifnot(identical(length(result), 2L))
stopifnot(identical(result[[1]], c("a", "b")))
stopifnot(identical(result[[2]], c("c", "d", "e")))
"#,
    )
    .unwrap();
}

#[test]
fn strsplit_na_propagation() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- strsplit(c("a-b", NA, "c-d"), "-")
stopifnot(identical(length(result), 3L))
stopifnot(identical(result[[1]], c("a", "b")))
stopifnot(is.na(result[[2]]))
stopifnot(identical(result[[3]], c("c", "d")))
"#,
    )
    .unwrap();
}

#[test]
fn strsplit_fixed_vectorized() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- strsplit(c("a.b.c", "d.e"), ".", fixed = TRUE)
stopifnot(identical(result[[1]], c("a", "b", "c")))
stopifnot(identical(result[[2]], c("d", "e")))
"#,
    )
    .unwrap();
}

#[test]
fn strsplit_empty_split_vectorized() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- strsplit(c("ab", "cd"), "")
stopifnot(identical(result[[1]], c("a", "b")))
stopifnot(identical(result[[2]], c("c", "d")))
"#,
    )
    .unwrap();
}

// endregion

// region: chartr vectorization

#[test]
fn chartr_vectorized_over_x() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- chartr("aeiou", "AEIOU", c("hello", "world", "test"))
stopifnot(identical(result, c("hEllO", "wOrld", "tEst")))
"#,
    )
    .unwrap();
}

#[test]
fn chartr_na_propagation() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- chartr("a", "A", c("abc", NA, "aaa"))
stopifnot(identical(result[1], "Abc"))
stopifnot(is.na(result[2]))
stopifnot(identical(result[3], "AAA"))
"#,
    )
    .unwrap();
}

// endregion

// region: basename / dirname vectorization

#[test]
fn basename_vectorized() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- basename(c("/home/user/file.txt", "/tmp/test.R", "simple.csv"))
stopifnot(identical(result, c("file.txt", "test.R", "simple.csv")))
"#,
    )
    .unwrap();
}

#[test]
fn basename_na_propagation() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- basename(c("/home/file.txt", NA))
stopifnot(identical(result[1], "file.txt"))
stopifnot(is.na(result[2]))
"#,
    )
    .unwrap();
}

#[test]
fn dirname_vectorized() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- dirname(c("/home/user/file.txt", "/tmp/test.R"))
stopifnot(identical(result, c("/home/user", "/tmp")))
"#,
    )
    .unwrap();
}

#[test]
fn dirname_na_propagation() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- dirname(c("/home/file.txt", NA))
stopifnot(identical(result[1], "/home"))
stopifnot(is.na(result[2]))
"#,
    )
    .unwrap();
}

#[test]
fn dirname_empty_input() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- dirname(character(0))
stopifnot(identical(length(result), 0L))
"#,
    )
    .unwrap();
}

// endregion

// region: strtoi vectorization

#[test]
fn strtoi_vectorized() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- strtoi(c("10", "20", "30"))
stopifnot(identical(result, c(10L, 20L, 30L)))
"#,
    )
    .unwrap();
}

#[test]
fn strtoi_vectorized_with_base() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- strtoi(c("ff", "10", "a"), base = 16L)
stopifnot(identical(result, c(255L, 16L, 10L)))
"#,
    )
    .unwrap();
}

#[test]
fn strtoi_na_propagation() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- strtoi(c("10", NA, "xyz", "30"))
stopifnot(identical(result[1], 10L))
stopifnot(is.na(result[2]))
stopifnot(is.na(result[3]))
stopifnot(identical(result[4], 30L))
"#,
    )
    .unwrap();
}

#[test]
fn strtoi_empty_string_is_na() {
    let mut s = Session::new();
    s.eval_source(
        r#"
stopifnot(is.na(strtoi("")))
stopifnot(is.na(strtoi("", 2L)))
"#,
    )
    .unwrap();
}

// endregion

// region: URLencode / URLdecode vectorization

#[test]
fn urlencode_vectorized() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- URLencode(c("hello world", "a&b", "plain"))
stopifnot(identical(result[1], "hello%20world"))
stopifnot(identical(result[2], "a%26b"))
stopifnot(identical(result[3], "plain"))
"#,
    )
    .unwrap();
}

#[test]
fn urlencode_na_propagation() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- URLencode(c("hello", NA, "world"))
stopifnot(identical(result[1], "hello"))
stopifnot(is.na(result[2]))
stopifnot(identical(result[3], "world"))
"#,
    )
    .unwrap();
}

#[test]
fn urldecode_vectorized() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- URLdecode(c("hello%20world", "a%26b", "plain"))
stopifnot(identical(result[1], "hello world"))
stopifnot(identical(result[2], "a&b"))
stopifnot(identical(result[3], "plain"))
"#,
    )
    .unwrap();
}

#[test]
fn urldecode_na_propagation() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- URLdecode(c("hello%20world", NA, "test"))
stopifnot(identical(result[1], "hello world"))
stopifnot(is.na(result[2]))
stopifnot(identical(result[3], "test"))
"#,
    )
    .unwrap();
}

// endregion

// region: substr<- vectorization

#[test]
fn substr_assign_vectorized_over_x() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- c("abcdef", "ghijkl")
substr(x, 2, 3) <- "wx"
stopifnot(identical(x, c("awxdef", "gwxjkl")))
"#,
    )
    .unwrap();
}

#[test]
fn substr_assign_replacement_longer_than_range() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- "aaa"
substr(x, 2, 3) <- "wxy"
stopifnot(identical(x, "awx"))
"#,
    )
    .unwrap();
}

#[test]
fn substr_assign_replacement_shorter_than_range() {
    let mut s = Session::new();
    s.eval_source(
        r#"
x <- "aaa"
substr(x, 2, 3) <- "w"
stopifnot(identical(x, "awa"))
"#,
    )
    .unwrap();
}

#[test]
fn substr_assign_na_propagation() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- `substr<-`(c("abc", NA, "def"), 1, 2, "xy")
stopifnot(identical(result[1], "xyc"))
stopifnot(is.na(result[2]))
stopifnot(identical(result[3], "xyf"))
"#,
    )
    .unwrap();
}

// endregion
