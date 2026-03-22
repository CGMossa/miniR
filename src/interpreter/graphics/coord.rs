//! Coordinate transform helpers for mapping between user and device coordinate systems.
//!
//! R's graphics system uses two coordinate spaces:
//! - **User coordinates** (`usr`): the data-space range set by `plot.window(xlim, ylim)`
//! - **Device coordinates** (`dev`): pixel/point positions on the output canvas
//!
//! The transform is a simple linear map. Note that device Y is typically
//! inverted (Y=0 at top) while user Y increases upward, so `usr_to_dev_y`
//! reverses the mapping direction.

/// Linear mapping between user (data) coordinates and device (pixel/point) coordinates.
#[derive(Debug, Clone)]
pub struct CoordTransform {
    /// User-coordinate limits: [xmin, xmax, ymin, ymax].
    pub usr: [f64; 4],
    /// Device-coordinate limits: [xmin, xmax, ymin, ymax].
    pub dev: [f64; 4],
}

impl CoordTransform {
    /// Create a new coordinate transform from user and device bounds.
    pub fn new(usr: [f64; 4], dev: [f64; 4]) -> Self {
        CoordTransform { usr, dev }
    }

    /// Map a user X coordinate to device X.
    ///
    /// Linear interpolation: `dev_xmin + (x - usr_xmin) / (usr_xmax - usr_xmin) * (dev_xmax - dev_xmin)`
    pub fn usr_to_dev_x(&self, x: f64) -> f64 {
        let usr_range = self.usr[1] - self.usr[0];
        if usr_range == 0.0 {
            return (self.dev[0] + self.dev[1]) / 2.0;
        }
        self.dev[0] + (x - self.usr[0]) / usr_range * (self.dev[1] - self.dev[0])
    }

    /// Map a user Y coordinate to device Y.
    ///
    /// Device Y is typically flipped (0 at top, max at bottom), so this maps
    /// user ymin to device ymax and user ymax to device ymin.
    pub fn usr_to_dev_y(&self, y: f64) -> f64 {
        let usr_range = self.usr[3] - self.usr[2];
        if usr_range == 0.0 {
            return (self.dev[2] + self.dev[3]) / 2.0;
        }
        // Flip: usr ymin -> dev ymax, usr ymax -> dev ymin
        self.dev[3] - (y - self.usr[2]) / usr_range * (self.dev[3] - self.dev[2])
    }

    /// Map a device X coordinate back to user X (inverse of `usr_to_dev_x`).
    pub fn dev_to_usr_x(&self, x: f64) -> f64 {
        let dev_range = self.dev[1] - self.dev[0];
        if dev_range == 0.0 {
            return (self.usr[0] + self.usr[1]) / 2.0;
        }
        self.usr[0] + (x - self.dev[0]) / dev_range * (self.usr[1] - self.usr[0])
    }

    /// Map a device Y coordinate back to user Y (inverse of `usr_to_dev_y`).
    pub fn dev_to_usr_y(&self, y: f64) -> f64 {
        let dev_range = self.dev[3] - self.dev[2];
        if dev_range == 0.0 {
            return (self.usr[2] + self.usr[3]) / 2.0;
        }
        // Inverse of the flip in usr_to_dev_y
        self.usr[2] + (self.dev[3] - y) / dev_range * (self.usr[3] - self.usr[2])
    }
}

/// Compute "pretty" tick positions for axis labels, similar to R's `pretty()`.
///
/// Given a data range `[min, max]` and a target number of ticks `n`, returns
/// tick positions that:
/// - Use "nice" step sizes (1, 2, or 5 times a power of 10)
/// - Extend the range to nice boundaries
/// - Produce approximately `n` ticks
///
/// This implements a simplified version of R's pretty algorithm (Heckbert's
/// nice-numbers approach).
pub fn pretty_ticks(min: f64, max: f64, n: usize) -> Vec<f64> {
    if n == 0 || !min.is_finite() || !max.is_finite() {
        return vec![];
    }

    let (lo, hi) = if min < max {
        (min, max)
    } else if min > max {
        (max, min)
    } else {
        // min == max: center a range around the value
        if min == 0.0 {
            (-1.0, 1.0)
        } else {
            (min - min.abs() * 0.1, max + max.abs() * 0.1)
        }
    };

    let range = hi - lo;
    let step = nice_step(range, n);

    if step == 0.0 || !step.is_finite() {
        return vec![lo];
    }

    // Extend to nice boundaries
    let nice_min = (lo / step).floor() * step;
    let nice_max = (hi / step).ceil() * step;

    let mut ticks = Vec::new();
    let mut t = nice_min;
    // Safety: limit iterations to prevent infinite loops from floating-point edge cases
    let max_iters = n * 10 + 20;
    let mut iters = 0;
    while t <= nice_max + step * 0.5e-10 && iters < max_iters {
        // Round to remove floating-point noise
        let rounded = (t / step).round() * step;
        ticks.push(rounded);
        t += step;
        iters += 1;
    }

    ticks
}

