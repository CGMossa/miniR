//! Tests for the grid viewport system, display list, grob store, and replay.

use r::interpreter::grid::display::{DisplayItem, DisplayList};
use r::interpreter::grid::gpar::Gpar;
use r::interpreter::grid::grob::{Grob, GrobStore};
use r::interpreter::grid::render::{replay, GridRenderer};
use r::interpreter::grid::units::{Unit, UnitContext, UnitType};
use r::interpreter::grid::viewport::{
    compute_transform, Justification, Viewport, ViewportStack, ViewportTransform,
};

// region: Unit tests

#[test]
fn unit_constructors() {
    let u = Unit::npc(0.5);
    assert_eq!(u.units[0], UnitType::Npc);
    assert!((u.value() - 0.5).abs() < 1e-12);
    assert_eq!(u.len(), 1);
    assert!(!u.is_empty());

    let u2 = Unit::cm(2.54);
    assert_eq!(u2.units[0], UnitType::Cm);
    assert!((u2.value() - 2.54).abs() < 1e-12);

    let u3 = Unit {
        values: vec![1.0, 2.0, 3.0],
        units: vec![UnitType::Native, UnitType::Native, UnitType::Native],
    };
    assert_eq!(u3.len(), 3);
}

// endregion

// region: UnitContext resolution tests

#[test]
fn resolve_cm_is_identity() {
    let ctx = UnitContext::default();
    let u = Unit::cm(5.0);
    assert!((ctx.resolve_x(&u, 0) - 5.0).abs() < 1e-12);
    assert!((ctx.resolve_y(&u, 0) - 5.0).abs() < 1e-12);
}

#[test]
fn resolve_inches_to_cm() {
    let ctx = UnitContext::default();
    let u = Unit::inches(1.0);
    assert!((ctx.resolve_x(&u, 0) - 2.54).abs() < 1e-12);
}

#[test]
fn resolve_npc_scales_to_viewport() {
    let ctx = UnitContext {
        viewport_width_cm: 10.0,
        viewport_height_cm: 20.0,
        ..Default::default()
    };
    let u = Unit::npc(0.5);
    assert!((ctx.resolve_x(&u, 0) - 5.0).abs() < 1e-12);
    assert!((ctx.resolve_y(&u, 0) - 10.0).abs() < 1e-12);
}

#[test]
fn resolve_native_maps_through_scale() {
    let ctx = UnitContext {
        xscale: (0.0, 100.0),
        yscale: (0.0, 200.0),
        viewport_width_cm: 10.0,
        viewport_height_cm: 20.0,
        ..Default::default()
    };
    let u = Unit::new(50.0, UnitType::Native);
    // 50 out of 0..100 = 0.5 * 10 cm = 5 cm
    assert!((ctx.resolve_x(&u, 0) - 5.0).abs() < 1e-12);
    // 50 out of 0..200 = 0.25 * 20 cm = 5 cm
    assert!((ctx.resolve_y(&u, 0) - 5.0).abs() < 1e-12);
}

#[test]
fn resolve_mm_to_cm() {
    let ctx = UnitContext::default();
    let u = Unit::new(25.4, UnitType::Mm);
    assert!((ctx.resolve_x(&u, 0) - 2.54).abs() < 1e-12);
}

#[test]
fn resolve_points_to_cm() {
    let ctx = UnitContext::default();
    // 72 points = 1 inch = 2.54 cm
    let u = Unit::new(72.0, UnitType::Points);
    assert!((ctx.resolve_x(&u, 0) - 2.54).abs() < 1e-10);
}

#[test]
fn resolve_size_for_npc_uses_geometric_mean() {
    let ctx = UnitContext {
        viewport_width_cm: 4.0,
        viewport_height_cm: 9.0,
        ..Default::default()
    };
    // geometric mean of 4*9 = 36, sqrt = 6
    let u = Unit::npc(1.0);
    assert!((ctx.resolve_size(&u, 0) - 6.0).abs() < 1e-12);
}

// endregion

// region: Justification tests

