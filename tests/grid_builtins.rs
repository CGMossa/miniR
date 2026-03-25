use r::Session;

#[test]
fn grid_unit_creates_proper_unit_object() {
    let mut s = Session::new();
    s.eval_source(
        r#"
u <- unit(1, "cm")
stopifnot(inherits(u, "unit"))
stopifnot(u$value == 1)
stopifnot(u$units == "cm")
"#,
    )
    .expect("unit(1, 'cm') should create a unit object");
}

#[test]
fn grid_unit_vectorized() {
    let mut s = Session::new();
    s.eval_source(
        r#"
u <- unit(c(1, 2, 3), "cm")
stopifnot(inherits(u, "unit"))
stopifnot(length(u$value) == 3)
stopifnot(length(u$units) == 3)
"#,
    )
    .expect("unit() should vectorize");
}

#[test]
fn grid_unit_invalid_units_error() {
    let mut s = Session::new();
    let result = s.eval_source(r#"unit(1, "foobar")"#);
    assert!(result.is_err(), "unit() with invalid units should error");
}

#[test]
fn grid_gpar_creates_gpar_object() {
    let mut s = Session::new();
    s.eval_source(
        r#"
gp <- gpar(col = "red")
stopifnot(inherits(gp, "gpar"))
stopifnot(gp$col == "red")
"#,
    )
    .expect("gpar(col='red') should create a gpar object");
}

#[test]
fn grid_gpar_multiple_params() {
    let mut s = Session::new();
    s.eval_source(
        r#"
gp <- gpar(col = "blue", fill = "yellow", lwd = 2, fontsize = 12)
stopifnot(inherits(gp, "gpar"))
stopifnot(gp$col == "blue")
stopifnot(gp$fill == "yellow")
stopifnot(gp$lwd == 2)
stopifnot(gp$fontsize == 12)
"#,
    )
    .expect("gpar() should accept multiple parameters");
}

#[test]
fn grid_viewport_creates_viewport_with_defaults() {
    let mut s = Session::new();
    s.eval_source(
        r#"
vp <- viewport()
stopifnot(inherits(vp, "viewport"))
"#,
    )
    .expect("viewport() should create a viewport with defaults");
}

#[test]
fn grid_viewport_with_params() {
    let mut s = Session::new();
    s.eval_source(
        r#"
vp <- viewport(name = "myVP", clip = "on")
stopifnot(inherits(vp, "viewport"))
stopifnot(vp$name == "myVP")
stopifnot(vp$clip == "on")
"#,
    )
    .expect("viewport() should accept named params");
}

#[test]
fn grid_newpage_and_rect() {
    let mut s = Session::new();
    s.eval_source(
        r#"
grid.newpage()
grid.rect()
"#,
    )
    .expect("grid.newpage(); grid.rect() should not error");
}

#[test]
fn grid_push_pop_viewport() {
    let mut s = Session::new();
    s.eval_source(
        r#"
grid.newpage()
pushViewport(viewport())
popViewport()
"#,
    )
    .expect("pushViewport/popViewport should work");
}

#[test]
fn grid_current_viewport_returns_root() {
    let mut s = Session::new();
    s.eval_source(
        r#"
grid.newpage()
vp <- current.viewport()
stopifnot(inherits(vp, "viewport"))
stopifnot(vp$name == "ROOT")
"#,
    )
    .expect("current.viewport() on empty stack should return ROOT");
}

#[test]
fn grid_push_and_current_viewport() {
    let mut s = Session::new();
    s.eval_source(
        r#"
grid.newpage()
pushViewport(viewport(name = "test"))
vp <- current.viewport()
stopifnot(vp$name == "test")
popViewport()
"#,
    )
    .expect("current.viewport() should return the pushed viewport");
}

#[test]
fn grid_lines_creates_grob() {
    let mut s = Session::new();
    s.eval_source(
        r#"
grid.newpage()
g <- grid.lines()
stopifnot(inherits(g, "grob"))
stopifnot(inherits(g, "lines"))
"#,
    )
    .expect("grid.lines() should create a lines grob");
}

#[test]
fn grid_text_creates_grob() {
    let mut s = Session::new();
    s.eval_source(
        r#"
grid.newpage()
g <- grid.text("hello")
stopifnot(inherits(g, "grob"))
stopifnot(inherits(g, "text"))
stopifnot(g$label == "hello")
"#,
    )
    .expect("grid.text() should create a text grob");
}

#[test]
fn grid_circle_creates_grob() {
    let mut s = Session::new();
    s.eval_source(
        r#"
grid.newpage()
g <- grid.circle()
stopifnot(inherits(g, "grob"))
stopifnot(inherits(g, "circle"))
"#,
    )
    .expect("grid.circle() should create a circle grob");
}

#[test]
fn grid_layout_creates_layout() {
    let mut s = Session::new();
    s.eval_source(
        r#"
ly <- grid.layout(nrow = 2, ncol = 3)
stopifnot(inherits(ly, "layout"))
stopifnot(ly$nrow == 2)
stopifnot(ly$ncol == 3)
"#,
    )
    .expect("grid.layout() should create a layout object");
}

#[test]
fn grid_grob_name_and_get() {
    let mut s = Session::new();
    s.eval_source(
        r#"
grid.newpage()
grid.rect(name = "myRect")
g <- grid.get("myRect")
stopifnot(!is.null(g))
stopifnot(g$name == "myRect")
"#,
    )
    .expect("grid.get() should retrieve named grobs");
}

#[test]
fn grid_remove_grob() {
    let mut s = Session::new();
    s.eval_source(
        r#"
grid.newpage()
grid.rect(name = "toRemove")
grid.remove("toRemove")
g <- grid.get("toRemove")
stopifnot(is.null(g))
"#,
    )
    .expect("grid.remove() should remove named grobs");
}

#[test]
fn grid_draw_records_grob() {
    let mut s = Session::new();
    s.eval_source(
        r#"
grid.newpage()
g <- grid.rect(draw = FALSE)
grid.draw(g)
"#,
    )
    .expect("grid.draw() should record a grob");
}

#[test]
fn grid_data_viewport() {
    let mut s = Session::new();
    s.eval_source(
        r#"
vp <- dataViewport(xData = c(1, 5, 10), yData = c(0, 100))
stopifnot(inherits(vp, "viewport"))
"#,
    )
    .expect("dataViewport() should create a viewport with data scales");
}

#[test]
fn grid_plot_viewport() {
    let mut s = Session::new();
    s.eval_source(
        r#"
vp <- plotViewport()
stopifnot(inherits(vp, "viewport"))
"#,
    )
    .expect("plotViewport() should create a viewport");
}

#[test]
fn grid_xaxis_and_yaxis() {
    let mut s = Session::new();
    s.eval_source(
        r#"
grid.newpage()
g <- grid.xaxis()
stopifnot(inherits(g, "grob"))
stopifnot(inherits(g, "xaxis"))

g2 <- grid.yaxis()
stopifnot(inherits(g2, "grob"))
stopifnot(inherits(g2, "yaxis"))
"#,
    )
    .expect("grid.xaxis() and grid.yaxis() should create axis grobs");
}

#[test]
fn grid_segments_creates_grob() {
    let mut s = Session::new();
    s.eval_source(
        r#"
grid.newpage()
g <- grid.segments()
stopifnot(inherits(g, "grob"))
stopifnot(inherits(g, "segments"))
"#,
    )
    .expect("grid.segments() should create a segments grob");
}

#[test]
fn grid_points_creates_grob() {
    let mut s = Session::new();
    s.eval_source(
        r#"
grid.newpage()
g <- grid.points(x = c(0.2, 0.5, 0.8), y = c(0.3, 0.6, 0.9))
stopifnot(inherits(g, "grob"))
stopifnot(inherits(g, "points"))
"#,
    )
    .expect("grid.points() should create a points grob");
}

#[test]
fn grid_polygon_creates_grob() {
    let mut s = Session::new();
    s.eval_source(
        r#"
grid.newpage()
g <- grid.polygon(x = c(0, 0.5, 1), y = c(0, 1, 0))
stopifnot(inherits(g, "grob"))
stopifnot(inherits(g, "polygon"))
"#,
    )
    .expect("grid.polygon() should create a polygon grob");
}

#[test]
fn grid_viewport_with_layout() {
    let mut s = Session::new();
    s.eval_source(
        r#"
ly <- grid.layout(nrow = 2, ncol = 2)
vp <- viewport(layout = ly)
stopifnot(inherits(vp, "viewport"))
stopifnot(inherits(vp$layout, "layout"))
"#,
    )
    .expect("viewport() should accept layout parameter");
}

#[test]
fn grid_pop_viewport_error_on_empty() {
    let mut s = Session::new();
    let result = s.eval_source(
        r#"
grid.newpage()
popViewport()
"#,
    );
    assert!(result.is_err(), "popViewport() on empty stack should error");
}

#[test]
fn grid_seek_viewport_not_found() {
    let mut s = Session::new();
    let result = s.eval_source(
        r#"
grid.newpage()
seekViewport("nonexistent")
"#,
    );
    assert!(
        result.is_err(),
        "seekViewport() for missing viewport should error"
    );
}

#[test]
fn grid_edit_grob_properties() {
    let mut s = Session::new();
    s.eval_source(
        r#"
grid.newpage()
grid.rect(name = "editMe", gp = gpar(col = "red"))
grid.edit("editMe", gp = gpar(col = "blue"))
g <- grid.get("editMe")
stopifnot(g$gp$col == "blue")
"#,
    )
    .expect("grid.edit() should modify grob properties");
}
