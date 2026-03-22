use r::Session;

// region: par() query

#[test]
fn par_no_args_returns_named_list() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        p <- par()
        stopifnot(is.list(p))
        stopifnot(length(p) > 0)
        nms <- names(p)
        stopifnot("col" %in% nms)
        stopifnot("bg" %in% nms)
        stopifnot("lwd" %in% nms)
        stopifnot("cex" %in% nms)
        stopifnot("mar" %in% nms)
        stopifnot("mfrow" %in% nms)
        "#,
    )
    .unwrap();
}

#[test]
fn par_query_single_param() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        lwd <- par("lwd")
        stopifnot(lwd == 1)
        "#,
    )
    .unwrap();
}

#[test]
fn par_query_cex_default() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        cex <- par("cex")
        stopifnot(cex == 1)
        "#,
    )
    .unwrap();
}

#[test]
fn par_query_ps_default() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        ps <- par("ps")
        stopifnot(ps == 12)
        "#,
    )
    .unwrap();
}

// endregion

// region: par() set

#[test]
fn par_set_lwd_returns_old() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        old <- par(lwd = 2)
        stopifnot(old$lwd == 1)
        stopifnot(par("lwd") == 2)
        "#,
    )
    .unwrap();
}

#[test]
fn par_set_cex() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        par(cex = 1.5)
        stopifnot(par("cex") == 1.5)
        "#,
    )
    .unwrap();
}

#[test]
fn par_set_mar() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        par(mar = c(4, 4, 2, 1))
        m <- par("mar")
        stopifnot(length(m) == 4)
        stopifnot(m[1] == 4)
        stopifnot(m[2] == 4)
        stopifnot(m[3] == 2)
        stopifnot(m[4] == 1)
        "#,
    )
    .unwrap();
}

#[test]
fn par_set_mfrow() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        old <- par(mfrow = c(2L, 3L))
        stopifnot(old$mfrow[1] == 1)
        stopifnot(old$mfrow[2] == 1)
        mf <- par("mfrow")
        stopifnot(mf[1] == 2)
        stopifnot(mf[2] == 3)
        "#,
    )
    .unwrap();
}

#[test]
fn par_set_las() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        par(las = 1L)
        stopifnot(par("las") == 1)
        "#,
    )
    .unwrap();
}

#[test]
fn par_set_family() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        par(family = "serif")
        stopifnot(par("family") == "serif")
        "#,
    )
    .unwrap();
}

#[test]
fn par_set_new() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        stopifnot(par("new") == FALSE)
        par(new = TRUE)
        stopifnot(par("new") == TRUE)
        "#,
    )
    .unwrap();
}

// endregion

// region: par() defaults

#[test]
fn par_default_mar() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        m <- par("mar")
        stopifnot(length(m) == 4)
        stopifnot(m[1] == 5.1)
        stopifnot(m[2] == 4.1)
        stopifnot(m[3] == 4.1)
        stopifnot(m[4] == 2.1)
        "#,
    )
    .unwrap();
}

#[test]
fn par_default_mfrow() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        mf <- par("mfrow")
        stopifnot(length(mf) == 2)
        stopifnot(mf[1] == 1)
        stopifnot(mf[2] == 1)
        "#,
    )
    .unwrap();
}

#[test]
fn par_default_usr() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        u <- par("usr")
        stopifnot(length(u) == 4)
        stopifnot(u[1] == 0)
        stopifnot(u[2] == 1)
        stopifnot(u[3] == 0)
        stopifnot(u[4] == 1)
        "#,
    )
    .unwrap();
}

#[test]
fn par_default_xaxs_yaxs() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        stopifnot(par("xaxs") == "r")
        stopifnot(par("yaxs") == "r")
        "#,
    )
    .unwrap();
}

#[test]
fn par_default_font() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        stopifnot(par("font") == 1)
        "#,
    )
    .unwrap();
}

#[test]
fn par_default_pch() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        stopifnot(par("pch") == 1)
        "#,
    )
    .unwrap();
}

// endregion

// region: par() multiple params

#[test]
fn par_set_multiple_params() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        old <- par(lwd = 3, cex = 2)
        stopifnot(old$lwd == 1)
        stopifnot(old$cex == 1)
        stopifnot(par("lwd") == 3)
        stopifnot(par("cex") == 2)
        "#,
    )
    .unwrap();
}

// endregion

// region: par() error handling

#[test]
fn par_invalid_param_errors() {
    let mut s = Session::new();
    let result = s.eval_source(r#"par("nonexistent")"#);
    assert!(result.is_err());
}

// endregion
