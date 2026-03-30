//! Color palette generation functions — `rainbow()`, `heat.colors()`,
//! `terrain.colors()`, `topo.colors()`, `cm.colors()`, `gray.colors()`,
//! `hsv()`, `hcl()`, and `colorRampPalette()`.

use crate::interpreter::builtins::CallArgs;
use crate::interpreter::environment::Environment;
use crate::interpreter::value::*;
use crate::interpreter::BuiltinContext;
use crate::parser::ast::{Arg, Expr, Param};
use minir_macros::{builtin, interpreter_builtin};

// region: Color space conversions

/// Convert HSV (h in [0,1], s in [0,1], v in [0,1]) to RGB (each in [0,1]).
fn hsv_to_rgb(h: f64, s: f64, v: f64) -> (f64, f64, f64) {
    if s <= 0.0 {
        return (v, v, v);
    }
    // Wrap h to [0,1)
    let h = h - h.floor();
    let h6 = h * 6.0;
    let sector = h6.floor();
    let f = h6 - sector;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));

    let sector_i = sector as i32;
    match sector_i {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q), // sector 5
    }
}

/// Convert HCL (Hue in degrees, Chroma, Luminance) to RGB (each in [0,1]).
/// Uses the CIE LCH -> Lab -> XYZ -> sRGB pipeline.
fn hcl_to_rgb(h_deg: f64, c: f64, l: f64) -> (f64, f64, f64) {
    // LCH -> Lab
    let h_rad = h_deg.to_radians();
    let lab_a = c * h_rad.cos();
    let lab_b = c * h_rad.sin();

    // Lab -> XYZ (D65 illuminant)
    const XN: f64 = 0.950_470;
    const YN: f64 = 1.0;
    const ZN: f64 = 1.088_830;
    const KAPPA: f64 = 903.296_3; // (29/3)^3
    const EPSILON: f64 = 0.008_856; // (6/29)^3

    let fy = (l + 16.0) / 116.0;
    let fx = fy + lab_a / 500.0;
    let fz = fy - lab_b / 200.0;

    let x = if fx.powi(3) > EPSILON {
        XN * fx.powi(3)
    } else {
        XN * (116.0 * fx - 16.0) / KAPPA
    };
    let y = if l > KAPPA * EPSILON {
        YN * fy.powi(3)
    } else {
        YN * l / KAPPA
    };
    let z = if fz.powi(3) > EPSILON {
        ZN * fz.powi(3)
    } else {
        ZN * (116.0 * fz - 16.0) / KAPPA
    };

    // XYZ -> linear sRGB (D65)
    let rl = 3.240_479_f64 * x - 1.537_150 * y - 0.498_535 * z;
    let gl = -0.969_256_f64 * x + 1.875_992 * y + 0.041_556 * z;
    let bl = 0.055_648_f64 * x - 0.204_043 * y + 1.057_311 * z;

    // Linear -> sRGB gamma
    fn gamma(u: f64) -> f64 {
        if u <= 0.003_130_8 {
            12.92 * u
        } else {
            1.055 * u.powf(1.0 / 2.4) - 0.055
        }
    }

    (
        gamma(rl).clamp(0.0, 1.0),
        gamma(gl).clamp(0.0, 1.0),
        gamma(bl).clamp(0.0, 1.0),
    )
}

/// Parse a hex color string like "#RRGGBB" or "#RRGGBBAA" into (r, g, b, a) with values in [0,1].
fn parse_hex_color(s: &str) -> Result<(f64, f64, f64, f64), RError> {
    let s = s.trim_start_matches('#');
    let (r, g, b, a) = match s.len() {
        6 => {
            let r = u8::from_str_radix(&s[0..2], 16);
            let g = u8::from_str_radix(&s[2..4], 16);
            let b = u8::from_str_radix(&s[4..6], 16);
            match (r, g, b) {
                (Ok(r), Ok(g), Ok(b)) => (r, g, b, 255u8),
                _ => {
                    return Err(RError::new(
                        RErrorKind::Argument,
                        format!("invalid hex color: #{s}"),
                    ))
                }
            }
        }
        8 => {
            let r = u8::from_str_radix(&s[0..2], 16);
            let g = u8::from_str_radix(&s[2..4], 16);
            let b = u8::from_str_radix(&s[4..6], 16);
            let a = u8::from_str_radix(&s[6..8], 16);
            match (r, g, b, a) {
                (Ok(r), Ok(g), Ok(b), Ok(a)) => (r, g, b, a),
                _ => {
                    return Err(RError::new(
                        RErrorKind::Argument,
                        format!("invalid hex color: #{s}"),
                    ))
                }
            }
        }
        _ => {
            return Err(RError::new(
                RErrorKind::Argument,
                format!("invalid hex color: #{s} (expected 6 or 8 hex digits)"),
            ))
        }
    };
    Ok((
        f64::from(r) / 255.0,
        f64::from(g) / 255.0,
        f64::from(b) / 255.0,
        f64::from(a) / 255.0,
    ))
}

