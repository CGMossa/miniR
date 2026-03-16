use r::Session;

#[test]
fn digest_sha256_basic() {
    let mut s = Session::new();
    // SHA-256 of empty string is well-known
    s.eval_source(
        r#"
h <- digest("")
stopifnot(h == "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
"#,
    )
    .expect("sha256 of empty string");
}

#[test]
fn digest_sha256_hello() {
    let mut s = Session::new();
    // SHA-256 of "hello"
    s.eval_source(
        r#"
h <- digest("hello")
stopifnot(h == "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824")
"#,
    )
    .expect("sha256 of hello");
}

#[test]
fn digest_sha512() {
    let mut s = Session::new();
    // SHA-512 of "hello"
    s.eval_source(
        r#"
h <- digest("hello", algo = "sha512")
stopifnot(h == "9b71d224bd62f3785d96d46ad3ea3d73319bfbc2890caadae2dff72519673ca72323c3d99ba5c11d7c7acc6e14b8c5da0c4663475c2e5c3adef46f73bcdec043")
"#,
    )
    .expect("sha512 of hello");
}

#[test]
fn digest_sha256_is_default() {
    let mut s = Session::new();
    // Verify that the default algo is sha256 by comparing with explicit call
    s.eval_source(
        r#"
stopifnot(digest("test") == digest("test", algo = "sha256"))
"#,
    )
    .expect("default algo should be sha256");
}

#[test]
fn digest_raw_vector() {
    let mut s = Session::new();
    // Hash raw bytes: as.raw(c(0x68, 0x65, 0x6c, 0x6c, 0x6f)) == "hello" in UTF-8
    s.eval_source(
        r#"
r <- as.raw(c(0x68, 0x65, 0x6c, 0x6c, 0x6f))
h <- digest(r)
stopifnot(h == "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824")
"#,
    )
    .expect("digest of raw vector matching 'hello' bytes");
}

#[test]
fn digest_bad_algo_error() {
    let mut s = Session::new();
    s.eval_source(
        r#"
err <- tryCatch(digest("x", algo = "md5"), error = function(e) conditionMessage(e))
stopifnot(grepl("unsupported algorithm", err))
"#,
    )
    .expect("unsupported algorithm should produce error");
}

#[test]
fn md5_stub_error() {
    let mut s = Session::new();
    s.eval_source(
        r#"
err <- tryCatch(md5("hello"), error = function(e) conditionMessage(e))
stopifnot(grepl("MD5 is cryptographically broken", err))
stopifnot(grepl("sha256", err))
"#,
    )
    .expect("md5 stub should error with helpful message");
}

#[test]
fn digest_returns_character() {
    let mut s = Session::new();
    s.eval_source(
        r#"
h <- digest("test")
stopifnot(is.character(h))
stopifnot(length(h) == 1L)
# SHA-256 hex digest is always 64 characters
stopifnot(nchar(h) == 64L)
"#,
    )
    .expect("digest should return 64-char hex string");
}

#[test]
fn digest_sha512_length() {
    let mut s = Session::new();
    s.eval_source(
        r#"
h <- digest("test", algo = "sha512")
stopifnot(is.character(h))
# SHA-512 hex digest is always 128 characters
stopifnot(nchar(h) == 128L)
"#,
    )
    .expect("sha512 digest should return 128-char hex string");
}
