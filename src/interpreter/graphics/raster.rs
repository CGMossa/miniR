//! SVG-to-raster conversion using resvg + tiny-skia.
//!
//! Converts an SVG string into a `tiny_skia::Pixmap` which can then be
//! encoded to PNG (via tiny-skia's built-in encoder), JPEG, or BMP
//! (via the `image` crate).

use crate::interpreter::value::{RError, RErrorKind};

/// Rasterize an SVG string into a pixel buffer.
///
/// Returns a `tiny_skia::Pixmap` of the given dimensions with the SVG
/// rendered onto a white background.
pub fn svg_to_raster(
    svg_str: &str,
    width_px: u32,
    height_px: u32,
) -> Result<tiny_skia::Pixmap, RError> {
    // Parse SVG into usvg tree
    let tree = usvg::Tree::from_str(svg_str, &usvg::Options::default()).map_err(|e| {
        RError::new(
            RErrorKind::Other,
            format!("failed to parse SVG for rasterization: {e}"),
        )
    })?;

    // Create pixel buffer
    let mut pixmap = tiny_skia::Pixmap::new(width_px, height_px).ok_or_else(|| {
        RError::new(
            RErrorKind::Other,
            format!("failed to create {width_px}x{height_px} pixel buffer"),
        )
    })?;

    // Fill with white background
    pixmap.fill(tiny_skia::Color::WHITE);

    // Compute transform to fit SVG viewbox into pixel dimensions
    let svg_size = tree.size();
    let sx = width_px as f32 / svg_size.width();
    let sy = height_px as f32 / svg_size.height();
    let transform = tiny_skia::Transform::from_scale(sx, sy);

    // Render
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    Ok(pixmap)
}

/// Encode a pixmap as JPEG bytes.
pub fn pixmap_to_jpeg(pixmap: &tiny_skia::Pixmap, quality: u8) -> Result<Vec<u8>, RError> {
    let width = pixmap.width();
    let height = pixmap.height();

    // Convert RGBA premultiplied -> RGB for JPEG (JPEG doesn't support alpha)
    let mut rgb_data = Vec::with_capacity((width * height * 3) as usize);
    for pixel in pixmap.pixels() {
        // tiny-skia stores premultiplied RGBA; demultiply
        let a = pixel.alpha() as f32 / 255.0;
        if a > 0.0 {
            rgb_data.push((pixel.red() as f32 / a).min(255.0) as u8);
            rgb_data.push((pixel.green() as f32 / a).min(255.0) as u8);
            rgb_data.push((pixel.blue() as f32 / a).min(255.0) as u8);
        } else {
            // Transparent → white background
            rgb_data.push(255);
            rgb_data.push(255);
            rgb_data.push(255);
        }
    }

    let img: image::ImageBuffer<image::Rgb<u8>, _> =
        image::ImageBuffer::from_raw(width, height, rgb_data).ok_or_else(|| {
            RError::new(
                RErrorKind::Other,
                "failed to create image buffer".to_string(),
            )
        })?;

    let mut buf = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut buf);
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut cursor, quality);
    image::ImageEncoder::write_image(
        encoder,
        img.as_raw(),
        width,
        height,
        image::ExtendedColorType::Rgb8,
    )
    .map_err(|e| RError::new(RErrorKind::Other, format!("JPEG encoding failed: {e}")))?;

    Ok(buf)
}

/// Encode a pixmap as BMP bytes.
pub fn pixmap_to_bmp(pixmap: &tiny_skia::Pixmap) -> Result<Vec<u8>, RError> {
    let width = pixmap.width();
    let height = pixmap.height();

    // BMP supports RGBA, but we'll use RGB for compatibility
    let mut rgb_data = Vec::with_capacity((width * height * 3) as usize);
    for pixel in pixmap.pixels() {
        let a = pixel.alpha() as f32 / 255.0;
        if a > 0.0 {
            rgb_data.push((pixel.red() as f32 / a).min(255.0) as u8);
            rgb_data.push((pixel.green() as f32 / a).min(255.0) as u8);
            rgb_data.push((pixel.blue() as f32 / a).min(255.0) as u8);
        } else {
            rgb_data.push(255);
            rgb_data.push(255);
            rgb_data.push(255);
        }
    }

    let img: image::ImageBuffer<image::Rgb<u8>, _> =
        image::ImageBuffer::from_raw(width, height, rgb_data).ok_or_else(|| {
            RError::new(
                RErrorKind::Other,
                "failed to create image buffer".to_string(),
            )
        })?;

    let mut buf = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut buf);
    let encoder = image::codecs::bmp::BmpEncoder::new(&mut cursor);
    image::ImageEncoder::write_image(
        encoder,
        img.as_raw(),
        width,
        height,
        image::ExtendedColorType::Rgb8,
    )
    .map_err(|e| RError::new(RErrorKind::Other, format!("BMP encoding failed: {e}")))?;

    Ok(buf)
}