/// Format (r, g, b, a) each in [0,1] as a hex color string.
/// If alpha is 1.0, returns "#RRGGBB"; otherwise "#RRGGBBAA".
fn rgb_to_hex(r: f64, g: f64, b: f64, a: f64) -> String {
    let ri = (r.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
    let gi = (g.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
    let bi = (b.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
    if (a - 1.0).abs() < 1e-10 {
        format!("#{:02X}{:02X}{:02X}", ri, gi, bi)
    } else {
        let ai = (a.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
        format!("#{:02X}{:02X}{:02X}{:02X}", ri, gi, bi, ai)
    }
}

// endregion

// region: Helper to extract double from value

/// Extract a scalar f64 from an RValue, returning a default if NULL or missing.
fn double_scalar(val: Option<&RValue>, default: f64) -> f64 {
    match val {
        Some(RValue::Null) | None => default,
        Some(v) => v
            .as_vector()
            .and_then(|v| v.as_double_scalar())
            .unwrap_or(default),
    }
}

/// Extract n (first positional arg, required) as a non-negative integer.
fn extract_n(args: &CallArgs) -> Result<usize, RError> {
    let n_val = args.value("n", 0).ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "argument 'n' is missing, with no default".to_string(),
        )
    })?;
    let n = n_val
        .as_vector()
        .and_then(|v| v.as_integer_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "'n' must be a positive integer".to_string(),
            )
        })?;
    if n < 0 {
        return Err(RError::new(
            RErrorKind::Argument,
            "'n' must be a non-negative integer".to_string(),
        ));
    }
    Ok(n as usize)
}

// endregion

// region: hsv() builtin

/// Convert HSV color values to hex color strings.
///
/// Vectorized over h, s, v, alpha — all inputs are recycled to the length
/// of the longest input.
///
/// @param h hue, values in [0,1] (default 0)
/// @param s saturation, values in [0,1] (default 1)
/// @param v value (brightness), values in [0,1] (default 1)
/// @param alpha transparency, values in [0,1] (default 1)
/// @return character vector of hex color strings
#[builtin(namespace = "grDevices")]
fn builtin_hsv(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let ca = CallArgs::new(args, named);

    // Get vectorized inputs
    let h_vec = extract_doubles(&ca, "h", 0, 0.0);
    let s_vec = extract_doubles(&ca, "s", 1, 1.0);
    let v_vec = extract_doubles(&ca, "v", 2, 1.0);
    let alpha_vec = extract_doubles(&ca, "alpha", 3, 1.0);

    let n = h_vec
        .len()
        .max(s_vec.len())
        .max(v_vec.len())
        .max(alpha_vec.len());

    let mut result = Vec::with_capacity(n);
    for i in 0..n {
        let h = h_vec[i % h_vec.len()];
        let s = s_vec[i % s_vec.len()];
        let v = v_vec[i % v_vec.len()];
        let a = alpha_vec[i % alpha_vec.len()];
        let (r, g, b) = hsv_to_rgb(h, s, v);
        result.push(Some(rgb_to_hex(r, g, b, a)));
    }

    Ok(RValue::vec(Vector::Character(result.into())))
}

