pub mod character;
pub mod complex;
pub mod double;
pub mod integer;
pub mod logical;

pub use character::Character;
pub use complex::ComplexVec;
pub use double::Double;
pub use integer::Integer;
pub use logical::Logical;

use std::collections::HashMap;
use std::fmt;
use std::num::TryFromIntError;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use derive_more::{Deref, DerefMut};

use crate::interpreter::coerce;
use crate::interpreter::environment::Environment;
use crate::parser::ast::{Expr, Param};

pub type BuiltinFn = fn(&[RValue], &[(String, RValue)]) -> Result<RValue, RError>;

/// Attribute map — every R object can carry named attributes
pub type Attributes = HashMap<String, RValue>;

/// Unevaluated expression (language object) — returned by quote(), parse().
///
/// Wraps a boxed AST node. Derefs to `Expr` for pattern matching.
#[derive(Debug, Clone, Deref, DerefMut)]
pub struct Language(pub Box<Expr>);

impl Language {
    pub fn new(expr: Expr) -> Self {
        Language(Box::new(expr))
    }
}

#[derive(Debug, Clone)]
pub enum RValue {
    /// NULL
    Null,
    /// Atomic vector (with optional attributes)
    Vector(RVector),
    /// List (generic vector)
    List(RList),
    /// Function (closure)
    Function(RFunction),
    /// Environment reference
    Environment(Environment),
    /// Language object (unevaluated expression)
    Language(Language),
}

/// Atomic vector with optional attributes (names, class, dim, etc.)
#[derive(Debug, Clone)]
pub struct RVector {
    pub inner: Vector,
    pub attrs: Option<Box<Attributes>>,
}

impl Deref for RVector {
    type Target = Vector;
    fn deref(&self) -> &Vector {
        &self.inner
    }
}

impl DerefMut for RVector {
    fn deref_mut(&mut self) -> &mut Vector {
        &mut self.inner
    }
}

impl From<Vector> for RVector {
    fn from(v: Vector) -> Self {
        RVector {
            inner: v,
            attrs: None,
        }
    }
}

impl RVector {
    pub fn get_attr(&self, name: &str) -> Option<&RValue> {
        self.attrs.as_ref().and_then(|a| a.get(name))
    }

    pub fn set_attr(&mut self, name: String, value: RValue) {
        self.attrs
            .get_or_insert_with(|| Box::new(HashMap::new()))
            .insert(name, value);
    }