#[test]
fn justification_fractions() {
    assert!((Justification::Left.as_fraction() - 0.0).abs() < 1e-12);
    assert!((Justification::Centre.as_fraction() - 0.5).abs() < 1e-12);
    assert!((Justification::Right.as_fraction() - 1.0).abs() < 1e-12);
    assert!((Justification::Bottom.as_fraction() - 0.0).abs() < 1e-12);
    assert!((Justification::Top.as_fraction() - 1.0).abs() < 1e-12);
}

#[test]
fn justification_from_str() {
    assert_eq!(Justification::parse("left"), Some(Justification::Left));
    assert_eq!(Justification::parse("centre"), Some(Justification::Centre));
    assert_eq!(Justification::parse("center"), Some(Justification::Centre));
    assert_eq!(Justification::parse("right"), Some(Justification::Right));
    assert_eq!(Justification::parse("top"), Some(Justification::Top));
    assert_eq!(Justification::parse("bottom"), Some(Justification::Bottom));
    assert_eq!(Justification::parse("invalid"), None);
}

// endregion

// region: Viewport tests

#[test]
fn viewport_root_covers_device() {
    let vp = Viewport::root(20.0, 15.0);
    assert_eq!(vp.name.as_deref(), Some("ROOT"));
    assert!((vp.width.value() - 20.0).abs() < 1e-12);
    assert!((vp.height.value() - 15.0).abs() < 1e-12);
    assert!(vp.clip);
}

#[test]
fn viewport_default_is_full_npc() {
    let vp = Viewport::new();
    assert!(vp.name.is_none());
    assert_eq!(vp.x.units[0], UnitType::Npc);
    assert!((vp.x.value() - 0.5).abs() < 1e-12);
    assert!((vp.width.value() - 1.0).abs() < 1e-12);
    assert_eq!(vp.just.0, Justification::Centre);
    assert_eq!(vp.just.1, Justification::Centre);
    assert!(!vp.clip);
}

// endregion

// region: ViewportStack tests

#[test]
fn viewport_stack_starts_with_root() {
    let stack = ViewportStack::new(20.0, 15.0);
    assert_eq!(stack.depth(), 1);
    assert_eq!(stack.current().name.as_deref(), Some("ROOT"));
}

#[test]
fn viewport_stack_push_pop() {
    let mut stack = ViewportStack::new(20.0, 15.0);

    let mut vp = Viewport::new();
    vp.name = Some("child".to_string());
    stack.push(vp);
    assert_eq!(stack.depth(), 2);
    assert_eq!(stack.current().name.as_deref(), Some("child"));

    let popped = stack.pop();
    assert!(popped.is_some());
    assert_eq!(
        popped.as_ref().and_then(|v| v.name.as_deref()),
        Some("child")
    );
    assert_eq!(stack.depth(), 1);
}

#[test]
fn viewport_stack_cannot_pop_root() {
    let mut stack = ViewportStack::new(20.0, 15.0);
    let popped = stack.pop();
    assert!(popped.is_none());
    assert_eq!(stack.depth(), 1);
}

// endregion

// region: ViewportTransform tests

#[test]
fn root_transform_matches_device() {
    let t = ViewportTransform::root(20.0, 15.0);
    assert!((t.x_offset_cm - 0.0).abs() < 1e-12);
    assert!((t.y_offset_cm - 0.0).abs() < 1e-12);
    assert!((t.width_cm - 20.0).abs() < 1e-12);
    assert!((t.height_cm - 15.0).abs() < 1e-12);
    assert!((t.angle - 0.0).abs() < 1e-12);
}

#[test]
fn child_viewport_transform_centered() {
    let parent = ViewportTransform::root(20.0, 20.0);
    let mut vp = Viewport::new();
    vp.width = Unit::npc(0.5);
    vp.height = Unit::npc(0.5);
    // Centre-justified at (0.5, 0.5) npc, size 0.5x0.5 npc
    let child = ViewportTransform::from_viewport(&vp, &parent);
    // 0.5 npc in 20cm = 10 cm position, minus 0.5 * 10cm width = 5cm offset
    assert!((child.x_offset_cm - 5.0).abs() < 1e-10);
    assert!((child.y_offset_cm - 5.0).abs() < 1e-10);
    assert!((child.width_cm - 10.0).abs() < 1e-10);
    assert!((child.height_cm - 10.0).abs() < 1e-10);
}

