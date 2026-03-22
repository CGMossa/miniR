//! PDF output via krilla + krilla-svg.
//!
//! Converts an SVG string (produced by the SVG renderer) into PDF bytes
//! by parsing the SVG with `usvg` and drawing it onto a krilla `Document`.

use krilla::geom::Size;
use krilla::page::PageSettings;
use krilla::Document;
use krilla_svg::{SurfaceExt, SvgSettings};

use crate::interpreter::value::{RError, RErrorKind};

/// Convert an SVG string to PDF bytes.
///
/// `width_pt` and `height_pt` are the page dimensions in points (72 pt = 1 inch),
/// matching the coordinate system used by the SVG renderer.
pub(crate) fn svg_to_pdf(svg_str: &str, width_pt: f32, height_pt: f32) -> Result<Vec<u8>, RError> {
    // Parse the SVG into a usvg tree.
    let opts = usvg::Options::default();
    let tree = usvg::Tree::from_str(svg_str, &opts).map_err(|e| {
        RError::new(
            RErrorKind::Other,
            format!("failed to parse SVG for PDF conversion: {e}"),
        )
    })?;

    // Create a krilla document with a single page matching the SVG dimensions.
    let mut document = Document::new();
    let size = Size::from_wh(width_pt, height_pt).ok_or_else(|| {
        RError::new(
            RErrorKind::Other,
            format!("invalid PDF page dimensions: {width_pt} x {height_pt}"),
        )
    })?;

    let mut page = document.start_page_with(PageSettings::new(width_pt, height_pt));
    let mut surface = page.surface();

    // Draw the SVG tree onto the PDF surface.
    surface.draw_svg(&tree, size, SvgSettings::default());

    surface.finish();
    page.finish();

    // Serialize the document to PDF bytes.
    let pdf_bytes = document
        .finish()
        .map_err(|e| RError::new(RErrorKind::Other, format!("failed to generate PDF: {e:?}")))?;

    Ok(pdf_bytes)
}