    pub fn class(&self) -> Option<Vec<String>> {
        match self.get_attr("class") {
            Some(RValue::Vector(rv)) => match &rv.inner {
                Vector::Character(v) => Some(v.iter().filter_map(|s| s.clone()).collect()),
                _ => None,
            },
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RList {
    pub values: Vec<(Option<String>, RValue)>,
    pub attrs: Option<Box<Attributes>>,
}

impl RList {
    pub fn new(values: Vec<(Option<String>, RValue)>) -> Self {
        RList {
            values,
            attrs: None,
        }
    }

    pub fn get_attr(&self, name: &str) -> Option<&RValue> {
        self.attrs.as_ref().and_then(|a| a.get(name))
    }

    pub fn set_attr(&mut self, name: String, value: RValue) {
        self.attrs
            .get_or_insert_with(|| Box::new(HashMap::new()))
            .insert(name, value);
    }

    #[allow(dead_code)]
    pub fn class(&self) -> Option<Vec<String>> {
        match self.get_attr("class") {
            Some(RValue::Vector(rv)) => match &rv.inner {
                Vector::Character(v) => Some(v.iter().filter_map(|s| s.clone()).collect()),
                _ => None,
            },
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum RFunction {
    Closure {
        params: Vec<Param>,
        body: Expr,
        env: Environment,
    },
    Builtin {
        name: String,
        func: BuiltinFn,
    },
}

/// Atomic vector types in R
#[derive(Debug, Clone)]
pub enum Vector {
    Logical(Logical),
    Integer(Integer),
    Double(Double),
    Complex(ComplexVec),
    Character(Character),
}

impl Vector {
    pub fn len(&self) -> usize {
        match self {
            Vector::Logical(v) => v.len(),
            Vector::Integer(v) => v.len(),
            Vector::Double(v) => v.len(),
            Vector::Complex(v) => v.len(),
            Vector::Character(v) => v.len(),
        }
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get the first element as a boolean (for conditions)
    pub fn as_logical_scalar(&self) -> Option<bool> {
        match self {
            Vector::Logical(v) => v.first().copied().flatten(),
            Vector::Integer(v) => v.first().copied().flatten().map(|i| i != 0),
            Vector::Double(v) => v.first().copied().flatten().map(|f| f != 0.0),
            Vector::Complex(v) => v
                .first()
                .copied()
                .flatten()
                .map(|c| c.re != 0.0 || c.im != 0.0),
            Vector::Character(_) => None,
        }
    }

    /// Get the first element as f64
    pub fn as_double_scalar(&self) -> Option<f64> {
        match self {
            Vector::Double(v) => v.first().copied().flatten(),
            Vector::Integer(v) => v.first().copied().flatten().map(coerce::i64_to_f64),
            Vector::Logical(v) => v
                .first()
                .copied()
                .flatten()
                .map(|b| if b { 1.0 } else { 0.0 }),
            Vector::Complex(_) => None, // complex cannot coerce to double without losing info
            Vector::Character(v) => v.first().cloned().flatten().and_then(|s| s.parse().ok()),
        }
    }

    /// Get the first element as i64
    pub fn as_integer_scalar(&self) -> Option<i64> {
        match self {
            Vector::Integer(v) => v.first().copied().flatten(),
            Vector::Double(v) => v
                .first()
                .copied()
                .flatten()
                .and_then(|f| coerce::f64_to_i64(f).ok()),
            Vector::Logical(v) => v.first().copied().flatten().map(|b| if b { 1 } else { 0 }),
            Vector::Complex(_) => None,
            Vector::Character(v) => v.first().cloned().flatten().and_then(|s| s.parse().ok()),
        }
    }

    /// Get the first element as String
    pub fn as_character_scalar(&self) -> Option<String> {
        match self {
            Vector::Character(v) => v.first().cloned().flatten(),
            Vector::Double(v) => v.first().copied().flatten().map(format_r_double),
            Vector::Integer(v) => v.first().copied().flatten().map(|i| i.to_string()),
            Vector::Logical(v) => v.first().copied().flatten().map(|b| {
                if b {
                    "TRUE".to_string()
                } else {
                    "FALSE".to_string()
                }
            }),
            Vector::Complex(v) => v.first().copied().flatten().map(format_r_complex),
        }
    }

    /// Convert entire vector to doubles
    pub fn to_doubles(&self) -> Vec<Option<f64>> {
        match self {
            Vector::Double(v) => v.0.clone(),
            Vector::Integer(v) => v.iter().map(|x| x.map(coerce::i64_to_f64)).collect(),
            Vector::Logical(v) => v
                .iter()
                .map(|x| x.map(|b| if b { 1.0 } else { 0.0 }))
                .collect(),
            Vector::Complex(v) => v.iter().map(|x| x.map(|c| c.re)).collect(),
            Vector::Character(v) => v
                .iter()
                .map(|x| x.as_ref().and_then(|s| s.parse().ok()))
                .collect(),
        }
    }

    /// Convert entire vector to integers
    pub fn to_integers(&self) -> Vec<Option<i64>> {
        match self {
            Vector::Integer(v) => v.0.clone(),
            Vector::Double(v) => v
                .iter()
                .map(|x| x.and_then(|f| coerce::f64_to_i64(f).ok()))
                .collect(),
            Vector::Logical(v) => v.iter().map(|x| x.map(|b| if b { 1 } else { 0 })).collect(),
            Vector::Complex(v) => v
                .iter()
                .map(|x| x.and_then(|c| coerce::f64_to_i64(c.re).ok()))
                .collect(),
            Vector::Character(v) => v
                .iter()
                .map(|x| x.as_ref().and_then(|s| s.parse().ok()))
                .collect(),
        }
    }

    /// Convert entire vector to characters
    pub fn to_characters(&self) -> Vec<Option<String>> {
        match self {
            Vector::Character(v) => v.0.clone(),
            Vector::Double(v) => v.iter().map(|x| x.map(format_r_double)).collect(),
            Vector::Integer(v) => v.iter().map(|x| x.map(|i| i.to_string())).collect(),
            Vector::Logical(v) => v
                .iter()
                .map(|x| {
                    x.map(|b| {
                        if b {
                            "TRUE".to_string()
                        } else {
                            "FALSE".to_string()
                        }
                    })
                })
                .collect(),
            Vector::Complex(v) => v.iter().map(|x| x.map(format_r_complex)).collect(),
        }
    }

    /// Convert to logicals
    pub fn to_logicals(&self) -> Vec<Option<bool>> {
        match self {
            Vector::Logical(v) => v.0.clone(),
            Vector::Integer(v) => v.iter().map(|x| x.map(|i| i != 0)).collect(),
            Vector::Double(v) => v.iter().map(|x| x.map(|f| f != 0.0)).collect(),
            Vector::Complex(v) => v
                .iter()
                .map(|x| x.map(|c| c.re != 0.0 || c.im != 0.0))
                .collect(),
            Vector::Character(_) => vec![None; self.len()],
        }
    }

    /// Convert entire vector to complex
    pub fn to_complex(&self) -> Vec<Option<num_complex::Complex64>> {
        match self {
            Vector::Complex(v) => v.0.clone(),
            Vector::Double(v) => v
                .iter()
                .map(|x| x.map(|f| num_complex::Complex64::new(f, 0.0)))
                .collect(),
            Vector::Integer(v) => v
                .iter()
                .map(|x| x.map(|i| num_complex::Complex64::new(coerce::i64_to_f64(i), 0.0)))
                .collect(),
            Vector::Logical(v) => v
                .iter()
                .map(|x| x.map(|b| num_complex::Complex64::new(if b { 1.0 } else { 0.0 }, 0.0)))
                .collect(),
            Vector::Character(v) => v
                .iter()
                .map(|x| {
                    x.as_ref()
                        .and_then(|s| s.parse::<f64>().ok())
                        .map(|f| num_complex::Complex64::new(f, 0.0))
                })
                .collect(),
        }
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Vector::Logical(_) => "logical",
            Vector::Integer(_) => "integer",
            Vector::Double(_) => "double",
            Vector::Complex(_) => "complex",
            Vector::Character(_) => "character",
        }
    }
}

pub fn format_r_double(f: f64) -> String {
    if f.is_nan() {
        "NaN".to_string()
    } else if f.is_infinite() {
        if f > 0.0 {
            "Inf".to_string()
        } else {
            "-Inf".to_string()
        }
    } else if f == f.floor() && f.abs() < 1e15 {
        // Safe: we checked f is finite, integer-valued, and within range
        format!("{}", coerce::f64_to_i64(f).unwrap_or(0))
    } else {
        format!("{}", f)
    }
}

pub fn format_r_complex(c: num_complex::Complex64) -> String {
    let re = format_r_double(c.re);
    if c.im >= 0.0 || c.im.is_nan() {
        format!("{}+{}i", re, format_r_double(c.im))
    } else {
        format!("{}{}i", re, format_r_double(c.im))
    }
}

impl RValue {
    /// Convenience: wrap an atomic Vector into RValue::Vector with no attributes.
    pub fn vec(v: Vector) -> Self {
        RValue::Vector(RVector {
            inner: v,
            attrs: None,
        })
    }

    pub fn is_null(&self) -> bool {
        matches!(self, RValue::Null)
    }

    pub fn as_vector(&self) -> Option<&Vector> {
        match self {
            RValue::Vector(rv) => Some(&rv.inner),
            _ => None,
        }
    }

    #[allow(dead_code)]
    pub fn into_vector(self) -> Result<Vector, RError> {
        match self {
            RValue::Vector(rv) => Ok(rv.inner),
            RValue::Null => Ok(Vector::Logical(Logical(vec![]))),
            _ => Err(RError::new(
                RErrorKind::Type,
                "cannot coerce to vector".to_string(),
            )),
        }
    }

    pub fn type_name(&self) -> &str {
        match self {
            RValue::Null => "NULL",
            RValue::Vector(rv) => rv.inner.type_name(),
            RValue::List(_) => "list",
            RValue::Function(_) => "function",
            RValue::Environment(_) => "environment",
            RValue::Language(_) => "language",
        }
    }

    pub fn length(&self) -> usize {
        match self {
            RValue::Null => 0,
            RValue::Vector(rv) => rv.inner.len(),
            RValue::List(l) => l.values.len(),
            RValue::Function(_) => 1,
            RValue::Environment(_) => 0,
            RValue::Language(_) => 1,
        }
    }
}

impl fmt::Display for RValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RValue::Null => write!(f, "NULL"),
            RValue::Vector(rv) => write!(f, "{}", format_vector(&rv.inner)),
            RValue::List(list) => {
                for (i, (name, val)) in list.values.iter().enumerate() {
                    match name {
                        Some(n) => writeln!(f, "${}", n)?,
                        None => writeln!(f, "[[{}]]", i + 1)?,
                    }
                    writeln!(f, "{}", val)?;
                }
                Ok(())
            }
            RValue::Function(func) => match func {
                RFunction::Closure { .. } => write!(f, "function(...)"),
                RFunction::Builtin { name, .. } => write!(f, ".Primitive(\"{}\")", name),
            },
            RValue::Environment(_env) => write!(f, "<environment>"),
            RValue::Language(expr) => write!(f, "{}", deparse_expr(expr)),
        }
    }
}

pub fn format_vector(v: &Vector) -> String {
    let len = v.len();
    if len == 0 {
        return match v {
            Vector::Logical(_) => "logical(0)".to_string(),
            Vector::Integer(_) => "integer(0)".to_string(),
            Vector::Double(_) => "numeric(0)".to_string(),
            Vector::Complex(_) => "complex(0)".to_string(),
            Vector::Character(_) => "character(0)".to_string(),
        };
    }

    let elements: Vec<String> = match v {
        Vector::Logical(vals) => vals
            .iter()
            .map(|x| match x {
                Some(true) => "TRUE".to_string(),
                Some(false) => "FALSE".to_string(),
                None => "NA".to_string(),
            })
            .collect(),
        Vector::Integer(vals) => vals
            .iter()
            .map(|x| match x {
                Some(i) => i.to_string(),
                None => "NA".to_string(),
            })
            .collect(),
        Vector::Double(vals) => vals
            .iter()
            .map(|x| match x {
                Some(f) => format_r_double(*f),
                None => "NA".to_string(),
            })
            .collect(),
        Vector::Complex(vals) => vals
            .iter()
            .map(|x| match x {
                Some(c) => format_r_complex(*c),
                None => "NA".to_string(),
            })
            .collect(),
        Vector::Character(vals) => vals
            .iter()
            .map(|x| match x {
                Some(s) => format!("\"{}\"", s),
                None => "NA".to_string(),
            })
            .collect(),
    };

    if len == 1 {
        return format!("[1] {}", elements[0]);
    }

    // Format with line indices like R does
    let max_width = 80;
    let mut result = String::new();
    let mut pos = 0;

    while pos < elements.len() {
        let label = format!("[{}]", pos + 1);
        let label_width = label.len();
        let mut line = format!("{} ", label);
        let mut current_width = label_width + 1;
        let line_start = pos;

        while pos < elements.len() {
            let elem = &elements[pos];
            let elem_width = elem.len() + 1; // +1 for space
            if current_width + elem_width > max_width && pos > line_start {
                break;
            }
            line.push_str(elem);
            if pos + 1 < elements.len() {
                line.push(' ');
            }
            current_width += elem_width;
            pos += 1;
        }

        if !result.is_empty() {
            result.push('\n');
        }
        result.push_str(&line);
    }

    result
}

/// The kind of R condition (error, warning, or message).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConditionKind {
    Error,
    Warning,
    Message,
}

// region: RError

/// The R-facing error category — determines how R's condition system classifies the error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RErrorKind {
    Type,
    Argument,
    Name,
    Index,
    Parse,
    Other,
}

/// R runtime error — wraps any module error with an R-facing category and preserves
/// the full error chain via `Arc<dyn Error>`.
///
/// Construct via `RError::new()`, `RError::from_source()`, or `From<T: Error>` impls.
/// Module errors (IoError, MathError, etc.) convert automatically via the blanket impl.
#[derive(Debug, Clone)]
pub enum RError {
    /// Standard error with kind, message, and optional source chain.
    Standard {
        kind: RErrorKind,
        message: String,
        #[allow(dead_code)]
        source: Option<Arc<dyn std::error::Error + Send + Sync>>,
    },
    /// R condition signal — carries a condition object (list with class attribute).
    /// This is distinct from standard errors because it carries an RValue, not a
    /// std::error::Error.
    Condition {
        condition: RValue,
        kind: ConditionKind,
    },
}

// endregion

/// Control flow signals — not errors, but propagated via Result for convenience
#[derive(Debug, Clone)]
pub enum RSignal {
    Return(RValue),
    Break,
    Next,
}

/// Combined error/signal type for the evaluator.
/// Builtins return `Result<RValue, RError>`, the evaluator returns `Result<RValue, RFlow>`.
#[derive(Debug, Clone)]
pub enum RFlow {
    Error(RError),
    Signal(RSignal),
}

impl From<RSignal> for RFlow {
    fn from(s: RSignal) -> Self {
        RFlow::Signal(s)
    }
}

impl From<RFlow> for RError {
    /// Convert back from RFlow to RError. Signals become Other errors (they shouldn't
    /// normally reach builtin boundaries, but if they do we don't lose information).
    fn from(f: RFlow) -> Self {
        match f {
            RFlow::Error(e) => e,
            RFlow::Signal(s) => RError::other(format!("{}", s)),
        }
    }
}

impl From<TryFromIntError> for RFlow {
    fn from(e: TryFromIntError) -> Self {
        RFlow::Error(RError::from_source(RErrorKind::Type, e))
    }
}

impl fmt::Display for RFlow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RFlow::Error(e) => write!(f, "{}", e),
            RFlow::Signal(s) => write!(f, "{}", s),
        }
    }
}