/// Extract a vector of doubles from named/positional arg, defaulting to a single-element vector.
fn extract_doubles(ca: &CallArgs, name: &str, pos: usize, default: f64) -> Vec<f64> {
    match ca.value(name, pos) {
        Some(RValue::Vector(rv)) => {
            let doubles = rv.to_doubles();
            if doubles.is_empty() {
                vec![default]
            } else {
                doubles.into_iter().map(|d| d.unwrap_or(default)).collect()
            }
        }
        Some(RValue::Null) | None => vec![default],
        _ => vec![default],
    }
}

// endregion

// region: hcl() builtin

/// Convert HCL (Hue-Chroma-Luminance) color values to hex color strings.
///
/// Vectorized over h, c, l, alpha — all inputs are recycled to the length
/// of the longest input.
///
/// @param h hue in degrees [0,360] (default 0)
/// @param c chroma (default 35)
/// @param l luminance in [0,100] (default 85)
/// @param alpha transparency in [0,1] (default 1)
/// @return character vector of hex color strings
#[builtin(namespace = "grDevices")]
fn builtin_hcl(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let ca = CallArgs::new(args, named);

    let h_vec = extract_doubles(&ca, "h", 0, 0.0);
    let c_vec = extract_doubles(&ca, "c", 1, 35.0);
    let l_vec = extract_doubles(&ca, "l", 2, 85.0);
    let alpha_vec = extract_doubles(&ca, "alpha", 3, 1.0);

    let n = h_vec
        .len()
        .max(c_vec.len())
        .max(l_vec.len())
        .max(alpha_vec.len());

    let mut result = Vec::with_capacity(n);
    for i in 0..n {
        let h = h_vec[i % h_vec.len()];
        let c = c_vec[i % c_vec.len()];
        let l = l_vec[i % l_vec.len()];
        let a = alpha_vec[i % alpha_vec.len()];
        let (r, g, b) = hcl_to_rgb(h, c, l);
        result.push(Some(rgb_to_hex(r, g, b, a)));
    }

    Ok(RValue::vec(Vector::Character(result.into())))
}

// endregion

// region: rainbow()

/// Generate a rainbow color palette using HSV color space.
///
/// @param n number of colors to generate
/// @param s saturation (default 1)
/// @param v value/brightness (default 1)
/// @param start starting hue in [0,1] (default 0)
/// @param end ending hue (default max(1, n-1)/n)
/// @param alpha transparency in [0,1] (default 1)
/// @return character vector of hex color strings
#[builtin(namespace = "grDevices", min_args = 1)]
fn builtin_rainbow(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let ca = CallArgs::new(args, named);
    let n = extract_n(&ca)?;

    if n == 0 {
        return Ok(RValue::vec(Vector::Character(
            Vec::<Option<String>>::new().into(),
        )));
    }

    let s = double_scalar(ca.value("s", 1), 1.0);
    let v = double_scalar(ca.value("v", 2), 1.0);
    let start = double_scalar(ca.value("start", 3), 0.0);
    let default_end = if n > 1 {
        (n as f64 - 1.0) / n as f64
    } else {
        1.0
    };
    let end = double_scalar(ca.value("end", 4), default_end);
    let alpha = double_scalar(ca.value("alpha", 5), 1.0);

    let mut result = Vec::with_capacity(n);
    for i in 0..n {
        let h = if n == 1 {
            start
        } else {
            start + (end - start) * (i as f64) / (n as f64 - 1.0)
        };
        let (r, g, b) = hsv_to_rgb(h, s, v);
        result.push(Some(rgb_to_hex(r, g, b, alpha)));
    }

    Ok(RValue::vec(Vector::Character(result.into())))
}

// endregion

// region: heat.colors()

