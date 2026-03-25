//! Grid graphical parameters (`gpar`) — per-grob styling with inheritance.
//!
//! Unlike base R's `par()` which is a global state, grid's `gpar()` creates
//! immutable parameter sets that are attached to individual grobs and viewports.
//! When a parameter is `None`, it inherits from the parent viewport's gpar.
//!
//! This module reuses `LineType` and `FontFace` from `graphics::par` to avoid
//! duplication.

use crate::interpreter::graphics::par::{FontFace, LineType};

// region: Gpar

/// Grid graphical parameter set.
///
/// Each field is `Option` — `None` means "inherit from parent viewport".
/// Use `inherit_from()` to fill in missing values from a parent `Gpar`,
/// or use the `effective_*()` methods to get resolved values with defaults.
#[derive(Clone, Debug, Default)]
pub struct Gpar {
    /// Stroke color as RGBA.
    pub col: Option<[u8; 4]>,
    /// Fill color as RGBA.
    pub fill: Option<[u8; 4]>,
    /// Line width (default 1.0).
    pub lwd: Option<f64>,
    /// Line type (solid, dashed, etc.).
    pub lty: Option<LineType>,
    /// Font size in points (default 12.0).
    pub fontsize: Option<f64>,
    /// Line height multiplier (default 1.2).
    pub lineheight: Option<f64>,
    /// Font face (plain, bold, italic, etc.).
    pub font: Option<FontFace>,
    /// Font family name.
    pub fontfamily: Option<String>,
    /// Character expansion factor (multiplier on fontsize).
    pub cex: Option<f64>,
    /// Alpha transparency (0.0 = fully transparent, 1.0 = fully opaque).
    pub alpha: Option<f64>,
    /// Line end style.
    pub lineend: Option<LineEnd>,
    /// Line join style.
    pub linejoin: Option<LineJoin>,
    /// Mitre limit for line joins.
    pub linemitre: Option<f64>,
    /// Horizontal justification (0 = left, 0.5 = center, 1 = right).
    pub just_x: Option<f64>,
    /// Vertical justification (0 = bottom, 0.5 = center, 1 = top).
    pub just_y: Option<f64>,
}

/// Line end cap styles, matching R's `lineend` parameter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineEnd {
    Round,
    Butt,
    Square,
}

/// Line join styles, matching R's `linejoin` parameter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineJoin {
    Round,
    Mitre,
    Bevel,
}

// Default RGBA constants
const BLACK_RGBA: [u8; 4] = [0, 0, 0, 255];
const TRANSPARENT_RGBA: [u8; 4] = [255, 255, 255, 0];

impl Gpar {
    /// Create a new empty gpar (all fields `None`).
    pub fn new() -> Self {
        Gpar::default()
    }

    /// Fill in any `None` fields from the parent gpar.
    ///
    /// This implements grid's inheritance: child viewports/grobs inherit
    /// graphical parameters from their parent when not explicitly set.
    pub fn inherit_from(&mut self, parent: &Gpar) {
        if self.col.is_none() {
            self.col = parent.col;
        }
        if self.fill.is_none() {
            self.fill = parent.fill;
        }
        if self.lwd.is_none() {
            self.lwd = parent.lwd;
        }
        if self.lty.is_none() {
            self.lty = parent.lty;
        }
        if self.fontsize.is_none() {
            self.fontsize = parent.fontsize;
        }
        if self.lineheight.is_none() {
            self.lineheight = parent.lineheight;
        }
        if self.font.is_none() {
            self.font = parent.font;
        }
        if self.fontfamily.is_none() {
            self.fontfamily.clone_from(&parent.fontfamily);
        }
        if self.cex.is_none() {
            self.cex = parent.cex;
        }
        if self.alpha.is_none() {
            self.alpha = parent.alpha;
        }
        if self.lineend.is_none() {
            self.lineend = parent.lineend;
        }
        if self.linejoin.is_none() {
            self.linejoin = parent.linejoin;
        }
        if self.linemitre.is_none() {
            self.linemitre = parent.linemitre;
        }
        if self.just_x.is_none() {
            self.just_x = parent.just_x;
        }
        if self.just_y.is_none() {
            self.just_y = parent.just_y;
        }
    }

    /// Create a new gpar that is this gpar with parent values filled in.
    pub fn with_parent(&self, parent: &Gpar) -> Gpar {
        let mut result = self.clone();
        result.inherit_from(parent);
        result
    }

    /// Effective stroke color: this gpar's col, or black.
    pub fn effective_col(&self) -> [u8; 4] {
        self.col.unwrap_or(BLACK_RGBA)
    }

    /// Effective fill color: this gpar's fill, or transparent.
    pub fn effective_fill(&self) -> [u8; 4] {
        self.fill.unwrap_or(TRANSPARENT_RGBA)
    }

