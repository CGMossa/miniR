//! Graphics context types: colors, line styles, fonts, and the composite `GraphicsContext`.

use std::fmt;

// region: RColor

/// An RGBA color with 8 bits per channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl RColor {
    pub const BLACK: RColor = RColor {
        r: 0,
        g: 0,
        b: 0,
        a: 255,
    };
    pub const WHITE: RColor = RColor {
        r: 255,
        g: 255,
        b: 255,
        a: 255,
    };
    pub const TRANSPARENT: RColor = RColor {
        r: 255,
        g: 255,
        b: 255,
        a: 0,
    };

    /// Parse a hex color string in "#RRGGBB" or "#RRGGBBAA" format.
    ///
    /// Returns `None` if the string is not a valid hex color.
    pub fn from_hex(s: &str) -> Option<RColor> {
        let s = s.strip_prefix('#')?;
        match s.len() {
            6 => {
                let r = u8::from_str_radix(&s[0..2], 16).ok()?;
                let g = u8::from_str_radix(&s[2..4], 16).ok()?;
                let b = u8::from_str_radix(&s[4..6], 16).ok()?;
                Some(RColor { r, g, b, a: 255 })
            }
            8 => {
                let r = u8::from_str_radix(&s[0..2], 16).ok()?;
                let g = u8::from_str_radix(&s[2..4], 16).ok()?;
                let b = u8::from_str_radix(&s[4..6], 16).ok()?;
                let a = u8::from_str_radix(&s[6..8], 16).ok()?;
                Some(RColor { r, g, b, a })
            }
            _ => None,
        }
    }

    /// Render as a hex string: "#RRGGBB" if fully opaque, "#RRGGBBAA" otherwise.
    pub fn to_hex(&self) -> String {
        if self.a == 255 {
            format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
        } else {
            format!("#{:02X}{:02X}{:02X}{:02X}", self.r, self.g, self.b, self.a)
        }
    }
}

impl fmt::Display for RColor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

// endregion

// region: LineType

/// R line type (lty) values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineType {
    Blank,
    Solid,
    Dashed,
    Dotted,
    DotDash,
    LongDash,
    TwoDash,
}

impl LineType {
    /// Convert from an R integer (0-6) or string name to a `LineType`.
    ///
    /// Integer mapping matches R's `par("lty")`:
    ///   0=blank, 1=solid, 2=dashed, 3=dotted, 4=dotdash, 5=longdash, 6=twodash
    ///
    /// String matching is case-insensitive.
    pub fn from_r_value(value: &str) -> Option<LineType> {
        // Try integer first
        if let Ok(n) = value.parse::<i32>() {
            return LineType::from_integer(n);
        }
        // Try string name (case-insensitive)
        match value.to_ascii_lowercase().as_str() {
            "blank" => Some(LineType::Blank),
            "solid" => Some(LineType::Solid),
            "dashed" => Some(LineType::Dashed),
            "dotted" => Some(LineType::Dotted),
            "dotdash" => Some(LineType::DotDash),
            "longdash" => Some(LineType::LongDash),
            "twodash" => Some(LineType::TwoDash),
            _ => None,
        }
    }

    /// Convert from an R integer (0-6) to a `LineType`.
    pub fn from_integer(n: i32) -> Option<LineType> {
        match n {
            0 => Some(LineType::Blank),
            1 => Some(LineType::Solid),
            2 => Some(LineType::Dashed),
            3 => Some(LineType::Dotted),
            4 => Some(LineType::DotDash),
            5 => Some(LineType::LongDash),
            6 => Some(LineType::TwoDash),
            _ => None,
        }
    }
}

// endregion

// region: FontFace

/// R font face (font) values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontFace {
    Plain,
    Bold,
    Italic,
    BoldItalic,
}

impl FontFace {
    /// Convert from an R integer (1-4) to a `FontFace`.
    ///
    /// 1=plain, 2=bold, 3=italic, 4=bold-italic (matches R's `par("font")`).
    pub fn from_integer(n: i32) -> Option<FontFace> {
        match n {
            1 => Some(FontFace::Plain),
            2 => Some(FontFace::Bold),
            3 => Some(FontFace::Italic),
            4 => Some(FontFace::BoldItalic),
            _ => None,
        }
    }
}

// endregion

// region: PointChar

/// R plotting character (pch) — either a symbol code (0-25) or an ASCII character (32-126).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PointChar(pub i32);