/// Generate a heat map color palette (red through yellow to white).
///
/// @param n number of colors to generate
/// @param alpha transparency in [0,1] (default 1)
/// @return character vector of hex color strings
#[builtin(name = "heat.colors", namespace = "grDevices", min_args = 1)]
fn builtin_heat_colors(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let ca = CallArgs::new(args, named);
    let n = extract_n(&ca)?;

    if n == 0 {
        return Ok(RValue::vec(Vector::Character(
            Vec::<Option<String>>::new().into(),
        )));
    }

    let alpha = double_scalar(ca.value("alpha", 1), 1.0);

    // R's heat.colors: first 1/4 are pure reds with increasing intensity,
    // next 1/4 go from red to yellow, then yellow to white.
    // Simplified: n colors from red (h=0) through yellow (h=1/6) with
    // increasing value and saturation changes.
    // Actual R implementation uses hsv:
    //   j <- n %/% 4; i <- 1:n
    //   hsv(h = 1/6 * (i-1)/(j-1) clamped, s = 1 - (i-1)/n clamped, v = 1)
    // More precisely from R source:
    //   heat.colors(n) uses:
    //     h = (1:n - 1) / (3 * n)   # hue from 0 to 1/3
    //     s = 1 - (1:n - 1) / (n - 1) when n > 1  # saturation from 1 to 0
    //     v = 1
    // Actually in R source, heat.colors is defined as:
    //   j <- n %/% 4
    //   i <- 1:n
    //   c(rainbow(n - j, start = 0, end = 1/6),
    //     if (j > 0) hsv(h = 1/6, s = seq.int(1 - 1/(2*j), 1/(2*j), length.out = j), v = 1))
    let j = n / 4;
    let nrainbow = n - j;

    let mut result = Vec::with_capacity(n);

    // First part: rainbow from red to yellow
    for i in 0..nrainbow {
        let h = if nrainbow == 1 {
            0.0
        } else {
            (1.0 / 6.0) * (i as f64) / (nrainbow as f64 - 1.0)
        };
        let (r, g, b) = hsv_to_rgb(h, 1.0, 1.0);
        result.push(Some(rgb_to_hex(r, g, b, alpha)));
    }

    // Second part: yellow to white (decreasing saturation at h=1/6)
    if j > 0 {
        for i in 0..j {
            let s = if j == 1 {
                0.5
            } else {
                let start_s = 1.0 - 1.0 / (2.0 * j as f64);
                let end_s = 1.0 / (2.0 * j as f64);
                start_s + (end_s - start_s) * (i as f64) / (j as f64 - 1.0)
            };
            let (r, g, b) = hsv_to_rgb(1.0 / 6.0, s, 1.0);
            result.push(Some(rgb_to_hex(r, g, b, alpha)));
        }
    }

    Ok(RValue::vec(Vector::Character(result.into())))
}

// endregion

// region: terrain.colors()

/// Generate a terrain color palette (green through yellow/brown to white).
///
/// @param n number of colors to generate
/// @param alpha transparency in [0,1] (default 1)
/// @return character vector of hex color strings
#[builtin(name = "terrain.colors", namespace = "grDevices", min_args = 1)]
fn builtin_terrain_colors(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let ca = CallArgs::new(args, named);
    let n = extract_n(&ca)?;

    if n == 0 {
        return Ok(RValue::vec(Vector::Character(
            Vec::<Option<String>>::new().into(),
        )));
    }

    let alpha = double_scalar(ca.value("alpha", 1), 1.0);

    // R's terrain.colors uses:
    //   k <- n %/% 2
    //   h <- c(4/12, 2/12, 0/12)   # green -> yellow -> red-ish
    //   terrain(n):
    //     j <- n %/% 3
    //     c(hsv(h=2/6 to 1/6, s=1, v=0.65 to 0.9), hsv(h=1/6, s=1 to 0, v=0.9 to 0.95), grey(0.95, 1.0))
    // Simplified implementation matching R's output:
    let j = n / 3;
    let k = n - 2 * j;

    let mut result = Vec::with_capacity(n);

    // First third: green to yellow (h: 2/6 -> 1/6, s=1, v=0.65 -> 0.9)
    for i in 0..j {
        let t = if j <= 1 {
            0.0
        } else {
            i as f64 / (j as f64 - 1.0)
        };
        let h = 2.0 / 6.0 + (1.0 / 6.0 - 2.0 / 6.0) * t;
        let v = 0.65 + (0.9 - 0.65) * t;
        let (r, g, b) = hsv_to_rgb(h, 1.0, v);
        result.push(Some(rgb_to_hex(r, g, b, alpha)));
    }

    // Second third: yellow to near-white (h=1/6, s: 1->0, v: 0.9->0.95)
    for i in 0..j {
        let t = if j <= 1 {
            0.0
        } else {
            i as f64 / (j as f64 - 1.0)
        };
        let s = 1.0 - t;
        let v = 0.9 + (0.95 - 0.9) * t;
        let (r, g, b) = hsv_to_rgb(1.0 / 6.0, s, v);
        result.push(Some(rgb_to_hex(r, g, b, alpha)));
    }

    // Final third: grays from 0.95 to 1.0
    for i in 0..k {
        let t = if k <= 1 {
            0.0
        } else {
            i as f64 / (k as f64 - 1.0)
        };
        let grey = 0.95 + 0.05 * t;
        result.push(Some(rgb_to_hex(grey, grey, grey, alpha)));
    }

    Ok(RValue::vec(Vector::Character(result.into())))
}

