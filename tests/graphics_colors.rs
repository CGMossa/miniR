use r::Session;

// region: Color name resolution

#[test]
fn colors_returns_657_names() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        cols <- colors()
        stopifnot(is.character(cols))
        stopifnot(length(cols) == 657)
        "#,
    )
    .unwrap();
}

#[test]
fn colours_is_alias_for_colors() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        stopifnot(identical(colors(), colours()))
        "#,
    )
    .unwrap();
}

#[test]
fn color_names_are_sorted() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        cols <- colors()
        stopifnot(identical(cols, sort(cols)))
        "#,
    )
    .unwrap();
}

#[test]
fn colors_include_expected_names() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        cols <- colors()
        stopifnot("red" %in% cols)
        stopifnot("green" %in% cols)
        stopifnot("blue" %in% cols)
        stopifnot("black" %in% cols)
        stopifnot("white" %in% cols)
        stopifnot("gray0" %in% cols)
        stopifnot("gray100" %in% cols)
        stopifnot("grey0" %in% cols)
        stopifnot("grey100" %in% cols)
        stopifnot("cornflowerblue" %in% cols)
        stopifnot("darkslategray" %in% cols)
        stopifnot("yellowgreen" %in% cols)
        "#,
    )
    .unwrap();
}

// endregion

// region: col2rgb

#[test]
fn col2rgb_named_colors() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        m <- col2rgb("red")
        stopifnot(m[1,1] == 255)
        stopifnot(m[2,1] == 0)
        stopifnot(m[3,1] == 0)
        "#,
    )
    .unwrap();
}

#[test]
fn col2rgb_hex_color() {
    let mut s = Session::new();
    s.eval_source(
        r##"
        m <- col2rgb("#FF8000")
        stopifnot(m[1,1] == 255)
        stopifnot(m[2,1] == 128)
        stopifnot(m[3,1] == 0)
        "##,
    )
    .unwrap();
}

#[test]
fn col2rgb_with_alpha() {
    let mut s = Session::new();
    s.eval_source(
        r##"
        m <- col2rgb("#FF000080", alpha = TRUE)
        stopifnot(nrow(m) == 4)
        stopifnot(m[1,1] == 255)
        stopifnot(m[2,1] == 0)
        stopifnot(m[3,1] == 0)
        stopifnot(m[4,1] == 128)
        "##,
    )
    .unwrap();
}

#[test]
fn col2rgb_multiple_colors() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        m <- col2rgb(c("red", "green", "blue"))
        stopifnot(ncol(m) == 3)
        stopifnot(nrow(m) == 3)
        # red column
        stopifnot(m[1,1] == 255)
        stopifnot(m[2,1] == 0)
        stopifnot(m[3,1] == 0)
        # green column
        stopifnot(m[1,2] == 0)
        stopifnot(m[2,2] == 255)
        stopifnot(m[3,2] == 0)
        # blue column
        stopifnot(m[1,3] == 0)
        stopifnot(m[2,3] == 0)
        stopifnot(m[3,3] == 255)
        "#,
    )
    .unwrap();
}

#[test]
fn col2rgb_black_and_white() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        mb <- col2rgb("black")
        stopifnot(mb[1,1] == 0)
        stopifnot(mb[2,1] == 0)
        stopifnot(mb[3,1] == 0)

        mw <- col2rgb("white")
        stopifnot(mw[1,1] == 255)
        stopifnot(mw[2,1] == 255)
        stopifnot(mw[3,1] == 255)
        "#,
    )
    .unwrap();
}

// endregion

// region: rgb()

#[test]
fn rgb_basic() {
    let mut s = Session::new();
    s.eval_source(
        r##"
        h <- rgb(1, 0, 0)
        stopifnot(h == "#FF0000")
        "##,
    )
    .unwrap();
}

#[test]
fn rgb_with_alpha() {
    let mut s = Session::new();
    s.eval_source(
        r##"
        h <- rgb(1, 0, 0, 0.5)
        stopifnot(h == "#FF000080")
        "##,
    )
    .unwrap();
}

#[test]
fn rgb_max_color_value_255() {
    let mut s = Session::new();
    s.eval_source(
        r##"
        h <- rgb(255, 128, 0, maxColorValue = 255)
        stopifnot(h == "#FF8000")
        "##,
    )
    .unwrap();
}

#[test]
fn rgb_vectorized() {
    let mut s = Session::new();
    s.eval_source(
        r##"
        h <- rgb(c(255, 0), c(0, 255), c(0, 0), maxColorValue = 255)
        stopifnot(length(h) == 2)
        stopifnot(h[1] == "#FF0000")
        stopifnot(h[2] == "#00FF00")
        "##,
    )
    .unwrap();
}

// endregion

// region: palette()

#[test]
fn palette_default_has_8_colors() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        p <- palette()
        stopifnot(length(p) == 8)
        stopifnot(is.character(p))
        "#,
    )
    .unwrap();
}

#[test]
fn palette_default_starts_with_black() {
    let mut s = Session::new();
    s.eval_source(
        r##"
        p <- palette()
        stopifnot(p[1] == "#000000")
        "##,
    )
    .unwrap();
}

#[test]
fn palette_set_and_get() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        old <- palette(c("red", "blue", "green"))
        stopifnot(length(old) == 8)
        new_p <- palette()
        stopifnot(length(new_p) == 3)
        "#,
    )
    .unwrap();
}

#[test]
fn palette_reset_to_default() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        palette(c("red", "blue"))
        stopifnot(length(palette()) == 2)
        palette("default")
        stopifnot(length(palette()) == 8)
        "#,
    )
    .unwrap();
}

// endregion

// region: Hex parsing edge cases

#[test]
fn col2rgb_case_insensitive() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        m1 <- col2rgb("Red")
        m2 <- col2rgb("RED")
        m3 <- col2rgb("red")
        # Values should be identical even though dimnames differ
        stopifnot(all(m1 == m2))
        stopifnot(all(m2 == m3))
        stopifnot(m1[1,1] == 255)
        stopifnot(m1[2,1] == 0)
        stopifnot(m1[3,1] == 0)
        "#,
    )
    .unwrap();
}

#[test]
fn col2rgb_palette_index() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        # Palette index 1 is black in the default palette
        m <- col2rgb("1")
        stopifnot(m[1,1] == 0)
        stopifnot(m[2,1] == 0)
        stopifnot(m[3,1] == 0)
        "#,
    )
    .unwrap();
}

// endregion
