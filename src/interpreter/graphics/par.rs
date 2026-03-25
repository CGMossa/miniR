//! Graphical parameter state (`par()`) — stores R's graphical parameters
//! per-interpreter, matching R's `par()` system.

use crate::interpreter::value::*;
use crate::interpreter::BuiltinContext;
use minir_macros::interpreter_builtin;

use super::color::RColor;

// region: LineType and FontFace

/// R line types, matching `par("lty")` values.
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
    pub fn from_r_value(val: &RValue) -> Result<Self, RError> {
        match val {
            RValue::Vector(rv) => match &rv.inner {
                Vector::Character(c) => {
                    let s = c.first().and_then(|o| o.as_deref()).unwrap_or("solid");
                    Self::from_str(s)
                }
                Vector::Integer(i) => {
                    let n = i.first().copied().flatten().unwrap_or(1);
                    Self::from_int(n)
                }
                Vector::Double(d) => {
                    let n = d.first().copied().flatten().map(|v| v as i64).unwrap_or(1);
                    Self::from_int(n)
                }
                _ => Err(RError::new(
                    RErrorKind::Other,
                    "invalid line type specification",
                )),
            },
            _ => Err(RError::new(
                RErrorKind::Other,
                "invalid line type specification",
            )),
        }
    }

    fn from_str(s: &str) -> Result<Self, RError> {
        match s.to_lowercase().as_str() {
            "blank" | "0" => Ok(LineType::Blank),
            "solid" | "1" => Ok(LineType::Solid),
            "dashed" | "2" => Ok(LineType::Dashed),
            "dotted" | "3" => Ok(LineType::Dotted),
            "dotdash" | "4" => Ok(LineType::DotDash),
            "longdash" | "5" => Ok(LineType::LongDash),
            "twodash" | "6" => Ok(LineType::TwoDash),
            _ => Err(RError::new(
                RErrorKind::Other,
                format!("invalid line type '{s}'"),
            )),
        }
    }

    fn from_int(n: i64) -> Result<Self, RError> {
        match n {
            0 => Ok(LineType::Blank),
            1 => Ok(LineType::Solid),
            2 => Ok(LineType::Dashed),
            3 => Ok(LineType::Dotted),
            4 => Ok(LineType::DotDash),
            5 => Ok(LineType::LongDash),
            6 => Ok(LineType::TwoDash),
            _ => Err(RError::new(
                RErrorKind::Other,
                format!("invalid line type integer {n}: must be 0–6"),
            )),
        }
    }

    #[allow(dead_code)] // used when serializing lty to integer for graphics devices
    fn to_int(self) -> i64 {
        match self {
            LineType::Blank => 0,
            LineType::Solid => 1,
            LineType::Dashed => 2,
            LineType::Dotted => 3,
            LineType::DotDash => 4,
            LineType::LongDash => 5,
            LineType::TwoDash => 6,
        }
    }

    fn to_str(self) -> &'static str {
        match self {
            LineType::Blank => "blank",
            LineType::Solid => "solid",
            LineType::Dashed => "dashed",
            LineType::Dotted => "dotted",
            LineType::DotDash => "dotdash",
            LineType::LongDash => "longdash",
            LineType::TwoDash => "twodash",
        }
    }
}

/// R font faces, matching `par("font")` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontFace {
    Plain,
    Bold,
    Italic,
    BoldItalic,
    Symbol,
}

impl FontFace {
    fn from_int(n: i64) -> Result<Self, RError> {
        match n {
            1 => Ok(FontFace::Plain),
            2 => Ok(FontFace::Bold),
            3 => Ok(FontFace::Italic),
            4 => Ok(FontFace::BoldItalic),
            5 => Ok(FontFace::Symbol),
            _ => Err(RError::new(
                RErrorKind::Other,
                format!("invalid font face {n}: must be 1–5"),
            )),
        }
    }

    fn to_int(self) -> i64 {
        match self {
            FontFace::Plain => 1,
            FontFace::Bold => 2,
            FontFace::Italic => 3,
            FontFace::BoldItalic => 4,
            FontFace::Symbol => 5,
        }
    }
}