// endregion

// region: topo.colors()

/// Generate a topographic color palette (blue through green to yellow).
///
/// @param n number of colors to generate
/// @param alpha transparency in [0,1] (default 1)
/// @return character vector of hex color strings
#[builtin(name = "topo.colors", namespace = "grDevices", min_args = 1)]
fn builtin_topo_colors(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let ca = CallArgs::new(args, named);
    let n = extract_n(&ca)?;

    if n == 0 {
        return Ok(RValue::vec(Vector::Character(
            Vec::<Option<String>>::new().into(),
        )));
    }

    let alpha = double_scalar(ca.value("alpha", 1), 1.0);

    // R's topo.colors uses a fixed set of anchor colors interpolated:
    // From R source: c(hsv(h=43/60, s=1, v=seq(0.4,1,...)),
    //                  hsv(h=seq(43,31)/60, s=1, v=1),
    //                  hsv(h=seq(31,23)/60, s=1, v=1),
    //                  hsv(h=seq(23,11)/60, s=seq(1,0,...), v=1))
    // Simplified: interpolate between blue -> cyan -> green -> yellow
    let anchors: [(f64, f64, f64); 5] = [
        (0.55, 0.0, 1.0), // deep blue (dark)
        (0.55, 1.0, 1.0), // blue
        (0.43, 1.0, 1.0), // cyan
        (0.25, 1.0, 1.0), // green
        (0.17, 0.0, 1.0), // light yellow
    ];

    let mut result = Vec::with_capacity(n);
    for i in 0..n {
        let t = if n == 1 {
            0.0
        } else {
            i as f64 / (n as f64 - 1.0)
        };
        let pos = t * (anchors.len() - 1) as f64;
        let idx = (pos.floor() as usize).min(anchors.len() - 2);
        let frac = pos - idx as f64;

        let (h1, s1, v1) = anchors[idx];
        let (h2, s2, v2) = anchors[idx + 1];
        let h = h1 + (h2 - h1) * frac;
        let s = s1 + (s2 - s1) * frac;
        let v = v1 + (v2 - v1) * frac;

        let (r, g, b) = hsv_to_rgb(h, s, v);
        result.push(Some(rgb_to_hex(r, g, b, alpha)));
    }

    Ok(RValue::vec(Vector::Character(result.into())))
}

// endregion

// region: cm.colors()

/// Generate a cyan-magenta diverging color palette.
///
/// @param n number of colors to generate
/// @param alpha transparency in [0,1] (default 1)
/// @return character vector of hex color strings
#[builtin(name = "cm.colors", namespace = "grDevices", min_args = 1)]
fn builtin_cm_colors(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let ca = CallArgs::new(args, named);
    let n = extract_n(&ca)?;

    if n == 0 {
        return Ok(RValue::vec(Vector::Character(
            Vec::<Option<String>>::new().into(),
        )));
    }

    let alpha = double_scalar(ca.value("alpha", 1), 1.0);

    // R's cm.colors: cyan (#00FFFF) -> white (#FFFFFF) -> magenta (#FF00FF)
    // i <- 1:n; even <- n %% 2 == 0
    // For odd n, the middle color is white.
    // Lower half: cyan to white; upper half: white to magenta.
    let mut result = Vec::with_capacity(n);
    for i in 0..n {
        let t = if n == 1 {
            0.5
        } else {
            i as f64 / (n as f64 - 1.0)
        };
        let (r, g, b) = if t < 0.5 {
            // Cyan to white
            let s = t * 2.0; // 0 at i=0, 1 at midpoint
            (s, 1.0, 1.0)
        } else {
            // White to magenta
            let s = (t - 0.5) * 2.0; // 0 at midpoint, 1 at i=n-1
            (1.0, 1.0 - s, 1.0)
        };
        result.push(Some(rgb_to_hex(r, g, b, alpha)));
    }

    Ok(RValue::vec(Vector::Character(result.into())))
}

