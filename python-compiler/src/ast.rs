/// The set of supported binary operators.
#[derive(Debug, Clone, PartialEq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
}

/// A simplified Intermediate Representation for expressions.
#[derive(Debug, Clone, PartialEq)]
pub enum IRExpr {
    /// A constant integer value.
    Constant(i64),
    /// A variable lookup.
    Variable(String),
    /// A binary operation.
    BinaryOp {
        op: BinOp,
        left: Box<IRExpr>,
        right: Box<IRExpr>,
    },
}

/// A simplified Intermediate Representation for statements.
#[derive(Debug, Clone, PartialEq)]
pub enum IRStmt {
    /// A print statement.
    Print(IRExpr),
    /// An assignment statement.
    Assign { target: String, value: IRExpr },
}
