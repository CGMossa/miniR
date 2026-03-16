//! R error types, control flow signals, and condition constructors.

use std::fmt;
use std::num::TryFromIntError;
use std::sync::Arc;

use super::{RList, RValue, Vector};

/// The kind of R condition (error, warning, or message).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConditionKind {
    Error,
    Warning,
    Message,
}

/// The R-facing error category — determines how R's condition system classifies the error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RErrorKind {
    Type,
    Argument,
    Name,
    Index,
    Parse,
    Interrupt,
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

    /// Create an interrupt error (Ctrl+C during computation).
    pub fn interrupt() -> Self {
        RError::new(RErrorKind::Interrupt, "Ctrl+C: computation interrupted")
    }

    /// Returns true if this error represents a user interrupt (Ctrl+C).
    pub fn is_interrupt(&self) -> bool {
        matches!(
            self,
            RError::Standard {
                kind: RErrorKind::Interrupt,
                ..
            }
        )
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
                    RErrorKind::Interrupt => return write!(f, "Interrupted"),
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

// region: RFlow impls

impl From<RError> for RFlow {
    fn from(e: RError) -> Self {
        RFlow::Error(e)
    }
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

impl From<TryFromIntError> for RError {
    fn from(e: TryFromIntError) -> Self {
        RError::from_source(RErrorKind::Type, e)
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

// endregion

// region: Condition constructors

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
        RValue::Language(lang) => lang.attrs.as_ref(),
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

// endregion