// endregion

// region: gray.colors()

/// Generate a gray-scale color palette.
///
/// @param n number of colors to generate
/// @param start starting gray level in [0,1] (default 0.3)
/// @param end ending gray level in [0,1] (default 0.9)
/// @param gamma gamma correction (default 2.2)
/// @param alpha transparency in [0,1] (default 1)
/// @return character vector of hex color strings
#[builtin(name = "gray.colors", namespace = "grDevices", min_args = 1, names = ["grey.colors"])]
fn builtin_gray_colors(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let ca = CallArgs::new(args, named);
    let n = extract_n(&ca)?;

    if n == 0 {
        return Ok(RValue::vec(Vector::Character(
            Vec::<Option<String>>::new().into(),
        )));
    }

    let start = double_scalar(ca.value("start", 1), 0.3);
    let end = double_scalar(ca.value("end", 2), 0.9);
    let gamma = double_scalar(ca.value("gamma", 3), 2.2);
    let alpha = double_scalar(ca.value("alpha", 4), 1.0);

    let mut result = Vec::with_capacity(n);
    for i in 0..n {
        let t = if n == 1 {
            0.0
        } else {
            i as f64 / (n as f64 - 1.0)
        };
        // Apply gamma correction like R does
        let grey = (start + (end - start) * t).powf(1.0 / gamma);
        let grey = grey.clamp(0.0, 1.0);
        result.push(Some(rgb_to_hex(grey, grey, grey, alpha)));
    }

    Ok(RValue::vec(Vector::Character(result.into())))
}

// endregion

// region: grey() / gray()

/// Convert grey levels to hex color strings.
///
/// Vectorized: accepts a numeric vector of levels in [0,1] where 0=black, 1=white.
///
/// @param level numeric vector of grey levels in [0,1]
/// @param alpha optional transparency in [0,1] (default 1, fully opaque)
/// @return character vector of hex color strings
#[builtin(name = "grey", namespace = "grDevices", min_args = 1, names = ["gray"])]
fn builtin_grey(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let ca = CallArgs::new(args, named);
    let levels = extract_doubles(&ca, "level", 0, 0.0);
    let alpha_val = ca.value("alpha", 1);
    let alpha = match alpha_val {
        Some(RValue::Null) | None => None,
        Some(v) => Some(
            v.as_vector()
                .and_then(|rv| rv.as_double_scalar())
                .unwrap_or(1.0),
        ),
    };

    let mut result = Vec::with_capacity(levels.len());
    for level in &levels {
        let g = level.clamp(0.0, 1.0);
        let a = alpha.unwrap_or(1.0);
        result.push(Some(rgb_to_hex(g, g, g, a)));
    }

    Ok(RValue::vec(Vector::Character(result.into())))
}

// endregion

// region: colorRampPalette()