// endregion

// region: ParState

/// Full graphical parameter state, mirroring R's `par()` settings.
#[derive(Debug, Clone)]
pub struct ParState {
    /// Foreground (drawing) color
    pub col: RColor,
    /// Background color
    pub bg: RColor,
    /// Line width
    pub lwd: f64,
    /// Line type
    pub lty: LineType,
    /// Plotting character (symbol)
    pub pch: i64,
    /// Character expansion factor
    pub cex: f64,
    /// Point size of text (in big points)
    pub ps: f64,
    /// Font face
    pub font: FontFace,
    /// Font family name
    pub family: String,
    /// Margins (bottom, left, top, right) in lines
    pub mar: [f64; 4],
    /// Multi-figure layout: rows, cols (set by mfrow)
    pub mfrow: [i64; 2],
    /// Multi-figure layout: rows, cols (set by mfcol)
    pub mfcol: [i64; 2],
    /// User coordinate limits: xmin, xmax, ymin, ymax
    pub usr: [f64; 4],
    /// Axis label style: 0=parallel, 1=horizontal, 2=perpendicular, 3=vertical
    pub las: i64,
    /// X-axis interval calculation style ("r" or "i")
    pub xaxs: String,
    /// Y-axis interval calculation style ("r" or "i")
    pub yaxs: String,
    /// Whether plot.new() has been called on the current device
    pub new: bool,
}

impl Default for ParState {
    fn default() -> Self {
        ParState {
            col: RColor::BLACK,
            bg: RColor::WHITE,
            lwd: 1.0,
            lty: LineType::Solid,
            pch: 1,
            cex: 1.0,
            ps: 12.0,
            font: FontFace::Plain,
            family: "sans".to_string(),
            mar: [5.1, 4.1, 4.1, 2.1],
            mfrow: [1, 1],
            mfcol: [1, 1],
            usr: [0.0, 1.0, 0.0, 1.0],
            las: 0,
            xaxs: "r".to_string(),
            yaxs: "r".to_string(),
            new: false,
        }
    }
}

impl ParState {
    /// Get a graphical parameter value by name, returning it as an RValue.
    pub fn get(&self, name: &str) -> Option<RValue> {
        match name {
            "col" => Some(RValue::vec(Vector::Character(
                vec![Some(self.col.to_hex())].into(),
            ))),
            "bg" => Some(RValue::vec(Vector::Character(
                vec![Some(self.bg.to_hex())].into(),
            ))),
            "lwd" => Some(RValue::vec(Vector::Double(vec![Some(self.lwd)].into()))),
            "lty" => Some(RValue::vec(Vector::Character(
                vec![Some(self.lty.to_str().to_string())].into(),
            ))),
            "pch" => Some(RValue::vec(Vector::Integer(vec![Some(self.pch)].into()))),
            "cex" => Some(RValue::vec(Vector::Double(vec![Some(self.cex)].into()))),
            "ps" => Some(RValue::vec(Vector::Double(vec![Some(self.ps)].into()))),
            "font" => Some(RValue::vec(Vector::Integer(
                vec![Some(self.font.to_int())].into(),
            ))),
            "family" => Some(RValue::vec(Vector::Character(
                vec![Some(self.family.clone())].into(),
            ))),
            "mar" => Some(RValue::vec(Vector::Double(
                self.mar.iter().map(|&v| Some(v)).collect::<Vec<_>>().into(),
            ))),
            "mfrow" => Some(RValue::vec(Vector::Integer(
                self.mfrow
                    .iter()
                    .map(|&v| Some(v))
                    .collect::<Vec<_>>()
                    .into(),
            ))),
            "mfcol" => Some(RValue::vec(Vector::Integer(
                self.mfcol
                    .iter()
                    .map(|&v| Some(v))
                    .collect::<Vec<_>>()
                    .into(),
            ))),
            "usr" => Some(RValue::vec(Vector::Double(
                self.usr.iter().map(|&v| Some(v)).collect::<Vec<_>>().into(),
            ))),
            "las" => Some(RValue::vec(Vector::Integer(vec![Some(self.las)].into()))),
            "xaxs" => Some(RValue::vec(Vector::Character(
                vec![Some(self.xaxs.clone())].into(),
            ))),
            "yaxs" => Some(RValue::vec(Vector::Character(
                vec![Some(self.yaxs.clone())].into(),
            ))),
            "new" => Some(RValue::vec(Vector::Logical(vec![Some(self.new)].into()))),
            _ => None,
        }
    }

