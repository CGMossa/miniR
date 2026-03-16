use r::Session;

// region: sprintf %x / %o

#[test]
fn sprintf_hex_lowercase() {
    let mut r = Session::new();
    let val = r.eval_source(r#"sprintf("%x", 255L)"#).unwrap().value;
    assert_eq!(
        val.as_vector().unwrap().as_character_scalar().unwrap(),
        "ff"
    );
}

#[test]
fn sprintf_hex_uppercase() {
    let mut r = Session::new();
    let val = r.eval_source(r#"sprintf("%X", 255L)"#).unwrap().value;
    assert_eq!(
        val.as_vector().unwrap().as_character_scalar().unwrap(),
        "FF"
    );
}

#[test]
fn sprintf_hex_with_hash_flag() {
    let mut r = Session::new();
    let val = r.eval_source(r#"sprintf("%#x", 255L)"#).unwrap().value;
    assert_eq!(
        val.as_vector().unwrap().as_character_scalar().unwrap(),
        "0xff"
    );
}

#[test]
fn sprintf_octal() {
    let mut r = Session::new();
    let val = r.eval_source(r#"sprintf("%o", 8L)"#).unwrap().value;
    assert_eq!(
        val.as_vector().unwrap().as_character_scalar().unwrap(),
        "10"
    );
}

#[test]
fn sprintf_octal_with_hash_flag() {
    let mut r = Session::new();
    let val = r.eval_source(r#"sprintf("%#o", 8L)"#).unwrap().value;
    assert_eq!(
        val.as_vector().unwrap().as_character_scalar().unwrap(),
        "010"
    );
}

// endregion

// region: formatC

#[test]
fn format_c_hex() {
    let mut r = Session::new();
    let val = r
        .eval_source(r#"formatC(255L, format = "x")"#)
        .unwrap()
        .value;
    assert_eq!(
        val.as_vector().unwrap().as_character_scalar().unwrap(),
        "ff"
    );
}

#[test]
fn format_c_with_width() {
    let mut r = Session::new();
    let val = r
        .eval_source(r#"formatC(42L, width = 5, format = "d")"#)
        .unwrap()
        .value;
    assert_eq!(
        val.as_vector().unwrap().as_character_scalar().unwrap(),
        "   42"
    );
}

#[test]
fn format_c_zero_padded() {
    let mut r = Session::new();
    let val = r
        .eval_source(r#"formatC(42L, width = 5, format = "d", flag = "0")"#)
        .unwrap()
        .value;
    assert_eq!(
        val.as_vector().unwrap().as_character_scalar().unwrap(),
        "00042"
    );
}

#[test]
fn format_c_string_left_aligned() {
    let mut r = Session::new();
    let val = r
        .eval_source(r#"formatC("hi", width = 5, format = "s", flag = "-")"#)
        .unwrap()
        .value;
    assert_eq!(
        val.as_vector().unwrap().as_character_scalar().unwrap(),
        "hi   "
    );
}

// endregion

// region: strtrim

#[test]
fn strtrim_basic() {
    let mut r = Session::new();
    let val = r
        .eval_source(r#"strtrim("Hello, world!", 5L)"#)
        .unwrap()
        .value;
    assert_eq!(
        val.as_vector().unwrap().as_character_scalar().unwrap(),
        "Hello"
    );
}

#[test]
fn strtrim_wider_than_string() {
    let mut r = Session::new();
    let val = r.eval_source(r#"strtrim("Hi", 10L)"#).unwrap().value;
    assert_eq!(
        val.as_vector().unwrap().as_character_scalar().unwrap(),
        "Hi"
    );
}

// endregion

// region: casefold

#[test]
fn casefold_to_lower() {
    let mut r = Session::new();
    let val = r
        .eval_source(r#"casefold("HELLO", upper = FALSE)"#)
        .unwrap()
        .value;
    assert_eq!(
        val.as_vector().unwrap().as_character_scalar().unwrap(),
        "hello"
    );
}

#[test]
fn casefold_to_upper() {
    let mut r = Session::new();
    let val = r
        .eval_source(r#"casefold("hello", upper = TRUE)"#)
        .unwrap()
        .value;
    assert_eq!(
        val.as_vector().unwrap().as_character_scalar().unwrap(),
        "HELLO"
    );
}

#[test]
fn casefold_default_is_lower() {
    let mut r = Session::new();
    let val = r.eval_source(r#"casefold("WORLD")"#).unwrap().value;
    assert_eq!(
        val.as_vector().unwrap().as_character_scalar().unwrap(),
        "world"
    );
}

// endregion

// region: encodeString

#[test]
fn encode_string_with_double_quotes() {
    let mut r = Session::new();
    let val = r
        .eval_source(r#"encodeString("hello", quote = '"')"#)
        .unwrap()
        .value;
    assert_eq!(
        val.as_vector().unwrap().as_character_scalar().unwrap(),
        r#""hello""#
    );
}

#[test]
fn encode_string_escapes_newlines() {
    let mut r = Session::new();
    let val = r.eval_source(r#"encodeString("a\nb")"#).unwrap().value;
    assert_eq!(
        val.as_vector().unwrap().as_character_scalar().unwrap(),
        "a\\nb"
    );
}

#[test]
fn encode_string_na_encode() {
    let mut r = Session::new();
    // NA_character_ encoded as "NA" by default
    let val = r
        .eval_source(r#"encodeString(NA_character_)"#)
        .unwrap()
        .value;
    assert_eq!(
        val.as_vector().unwrap().as_character_scalar().unwrap(),
        "NA"
    );
}

// endregion

// region: Sys.getlocale / Sys.setlocale

#[test]
fn sys_getlocale_returns_c() {
    let mut r = Session::new();
    let val = r.eval_source(r#"Sys.getlocale()"#).unwrap().value;
    assert_eq!(val.as_vector().unwrap().as_character_scalar().unwrap(), "C");
}

#[test]
fn sys_setlocale_returns_c() {
    let mut r = Session::new();
    let val = r
        .eval_source(r#"Sys.setlocale("LC_ALL", "C")"#)
        .unwrap()
        .value;
    assert_eq!(val.as_vector().unwrap().as_character_scalar().unwrap(), "C");
}

// endregion

// region: iconv

#[test]
fn iconv_stub_returns_input() {
    let mut r = Session::new();
    let val = r
        .eval_source(r#"iconv("hello", from = "UTF-8", to = "UTF-8")"#)
        .unwrap()
        .value;
    assert_eq!(
        val.as_vector().unwrap().as_character_scalar().unwrap(),
        "hello"
    );
}

// endregion

// region: URLencode / URLdecode

#[test]
fn urlencode_encodes_spaces() {
    let mut r = Session::new();
    let val = r
        .eval_source(r#"URLencode("hello world", reserved = TRUE)"#)
        .unwrap()
        .value;
    assert_eq!(
        val.as_vector().unwrap().as_character_scalar().unwrap(),
        "hello%20world"
    );
}

#[test]
fn urlencode_preserves_unreserved() {
    let mut r = Session::new();
    let val = r
        .eval_source(r#"URLencode("abc-123_test.txt~", reserved = TRUE)"#)
        .unwrap()
        .value;
    assert_eq!(
        val.as_vector().unwrap().as_character_scalar().unwrap(),
        "abc-123_test.txt~"
    );
}

#[test]
fn urldecode_decodes_percent_encoded() {
    let mut r = Session::new();
    let val = r
        .eval_source(r#"URLdecode("hello%20world")"#)
        .unwrap()
        .value;
    assert_eq!(
        val.as_vector().unwrap().as_character_scalar().unwrap(),
        "hello world"
    );
}

#[test]
fn urlencode_urldecode_roundtrip() {
    let mut r = Session::new();
    let val = r
        .eval_source(r#"URLdecode(URLencode("foo bar/baz?x=1", reserved = TRUE))"#)
        .unwrap()
        .value;
    assert_eq!(
        val.as_vector().unwrap().as_character_scalar().unwrap(),
        "foo bar/baz?x=1"
    );
}

// endregion

// region: substr<-

#[test]
fn substr_replacement_basic() {
    let mut r = Session::new();
    r.eval_source(r#"x <- "Hello, world!""#).unwrap();
    r.eval_source(r#"substr(x, 1L, 5L) <- "HELLO""#).unwrap();
    let val = r.eval_source(r#"x"#).unwrap().value;
    assert_eq!(
        val.as_vector().unwrap().as_character_scalar().unwrap(),
        "HELLO, world!"
    );
}

#[test]
fn substr_replacement_shorter_value() {
    let mut r = Session::new();
    r.eval_source(r#"x <- "abcdef""#).unwrap();
    r.eval_source(r#"substr(x, 2L, 4L) <- "XY""#).unwrap();
    let val = r.eval_source(r#"x"#).unwrap().value;
    assert_eq!(
        val.as_vector().unwrap().as_character_scalar().unwrap(),
        "aXYdef"
    );
}

#[test]
fn substr_replacement_longer_value_truncated() {
    let mut r = Session::new();
    r.eval_source(r#"x <- "abcdef""#).unwrap();
    r.eval_source(r#"substr(x, 2L, 3L) <- "WXYZ""#).unwrap();
    let val = r.eval_source(r#"x"#).unwrap().value;
    assert_eq!(
        val.as_vector().unwrap().as_character_scalar().unwrap(),
        "aWXdef"
    );
}

// endregion