impl fmt::Display for RSignal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RSignal::Return(_) => write!(f, "no function to return from"),
            RSignal::Break => write!(f, "no loop for break/next, jumping to top level"),
            RSignal::Next => write!(f, "no loop for break/next, jumping to top level"),
        }
    }
}

impl From<TryFromIntError> for RError {
    fn from(e: TryFromIntError) -> Self {
        RError::from_source(RErrorKind::Type, e)
    }
}

// region: RError impls

impl RError {
    /// Create an error with a kind and message (no source chain).
    pub fn new(kind: RErrorKind, message: impl Into<String>) -> Self {
        RError::Standard {
            kind,
            message: message.into(),
            source: None,
        }
    }

    /// Create an error that wraps a source error, using its Display as the message.
    pub fn from_source(
        kind: RErrorKind,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        let message = format!("{}", source);
        RError::Standard {
            kind,
            message,
            source: Some(Arc::new(source)),
        }
    }

    /// Convenience: create an Other error with just a message.
    pub fn other(message: impl Into<String>) -> Self {
        RError::new(RErrorKind::Other, message)
    }

    /// Return the error kind (or None for Condition).
    #[allow(dead_code)]
    pub fn kind(&self) -> Option<RErrorKind> {
        match self {
            RError::Standard { kind, .. } => Some(*kind),
            RError::Condition { .. } => None,
        }
    }