    /// Set a graphical parameter by name. Returns the old value on success.
    pub fn set(&mut self, name: &str, value: &RValue) -> Result<Option<RValue>, RError> {
        let old = self.get(name);
        match name {
            "col" => {
                self.col = parse_color_param(value, name)?;
            }
            "bg" => {
                self.bg = parse_color_param(value, name)?;
            }
            "lwd" => {
                self.lwd = parse_double_param(value, name)?;
            }
            "lty" => {
                self.lty = LineType::from_r_value(value)?;
            }
            "pch" => {
                self.pch = parse_int_param(value, name)?;
            }
            "cex" => {
                self.cex = parse_double_param(value, name)?;
            }
            "ps" => {
                self.ps = parse_double_param(value, name)?;
            }
            "font" => {
                let n = parse_int_param(value, name)?;
                self.font = FontFace::from_int(n)?;
            }
            "family" => {
                self.family = parse_string_param(value, name)?;
            }
            "mar" => {
                self.mar = parse_double4_param(value, name)?;
            }
            "mfrow" => {
                self.mfrow = parse_int2_param(value, name)?;
            }
            "mfcol" => {
                self.mfcol = parse_int2_param(value, name)?;
            }
            "usr" => {
                self.usr = parse_double4_param(value, name)?;
            }
            "las" => {
                self.las = parse_int_param(value, name)?;
            }
            "xaxs" => {
                self.xaxs = parse_string_param(value, name)?;
            }
            "yaxs" => {
                self.yaxs = parse_string_param(value, name)?;
            }
            "new" => {
                self.new = parse_logical_param(value, name)?;
            }
            _ => {
                return Err(RError::new(
                    RErrorKind::Other,
                    format!("'{name}' is not a graphical parameter"),
                ));
            }
        }
        Ok(old)
    }

    /// Return all parameter names that this state knows about.
    pub fn known_params() -> &'static [&'static str] {
        &[
            "bg", "cex", "col", "family", "font", "las", "lty", "lwd", "mar", "mfcol", "mfrow",
            "new", "pch", "ps", "usr", "xaxs", "yaxs",
        ]
    }
}

// endregion

// region: Parameter parsing helpers

fn parse_color_param(value: &RValue, name: &str) -> Result<RColor, RError> {
    match value {
        RValue::Vector(rv) => match &rv.inner {
            Vector::Character(c) => {
                let s = c.first().and_then(|o| o.as_deref()).ok_or_else(|| {
                    RError::new(
                        RErrorKind::Other,
                        format!("invalid color for parameter '{name}'"),
                    )
                })?;
                RColor::from_r_value(s, &[]).map_err(|e| {
                    RError::new(
                        RErrorKind::Other,
                        format!("invalid color for parameter '{name}': {e}"),
                    )
                })
            }
            _ => Err(RError::new(
                RErrorKind::Other,
                format!("invalid color for parameter '{name}'"),
            )),
        },
        _ => Err(RError::new(
            RErrorKind::Other,
            format!("invalid color for parameter '{name}'"),
        )),
    }
}

fn parse_double_param(value: &RValue, name: &str) -> Result<f64, RError> {
    value
        .as_vector()
        .and_then(|v| v.as_double_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Other,
                format!("invalid value for parameter '{name}'"),
            )
        })
}

fn parse_int_param(value: &RValue, name: &str) -> Result<i64, RError> {
    value
        .as_vector()
        .and_then(|v| v.as_integer_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Other,
                format!("invalid value for parameter '{name}'"),
            )
        })
}

fn parse_string_param(value: &RValue, name: &str) -> Result<String, RError> {
    value
        .as_vector()
        .and_then(|v| v.as_character_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Other,
                format!("invalid value for parameter '{name}'"),
            )
        })
}

