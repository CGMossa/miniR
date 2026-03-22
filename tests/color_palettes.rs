use r::Session;

#[test]
fn hsv_basic_conversion() {
    let mut s = Session::new();
    s.eval_source(
        r##"
# Pure red: h=0, s=1, v=1
col <- hsv(0, 1, 1)
stopifnot(col == "#FF0000")

# Pure green: h=1/3, s=1, v=1
col <- hsv(1/3, 1, 1)
stopifnot(col == "#00FF00")

# Pure blue: h=2/3, s=1, v=1
col <- hsv(2/3, 1, 1)
stopifnot(col == "#0000FF")

# White: s=0, v=1
col <- hsv(0, 0, 1)
stopifnot(col == "#FFFFFF")

# Black: v=0
col <- hsv(0, 0, 0)
stopifnot(col == "#000000")
"##,
    );
}

#[test]
fn hsv_vectorized() {
    let mut s = Session::new();
    s.eval_source(
        r##"
cols <- hsv(c(0, 1/3, 2/3), 1, 1)
stopifnot(length(cols) == 3)
stopifnot(cols[1] == "#FF0000")
stopifnot(cols[2] == "#00FF00")
stopifnot(cols[3] == "#0000FF")
"##,
    );
}

#[test]
fn hsv_with_alpha() {
    let mut s = Session::new();
    s.eval_source(
        r##"
col <- hsv(0, 1, 1, alpha = 0.5)
# Should have 8 hex digits (RRGGBBAA)
stopifnot(nchar(col) == 9)
"##,
    );
}

#[test]
fn hcl_basic_conversion() {
    let mut s = Session::new();
    s.eval_source(
        r##"
# Default values: h=0, c=35, l=85
col <- hcl()
stopifnot(is.character(col))
stopifnot(length(col) == 1)
stopifnot(nchar(col) == 7)
"##,
    );
}

#[test]
fn hcl_vectorized() {
    let mut s = Session::new();
    s.eval_source(
        r##"
cols <- hcl(h = c(0, 120, 240))
stopifnot(length(cols) == 3)
stopifnot(all(nchar(cols) == 7))
"##,
    );
}

#[test]
fn rainbow_basic() {
    let mut s = Session::new();
    s.eval_source(
        r##"
cols <- rainbow(5)
stopifnot(is.character(cols))
stopifnot(length(cols) == 5)
# First color should be red (h=0)
stopifnot(cols[1] == "#FF0000")
# All should be valid hex colors
stopifnot(all(nchar(cols) == 7))
"##,
    );
}

#[test]
fn rainbow_single_color() {
    let mut s = Session::new();
    s.eval_source(
        r##"
cols <- rainbow(1)
stopifnot(length(cols) == 1)
stopifnot(cols[1] == "#FF0000")
"##,
    );
}

#[test]
fn rainbow_zero_colors() {
    let mut s = Session::new();
    s.eval_source(
        r##"
cols <- rainbow(0)
stopifnot(length(cols) == 0)
stopifnot(is.character(cols))
"##,
    );
}

#[test]
fn rainbow_with_alpha() {
    let mut s = Session::new();
    s.eval_source(
        r##"
cols <- rainbow(3, alpha = 0.5)
stopifnot(length(cols) == 3)
# With alpha != 1, should have 8 hex digits + #
stopifnot(all(nchar(cols) == 9))
"##,
    );
}

#[test]
fn heat_colors_basic() {
    let mut s = Session::new();
    s.eval_source(
        r##"
cols <- heat.colors(10)
stopifnot(is.character(cols))
stopifnot(length(cols) == 10)
# First color should be red
stopifnot(cols[1] == "#FF0000")
# All valid hex colors
stopifnot(all(nchar(cols) == 7))
"##,
    );
}

#[test]
fn heat_colors_zero() {
    let mut s = Session::new();
    s.eval_source(
        r##"
cols <- heat.colors(0)
stopifnot(length(cols) == 0)
"##,
    );
}

#[test]
fn terrain_colors_basic() {
    let mut s = Session::new();
    s.eval_source(
        r##"
cols <- terrain.colors(10)
stopifnot(is.character(cols))
stopifnot(length(cols) == 10)
stopifnot(all(nchar(cols) == 7))
"##,
    );
}

#[test]
fn topo_colors_basic() {
    let mut s = Session::new();
    s.eval_source(
        r##"
cols <- topo.colors(10)
stopifnot(is.character(cols))
stopifnot(length(cols) == 10)
stopifnot(all(nchar(cols) == 7))
"##,
    );
}

#[test]
fn cm_colors_basic() {
    let mut s = Session::new();
    s.eval_source(
        r##"
cols <- cm.colors(5)
stopifnot(is.character(cols))
stopifnot(length(cols) == 5)
# First should be cyan, last should be magenta
stopifnot(cols[1] == "#00FFFF")
stopifnot(cols[5] == "#FF00FF")
# Middle should be white
stopifnot(cols[3] == "#FFFFFF")
"##,
    );
}

