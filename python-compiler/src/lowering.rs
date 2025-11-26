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
        ast::Stmt::Assign(ast::StmtAssign { targets, value, .. }) => {
            if targets.len() != 1 {
                return Err(LoweringError::UnsupportedStatement(stmt.clone()));
            }
            if let ast::Expr::Name(ast::ExprName { id, .. }) = &targets[0] {
                let value = lower_expression(value)?;
                Ok(IRStmt::Assign {
                    target: id.to_string(),
                    value,
                })
            } else {
                Err(LoweringError::UnsupportedStatement(stmt.clone()))
            }
        }
        ast::Stmt::FunctionDef(ast::StmtFunctionDef {
            name, args, body, ..
        }) => {
            let params = args
                .args
                .iter()
                .map(|arg| arg.def.arg.to_string())
                .collect();
            let body: Result<Vec<IRStmt>, LoweringError> =
                body.iter().map(lower_statement).collect();
            Ok(IRStmt::FunctionDef {
                name: name.to_string(),
                params,
                body: body?,
            })
        }
        ast::Stmt::Return(ast::StmtReturn { value, .. }) => {
            let value = value
                .as_ref()
                .ok_or_else(|| LoweringError::UnsupportedStatement(stmt.clone()))?;
            let expr = lower_expression(value)?;
            Ok(IRStmt::Return(expr))
        }
        _ => Err(LoweringError::UnsupportedStatement(stmt.clone())),
    }
}

/// Lowers a single expression.
fn lower_expression(expr: &ast::Expr) -> Result<IRExpr, LoweringError> {
    match expr {
        ast::Expr::Constant(ast::ExprConstant { value, .. }) => match value {
            ast::Constant::Int(n) => Ok(IRExpr::Constant(n.to_i64().unwrap())),
            ast::Constant::Float(f) => Ok(IRExpr::Float(*f)),
            _ => Err(LoweringError::UnsupportedExpression(expr.clone())),
        },
        ast::Expr::Name(ast::ExprName { id, .. }) => Ok(IRExpr::Variable(id.to_string())),
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
        ast::Expr::Call(ast::ExprCall { func, args, .. }) => {
            if let ast::Expr::Name(ast::ExprName { id, .. }) = func.as_ref() {
                // Don't handle print here - it's handled as a statement
                if id == "print" {
                    return Err(LoweringError::UnsupportedExpression(expr.clone()));
                }
                // Handle input() call
                if id == "input" {
                    if !args.is_empty() {
                        return Err(LoweringError::UnsupportedExpression(expr.clone()));
                    }
                    return Ok(IRExpr::Input);
                }
                let args: Result<Vec<IRExpr>, LoweringError> =
                    args.iter().map(lower_expression).collect();
                Ok(IRExpr::Call {
                    func: id.to_string(),
                    args: args?,
                })
            } else {
                Err(LoweringError::UnsupportedExpression(expr.clone()))
            }
        }
        _ => Err(LoweringError::UnsupportedExpression(expr.clone())),
    }
}