#[test]
fn child_viewport_transform_left_bottom() {
    let parent = ViewportTransform::root(20.0, 20.0);
    let mut vp = Viewport::new();
    vp.x = Unit::npc(0.0);
    vp.y = Unit::npc(0.0);
    vp.width = Unit::npc(0.5);
    vp.height = Unit::npc(0.5);
    vp.just = (Justification::Left, Justification::Bottom);
    let child = ViewportTransform::from_viewport(&vp, &parent);
    assert!((child.x_offset_cm - 0.0).abs() < 1e-10);
    assert!((child.y_offset_cm - 0.0).abs() < 1e-10);
    assert!((child.width_cm - 10.0).abs() < 1e-10);
    assert!((child.height_cm - 10.0).abs() < 1e-10);
}

#[test]
fn native_to_cm_conversion() {
    let t = ViewportTransform {
        x_offset_cm: 5.0,
        y_offset_cm: 5.0,
        width_cm: 10.0,
        height_cm: 10.0,
        angle: 0.0,
        xscale: (0.0, 100.0),
        yscale: (0.0, 200.0),
    };
    // x=50 out of 0..100 = 0.5 * 10 + 5 = 10 cm
    assert!((t.native_to_cm_x(50.0) - 10.0).abs() < 1e-10);
    // y=100 out of 0..200 = 0.5 * 10 + 5 = 10 cm
    assert!((t.native_to_cm_y(100.0) - 10.0).abs() < 1e-10);
}

#[test]
fn npc_to_cm_conversion() {
    let t = ViewportTransform {
        x_offset_cm: 2.0,
        y_offset_cm: 3.0,
        width_cm: 10.0,
        height_cm: 8.0,
        angle: 0.0,
        xscale: (0.0, 1.0),
        yscale: (0.0, 1.0),
    };
    assert!((t.npc_to_cm_x(0.5) - 7.0).abs() < 1e-10);
    assert!((t.npc_to_cm_y(0.5) - 7.0).abs() < 1e-10);
}

#[test]
fn compute_transform_matches_stack() {
    let mut stack = ViewportStack::new(20.0, 20.0);
    let mut vp = Viewport::new();
    vp.width = Unit::npc(0.5);
    vp.height = Unit::npc(0.5);
    stack.push(vp);

    let t = compute_transform(&stack);
    assert!((t.x_offset_cm - 5.0).abs() < 1e-10);
    assert!((t.y_offset_cm - 5.0).abs() < 1e-10);
    assert!((t.width_cm - 10.0).abs() < 1e-10);
    assert!((t.height_cm - 10.0).abs() < 1e-10);
}

#[test]
fn viewport_rotation_accumulates() {
    let parent = ViewportTransform::root(20.0, 20.0);
    let mut vp1 = Viewport::new();
    vp1.angle = 45.0;
    let t1 = ViewportTransform::from_viewport(&vp1, &parent);
    assert!((t1.angle - 45.0).abs() < 1e-12);

    let mut vp2 = Viewport::new();
    vp2.angle = 30.0;
    let t2 = ViewportTransform::from_viewport(&vp2, &t1);
    assert!((t2.angle - 75.0).abs() < 1e-12);
}

// endregion

// region: Gpar tests

#[test]
fn gpar_inherit_fills_from_parent() {
    let parent = Gpar {
        col: Some([0, 0, 0, 255]),
        fill: Some([255, 255, 255, 255]),
        lwd: Some(2.0),
        fontsize: Some(14.0),
        ..Default::default()
    };
    let child = Gpar {
        col: Some([255, 0, 0, 255]),
        ..Default::default()
    };
    let merged = child.with_parent(&parent);
    assert_eq!(merged.col, Some([255, 0, 0, 255])); // child overrides
    assert_eq!(merged.fill, Some([255, 255, 255, 255])); // inherited from parent
    assert_eq!(merged.lwd, Some(2.0)); // inherited
    assert_eq!(merged.fontsize, Some(14.0)); // inherited
}