    /// Effective line width: this gpar's lwd, or 1.0.
    pub fn effective_lwd(&self) -> f64 {
        self.lwd.unwrap_or(1.0)
    }

    /// Effective line type: this gpar's lty, or Solid.
    pub fn effective_lty(&self) -> LineType {
        self.lty.unwrap_or(LineType::Solid)
    }

    /// Effective font size in points: this gpar's fontsize, or 12.0.
    pub fn effective_fontsize(&self) -> f64 {
        self.fontsize.unwrap_or(12.0)
    }

    /// Effective line height multiplier: this gpar's lineheight, or 1.2.
    pub fn effective_lineheight(&self) -> f64 {
        self.lineheight.unwrap_or(1.2)
    }

    /// Effective font face: this gpar's font, or Plain.
    pub fn effective_font(&self) -> FontFace {
        self.font.unwrap_or(FontFace::Plain)
    }

    /// Effective font family: this gpar's fontfamily, or "sans".
    pub fn effective_fontfamily(&self) -> &str {
        self.fontfamily.as_deref().unwrap_or("sans")
    }

    /// Effective character expansion factor: this gpar's cex, or 1.0.
    pub fn effective_cex(&self) -> f64 {
        self.cex.unwrap_or(1.0)
    }

    /// Effective alpha: this gpar's alpha, or 1.0 (fully opaque).
    pub fn effective_alpha(&self) -> f64 {
        self.alpha.unwrap_or(1.0)
    }

    /// Effective line end style: this gpar's lineend, or Round.
    pub fn effective_lineend(&self) -> LineEnd {
        self.lineend.unwrap_or(LineEnd::Round)
    }

    /// Effective line join style: this gpar's linejoin, or Round.
    pub fn effective_linejoin(&self) -> LineJoin {
        self.linejoin.unwrap_or(LineJoin::Round)
    }

    /// Effective mitre limit: this gpar's linemitre, or 10.0 (R default).
    pub fn effective_linemitre(&self) -> f64 {
        self.linemitre.unwrap_or(10.0)
    }
}

// endregion

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpar_defaults() {
        let g = Gpar::new();
        assert_eq!(g.effective_col(), BLACK_RGBA);
        assert_eq!(g.effective_fill(), TRANSPARENT_RGBA);
        assert!((g.effective_lwd() - 1.0).abs() < f64::EPSILON);
        assert!((g.effective_fontsize() - 12.0).abs() < f64::EPSILON);
        assert!((g.effective_lineheight() - 1.2).abs() < f64::EPSILON);
        assert_eq!(g.effective_font(), FontFace::Plain);
        assert_eq!(g.effective_fontfamily(), "sans");
        assert!((g.effective_cex() - 1.0).abs() < f64::EPSILON);
        assert!((g.effective_alpha() - 1.0).abs() < f64::EPSILON);
        assert_eq!(g.effective_lty(), LineType::Solid);
        assert_eq!(g.effective_lineend(), LineEnd::Round);
        assert_eq!(g.effective_linejoin(), LineJoin::Round);
        assert!((g.effective_linemitre() - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn gpar_inherit_fills_none_fields() {
        let parent = Gpar {
            col: Some([255, 0, 0, 255]),
            fontsize: Some(16.0),
            lwd: Some(2.5),
            fontfamily: Some("serif".to_string()),
            ..Default::default()
        };

        let mut child = Gpar {
            fontsize: Some(10.0), // child overrides fontsize
            ..Default::default()
        };

        child.inherit_from(&parent);

        // Inherited from parent
        assert_eq!(child.col, Some([255, 0, 0, 255]));
        assert_eq!(child.lwd, Some(2.5));
        assert_eq!(child.fontfamily, Some("serif".to_string()));

        // Child's own value preserved
        assert_eq!(child.fontsize, Some(10.0));
    }

    #[test]
    fn gpar_with_parent_does_not_mutate_original() {
        let parent = Gpar {
            col: Some([0, 255, 0, 255]),
            ..Default::default()
        };
        let child = Gpar {
            lwd: Some(3.0),
            ..Default::default()
        };

        let resolved = child.with_parent(&parent);

        // resolved has both parent and child values
        assert_eq!(resolved.col, Some([0, 255, 0, 255]));
        assert_eq!(resolved.lwd, Some(3.0));

        // original child is unchanged
        assert!(child.col.is_none());
    }

    #[test]
    fn gpar_child_overrides_parent() {
        let parent = Gpar {
            col: Some([255, 0, 0, 255]),
            fontsize: Some(16.0),
            ..Default::default()
        };

        let child = Gpar {
            col: Some([0, 0, 255, 255]),
            ..Default::default()
        };

        let resolved = child.with_parent(&parent);

        // Child's col wins
        assert_eq!(resolved.col, Some([0, 0, 255, 255]));
        // Parent's fontsize inherited
        assert_eq!(resolved.fontsize, Some(16.0));
    }
}
