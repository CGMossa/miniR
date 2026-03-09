pub mod character;
pub mod double;
pub mod integer;
pub mod logical;

pub use character::Character;
pub use double::Double;
pub use integer::Integer;
pub use logical::Logical;

use std::collections::HashMap;
use std::fmt;
use std::ops::{Deref, DerefMut};

use derive_more::{Deref, DerefMut};

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
    Character(Character),
}

impl Vector {
    pub fn len(&self) -> usize {
        match self {
            Vector::Logical(v) => v.len(),
            Vector::Integer(v) => v.len(),
            Vector::Double(v) => v.len(),
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
            Vector::Character(_) => None,
        }
    }

    /// Get the first element as f64
    pub fn as_double_scalar(&self) -> Option<f64> {
        match self {
            Vector::Double(v) => v.first().copied().flatten(),
            Vector::Integer(v) => v.first().copied().flatten().map(|i| i as f64),
            Vector::Logical(v) => v
                .first()
                .copied()
                .flatten()
                .map(|b| if b { 1.0 } else { 0.0 }),
            Vector::Character(v) => v.first().cloned().flatten().and_then(|s| s.parse().ok()),
        }
    }

    /// Get the first element as i64
    pub fn as_integer_scalar(&self) -> Option<i64> {
        match self {
            Vector::Integer(v) => v.first().copied().flatten(),
            Vector::Double(v) => v.first().copied().flatten().map(|f| f as i64),
            Vector::Logical(v) => v.first().copied().flatten().map(|b| if b { 1 } else { 0 }),
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
        }
    }

    /// Convert entire vector to doubles
    pub fn to_doubles(&self) -> Vec<Option<f64>> {
        match self {
            Vector::Double(v) => v.0.clone(),
            Vector::Integer(v) => v.iter().map(|x| x.map(|i| i as f64)).collect(),
            Vector::Logical(v) => v
                .iter()
                .map(|x| x.map(|b| if b { 1.0 } else { 0.0 }))
                .collect(),
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
            Vector::Double(v) => v.iter().map(|x| x.map(|f| f as i64)).collect(),
            Vector::Logical(v) => v.iter().map(|x| x.map(|b| if b { 1 } else { 0 })).collect(),
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
        }
    }

    /// Convert to logicals
    pub fn to_logicals(&self) -> Vec<Option<bool>> {
        match self {
            Vector::Logical(v) => v.0.clone(),
            Vector::Integer(v) => v.iter().map(|x| x.map(|i| i != 0)).collect(),
            Vector::Double(v) => v.iter().map(|x| x.map(|f| f != 0.0)).collect(),
            Vector::Character(_) => vec![None; self.len()],
        }
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Vector::Logical(_) => "logical",
            Vector::Integer(_) => "integer",
            Vector::Double(_) => "double",
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
        format!("{}", f as i64)
    } else {
        format!("{}", f)
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
            _ => Err(RError::Type("cannot coerce to vector".to_string())),
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

/// R runtime errors
#[derive(Debug, Clone)]
pub enum RError {
    Type(String),
    Argument(String),
    Name(String),
    Index(String),
    #[allow(dead_code)]
    Parse(String),
    Other(String),
    Return(RValue),
    Break,
    Next,
}

impl fmt::Display for RError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RError::Type(msg) => write!(f, "Error: {}", msg),
            RError::Argument(msg) => write!(f, "Error in argument: {}", msg),
            RError::Name(msg) => write!(f, "Error: object '{}' not found", msg),
            RError::Index(msg) => write!(f, "Error in indexing: {}", msg),
            RError::Parse(msg) => write!(f, "Error in parse: {}", msg),
            RError::Other(msg) => write!(f, "Error: {}", msg),
            RError::Return(_) => write!(f, "no function to return from"),
            RError::Break => write!(f, "no loop for break/next, jumping to top level"),
            RError::Next => write!(f, "no loop for break/next, jumping to top level"),
        }
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
