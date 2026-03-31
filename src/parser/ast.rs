/// Source span — byte offsets into the source text.
/// Used to resolve file:line info for stack traces.
#[derive(Debug, Clone, Copy)]
pub struct Span {
    /// Byte offset of the start of this expression in the source.
    pub start: u32,
    /// Byte offset of the end of this expression.
    pub end: u32,
}

/// AST node types for the R language

#[derive(Debug, Clone)]
pub enum Expr {
    /// NULL literal
    Null,
    /// NA (with optional type)
    Na(NaType),
    /// Inf
    Inf,
    /// NaN
    NaN,
    /// Boolean literal
    Bool(bool),
    /// Integer literal
    Integer(i64),
    /// Double/float literal
    Double(f64),
    /// Complex literal (imaginary part only, e.g. 2i)
    Complex(f64),
    /// String literal
    String(String),
    /// Identifier/symbol
    Symbol(String),
    /// ... (dots)
    Dots,
    /// ..1, ..2, etc.
    DotDot(u32),

    /// Unary operation
    UnaryOp { op: UnaryOp, operand: Box<Expr> },
    /// Binary operation
    BinaryOp {
        op: BinaryOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    /// Assignment
    Assign {
        op: AssignOp,
        target: Box<Expr>,
        value: Box<Expr>,
    },

    /// Function call
    Call {
        func: Box<Expr>,
        args: Vec<Arg>,
        span: Option<Span>,
    },
    /// Single bracket indexing: x[i]
    Index {
        object: Box<Expr>,
        indices: Vec<Arg>,
    },
    /// Double bracket indexing: x[[i]]
    IndexDouble {
        object: Box<Expr>,
        indices: Vec<Arg>,
    },
    /// Dollar access: x$name
    Dollar { object: Box<Expr>, member: String },
    /// Slot access: x@slot
    Slot { object: Box<Expr>, member: String },
    /// Namespace access: pkg::name
    NsGet { namespace: Box<Expr>, name: String },
    /// Internal namespace access: pkg:::name
    NsGetInt { namespace: Box<Expr>, name: String },

    /// Formula: ~ expr or lhs ~ rhs
    Formula {
        lhs: Option<Box<Expr>>,
        rhs: Option<Box<Expr>>,
    },

    /// if/else expression
    If {
        condition: Box<Expr>,
        then_body: Box<Expr>,
        else_body: Option<Box<Expr>>,
    },
    /// for loop
    For {
        var: String,
        iter: Box<Expr>,
        body: Box<Expr>,
    },
    /// while loop
    While {
        condition: Box<Expr>,
        body: Box<Expr>,
    },
    /// repeat loop
    Repeat { body: Box<Expr> },
    /// break
    Break,
    /// next (continue)
    Next,
    /// return
    Return(Option<Box<Expr>>),

    /// Block (curly braces)
    Block(Vec<Expr>),

    /// Function definition
    Function { params: Vec<Param>, body: Box<Expr> },

    /// Program (sequence of top-level expressions)
    Program(Vec<Expr>),
}

#[derive(Debug, Clone)]
pub struct Arg {
    pub name: Option<String>,
    pub value: Option<Expr>,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub default: Option<Expr>,
    pub is_dots: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum NaType {
    Logical,
    Integer,
    Real,
    Character,
    Complex,
}

#[derive(Debug, Clone, Copy)]
pub enum UnaryOp {
    Neg,
    Pos,
    Not,
    #[allow(dead_code)]
    Formula,
}

#[derive(Debug, Clone, Copy)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
    Mod,
    IntDiv,
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    And,
    AndScalar,
    Or,
    OrScalar,
    Range,
    Pipe,
    AssignPipe, // %<>% — pipe and assign back to LHS
    TeePipe,    // %T>% — pipe for side effect, return LHS
    ExpoPipe,   // %$%  — expose LHS names to RHS
    Special(SpecialOp),
    #[allow(dead_code)]
    Tilde,
    #[allow(dead_code)]
    DoubleTilde,
}

#[derive(Debug, Clone, Copy)]
pub enum SpecialOp {
    In,
    MatMul,
    Kronecker,
    Walrus,
    Other,
}

#[derive(Debug, Clone, Copy)]
pub enum AssignOp {
    LeftAssign,       // <-
    SuperAssign,      // <<-
    Equals,           // =
    RightAssign,      // ->
    RightSuperAssign, // ->>
}
