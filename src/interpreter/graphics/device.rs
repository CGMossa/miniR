//! The `GraphicsDevice` trait — the core abstraction for all rendering backends.

use super::context::GraphicsContext;

/// Metrics for a single character glyph.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CharMetric {
    pub ascent: f64,
    pub descent: f64,
    pub width: f64,
}

/// The dimensions of a device's drawing surface, in points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeviceSize {
    pub width: f64,
    pub height: f64,
}

/// A graphics device capable of rendering R plot primitives.
///
/// Every drawing method receives a `GraphicsContext` describing the current
/// visual style (color, line type, font, etc.).  Backends implement this trait
/// to produce output on screen, to a file, or (for `NullDevice`) nowhere.
pub trait GraphicsDevice {
    /// Draw a circle centered at (x, y) with radius r.
    fn circle(&mut self, x: f64, y: f64, r: f64, gc: &GraphicsContext);

    /// Draw a line segment from (x0, y0) to (x1, y1).
    fn line(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, gc: &GraphicsContext);

    /// Draw a polyline through the given (x, y) coordinates.
    fn polyline(&mut self, x: &[f64], y: &[f64], gc: &GraphicsContext);

    /// Draw a filled polygon with the given (x, y) vertices.
    fn polygon(&mut self, x: &[f64], y: &[f64], gc: &GraphicsContext);

    /// Draw a rectangle from corner (x0, y0) to corner (x1, y1).
    fn rect(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, gc: &GraphicsContext);

    /// Draw text at (x, y) with rotation `rot` (degrees) and horizontal
    /// adjustment `hadj` (0=left, 0.5=center, 1=right).
    fn text(&mut self, x: f64, y: f64, text: &str, rot: f64, hadj: f64, gc: &GraphicsContext);

    /// Draw a compound path.  `nper` gives the number of vertices in each
    /// sub-path (summing to `x.len()`).  `winding` selects the fill rule.
    fn path(&mut self, x: &[f64], y: &[f64], nper: &[usize], winding: bool, gc: &GraphicsContext);

    /// Return the width of `text` in device units, using the font described
    /// by `gc`.
    fn str_width(&self, text: &str, gc: &GraphicsContext) -> f64;

    /// Return the ascent, descent and width of a single character.
    fn char_metric(&self, ch: char, gc: &GraphicsContext) -> CharMetric;

    /// Start a new page (clear the device).
    fn new_page(&mut self, gc: &GraphicsContext);

    /// Set the clipping rectangle.
    fn clip(&mut self, x0: f64, y0: f64, x1: f64, y1: f64);

    /// Return the current device size in points.
    fn size(&self) -> DeviceSize;

    /// Shut down the device, releasing any resources.
    fn close(&mut self);

    /// Called when this device becomes the active device.
    fn activate(&mut self) {}

    /// Called when another device becomes active.
    fn deactivate(&mut self) {}

    /// Human-readable device name (e.g. "pdf", "png", "null device").
    fn name(&self) -> &str;

    /// Whether this device supports interactive features (e.g. locator, event loop).
    fn is_interactive(&self) -> bool;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn char_metric_fields() {
        let m = CharMetric {
            ascent: 10.0,
            descent: 3.0,
            width: 7.0,
        };
        assert_eq!(m.ascent, 10.0);
        assert_eq!(m.descent, 3.0);
        assert_eq!(m.width, 7.0);
    }

    #[test]
    fn device_size_fields() {
        let s = DeviceSize {
            width: 504.0,
            height: 504.0,
        };
        assert_eq!(s.width, 504.0);
        assert_eq!(s.height, 504.0);
    }
}
