//! Grid renderer trait and display list replay.
//!
//! The `GridRenderer` trait defines the abstract drawing API that backends
//! (SVG, egui, PDF, etc.) must implement. The `replay` function walks a
//! display list and calls the renderer methods with resolved coordinates.

use super::display::{DisplayItem, DisplayList};
use super::gpar::Gpar;
use super::grob::{Grob, GrobStore};
use super::viewport::{ViewportStack, ViewportTransform};

// region: GridRenderer trait

/// Abstract renderer for grid graphics.
///
/// All coordinates are in centimeters from the device origin (bottom-left).
/// The renderer converts these to whatever coordinate system the backend uses.
pub trait GridRenderer {
    /// Draw a single line segment.
    fn line(&mut self, x0_cm: f64, y0_cm: f64, x1_cm: f64, y1_cm: f64, gp: &Gpar);

    /// Draw a connected polyline (multiple segments sharing endpoints).
    fn polyline(&mut self, x_cm: &[f64], y_cm: &[f64], gp: &Gpar);

    /// Draw a rectangle.
    fn rect(&mut self, x_cm: f64, y_cm: f64, w_cm: f64, h_cm: f64, gp: &Gpar);

    /// Draw a circle.
    fn circle(&mut self, x_cm: f64, y_cm: f64, r_cm: f64, gp: &Gpar);

    /// Draw a filled polygon.
    fn polygon(&mut self, x_cm: &[f64], y_cm: &[f64], gp: &Gpar);

    /// Draw a text label.
    fn text(&mut self, x_cm: f64, y_cm: f64, label: &str, rot: f64, gp: &Gpar);

    /// Draw a point (plotting symbol).
    fn point(&mut self, x_cm: f64, y_cm: f64, pch: u8, size_cm: f64, gp: &Gpar);

    /// Set a clipping rectangle. All subsequent drawing is clipped to this region.
    fn clip(&mut self, x_cm: f64, y_cm: f64, w_cm: f64, h_cm: f64);

    /// Remove the most recently set clipping rectangle.
    fn unclip(&mut self);

    /// Return the device size in centimeters (width, height).
    fn device_size_cm(&self) -> (f64, f64);
}

// endregion

// region: Replay

/// Replay a display list through a renderer, resolving all units to device cm.
///
/// This walks the display list items in order:
/// - `PushViewport` pushes a viewport and optionally clips
/// - `PopViewport` pops and unclips
/// - `Draw` resolves the grob's units against the current viewport transform
///   and calls the appropriate renderer method
pub fn replay(list: &DisplayList, store: &GrobStore, renderer: &mut dyn GridRenderer) {
    let (dev_w, dev_h) = renderer.device_size_cm();
    let mut vp_stack = ViewportStack::new(dev_w, dev_h);
    let mut transform = ViewportTransform::root(dev_w, dev_h);
    let mut transform_stack: Vec<ViewportTransform> = vec![transform.clone()];
    let mut clip_depth: usize = 0;

    for item in list.items() {
        match item {
            DisplayItem::PushViewport(vp) => {
                transform = ViewportTransform::from_viewport(vp, &transform);
                transform_stack.push(transform.clone());
                vp_stack.push((**vp).clone());

                if vp.clip {
                    renderer.clip(
                        transform.x_offset_cm,
                        transform.y_offset_cm,
                        transform.width_cm,
                        transform.height_cm,
                    );
                    clip_depth += 1;
                }
            }
            DisplayItem::PopViewport => {
                let popped = vp_stack.pop();
                if transform_stack.len() > 1 {
                    transform_stack.pop();
                }
                transform = transform_stack
                    .last()
                    .expect("transform stack always has at least the root")
                    .clone();

                if let Some(vp) = popped {
                    if vp.clip && clip_depth > 0 {
                        renderer.unclip();
                        clip_depth -= 1;
                    }
                }
            }
            DisplayItem::Draw(grob_id) => {
                if let Some(grob) = store.get(*grob_id) {
                    render_grob(grob, &transform, store, renderer);
                }
            }
        }
    }

    // Clean up any unclosed clips
    for _ in 0..clip_depth {
        renderer.unclip();
    }
}