    /// Extract the error message string from any RError variant.
    #[allow(dead_code)]
    pub fn message(&self) -> String {
        match self {
            RError::Standard { message, .. } => message.clone(),
            RError::Condition { condition, .. } => {
                if let RValue::List(list) = condition {
                    for (name, val) in &list.values {
                        if name.as_deref() == Some("message") {
                            if let RValue::Vector(rv) = val {
                                if let Some(s) = rv.as_character_scalar() {
                                    return s;
                                }
                            }
                        }
                    }
                }
                format!("{}", self)
            }
        }
    }

    /// Get the source error, if any.
    #[allow(dead_code)]
    pub fn source(&self) -> Option<&(dyn std::error::Error + Send + Sync)> {
        match self {
            RError::Standard { source, .. } => source.as_deref(),
            RError::Condition { .. } => None,
        }
    }
}

impl fmt::Display for RError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RError::Standard { kind, message, .. } => {
                let prefix = match kind {
                    RErrorKind::Type => "Error",
                    RErrorKind::Argument => "Error in argument",
                    RErrorKind::Name => "Error",
                    RErrorKind::Index => "Error in indexing",
                    RErrorKind::Parse => "Error in parse",
                    RErrorKind::Other => "Error",
                };
                // Name errors have a special format
                if *kind == RErrorKind::Name {
                    write!(f, "Error: object '{}' not found", message)
                } else {
                    write!(f, "{}: {}", prefix, message)
                }
            }
            RError::Condition { condition, .. } => {
                if let RValue::List(list) = condition {
                    for (name, val) in &list.values {
                        if name.as_deref() == Some("message") {
                            if let RValue::Vector(rv) = val {
                                if let Some(s) = rv.as_character_scalar() {
                                    return write!(f, "Error: {}", s);
                                }
                            }
                        }
                    }
                }
                write!(f, "Error: <condition>")
            }
        }
    }
}

