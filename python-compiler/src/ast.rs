/// The set of supported binary operators.
#[derive(Debug, Clone, PartialEq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,      // %
    BitAnd,   // &
    BitOr,    // |
    BitXor,   // ^
    LShift,   // <<
    RShift,   // >>
}

/// The set of supported comparison operators.
#[derive(Debug, Clone, PartialEq)]
pub enum CmpOp {
    Eq,   // ==
    NotEq, // !=
    Lt,   // <
    Gt,   // >
    LtE,  // <=
    GtE,  // >=
}

/// The set of supported unary operators.
#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOp {
    Not,      // not (logical NOT)
    Invert,   // ~ (bitwise NOT)
    UAdd,     // +x (unary plus)
    USub,     // -x (unary minus)
}

/// A simplified Intermediate Representation for expressions.
#[derive(Debug, Clone, PartialEq)]
pub enum IRExpr {
    /// A constant integer value.
    Constant(i64),
    /// A constant float value.
    Float(f64),
    /// A variable lookup.
    Variable(String),
    /// A binary operation.
    BinaryOp {
        op: BinOp,
        left: Box<IRExpr>,
        right: Box<IRExpr>,
    },
    /// A function call.
    Call {
        func: String,
        args: Vec<IRExpr>,
    },
    /// An input() call to read from stdin.
    Input,
    /// A comparison operation.
    Comparison {
        op: CmpOp,
        left: Box<IRExpr>,
        right: Box<IRExpr>,
    },
    /// A string literal.
    StringLiteral(String),
    /// A unary operation.
    UnaryOp {
        op: UnaryOp,
        operand: Box<IRExpr>,
    },
}

/// A simplified Intermediate Representation for statements.
#[derive(Debug, Clone, PartialEq)]
pub enum IRStmt {
    /// A print statement.
    Print(IRExpr),
    /// An assignment statement.
    Assign { target: String, value: IRExpr },
    /// A function definition.
    FunctionDef {
        name: String,
        params: Vec<String>,
        body: Vec<IRStmt>,
    },
    /// A return statement.
    Return(IRExpr),
    /// An if/else statement.
    If {
        condition: IRExpr,
        then_body: Vec<IRStmt>,
        else_body: Vec<IRStmt>,
    },
    /// A while loop.
    While {
        condition: IRExpr,
        body: Vec<IRStmt>,
    },
}