#[test]
fn gpar_effective_defaults() {
    let gp = Gpar::new();
    assert_eq!(gp.effective_col(), [0, 0, 0, 255]);
    assert_eq!(gp.effective_fill(), [255, 255, 255, 0]);
    assert!((gp.effective_alpha() - 1.0).abs() < 1e-12);
    assert!((gp.effective_lwd() - 1.0).abs() < 1e-12);
    assert!((gp.effective_fontsize() - 12.0).abs() < 1e-12);
    assert!((gp.effective_cex() - 1.0).abs() < 1e-12);
    assert!((gp.effective_lineheight() - 1.2).abs() < 1e-12);
}

// endregion

// region: Grob tests

#[test]
fn grob_store_add_and_get() {
    let mut store = GrobStore::new();
    assert!(store.is_empty());

    let id = store.add(Grob::Circle {
        x: Unit::npc(0.5),
        y: Unit::npc(0.5),
        r: Unit::cm(1.0),
        gp: Gpar::new(),
    });
    assert_eq!(id, 0);
    assert_eq!(store.len(), 1);
    assert!(!store.is_empty());

    let grob = store.get(id).expect("grob should exist");
    assert!(matches!(grob, Grob::Circle { .. }));
    assert!(store.get(999).is_none());
}

#[test]
fn grob_gpar_accessor() {
    let gp = Gpar {
        col: Some([0, 128, 255, 255]),
        ..Default::default()
    };
    let rect = Grob::Rect {
        x: Unit::npc(0.5),
        y: Unit::npc(0.5),
        width: Unit::npc(1.0),
        height: Unit::npc(1.0),
        just: (Justification::Centre, Justification::Centre),
        gp: gp.clone(),
    };
    assert!(rect.gpar().is_some());
    assert_eq!(
        rect.gpar().expect("should have gpar").col,
        Some([0, 128, 255, 255])
    );

    let collection = Grob::Collection {
        children: vec![0, 1],
    };
    assert!(collection.gpar().is_none());
}

// endregion

// region: DisplayList tests

#[test]
fn display_list_basics() {
    let mut dl = DisplayList::new();
    assert!(dl.is_empty());
    assert_eq!(dl.len(), 0);

    dl.record(DisplayItem::PushViewport(Box::new(Viewport::new())));
    dl.record(DisplayItem::Draw(0));
    dl.record(DisplayItem::PopViewport);

    assert_eq!(dl.len(), 3);
    assert!(!dl.is_empty());
    assert!(matches!(dl.items()[0], DisplayItem::PushViewport(_)));
    assert!(matches!(dl.items()[1], DisplayItem::Draw(0)));
    assert!(matches!(dl.items()[2], DisplayItem::PopViewport));

    dl.clear();
    assert!(dl.is_empty());
}

// endregion

// region: Renderer mock and replay tests

/// A mock renderer that records all calls for verification.
struct MockRenderer {
    calls: Vec<String>,
    device_w: f64,
    device_h: f64,
    clip_depth: usize,
}

impl MockRenderer {
    fn new(w: f64, h: f64) -> Self {
        MockRenderer {
            calls: Vec::new(),
            device_w: w,
            device_h: h,
            clip_depth: 0,
        }
    }
}

