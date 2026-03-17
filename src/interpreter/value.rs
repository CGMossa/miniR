pub mod character;
pub mod complex;
pub mod double;
pub mod error;
pub mod integer;
pub mod logical;
pub mod traits;
pub mod vector;

pub use character::Character;
pub use complex::ComplexVec;
pub use double::Double;
pub use error::{
    get_class, make_condition, make_condition_with_call, ConditionKind, RError, RErrorKind, RFlow,
    RSignal,
};
pub use integer::Integer;
pub use logical::Logical;
pub use traits::{coerce_arg, find_arg, Builtin, BuiltinInfo, CoerceArg, FromArgs};
pub use vector::{format_r_complex, format_r_double, Vector};

use std::fmt;
use std::ops::{Deref, DerefMut};

use indexmap::IndexMap;
use itertools::Itertools;
use unicode_width::UnicodeWidthStr;

use crate::interpreter::environment::Environment;
use crate::interpreter::BuiltinContext;
use crate::parser::ast::{Arg, Expr, Param};

pub type BuiltinFn = fn(&[RValue], &[(String, RValue)]) -> Result<RValue, RError>;
pub type InterpreterBuiltinFn =
    for<'a> fn(&[RValue], &[(String, RValue)], &BuiltinContext<'a>) -> Result<RValue, RError>;
pub type PreEvalBuiltinFn =
    for<'a> fn(&[Arg], &Environment, &BuiltinContext<'a>) -> Result<RValue, RError>;

#[derive(Debug, Clone, Copy)]
pub enum BuiltinImplementation {
    Eager(BuiltinFn),
    Interpreter(InterpreterBuiltinFn),
    PreEval(PreEvalBuiltinFn),
}

#[derive(Debug, Clone, Copy)]
pub struct BuiltinDescriptor {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub implementation: BuiltinImplementation,
    pub min_args: usize,
    pub max_args: Option<usize>,
    /// Raw doc string extracted from rustdoc comments (may contain @param/@title tags).
    pub doc: &'static str,
    /// Namespace this builtin belongs to (e.g. "base", "stats", "utils", "collections").
    pub namespace: &'static str,
}

/// Attribute map — every R object can carry named attributes.
///
/// Uses `IndexMap` to preserve insertion order, matching R's behavior where
/// `attributes(x)` returns attributes in the order they were set.
pub type Attributes = IndexMap<String, RValue>;

/// Unevaluated expression (language object) — returned by quote(), parse().
///
/// Wraps a boxed AST node. Derefs to `Expr` for pattern matching.
#[derive(Debug, Clone)]
pub struct Language {
    pub inner: Box<Expr>,
    pub attrs: Option<Box<Attributes>>,
}

impl Language {
    pub fn new(expr: Expr) -> Self {
        Language {
            inner: Box::new(expr),
            attrs: None,
        }
    }

    pub fn get_attr(&self, name: &str) -> Option<&RValue> {
        self.attrs.as_ref().and_then(|a| a.get(name))
    }

    pub fn set_attr(&mut self, name: String, value: RValue) {
        self.attrs
            .get_or_insert_with(|| Box::new(IndexMap::new()))
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

impl Deref for Language {
    type Target = Expr;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for Language {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
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
            .get_or_insert_with(|| Box::new(IndexMap::new()))
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
            .get_or_insert_with(|| Box::new(IndexMap::new()))
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
        implementation: BuiltinImplementation,
        min_args: usize,
        max_args: Option<usize>,
    },
}

// region: From / TryFrom impls

impl From<RVector> for RValue {
    fn from(rv: RVector) -> Self {
        RValue::Vector(rv)
    }
}

impl From<RList> for RValue {
    fn from(list: RList) -> Self {
        RValue::List(list)
    }
}

impl<'a> TryFrom<&'a RValue> for &'a RVector {
    type Error = RError;
    fn try_from(value: &'a RValue) -> Result<Self, Self::Error> {
        match value {
            RValue::Vector(rv) => Ok(rv),
            other => Err(RError::new(
                RErrorKind::Type,
                format!("expected vector, got {}", other.type_name()),
            )),
        }
    }
}

impl TryFrom<RValue> for RVector {
    type Error = RError;
    fn try_from(value: RValue) -> Result<Self, Self::Error> {
        match value {
            RValue::Vector(rv) => Ok(rv),
            other => Err(RError::new(
                RErrorKind::Type,
                format!("expected vector, got {}", other.type_name()),
            )),
        }
    }
}

impl<'a> TryFrom<&'a RValue> for &'a RList {
    type Error = RError;
    fn try_from(value: &'a RValue) -> Result<Self, Self::Error> {
        match value {
            RValue::List(l) => Ok(l),
            other => Err(RError::new(
                RErrorKind::Type,
                format!("expected list, got {}", other.type_name()),
            )),
        }
    }
}

impl TryFrom<RValue> for RList {
    type Error = RError;
    fn try_from(value: RValue) -> Result<Self, Self::Error> {
        match value {
            RValue::List(l) => Ok(l),
            other => Err(RError::new(
                RErrorKind::Type,
                format!("expected list, got {}", other.type_name()),
            )),
        }
    }
}

// endregion

// region: RValue impls

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

// endregion

// region: Display

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
            Vector::Raw(_) => "raw(0)".to_string(),
            Vector::Logical(_) => "logical(0)".to_string(),
            Vector::Integer(_) => "integer(0)".to_string(),
            Vector::Double(_) => "numeric(0)".to_string(),
            Vector::Complex(_) => "complex(0)".to_string(),
            Vector::Character(_) => "character(0)".to_string(),
        };
    }

    let elements: Vec<String> = match v {
        Vector::Raw(vals) => vals.iter().map(|b| format!("{:02x}", b)).collect(),
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
        let label_width = UnicodeWidthStr::width(label.as_str());
        let mut line = format!("{} ", label);
        let mut current_width = label_width + 1;
        let line_start = pos;

        while pos < elements.len() {
            let elem = &elements[pos];
            let elem_width = UnicodeWidthStr::width(elem.as_str()) + 1; // +1 for space
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

// endregion

// region: deparse

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
            format!("{}({})", f, args.iter().map(deparse_arg).join(", "))
        }
        Expr::Index { object, indices } => {
            let o = deparse_expr(object);
            format!("{}[{}]", o, indices.iter().map(deparse_arg).join(", "))
        }
        Expr::IndexDouble { object, indices } => {
            let o = deparse_expr(object);
            format!("{}[[{}]]", o, indices.iter().map(deparse_arg).join(", "))
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
                format!(
                    "{{\n    {}\n}}",
                    exprs.iter().map(deparse_expr).join("\n    ")
                )
            }
        }
        Expr::Function { params, body } => {
            let p = params
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
                .join(", ");
            format!("function({}) {}", p, deparse_expr(body))
        }
        Expr::Program(exprs) => exprs.iter().map(deparse_expr).join("\n"),
    }
}

fn deparse_arg(arg: &Arg) -> String {
    match (&arg.name, &arg.value) {
        (Some(n), Some(v)) => format!("{} = {}", n, deparse_expr(v)),
        (None, Some(v)) => deparse_expr(v),
        (Some(n), None) => format!("{} = ", n),
        (None, None) => String::new(),
    }
}

// endregion
