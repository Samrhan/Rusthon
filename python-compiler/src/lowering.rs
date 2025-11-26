use crate::ast::{BinOp, CmpOp, IRExpr, IRStmt};
use num_traits::ToPrimitive;
use rustpython_parser::ast;
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum LoweringError {
    #[error("Unsupported statement: {0:?}")]
    UnsupportedStatement(Box<ast::Stmt>),
    #[error("Unsupported expression: {0:?}")]
    UnsupportedExpression(Box<ast::Expr>),
    #[error("Unsupported operator: {0:?}")]
    UnsupportedOperator(ast::Operator),
    #[error("Unsupported comparison operator: {0:?}")]
    UnsupportedComparisonOperator(ast::CmpOp),
    #[error("Print statement expects 1 argument, but found {0}")]
    PrintArgumentMismatch(usize),
    #[error("Comparison must have exactly one operator and two operands")]
    InvalidComparison,
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
            Err(LoweringError::UnsupportedStatement(Box::new(stmt.clone())))
        }
        ast::Stmt::Assign(ast::StmtAssign { targets, value, .. }) => {
            if targets.len() != 1 {
                return Err(LoweringError::UnsupportedStatement(Box::new(stmt.clone())));
            }
            if let ast::Expr::Name(ast::ExprName { id, .. }) = &targets[0] {
                let value = lower_expression(value)?;
                Ok(IRStmt::Assign {
                    target: id.to_string(),
                    value,
                })
            } else {
                Err(LoweringError::UnsupportedStatement(Box::new(stmt.clone())))
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
                .ok_or_else(|| LoweringError::UnsupportedStatement(Box::new(stmt.clone())))?;
            let expr = lower_expression(value)?;
            Ok(IRStmt::Return(expr))
        }
        ast::Stmt::If(ast::StmtIf {
            test,
            body,
            orelse,
            ..
        }) => {
            let condition = lower_expression(test)?;
            let then_body: Result<Vec<IRStmt>, LoweringError> =
                body.iter().map(lower_statement).collect();

            // Handle else clause
            let else_body = if !orelse.is_empty() {
                // For simplicity, check if it's a plain else or elif
                // If orelse contains an If statement, it's an elif
                if orelse.len() == 1 {
                    if let ast::Stmt::If(_) = &orelse[0] {
                        // This is an elif - not supported yet
                        return Err(LoweringError::UnsupportedStatement(Box::new(stmt.clone())));
                    }
                }
                // It's a plain else clause
                let else_stmts: Result<Vec<IRStmt>, LoweringError> =
                    orelse.iter().map(lower_statement).collect();
                else_stmts?
            } else {
                Vec::new()
            };

            Ok(IRStmt::If {
                condition,
                then_body: then_body?,
                else_body,
            })
        }
        ast::Stmt::While(ast::StmtWhile { test, body, .. }) => {
            let condition = lower_expression(test)?;
            let body: Result<Vec<IRStmt>, LoweringError> =
                body.iter().map(lower_statement).collect();
            Ok(IRStmt::While {
                condition,
                body: body?,
            })
        }
        _ => Err(LoweringError::UnsupportedStatement(Box::new(stmt.clone()))),
    }
}

/// Lowers a single expression.
fn lower_expression(expr: &ast::Expr) -> Result<IRExpr, LoweringError> {
    match expr {
        ast::Expr::Constant(ast::ExprConstant { value, .. }) => match value {
            ast::Constant::Int(n) => Ok(IRExpr::Constant(n.to_i64().unwrap())),
            ast::Constant::Float(f) => Ok(IRExpr::Float(*f)),
            ast::Constant::Str(s) => Ok(IRExpr::StringLiteral(s.to_string())),
            _ => Err(LoweringError::UnsupportedExpression(Box::new(expr.clone()))),
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
                _ => return Err(LoweringError::UnsupportedOperator(*op)),
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
                    return Err(LoweringError::UnsupportedExpression(Box::new(expr.clone())));
                }
                // Handle input() call
                if id == "input" {
                    if !args.is_empty() {
                        return Err(LoweringError::UnsupportedExpression(Box::new(expr.clone())));
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
                Err(LoweringError::UnsupportedExpression(Box::new(expr.clone())))
            }
        }
        ast::Expr::Compare(ast::ExprCompare {
            left,
            ops,
            comparators,
            ..
        }) => {
            // For simplicity, only support single comparisons (e.g., a < b, not a < b < c)
            if ops.len() != 1 || comparators.len() != 1 {
                return Err(LoweringError::InvalidComparison);
            }

            let left = lower_expression(left)?;
            let right = lower_expression(&comparators[0])?;
            let op = match &ops[0] {
                ast::CmpOp::Eq => CmpOp::Eq,
                ast::CmpOp::NotEq => CmpOp::NotEq,
                ast::CmpOp::Lt => CmpOp::Lt,
                ast::CmpOp::Gt => CmpOp::Gt,
                ast::CmpOp::LtE => CmpOp::LtE,
                ast::CmpOp::GtE => CmpOp::GtE,
                _ => return Err(LoweringError::UnsupportedComparisonOperator(ops[0])),
            };

            Ok(IRExpr::Comparison {
                op,
                left: Box::new(left),
                right: Box::new(right),
            })
        }
        _ => Err(LoweringError::UnsupportedExpression(Box::new(expr.clone()))),
    }
}