impl GridRenderer for MockRenderer {
    fn line(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, _gp: &Gpar) {
        self.calls
            .push(format!("line({x0:.2},{y0:.2},{x1:.2},{y1:.2})"));
    }
    fn polyline(&mut self, x: &[f64], y: &[f64], _gp: &Gpar) {
        self.calls
            .push(format!("polyline(n={})", x.len().min(y.len())));
    }
    fn rect(&mut self, x: f64, y: f64, w: f64, h: f64, _gp: &Gpar) {
        self.calls
            .push(format!("rect({x:.2},{y:.2},{w:.2},{h:.2})"));
    }
    fn circle(&mut self, x: f64, y: f64, r: f64, _gp: &Gpar) {
        self.calls.push(format!("circle({x:.2},{y:.2},{r:.2})"));
    }
    fn polygon(&mut self, x: &[f64], y: &[f64], _gp: &Gpar) {
        self.calls
            .push(format!("polygon(n={})", x.len().min(y.len())));
    }
    fn text(&mut self, x: f64, y: f64, label: &str, _rot: f64, _gp: &Gpar) {
        self.calls.push(format!("text({x:.2},{y:.2},\"{label}\")"));
    }
    fn point(&mut self, x: f64, y: f64, pch: u8, size: f64, _gp: &Gpar) {
        self.calls
            .push(format!("point({x:.2},{y:.2},pch={pch},size={size:.2})"));
    }
    fn clip(&mut self, x: f64, y: f64, w: f64, h: f64) {
        self.clip_depth += 1;
        self.calls
            .push(format!("clip({x:.2},{y:.2},{w:.2},{h:.2})"));
    }
    fn unclip(&mut self) {
        if self.clip_depth > 0 {
            self.clip_depth -= 1;
        }
        self.calls.push("unclip".to_string());
    }
    fn device_size_cm(&self) -> (f64, f64) {
        (self.device_w, self.device_h)
    }
}

#[test]
fn replay_empty_display_list() {
    let dl = DisplayList::new();
    let store = GrobStore::new();
    let mut renderer = MockRenderer::new(20.0, 20.0);
    replay(&dl, &store, &mut renderer);
    assert!(renderer.calls.is_empty());
}

#[test]
fn replay_draws_circle_at_center() {
    let mut store = GrobStore::new();
    let circle_id = store.add(Grob::Circle {
        x: Unit::npc(0.5),
        y: Unit::npc(0.5),
        r: Unit::cm(2.0),
        gp: Gpar::new(),
    });

    let mut dl = DisplayList::new();
    dl.record(DisplayItem::Draw(circle_id));

    let mut renderer = MockRenderer::new(20.0, 20.0);
    replay(&dl, &store, &mut renderer);

    assert_eq!(renderer.calls.len(), 1);
    assert_eq!(renderer.calls[0], "circle(10.00,10.00,2.00)");
}

#[test]
fn replay_with_viewport_push_pop() {
    let mut store = GrobStore::new();
    let rect_id = store.add(Grob::Rect {
        x: Unit::npc(0.5),
        y: Unit::npc(0.5),
        width: Unit::npc(1.0),
        height: Unit::npc(1.0),
        just: (Justification::Centre, Justification::Centre),
        gp: Gpar::new(),
    });

    let mut vp = Viewport::new();
    vp.width = Unit::npc(0.5);
    vp.height = Unit::npc(0.5);

    let mut dl = DisplayList::new();
    dl.record(DisplayItem::PushViewport(Box::new(vp)));
    dl.record(DisplayItem::Draw(rect_id));
    dl.record(DisplayItem::PopViewport);

    let mut renderer = MockRenderer::new(20.0, 20.0);
    replay(&dl, &store, &mut renderer);

    // The viewport is centered (0.5 npc) with 0.5 npc width in a 20cm device
    // So viewport is 10cm wide, offset at 5cm.
    // The rect is 1.0 npc within that viewport = 10cm wide, centered at 0.5 npc = 5cm within viewport.
    // Absolute: x = 5 + 5 - 0.5*10 = 5, y same => rect(5.00, 5.00, 10.00, 10.00)
    assert_eq!(renderer.calls.len(), 1);
    assert_eq!(renderer.calls[0], "rect(5.00,5.00,10.00,10.00)");
}

#[test]
fn replay_clipping_viewport() {
    let mut store = GrobStore::new();
    let circle_id = store.add(Grob::Circle {
        x: Unit::npc(0.5),
        y: Unit::npc(0.5),
        r: Unit::cm(1.0),
        gp: Gpar::new(),
    });

    let mut vp = Viewport::new();
    vp.clip = true;

    let mut dl = DisplayList::new();
    dl.record(DisplayItem::PushViewport(Box::new(vp)));
    dl.record(DisplayItem::Draw(circle_id));
    dl.record(DisplayItem::PopViewport);

    let mut renderer = MockRenderer::new(20.0, 20.0);
    replay(&dl, &store, &mut renderer);

    // Should see: clip, circle, unclip
    assert_eq!(renderer.calls.len(), 3);
    assert!(renderer.calls[0].starts_with("clip("));
    assert!(renderer.calls[1].starts_with("circle("));
    assert_eq!(renderer.calls[2], "unclip");
}