impl PointChar {
    /// Returns `true` if this is a valid pch value: symbol codes 0-25 or ASCII 32-126.
    pub fn is_valid(&self) -> bool {
        (0..=25).contains(&self.0) || (32..=126).contains(&self.0)
    }

    /// If this pch is an ASCII character (32-126), return it.
    pub fn as_char(&self) -> Option<char> {
        if (32..=126).contains(&self.0) {
            // Safe: range 32-126 is valid ASCII and fits in u8
            Some(char::from(u8::try_from(self.0).ok()?))
        } else {
            None
        }
    }
}

// endregion

// region: GraphicsContext

/// The graphics context (gc) passed to every drawing operation.
///
/// Mirrors R's `R_GE_gcontext` structure with the most commonly used fields.
#[derive(Debug, Clone)]
pub struct GraphicsContext {
    /// Foreground (stroke/text) color.
    pub col: RColor,
    /// Fill color.
    pub fill: RColor,
    /// Line width (lwd), in points.
    pub lwd: f64,
    /// Line type.
    pub lty: LineType,
    /// Plotting character.
    pub pch: PointChar,
    /// Character expansion factor (cex).
    pub cex: f64,
    /// Point size (ps), in points.
    pub ps: f64,
    /// Font face.
    pub font: FontFace,
    /// Font family name.
    pub font_family: String,
}

impl Default for GraphicsContext {
    fn default() -> Self {
        GraphicsContext {
            col: RColor::BLACK,
            fill: RColor::TRANSPARENT,
            lwd: 1.0,
            lty: LineType::Solid,
            pch: PointChar(1),
            cex: 1.0,
            ps: 12.0,
            font: FontFace::Plain,
            font_family: "sans".to_string(),
        }
    }
}

// endregion

#[cfg(test)]
mod tests {
    use super::*;

    // region: RColor tests

    #[test]
    fn rcolor_constants() {
        assert_eq!(
            RColor::BLACK,
            RColor {
                r: 0,
                g: 0,
                b: 0,
                a: 255
            }
        );
        assert_eq!(
            RColor::WHITE,
            RColor {
                r: 255,
                g: 255,
                b: 255,
                a: 255,
            }
        );
        assert_eq!(RColor::TRANSPARENT.a, 0);
    }

    #[test]
    fn rcolor_from_hex_rrggbb() {
        let c = RColor::from_hex("#FF8000").unwrap();
        assert_eq!(c.r, 255);
        assert_eq!(c.g, 128);
        assert_eq!(c.b, 0);
        assert_eq!(c.a, 255);
    }

    #[test]
    fn rcolor_from_hex_rrggbbaa() {
        let c = RColor::from_hex("#FF800080").unwrap();
        assert_eq!(c.r, 255);
        assert_eq!(c.g, 128);
        assert_eq!(c.b, 0);
        assert_eq!(c.a, 128);
    }

    #[test]
    fn rcolor_from_hex_lowercase() {
        let c = RColor::from_hex("#ff8000").unwrap();
        assert_eq!(c.r, 255);
        assert_eq!(c.g, 128);
        assert_eq!(c.b, 0);
    }

    #[test]
    fn rcolor_from_hex_invalid() {
        assert!(RColor::from_hex("FF8000").is_none()); // missing #
        assert!(RColor::from_hex("#FF80").is_none()); // too short
        assert!(RColor::from_hex("#GGGGGG").is_none()); // invalid hex
        assert!(RColor::from_hex("#FF80001122").is_none()); // too long
        assert!(RColor::from_hex("").is_none());
    }

    #[test]
    fn rcolor_to_hex_opaque() {
        assert_eq!(RColor::BLACK.to_hex(), "#000000");
        assert_eq!(RColor::WHITE.to_hex(), "#FFFFFF");
    }

    #[test]
    fn rcolor_to_hex_transparent() {
        assert_eq!(RColor::TRANSPARENT.to_hex(), "#FFFFFF00");
        let c = RColor {
            r: 255,
            g: 128,
            b: 0,
            a: 128,
        };
        assert_eq!(c.to_hex(), "#FF800080");
    }

    #[test]
    fn rcolor_roundtrip() {
        let original = "#AB12CD";
        let c = RColor::from_hex(original).unwrap();
        assert_eq!(c.to_hex(), original);
    }

