use crate::ast::{BinOp, CmpOp, IRExpr, IRStmt, UnaryOp};
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
                        // Lower all arguments
                        let lowered_args: Result<Vec<IRExpr>, LoweringError> =
                            args.iter().map(lower_expression).collect();
                        return Ok(IRStmt::Print(lowered_args?));
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
        ast::Stmt::AugAssign(ast::StmtAugAssign { target, op, value, .. }) => {
            // Desugar augmented assignment: x += y => x = x + y
            if let ast::Expr::Name(ast::ExprName { id, .. }) = target.as_ref() {
                let current_value = IRExpr::Variable(id.to_string());
                let new_value = lower_expression(value)?;
                let op = lower_binop(op)?;
                let result = IRExpr::BinaryOp {
                    op,
                    left: Box::new(current_value),
                    right: Box::new(new_value),
                };
                Ok(IRStmt::Assign {
                    target: id.to_string(),
                    value: result,
                })
            } else {
                Err(LoweringError::UnsupportedStatement(Box::new(stmt.clone())))
            }
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
            let op = lower_binop(op)?;
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
        ast::Expr::UnaryOp(ast::ExprUnaryOp { op, operand, .. }) => {
            let operand = lower_expression(operand)?;
            let op = match op {
                ast::UnaryOp::Invert => UnaryOp::Invert,
                ast::UnaryOp::Not => UnaryOp::Not,
                ast::UnaryOp::UAdd => UnaryOp::UAdd,
                ast::UnaryOp::USub => UnaryOp::USub,
            };
            Ok(IRExpr::UnaryOp {
                op,
                operand: Box::new(operand),
            })
        }
        _ => Err(LoweringError::UnsupportedExpression(Box::new(expr.clone()))),
    }
}

/// Helper function to convert AST binary operators to IR binary operators.
fn lower_binop(op: &ast::Operator) -> Result<BinOp, LoweringError> {
    match op {
        ast::Operator::Add => Ok(BinOp::Add),
        ast::Operator::Sub => Ok(BinOp::Sub),
        ast::Operator::Mult => Ok(BinOp::Mul),
        ast::Operator::Div => Ok(BinOp::Div),
        ast::Operator::Mod => Ok(BinOp::Mod),
        ast::Operator::BitAnd => Ok(BinOp::BitAnd),
        ast::Operator::BitOr => Ok(BinOp::BitOr),
        ast::Operator::BitXor => Ok(BinOp::BitXor),
        ast::Operator::LShift => Ok(BinOp::LShift),
        ast::Operator::RShift => Ok(BinOp::RShift),
        _ => Err(LoweringError::UnsupportedOperator(*op)),
    }
}