#[test]
fn replay_nested_viewports() {
    let mut store = GrobStore::new();
    let point_id = store.add(Grob::Points {
        x: Unit::npc(0.5),
        y: Unit::npc(0.5),
        pch: 1,
        size: Unit::cm(0.5),
        gp: Gpar::new(),
    });

    // Outer viewport: left half of device
    let mut outer = Viewport::new();
    outer.x = Unit::npc(0.0);
    outer.y = Unit::npc(0.0);
    outer.width = Unit::npc(0.5);
    outer.height = Unit::npc(1.0);
    outer.just = (Justification::Left, Justification::Bottom);

    // Inner viewport: top half of outer
    let mut inner = Viewport::new();
    inner.x = Unit::npc(0.0);
    inner.y = Unit::npc(0.5);
    inner.width = Unit::npc(1.0);
    inner.height = Unit::npc(0.5);
    inner.just = (Justification::Left, Justification::Bottom);

    let mut dl = DisplayList::new();
    dl.record(DisplayItem::PushViewport(Box::new(outer)));
    dl.record(DisplayItem::PushViewport(Box::new(inner)));
    dl.record(DisplayItem::Draw(point_id));
    dl.record(DisplayItem::PopViewport);
    dl.record(DisplayItem::PopViewport);

    let mut renderer = MockRenderer::new(20.0, 20.0);
    replay(&dl, &store, &mut renderer);

    assert_eq!(renderer.calls.len(), 1);
    // Outer viewport: 0..10 cm x, 0..20 cm y
    // Inner viewport: within outer, x=0..10, y=10..20 (top half)
    // Point at (0.5, 0.5) npc within inner = (5, 5) within inner viewport
    // Absolute: x = 0 + 0 + 5 = 5, y = 0 + 10 + 5 = 15
    assert_eq!(renderer.calls[0], "point(5.00,15.00,pch=1,size=0.50)");
}

#[test]
fn replay_text_grob() {
    let mut store = GrobStore::new();
    let text_id = store.add(Grob::Text {
        label: vec!["Hello".to_string(), "World".to_string()],
        x: Unit {
            values: vec![0.25, 0.75],
            units: vec![UnitType::Npc, UnitType::Npc],
        },
        y: Unit {
            values: vec![0.5, 0.5],
            units: vec![UnitType::Npc, UnitType::Npc],
        },
        just: (Justification::Centre, Justification::Centre),
        rot: 0.0,
        gp: Gpar::new(),
    });

    let mut dl = DisplayList::new();
    dl.record(DisplayItem::Draw(text_id));

    let mut renderer = MockRenderer::new(20.0, 20.0);
    replay(&dl, &store, &mut renderer);

    assert_eq!(renderer.calls.len(), 2);
    assert_eq!(renderer.calls[0], "text(5.00,10.00,\"Hello\")");
    assert_eq!(renderer.calls[1], "text(15.00,10.00,\"World\")");
}

#[test]
fn replay_segments_grob() {
    let mut store = GrobStore::new();
    let seg_id = store.add(Grob::Segments {
        x0: Unit {
            values: vec![0.0, 0.5],
            units: vec![UnitType::Npc, UnitType::Npc],
        },
        y0: Unit {
            values: vec![0.0, 0.5],
            units: vec![UnitType::Npc, UnitType::Npc],
        },
        x1: Unit {
            values: vec![1.0, 0.5],
            units: vec![UnitType::Npc, UnitType::Npc],
        },
        y1: Unit {
            values: vec![1.0, 0.5],
            units: vec![UnitType::Npc, UnitType::Npc],
        },
        gp: Gpar::new(),
    });

    let mut dl = DisplayList::new();
    dl.record(DisplayItem::Draw(seg_id));

    let mut renderer = MockRenderer::new(20.0, 20.0);
    replay(&dl, &store, &mut renderer);

    assert_eq!(renderer.calls.len(), 2);
    assert_eq!(renderer.calls[0], "line(0.00,0.00,20.00,20.00)");
    assert_eq!(renderer.calls[1], "line(10.00,10.00,10.00,10.00)");
}