#[test]
fn cm_colors_single() {
    let mut s = Session::new();
    s.eval_source(
        r##"
# Single color should be white (midpoint)
cols <- cm.colors(1)
stopifnot(length(cols) == 1)
stopifnot(cols[1] == "#FFFFFF")
"##,
    );
}

#[test]
fn gray_colors_basic() {
    let mut s = Session::new();
    s.eval_source(
        r##"
cols <- gray.colors(5)
stopifnot(is.character(cols))
stopifnot(length(cols) == 5)
stopifnot(all(nchar(cols) == 7))
"##,
    );
}

#[test]
fn grey_colors_alias() {
    let mut s = Session::new();
    s.eval_source(
        r##"
# grey.colors should work as an alias for gray.colors
cols <- grey.colors(3)
stopifnot(is.character(cols))
stopifnot(length(cols) == 3)
"##,
    );
}

#[test]
fn gray_colors_custom_range() {
    let mut s = Session::new();
    s.eval_source(
        r##"
cols <- gray.colors(2, start = 0, end = 1)
stopifnot(length(cols) == 2)
"##,
    );
}

#[test]
fn color_ramp_palette_basic() {
    let mut s = Session::new();
    s.eval_source(
        r##"
pal <- colorRampPalette(c("#FF0000", "#0000FF"))
stopifnot(is.function(pal))

# Generate 3 colors: red, purple, blue
cols <- pal(3)
stopifnot(is.character(cols))
stopifnot(length(cols) == 3)
stopifnot(cols[1] == "#FF0000")
stopifnot(cols[3] == "#0000FF")
"##,
    );
}

#[test]
fn color_ramp_palette_single_output() {
    let mut s = Session::new();
    s.eval_source(
        r##"
pal <- colorRampPalette(c("#000000", "#FFFFFF"))
cols <- pal(1)
stopifnot(length(cols) == 1)
stopifnot(cols[1] == "#000000")
"##,
    );
}

#[test]
fn color_ramp_palette_two_colors() {
    let mut s = Session::new();
    s.eval_source(
        r##"
pal <- colorRampPalette(c("#000000", "#FFFFFF"))
cols <- pal(2)
stopifnot(length(cols) == 2)
stopifnot(cols[1] == "#000000")
stopifnot(cols[2] == "#FFFFFF")
"##,
    );
}

#[test]
fn color_ramp_palette_interpolation() {
    let mut s = Session::new();
    s.eval_source(
        r##"
# Black to white gradient should produce gray in middle
pal <- colorRampPalette(c("#000000", "#FFFFFF"))
cols <- pal(5)
stopifnot(length(cols) == 5)
# Middle should be approximately #808080 (gray)
stopifnot(cols[3] == "#808080")
"##,
    );
}

#[test]
fn color_ramp_palette_three_anchors() {
    let mut s = Session::new();
    s.eval_source(
        r##"
# Red -> Green -> Blue
pal <- colorRampPalette(c("#FF0000", "#00FF00", "#0000FF"))
cols <- pal(5)
stopifnot(length(cols) == 5)
stopifnot(cols[1] == "#FF0000")
stopifnot(cols[3] == "#00FF00")
stopifnot(cols[5] == "#0000FF")
"##,
    );
}

#[test]
fn color_ramp_palette_zero_output() {
    let mut s = Session::new();
    s.eval_source(
        r##"
pal <- colorRampPalette(c("#000000", "#FFFFFF"))
cols <- pal(0)
stopifnot(length(cols) == 0)
stopifnot(is.character(cols))
"##,
    );
}

#[test]
fn all_palettes_return_character_vectors() {
    let mut s = Session::new();
    s.eval_source(
        r##"
n <- 7
stopifnot(is.character(rainbow(n)))
stopifnot(is.character(heat.colors(n)))
stopifnot(is.character(terrain.colors(n)))
stopifnot(is.character(topo.colors(n)))
stopifnot(is.character(cm.colors(n)))
stopifnot(is.character(gray.colors(n)))
stopifnot(is.character(hsv(0, 1, 1)))
stopifnot(is.character(hcl(0, 35, 85)))
"##,
    );
}

#[test]
fn all_palettes_correct_length() {
    let mut s = Session::new();
    s.eval_source(
        r##"
for (n in c(1, 5, 10, 20)) {
    stopifnot(length(rainbow(n)) == n)
    stopifnot(length(heat.colors(n)) == n)
    stopifnot(length(terrain.colors(n)) == n)
    stopifnot(length(topo.colors(n)) == n)
    stopifnot(length(cm.colors(n)) == n)
    stopifnot(length(gray.colors(n)) == n)
}
"##,
    );
}
