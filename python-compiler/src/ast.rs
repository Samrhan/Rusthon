/// The set of supported binary operators.
#[derive(Debug, Clone, PartialEq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,    // %
    BitAnd, // &
    BitOr,  // |
    BitXor, // ^
    LShift, // <<
    RShift, // >>
}

/// The set of supported comparison operators.
#[derive(Debug, Clone, PartialEq)]
pub enum CmpOp {
    Eq,    // ==
    NotEq, // !=
    Lt,    // <
    Gt,    // >
    LtE,   // <=
    GtE,   // >=
}

/// The set of supported unary operators.
#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOp {
    Not,    // not (logical NOT)
    Invert, // ~ (bitwise NOT)
    UAdd,   // +x (unary plus)
    USub,   // -x (unary minus)
}

/// A simplified Intermediate Representation for expressions.
#[derive(Debug, Clone, PartialEq)]
pub enum IRExpr {
    /// A constant integer value.
    Constant(i64),
    /// A constant float value.
    Float(f64),
    /// A boolean literal value.
    Bool(bool),
    /// A variable lookup.
    Variable(String),
    /// A binary operation.
    BinaryOp {
        op: BinOp,
        left: Box<IRExpr>,
        right: Box<IRExpr>,
    },
    /// A function call.
    Call { func: String, args: Vec<IRExpr> },
    /// An input() call to read from stdin.
    Input,
    /// A len() call to get the length of a value.
    Len(Box<IRExpr>),
    /// A comparison operation.
    Comparison {
        op: CmpOp,
        left: Box<IRExpr>,
        right: Box<IRExpr>,
    },
    /// A string literal.
    StringLiteral(String),
    /// A unary operation.
    UnaryOp { op: UnaryOp, operand: Box<IRExpr> },
    /// A list literal.
    List(Vec<IRExpr>),
    /// List indexing.
    Index {
        list: Box<IRExpr>,
        index: Box<IRExpr>,
    },
    /// Array slicing `value[lower:upper]` with copy semantics.
    ///
    /// Omitted bounds default to `0` (lower) and the length (upper); a step is
    /// not supported. Currently only ndarrays can be sliced.
    Slice {
        value: Box<IRExpr>,
        lower: Option<Box<IRExpr>>,
        upper: Option<Box<IRExpr>>,
    },
    /// Attribute access on a value, e.g. `arr.size` or `arr.ndim`.
    ///
    /// Attribute access on an *imported module* (e.g. `np.pi`) is resolved to a
    /// module-level lookup during lowering and never produces this node.
    Attribute { value: Box<IRExpr>, attr: String },
    /// A call to a function exposed by an imported module, e.g. `np.array(...)`.
    ///
    /// `module` is the *canonical* module name (import aliases like `np` are
    /// resolved to `numpy` during lowering), so codegen can dispatch on it
    /// without knowing about the user's local aliases. This node is the generic
    /// mechanism through which any imported module exposes built-in functions.
    ModuleCall {
        module: String,
        func: String,
        args: Vec<IRExpr>,
    },
    /// A method call on a value, e.g. `arr.sum()`.
    MethodCall {
        receiver: Box<IRExpr>,
        method: String,
        args: Vec<IRExpr>,
    },
}

/// A simplified Intermediate Representation for statements.
#[derive(Debug, Clone, PartialEq)]
pub enum IRStmt {
    /// A print statement.
    Print(Vec<IRExpr>),
    /// An assignment statement.
    Assign { target: String, value: IRExpr },
    /// An item assignment `target[index] = value` (arrays and lists).
    IndexAssign {
        target: IRExpr,
        index: IRExpr,
        value: IRExpr,
    },
    /// An expression statement (evaluates an expression and discards the result).
    ExprStmt(IRExpr),
    /// A function definition.
    FunctionDef {
        name: String,
        params: Vec<String>,
        defaults: Vec<Option<IRExpr>>,
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
    /// A for loop (range-based only).
    For {
        var: String,
        start: IRExpr,
        end: IRExpr,
        body: Vec<IRStmt>,
    },
    /// A break statement.
    Break,
    /// A continue statement.
    Continue,
}
