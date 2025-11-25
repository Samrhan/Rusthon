use crate::ast::{BinOp, IRExpr, IRStmt};
use num_traits::ToPrimitive;
use rustpython_parser::ast;
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum LoweringError {
    #[error("Unsupported statement: {0:?}")]
    UnsupportedStatement(ast::Stmt),
    #[error("Unsupported expression: {0:?}")]
    UnsupportedExpression(ast::Expr),
    #[error("Unsupported operator: {0:?}")]
    UnsupportedOperator(ast::Operator),
    #[error("Print statement expects 1 argument, but found {0}")]
    PrintArgumentMismatch(usize),
}

/// Lowers a `rustpython-parser` AST to the custom IR.
pub fn lower_program(stmts: &[ast::Stmt]) -> Result<Vec<IRStmt>, LoweringError> {
    stmts.iter().map(lower_statement).collect()
}

/// Lowers a single statement.
fn lower_statement(stmt: &ast::Stmt) -> Result<IRStmt, LoweringError> {
    match stmt {
        ast::Stmt::Expr(ast::StmtExpr { value, .. }) => {
            if let ast::Expr::Call(ast::ExprCall { func, args, .. }) = value.as_ref() {
                if let ast::Expr::Name(ast::ExprName { id, .. }) = func.as_ref() {
                    if id == "print" {
                        if args.len() != 1 {
                            return Err(LoweringError::PrintArgumentMismatch(args.len()));
                        }
                        let arg = lower_expression(&args[0])?;
                        return Ok(IRStmt::Print(arg));
                    }
                }
            }
            Err(LoweringError::UnsupportedStatement(stmt.clone()))
        }
        _ => Err(LoweringError::UnsupportedStatement(stmt.clone())),
    }
}

/// Lowers a single expression.
fn lower_expression(expr: &ast::Expr) -> Result<IRExpr, LoweringError> {
    match expr {
        ast::Expr::Constant(ast::ExprConstant { value, .. }) => match value {
            ast::Constant::Int(n) => Ok(IRExpr::Constant(n.to_i64().unwrap())),
            _ => Err(LoweringError::UnsupportedExpression(expr.clone())),
        },
        ast::Expr::BinOp(ast::ExprBinOp {
            left,
            op,
            right,
            ..
        }) => {
            let left = lower_expression(left)?;
            let right = lower_expression(right)?;
            let op = match op {
                ast::Operator::Add => BinOp::Add,
                ast::Operator::Sub => BinOp::Sub,
                ast::Operator::Mult => BinOp::Mul,
                ast::Operator::Div => BinOp::Div,
                _ => return Err(LoweringError::UnsupportedOperator(op.clone())),
            };
            Ok(IRExpr::BinaryOp {
                op,
                left: Box::new(left),
                right: Box::new(right),
            })
        }
        _ => Err(LoweringError::UnsupportedExpression(expr.clone())),
    }
}