    #[test]
    fn rcolor_display() {
        assert_eq!(format!("{}", RColor::BLACK), "#000000");
    }

    // endregion

    // region: LineType tests

    #[test]
    fn linetype_from_integer() {
        assert_eq!(LineType::from_integer(0), Some(LineType::Blank));
        assert_eq!(LineType::from_integer(1), Some(LineType::Solid));
        assert_eq!(LineType::from_integer(2), Some(LineType::Dashed));
        assert_eq!(LineType::from_integer(3), Some(LineType::Dotted));
        assert_eq!(LineType::from_integer(4), Some(LineType::DotDash));
        assert_eq!(LineType::from_integer(5), Some(LineType::LongDash));
        assert_eq!(LineType::from_integer(6), Some(LineType::TwoDash));
        assert_eq!(LineType::from_integer(7), None);
        assert_eq!(LineType::from_integer(-1), None);
    }

    #[test]
    fn linetype_from_r_value_integer_string() {
        assert_eq!(LineType::from_r_value("0"), Some(LineType::Blank));
        assert_eq!(LineType::from_r_value("1"), Some(LineType::Solid));
        assert_eq!(LineType::from_r_value("6"), Some(LineType::TwoDash));
    }

    #[test]
    fn linetype_from_r_value_name() {
        assert_eq!(LineType::from_r_value("solid"), Some(LineType::Solid));
        assert_eq!(LineType::from_r_value("Dashed"), Some(LineType::Dashed));
        assert_eq!(LineType::from_r_value("DOTTED"), Some(LineType::Dotted));
        assert_eq!(LineType::from_r_value("dotdash"), Some(LineType::DotDash));
        assert_eq!(LineType::from_r_value("longdash"), Some(LineType::LongDash));
        assert_eq!(LineType::from_r_value("twodash"), Some(LineType::TwoDash));
        assert_eq!(LineType::from_r_value("blank"), Some(LineType::Blank));
    }

    #[test]
    fn linetype_from_r_value_invalid() {
        assert_eq!(LineType::from_r_value("squiggly"), None);
        assert_eq!(LineType::from_r_value("99"), None);
    }

    // endregion

    // region: FontFace tests

    #[test]
    fn fontface_from_integer() {
        assert_eq!(FontFace::from_integer(1), Some(FontFace::Plain));
        assert_eq!(FontFace::from_integer(2), Some(FontFace::Bold));
        assert_eq!(FontFace::from_integer(3), Some(FontFace::Italic));
        assert_eq!(FontFace::from_integer(4), Some(FontFace::BoldItalic));
        assert_eq!(FontFace::from_integer(0), None);
        assert_eq!(FontFace::from_integer(5), None);
    }

    // endregion

    // region: PointChar tests

    #[test]
    fn pointchar_valid_symbols() {
        for i in 0..=25 {
            assert!(PointChar(i).is_valid(), "pch {i} should be valid");
        }
    }

    #[test]
    fn pointchar_valid_ascii() {
        for i in 32..=126 {
            assert!(PointChar(i).is_valid(), "pch {i} should be valid");
        }
    }

    #[test]
    fn pointchar_invalid() {
        assert!(!PointChar(26).is_valid());
        assert!(!PointChar(31).is_valid());
        assert!(!PointChar(127).is_valid());
        assert!(!PointChar(-1).is_valid());
    }

    #[test]
    fn pointchar_as_char() {
        assert_eq!(PointChar(65).as_char(), Some('A'));
        assert_eq!(PointChar(32).as_char(), Some(' '));
        assert_eq!(PointChar(126).as_char(), Some('~'));
        assert_eq!(PointChar(0).as_char(), None); // symbol, not ASCII
        assert_eq!(PointChar(25).as_char(), None);
    }

    // endregion

    // region: GraphicsContext tests

    #[test]
    fn graphics_context_defaults() {
        let gc = GraphicsContext::default();
        assert_eq!(gc.col, RColor::BLACK);
        assert_eq!(gc.fill, RColor::TRANSPARENT);
        assert_eq!(gc.lwd, 1.0);
        assert_eq!(gc.lty, LineType::Solid);
        assert_eq!(gc.pch, PointChar(1));
        assert_eq!(gc.cex, 1.0);
        assert_eq!(gc.ps, 12.0);
        assert_eq!(gc.font, FontFace::Plain);
        assert_eq!(gc.font_family, "sans");
    }

    // endregion
}