/// Create a color interpolation function from a vector of colors.
///
/// Returns a function that takes an integer n and produces n interpolated
/// colors between the input colors.
///
/// @param colors character vector of hex color strings
/// @return a function(n) that generates n interpolated colors
#[interpreter_builtin(name = "colorRampPalette", namespace = "grDevices", min_args = 1)]
fn interp_color_ramp_palette(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let ca = CallArgs::new(args, named);

    // Extract the colors argument — must be a character vector of hex colors
    let colors_val = ca.value("colors", 0).ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "argument 'colors' is missing".to_string(),
        )
    })?;

    let colors_vec = match colors_val {
        RValue::Vector(rv) => match &rv.inner {
            Vector::Character(cv) => cv.iter().filter_map(|s| s.clone()).collect::<Vec<String>>(),
            _ => {
                return Err(RError::new(
                    RErrorKind::Argument,
                    "'colors' must be a character vector of color values".to_string(),
                ))
            }
        },
        _ => {
            return Err(RError::new(
                RErrorKind::Argument,
                "'colors' must be a character vector of color values".to_string(),
            ))
        }
    };

    if colors_vec.len() < 2 {
        return Err(RError::new(
            RErrorKind::Argument,
            "colorRampPalette requires at least 2 colors".to_string(),
        ));
    }

    // Store the colors in a closure environment so the returned function can use them
    let env = context.env();
    let closure_env = Environment::new_child(env);
    closure_env.set(
        ".CRP_COLORS".to_string(),
        RValue::vec(Vector::Character(
            colors_vec
                .into_iter()
                .map(Some)
                .collect::<Vec<Option<String>>>()
                .into(),
        )),
    );

    // Build: function(n) .colorRampInterp(.CRP_COLORS, n)
    let body = Expr::Call {
        func: Box::new(Expr::Symbol(".colorRampInterp".to_string())),
        args: vec![
            Arg {
                name: None,
                value: Some(Expr::Symbol(".CRP_COLORS".to_string())),
            },
            Arg {
                name: None,
                value: Some(Expr::Symbol("n".to_string())),
            },
        ],
    };

    let params = vec![Param {
        name: "n".to_string(),
        default: None,
        is_dots: false,
    }];

    Ok(RValue::Function(RFunction::Closure {
        params,
        body,
        env: closure_env,
    }))
}

/// Internal helper: interpolate between a vector of hex colors to produce n colors.
///
/// This is the hidden builtin that colorRampPalette closures call.
///
/// @param colors character vector of hex colors
/// @param n number of output colors
/// @return character vector of n interpolated hex colors
#[builtin(name = ".colorRampInterp", namespace = "grDevices", min_args = 2)]
fn builtin_color_ramp_interp(
    args: &[RValue],
    _named: &[(String, RValue)],
) -> Result<RValue, RError> {
    let colors_val = args.first().ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "missing 'colors' argument".to_string(),
        )
    })?;
    let n_val = args
        .get(1)
        .ok_or_else(|| RError::new(RErrorKind::Argument, "missing 'n' argument".to_string()))?;

    let colors: Vec<String> = match colors_val {
        RValue::Vector(rv) => match &rv.inner {
            Vector::Character(cv) => cv.iter().filter_map(|s| s.clone()).collect(),
            _ => {
                return Err(RError::new(
                    RErrorKind::Argument,
                    "colors must be a character vector".to_string(),
                ))
            }
        },
        _ => {
            return Err(RError::new(
                RErrorKind::Argument,
                "colors must be a character vector".to_string(),
            ))
        }
    };

    let n = n_val
        .as_vector()
        .and_then(|v| v.as_integer_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "'n' must be a positive integer".to_string(),
            )
        })?;

    if n < 0 {
        return Err(RError::new(
            RErrorKind::Argument,
            "'n' must be a non-negative integer".to_string(),
        ));
    }
    let n = n as usize;

    if n == 0 {
        return Ok(RValue::vec(Vector::Character(
            Vec::<Option<String>>::new().into(),
        )));
    }

    // Parse all anchor colors into RGBA
    let mut anchors = Vec::with_capacity(colors.len());
    for c in &colors {
        anchors.push(parse_hex_color(c)?);
    }

    let mut result = Vec::with_capacity(n);
    for i in 0..n {
        let t = if n == 1 {
            0.0
        } else {
            i as f64 / (n as f64 - 1.0)
        };
        let pos = t * (anchors.len() - 1) as f64;
        let idx = (pos.floor() as usize).min(anchors.len() - 2);
        let frac = pos - idx as f64;

        let (r1, g1, b1, a1) = anchors[idx];
        let (r2, g2, b2, a2) = anchors[idx + 1];
        let r = r1 + (r2 - r1) * frac;
        let g = g1 + (g2 - g1) * frac;
        let b = b1 + (b2 - b1) * frac;
        let a = a1 + (a2 - a1) * frac;

        result.push(Some(rgb_to_hex(r, g, b, a)));
    }

    Ok(RValue::vec(Vector::Character(result.into())))
}

// endregion
