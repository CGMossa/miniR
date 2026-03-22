//! A no-op graphics device used as the default when no real device is open.

use super::context::GraphicsContext;
use super::device::{CharMetric, DeviceSize, GraphicsDevice};

/// A graphics device that silently discards all drawing operations.
///
/// This is the default device — equivalent to R's "null device" (device 1).
/// It reports a 504x504-point surface (7 inches at 72 dpi, R's default).
pub struct NullDevice;

impl GraphicsDevice for NullDevice {
    fn circle(&mut self, _x: f64, _y: f64, _r: f64, _gc: &GraphicsContext) {}

    fn line(&mut self, _x0: f64, _y0: f64, _x1: f64, _y1: f64, _gc: &GraphicsContext) {}

    fn polyline(&mut self, _x: &[f64], _y: &[f64], _gc: &GraphicsContext) {}

    fn polygon(&mut self, _x: &[f64], _y: &[f64], _gc: &GraphicsContext) {}

    fn rect(&mut self, _x0: f64, _y0: f64, _x1: f64, _y1: f64, _gc: &GraphicsContext) {}

    fn text(
        &mut self,
        _x: f64,
        _y: f64,
        _text: &str,
        _rot: f64,
        _hadj: f64,
        _gc: &GraphicsContext,
    ) {
    }

    fn path(
        &mut self,
        _x: &[f64],
        _y: &[f64],
        _nper: &[usize],
        _winding: bool,
        _gc: &GraphicsContext,
    ) {
    }

    fn str_width(&self, _text: &str, _gc: &GraphicsContext) -> f64 {
        0.0
    }

    fn char_metric(&self, _ch: char, _gc: &GraphicsContext) -> CharMetric {
        CharMetric {
            ascent: 0.0,
            descent: 0.0,
            width: 0.0,
        }
    }

    fn new_page(&mut self, _gc: &GraphicsContext) {}

    fn clip(&mut self, _x0: f64, _y0: f64, _x1: f64, _y1: f64) {}

    fn size(&self) -> DeviceSize {
        // R's default: 7 inches * 72 dpi = 504 points
        DeviceSize {
            width: 504.0,
            height: 504.0,
        }
    }

    fn close(&mut self) {}

    fn name(&self) -> &str {
        "null device"
    }

    fn is_interactive(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_device_name() {
        let dev = NullDevice;
        assert_eq!(dev.name(), "null device");
    }

    #[test]
    fn null_device_not_interactive() {
        let dev = NullDevice;
        assert!(!dev.is_interactive());
    }

    #[test]
    fn null_device_default_size() {
        let dev = NullDevice;
        let size = dev.size();
        assert_eq!(size.width, 504.0);
        assert_eq!(size.height, 504.0);
    }

    #[test]
    fn null_device_str_width_zero() {
        let dev = NullDevice;
        let gc = GraphicsContext::default();
        assert_eq!(dev.str_width("hello", &gc), 0.0);
    }

    #[test]
    fn null_device_char_metric_zero() {
        let dev = NullDevice;
        let gc = GraphicsContext::default();
        let m = dev.char_metric('A', &gc);
        assert_eq!(m.ascent, 0.0);
        assert_eq!(m.descent, 0.0);
        assert_eq!(m.width, 0.0);
    }

    #[test]
    fn null_device_drawing_ops_are_noop() {
        let mut dev = NullDevice;
        let gc = GraphicsContext::default();
        // These should all complete without panic or side effects.
        dev.circle(0.0, 0.0, 5.0, &gc);
        dev.line(0.0, 0.0, 10.0, 10.0, &gc);
        dev.polyline(&[0.0, 1.0], &[0.0, 1.0], &gc);
        dev.polygon(&[0.0, 1.0, 0.5], &[0.0, 0.0, 1.0], &gc);
        dev.rect(0.0, 0.0, 100.0, 100.0, &gc);
        dev.text(0.0, 0.0, "test", 0.0, 0.0, &gc);
        dev.path(&[0.0, 1.0, 0.5], &[0.0, 0.0, 1.0], &[3], true, &gc);
        dev.new_page(&gc);
        dev.clip(0.0, 0.0, 100.0, 100.0);
        dev.activate();
        dev.deactivate();
        dev.close();
    }

    /// Verify NullDevice is usable through a trait object.
    #[test]
    fn null_device_as_trait_object() {
        let mut dev: Box<dyn GraphicsDevice> = Box::new(NullDevice);
        let gc = GraphicsContext::default();
        dev.circle(1.0, 2.0, 3.0, &gc);
        assert_eq!(dev.name(), "null device");
        assert!(!dev.is_interactive());
        assert_eq!(dev.size().width, 504.0);
    }
}