/// Render a single grob using the current viewport transform.
fn render_grob(
    grob: &Grob,
    transform: &ViewportTransform,
    store: &GrobStore,
    renderer: &mut dyn GridRenderer,
) {
    let ctx = transform.unit_context();

    match grob {
        Grob::Lines { x, y, gp } => {
            let n = x.len().min(y.len());
            if n < 2 {
                return;
            }
            let x_cm: Vec<f64> = (0..n)
                .map(|i| transform.x_offset_cm + ctx.resolve_x(x, i))
                .collect();
            let y_cm: Vec<f64> = (0..n)
                .map(|i| transform.y_offset_cm + ctx.resolve_y(y, i))
                .collect();
            renderer.polyline(&x_cm, &y_cm, gp);
        }

        Grob::Segments { x0, y0, x1, y1, gp } => {
            let n = x0.len().min(y0.len()).min(x1.len()).min(y1.len());
            for i in 0..n {
                renderer.line(
                    transform.x_offset_cm + ctx.resolve_x(x0, i),
                    transform.y_offset_cm + ctx.resolve_y(y0, i),
                    transform.x_offset_cm + ctx.resolve_x(x1, i),
                    transform.y_offset_cm + ctx.resolve_y(y1, i),
                    gp,
                );
            }
        }

        Grob::Points {
            x,
            y,
            pch,
            size,
            gp,
        } => {
            let n = x.len().min(y.len());
            for i in 0..n {
                let size_cm = ctx.resolve_size(size, i.min(size.len().saturating_sub(1)));
                renderer.point(
                    transform.x_offset_cm + ctx.resolve_x(x, i),
                    transform.y_offset_cm + ctx.resolve_y(y, i),
                    *pch,
                    size_cm,
                    gp,
                );
            }
        }

        Grob::Rect {
            x,
            y,
            width,
            height,
            just,
            gp,
        } => {
            let n = x.len().min(y.len());
            for i in 0..n {
                let cx = transform.x_offset_cm + ctx.resolve_x(x, i);
                let cy = transform.y_offset_cm + ctx.resolve_y(y, i);
                let w = ctx.resolve_x(width, i.min(width.len().saturating_sub(1)));
                let h = ctx.resolve_y(height, i.min(height.len().saturating_sub(1)));
                let rx = cx - just.0.as_fraction() * w;
                let ry = cy - just.1.as_fraction() * h;
                renderer.rect(rx, ry, w, h, gp);
            }
        }

        Grob::Circle { x, y, r, gp } => {
            let n = x.len().min(y.len());
            for i in 0..n {
                let r_cm = ctx.resolve_size(r, i.min(r.len().saturating_sub(1)));
                renderer.circle(
                    transform.x_offset_cm + ctx.resolve_x(x, i),
                    transform.y_offset_cm + ctx.resolve_y(y, i),
                    r_cm,
                    gp,
                );
            }
        }

        Grob::Polygon { x, y, gp } => {
            let n = x.len().min(y.len());
            if n < 3 {
                return;
            }
            let x_cm: Vec<f64> = (0..n)
                .map(|i| transform.x_offset_cm + ctx.resolve_x(x, i))
                .collect();
            let y_cm: Vec<f64> = (0..n)
                .map(|i| transform.y_offset_cm + ctx.resolve_y(y, i))
                .collect();
            renderer.polygon(&x_cm, &y_cm, gp);
        }

        Grob::Text {
            label,
            x,
            y,
            just: _just,
            rot,
            gp,
        } => {
            let n = label.len().min(x.len()).min(y.len());
            for (i, lbl) in label.iter().enumerate().take(n) {
                renderer.text(
                    transform.x_offset_cm + ctx.resolve_x(x, i),
                    transform.y_offset_cm + ctx.resolve_y(y, i),
                    lbl,
                    *rot,
                    gp,
                );
            }
        }

        Grob::Collection { children } => {
            for &child_id in children {
                if let Some(child) = store.get(child_id) {
                    render_grob(child, transform, store, renderer);
                }
            }
        }
    }
}

// endregion