// endregion

/// Convert an `RError` into an `RFlow`.
impl From<RError> for RFlow {
    fn from(e: RError) -> Self {
        RFlow::Error(e)
    }
}

/// Create an R condition object (a list with message, call, and class attributes).
pub fn make_condition(message: &str, classes: &[&str]) -> RValue {
    let mut list = RList::new(vec![
        (
            Some("message".to_string()),
            RValue::vec(Vector::Character(vec![Some(message.to_string())].into())),
        ),
        (Some("call".to_string()), RValue::Null),
    ]);
    let class_vec: Vec<Option<String>> = classes.iter().map(|s| Some(s.to_string())).collect();
    list.set_attr(
        "class".to_string(),
        RValue::vec(Vector::Character(class_vec.into())),
    );
    RValue::List(list)
}

/// Create an R condition object with an explicit call value.
pub fn make_condition_with_call(message: &str, call: RValue, classes: &[&str]) -> RValue {
    let mut list = RList::new(vec![
        (
            Some("message".to_string()),
            RValue::vec(Vector::Character(vec![Some(message.to_string())].into())),
        ),
        (Some("call".to_string()), call),
    ]);
    let class_vec: Vec<Option<String>> = classes.iter().map(|s| Some(s.to_string())).collect();
    list.set_attr(
        "class".to_string(),
        RValue::vec(Vector::Character(class_vec.into())),
    );
    RValue::List(list)
}