#[test]
fn replay_polygon_grob() {
    let mut store = GrobStore::new();
    let poly_id = store.add(Grob::Polygon {
        x: Unit {
            values: vec![0.0, 1.0, 0.5],
            units: vec![UnitType::Npc, UnitType::Npc, UnitType::Npc],
        },
        y: Unit {
            values: vec![0.0, 0.0, 1.0],
            units: vec![UnitType::Npc, UnitType::Npc, UnitType::Npc],
        },
        gp: Gpar::new(),
    });

    let mut dl = DisplayList::new();
    dl.record(DisplayItem::Draw(poly_id));

    let mut renderer = MockRenderer::new(20.0, 20.0);
    replay(&dl, &store, &mut renderer);

    assert_eq!(renderer.calls.len(), 1);
    assert_eq!(renderer.calls[0], "polygon(n=3)");
}

#[test]
fn replay_collection_grob() {
    let mut store = GrobStore::new();
    let c1 = store.add(Grob::Circle {
        x: Unit::npc(0.25),
        y: Unit::npc(0.5),
        r: Unit::cm(1.0),
        gp: Gpar::new(),
    });
    let c2 = store.add(Grob::Circle {
        x: Unit::npc(0.75),
        y: Unit::npc(0.5),
        r: Unit::cm(1.0),
        gp: Gpar::new(),
    });
    let collection = store.add(Grob::Collection {
        children: vec![c1, c2],
    });

    let mut dl = DisplayList::new();
    dl.record(DisplayItem::Draw(collection));

    let mut renderer = MockRenderer::new(20.0, 20.0);
    replay(&dl, &store, &mut renderer);

    assert_eq!(renderer.calls.len(), 2);
    assert_eq!(renderer.calls[0], "circle(5.00,10.00,1.00)");
    assert_eq!(renderer.calls[1], "circle(15.00,10.00,1.00)");
}

#[test]
fn replay_lines_grob_needs_at_least_two_points() {
    let mut store = GrobStore::new();
    // Only 1 point - should not draw
    let lines_1 = store.add(Grob::Lines {
        x: Unit::npc(0.5),
        y: Unit::npc(0.5),
        gp: Gpar::new(),
    });
    // 3 points - should draw
    let lines_3 = store.add(Grob::Lines {
        x: Unit {
            values: vec![0.0, 0.5, 1.0],
            units: vec![UnitType::Npc, UnitType::Npc, UnitType::Npc],
        },
        y: Unit {
            values: vec![0.0, 0.5, 1.0],
            units: vec![UnitType::Npc, UnitType::Npc, UnitType::Npc],
        },
        gp: Gpar::new(),
    });

    let mut dl = DisplayList::new();
    dl.record(DisplayItem::Draw(lines_1));
    dl.record(DisplayItem::Draw(lines_3));

    let mut renderer = MockRenderer::new(20.0, 20.0);
    replay(&dl, &store, &mut renderer);

    assert_eq!(renderer.calls.len(), 1);
    assert_eq!(renderer.calls[0], "polyline(n=3)");
}

// endregion

// region: GridLayout tests

#[test]
fn grid_layout_uniform() {
    use r::interpreter::grid::viewport::GridLayout;
    let layout = GridLayout::uniform(2, 3);
    assert_eq!(layout.nrow, 2);
    assert_eq!(layout.ncol, 3);
    assert_eq!(layout.heights.len(), 2);
    assert_eq!(layout.widths.len(), 3);
    assert!((layout.heights[0].value() - 0.5).abs() < 1e-12);
    assert!((layout.widths[0].value() - 1.0 / 3.0).abs() < 1e-12);
}

// endregion