fn parse_logical_param(value: &RValue, name: &str) -> Result<bool, RError> {
    value
        .as_vector()
        .and_then(|v| v.as_logical_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Other,
                format!("invalid value for parameter '{name}'"),
            )
        })
}

fn parse_double4_param(value: &RValue, name: &str) -> Result<[f64; 4], RError> {
    let v = value.as_vector().ok_or_else(|| {
        RError::new(
            RErrorKind::Other,
            format!("parameter '{name}' requires a numeric vector of length 4"),
        )
    })?;
    let doubles = v.to_doubles();
    if doubles.len() != 4 {
        return Err(RError::new(
            RErrorKind::Other,
            format!(
                "parameter '{name}' requires a numeric vector of length 4, got {}",
                doubles.len()
            ),
        ));
    }
    let mut result = [0.0; 4];
    for (i, d) in doubles.iter().enumerate() {
        result[i] = d.ok_or_else(|| {
            RError::new(RErrorKind::Other, format!("NA value in parameter '{name}'"))
        })?;
    }
    Ok(result)
}

fn parse_int2_param(value: &RValue, name: &str) -> Result<[i64; 2], RError> {
    let v = value.as_vector().ok_or_else(|| {
        RError::new(
            RErrorKind::Other,
            format!("parameter '{name}' requires an integer vector of length 2"),
        )
    })?;
    let ints = v.to_integers();
    if ints.len() != 2 {
        return Err(RError::new(
            RErrorKind::Other,
            format!(
                "parameter '{name}' requires a vector of length 2, got {}",
                ints.len()
            ),
        ));
    }
    let mut result = [0i64; 2];
    for (i, val) in ints.iter().enumerate() {
        result[i] = val.ok_or_else(|| {
            RError::new(RErrorKind::Other, format!("NA value in parameter '{name}'"))
        })?;
    }
    Ok(result)
}

// endregion

// region: par() builtin

/// Query or set graphical parameters.
///
/// When called with no arguments, returns all graphical parameters as a named
/// list. When called with string arguments, returns those parameters. When
/// called with named arguments, sets those parameters and returns the old values.
///
/// @param ... parameter names (as strings) or name=value pairs
/// @return named list of (old) parameter values
#[interpreter_builtin(namespace = "graphics")]
fn interp_par(
    args: &[RValue],
    named: &[(String, RValue)],
    ctx: &BuiltinContext,
) -> Result<RValue, RError> {
    let mut par = ctx.interpreter().par_state.borrow_mut();

    // If no args and no named args, return all parameters
    if args.is_empty() && named.is_empty() {
        let mut entries: Vec<(Option<String>, RValue)> = Vec::new();
        for &name in ParState::known_params() {
            if let Some(val) = par.get(name) {
                entries.push((Some(name.to_string()), val));
            }
        }
        return Ok(RValue::List(RList::new(entries)));
    }

    let mut result_entries: Vec<(Option<String>, RValue)> = Vec::new();

    // Handle positional string args: par("col") returns the value of "col"
    for arg in args {
        if let Some(v) = arg.as_vector() {
            if let Some(name) = v.as_character_scalar() {
                match par.get(&name) {
                    Some(val) => {
                        result_entries.push((Some(name), val));
                    }
                    None => {
                        return Err(RError::new(
                            RErrorKind::Other,
                            format!("'{name}' is not a graphical parameter"),
                        ));
                    }
                }
            }
        }
    }

    // Handle named args: par(col = "red") sets col and returns old value
    for (name, value) in named {
        let old = par.set(name, value)?;
        if let Some(old_val) = old {
            result_entries.push((Some(name.clone()), old_val));
        }
    }

    if result_entries.is_empty() {
        Ok(RValue::List(RList::new(vec![])))
    } else if result_entries.len() == 1 && args.len() == 1 && named.is_empty() {
        // When querying a single parameter by name, return just the value (not a list)
        Ok(result_entries
            .into_iter()
            .next()
            .expect("len() == 1 guarantees next()")
            .1)
    } else {
        Ok(RValue::List(RList::new(result_entries)))
    }
}

// endregion