/// Get the class vector from an RValue (if it has a class attribute).
pub fn get_class(val: &RValue) -> Vec<String> {
    let attrs = match val {
        RValue::Vector(rv) => rv.attrs.as_ref(),
        RValue::List(list) => list.attrs.as_ref(),
        _ => None,
    };
    match attrs.and_then(|a| a.get("class")) {
        Some(RValue::Vector(rv)) => match &rv.inner {
            Vector::Character(v) => v.iter().filter_map(|s| s.clone()).collect(),
            _ => vec![],
        },
        _ => vec![],
    }
}

use crate::parser::ast::{AssignOp, BinaryOp, NaType, SpecialOp, UnaryOp};

/// Convert an AST expression back to R source code (deparse).
pub fn deparse_expr(expr: &Expr) -> String {
    match expr {
        Expr::Null => "NULL".to_string(),
        Expr::Na(NaType::Logical) => "NA".to_string(),
        Expr::Na(NaType::Integer) => "NA_integer_".to_string(),
        Expr::Na(NaType::Real) => "NA_real_".to_string(),
        Expr::Na(NaType::Character) => "NA_character_".to_string(),
        Expr::Na(NaType::Complex) => "NA_complex_".to_string(),
        Expr::Inf => "Inf".to_string(),
        Expr::NaN => "NaN".to_string(),
        Expr::Bool(true) => "TRUE".to_string(),
        Expr::Bool(false) => "FALSE".to_string(),
        Expr::Integer(i) => format!("{}L", i),
        Expr::Double(d) => format_r_double(*d),
        Expr::Complex(d) => format!("{}i", d),
        Expr::String(s) => format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")),
        Expr::Symbol(s) => s.clone(),
        Expr::Dots => "...".to_string(),
        Expr::DotDot(n) => format!("..{}", n),
        Expr::UnaryOp { op, operand } => {
            let o = deparse_expr(operand);
            match op {
                UnaryOp::Neg => format!("-{}", o),
                UnaryOp::Pos => format!("+{}", o),
                UnaryOp::Not => format!("!{}", o),
                UnaryOp::Formula => format!("~{}", o),
            }
        }
        Expr::BinaryOp { op, lhs, rhs } => {
            let l = deparse_expr(lhs);
            let r = deparse_expr(rhs);
            let op_str = match op {
                BinaryOp::Add => "+",
                BinaryOp::Sub => "-",
                BinaryOp::Mul => "*",
                BinaryOp::Div => "/",
                BinaryOp::Pow => "^",
                BinaryOp::Mod => "%%",
                BinaryOp::IntDiv => "%/%",
                BinaryOp::Eq => "==",
                BinaryOp::Ne => "!=",
                BinaryOp::Lt => "<",
                BinaryOp::Gt => ">",
                BinaryOp::Le => "<=",
                BinaryOp::Ge => ">=",
                BinaryOp::And => "&",
                BinaryOp::AndScalar => "&&",
                BinaryOp::Or => "|",
                BinaryOp::OrScalar => "||",
                BinaryOp::Range => ":",
                BinaryOp::Pipe => "|>",
                BinaryOp::Special(SpecialOp::In) => "%in%",
                BinaryOp::Special(SpecialOp::MatMul) => "%*%",
                BinaryOp::Special(SpecialOp::Other) => "%%",
                BinaryOp::Tilde => "~",
            };
            format!("{} {} {}", l, op_str, r)
        }
        Expr::Assign { op, target, value } => {
            let t = deparse_expr(target);
            let v = deparse_expr(value);
            match op {
                AssignOp::LeftAssign => format!("{} <- {}", t, v),
                AssignOp::SuperAssign => format!("{} <<- {}", t, v),
                AssignOp::Equals => format!("{} = {}", t, v),
                AssignOp::RightAssign => format!("{} -> {}", v, t),
                AssignOp::RightSuperAssign => format!("{} ->> {}", v, t),
            }
        }
        Expr::Call { func, args } => {
            let f = deparse_expr(func);
            let a: Vec<String> = args.iter().map(deparse_arg).collect();
            format!("{}({})", f, a.join(", "))
        }
        Expr::Index { object, indices } => {
            let o = deparse_expr(object);
            let i: Vec<String> = indices.iter().map(deparse_arg).collect();
            format!("{}[{}]", o, i.join(", "))
        }
        Expr::IndexDouble { object, indices } => {
            let o = deparse_expr(object);
            let i: Vec<String> = indices.iter().map(deparse_arg).collect();
            format!("{}[[{}]]", o, i.join(", "))
        }
        Expr::Dollar { object, member } => format!("{}${}", deparse_expr(object), member),
        Expr::Slot { object, member } => format!("{}@{}", deparse_expr(object), member),
        Expr::NsGet { namespace, name } => format!("{}::{}", deparse_expr(namespace), name),
        Expr::NsGetInt { namespace, name } => format!("{}:::{}", deparse_expr(namespace), name),
        Expr::Formula { lhs, rhs } => {
            let l = lhs.as_ref().map(|e| deparse_expr(e)).unwrap_or_default();
            let r = rhs.as_ref().map(|e| deparse_expr(e)).unwrap_or_default();
            if l.is_empty() {
                format!("~{}", r)
            } else {
                format!("{} ~ {}", l, r)
            }
        }
        Expr::If {
            condition,
            then_body,
            else_body,
        } => {
            let c = deparse_expr(condition);
            let t = deparse_expr(then_body);
            match else_body {
                Some(e) => format!("if ({}) {} else {}", c, t, deparse_expr(e)),
                None => format!("if ({}) {}", c, t),
            }
        }
        Expr::For { var, iter, body } => {
            format!(
                "for ({} in {}) {}",
                var,
                deparse_expr(iter),
                deparse_expr(body)
            )
        }
        Expr::While { condition, body } => {
            format!("while ({}) {}", deparse_expr(condition), deparse_expr(body))
        }
        Expr::Repeat { body } => format!("repeat {}", deparse_expr(body)),
        Expr::Break => "break".to_string(),
        Expr::Next => "next".to_string(),
        Expr::Return(Some(e)) => format!("return({})", deparse_expr(e)),
        Expr::Return(None) => "return()".to_string(),
        Expr::Block(exprs) => {
            if exprs.len() == 1 {
                deparse_expr(&exprs[0])
            } else {
                let inner: Vec<String> = exprs.iter().map(deparse_expr).collect();
                format!("{{\n    {}\n}}", inner.join("\n    "))
            }
        }
        Expr::Function { params, body } => {
            let p: Vec<String> = params
                .iter()
                .map(|p| {
                    if p.is_dots {
                        "...".to_string()
                    } else if let Some(ref d) = p.default {
                        format!("{} = {}", p.name, deparse_expr(d))
                    } else {
                        p.name.clone()
                    }
                })
                .collect();
            format!("function({}) {}", p.join(", "), deparse_expr(body))
        }
        Expr::Program(exprs) => {
            let inner: Vec<String> = exprs.iter().map(deparse_expr).collect();
            inner.join("\n")
        }
    }
}

use crate::parser::ast::Arg;

fn deparse_arg(arg: &Arg) -> String {
    match (&arg.name, &arg.value) {
        (Some(n), Some(v)) => format!("{} = {}", n, deparse_expr(v)),
        (None, Some(v)) => deparse_expr(v),
        (Some(n), None) => format!("{} = ", n),
        (None, None) => String::new(),
    }
}