/// Find a "nice" step size for approximately `n` intervals over `range`.
///
/// A nice number is 1, 2, or 5 times a power of 10 — the same set R uses.
fn nice_step(range: f64, n: usize) -> f64 {
    if range <= 0.0 || n == 0 {
        return 0.0;
    }

    let raw_step = range / n as f64;
    let magnitude = 10.0_f64.powf(raw_step.log10().floor());
    let fraction = raw_step / magnitude;

    // Round to the nearest nice fraction: 1, 2, 5, or 10
    let nice_fraction = if fraction < 1.5 {
        1.0
    } else if fraction < 3.5 {
        2.0
    } else if fraction < 7.5 {
        5.0
    } else {
        10.0
    };

    nice_fraction * magnitude
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coord_round_trip_x() {
        let ct = CoordTransform::new([0.0, 10.0, 0.0, 10.0], [0.0, 500.0, 0.0, 400.0]);
        for &x in &[0.0, 2.5, 5.0, 7.5, 10.0] {
            let dev_x = ct.usr_to_dev_x(x);
            let back = ct.dev_to_usr_x(dev_x);
            assert!(
                (back - x).abs() < 1e-10,
                "round-trip failed for x={x}: got {back}"
            );
        }
    }

    #[test]
    fn coord_round_trip_y() {
        let ct = CoordTransform::new([0.0, 10.0, 0.0, 10.0], [0.0, 500.0, 0.0, 400.0]);
        for &y in &[0.0, 2.5, 5.0, 7.5, 10.0] {
            let dev_y = ct.usr_to_dev_y(y);
            let back = ct.dev_to_usr_y(dev_y);
            assert!(
                (back - y).abs() < 1e-10,
                "round-trip failed for y={y}: got {back}"
            );
        }
    }

    #[test]
    fn y_axis_flipped() {
        let ct = CoordTransform::new([0.0, 10.0, 0.0, 10.0], [0.0, 500.0, 0.0, 400.0]);
        // User ymin (0) should map to device ymax (400)
        assert!((ct.usr_to_dev_y(0.0) - 400.0).abs() < 1e-10);
        // User ymax (10) should map to device ymin (0)
        assert!((ct.usr_to_dev_y(10.0) - 0.0).abs() < 1e-10);
    }

    #[test]
    fn x_axis_not_flipped() {
        let ct = CoordTransform::new([0.0, 10.0, 0.0, 10.0], [0.0, 500.0, 0.0, 400.0]);
        assert!((ct.usr_to_dev_x(0.0) - 0.0).abs() < 1e-10);
        assert!((ct.usr_to_dev_x(10.0) - 500.0).abs() < 1e-10);
    }

    #[test]
    fn pretty_ticks_basic() {
        let ticks = pretty_ticks(0.0, 10.0, 5);
        // Should produce nice ticks like [0, 2, 4, 6, 8, 10]
        assert!(!ticks.is_empty());
        assert!(ticks.len() >= 3);
        assert!(ticks.len() <= 15);
        // First tick should be <= min, last tick should be >= max
        assert!(*ticks.first().unwrap() <= 0.0 + 1e-10);
        assert!(*ticks.last().unwrap() >= 10.0 - 1e-10);
    }

    #[test]
    fn pretty_ticks_small_range() {
        let ticks = pretty_ticks(0.0, 1.0, 5);
        assert!(!ticks.is_empty());
        // Step should be 0.2, so ticks = [0.0, 0.2, 0.4, 0.6, 0.8, 1.0]
        assert!(ticks.len() >= 3);
        assert!(ticks.len() <= 15);
    }

    #[test]
    fn pretty_ticks_large_range() {
        let ticks = pretty_ticks(0.0, 1_000_000.0, 5);
        assert!(!ticks.is_empty());
        // Should use steps of 200000
        for t in &ticks {
            assert!(t.is_finite());
        }
    }

    #[test]
    fn pretty_ticks_negative_range() {
        let ticks = pretty_ticks(-50.0, 50.0, 5);
        assert!(!ticks.is_empty());
        // Should include 0
        assert!(ticks.iter().any(|&t| t.abs() < 1e-10));
    }

    #[test]
    fn pretty_ticks_equal_min_max() {
        let ticks = pretty_ticks(5.0, 5.0, 5);
        assert!(!ticks.is_empty());
    }

    #[test]
    fn pretty_ticks_zero_n() {
        let ticks = pretty_ticks(0.0, 10.0, 0);
        assert!(ticks.is_empty());
    }

    #[test]
    fn coord_zero_range() {
        let ct = CoordTransform::new([5.0, 5.0, 3.0, 3.0], [0.0, 500.0, 0.0, 400.0]);
        // With zero user range, should map to center of device range
        assert!((ct.usr_to_dev_x(5.0) - 250.0).abs() < 1e-10);
        assert!((ct.usr_to_dev_y(3.0) - 200.0).abs() < 1e-10);
    }

    #[test]
    fn coord_non_origin_ranges() {
        let ct = CoordTransform::new([10.0, 20.0, -5.0, 5.0], [50.0, 450.0, 50.0, 350.0]);
        // x=15 (midpoint) -> dev midpoint = 250
        assert!((ct.usr_to_dev_x(15.0) - 250.0).abs() < 1e-10);
        // y=0 (midpoint) -> dev midpoint for flipped = 200
        assert!((ct.usr_to_dev_y(0.0) - 200.0).abs() < 1e-10);
    }
}
