use crate::ast::{BinOp, CmpOp, IRExpr, IRStmt, UnaryOp};
use num_traits::ToPrimitive;
use rustpython_parser::ast;
use std::collections::HashMap;
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

/// Tracks state that must persist across statements while lowering.
///
/// Imports are the motivating case: `import numpy as np` has no runtime effect
/// but binds the local name `np` to the module `numpy` for the rest of the
/// program. The context makes that binding available to later expressions so
/// `np.array(...)` can be resolved to an [`IRExpr::ModuleCall`].
///
/// This mechanism is deliberately module-agnostic: nothing here knows what
/// `numpy` is. Codegen owns the list of modules it can actually generate code
/// for; lowering only records the name bindings the user declared.
#[derive(Debug, Default)]
struct LoweringContext {
    /// Maps a local module alias to its canonical module name.
    /// `import numpy as np` inserts `"np" -> "numpy"`;
    /// `import numpy` inserts `"numpy" -> "numpy"`.
    module_aliases: HashMap<String, String>,
    /// Maps a name bound by `from <module> import <name> [as <alias>]` to the
    /// `(canonical_module, original_name)` pair it refers to.
    from_imports: HashMap<String, (String, String)>,
}

impl LoweringContext {
    /// Resolves a bare name to a module if it was imported as a module alias.
    fn module_for_alias(&self, name: &str) -> Option<&str> {
        self.module_aliases.get(name).map(String::as_str)
    }
}

/// Lowers a `rustpython-parser` AST to the custom IR.
pub fn lower_program(stmts: &[ast::Stmt]) -> Result<Vec<IRStmt>, LoweringError> {
    let mut ctx = LoweringContext::default();
    lower_block(stmts, &mut ctx)
}

/// Lowers a block of statements, threading the lowering context.
///
/// Import statements update the context and produce no IR (they have no runtime
/// effect), so this cannot be a simple 1:1 `map` over statements.
fn lower_block(
    stmts: &[ast::Stmt],
    ctx: &mut LoweringContext,
) -> Result<Vec<IRStmt>, LoweringError> {
    let mut out = Vec::with_capacity(stmts.len());
    for stmt in stmts {
        match stmt {
            ast::Stmt::Import(ast::StmtImport { names, .. }) => {
                for alias in names {
                    let module = alias.name.to_string();
                    let local = alias
                        .asname
                        .as_ref()
                        .map(|n| n.to_string())
                        .unwrap_or_else(|| module.clone());
                    ctx.module_aliases.insert(local, module);
                }
            }
            ast::Stmt::ImportFrom(ast::StmtImportFrom { module, names, .. }) => {
                // `from numpy import array as arr` binds `arr -> (numpy, array)`.
                // A relative import (`from . import x`) has no module name; skip it.
                if let Some(module) = module {
                    let module = module.to_string();
                    for alias in names {
                        let orig = alias.name.to_string();
                        let local = alias
                            .asname
                            .as_ref()
                            .map(|n| n.to_string())
                            .unwrap_or_else(|| orig.clone());
                        ctx.from_imports.insert(local, (module.clone(), orig));
                    }
                }
            }
            other => out.push(lower_statement(other, ctx)?),
        }
    }
    Ok(out)
}

/// Lowers a single statement.
fn lower_statement(stmt: &ast::Stmt, ctx: &mut LoweringContext) -> Result<IRStmt, LoweringError> {
    match stmt {
        ast::Stmt::Expr(ast::StmtExpr { value, .. }) => {
            // Special handling for print() calls
            if let ast::Expr::Call(ast::ExprCall { func, args, .. }) = value.as_ref() {
                if let ast::Expr::Name(ast::ExprName { id, .. }) = func.as_ref() {
                    if id == "print" {
                        // Lower all arguments
                        let lowered_args = lower_expressions(args, ctx)?;
                        return Ok(IRStmt::Print(lowered_args));
                    }
                }
            }
            // General expression statement (e.g., function call without using result)
            let expr = lower_expression(value, ctx)?;
            Ok(IRStmt::ExprStmt(expr))
        }
        ast::Stmt::Assign(ast::StmtAssign { targets, value, .. }) => {
            if targets.len() != 1 {
                return Err(LoweringError::UnsupportedStatement(Box::new(stmt.clone())));
            }
            match &targets[0] {
                ast::Expr::Name(ast::ExprName { id, .. }) => {
                    let value = lower_expression(value, ctx)?;
                    Ok(IRStmt::Assign {
                        target: id.to_string(),
                        value,
                    })
                }
                // `target[index] = value` — item assignment. Slice assignment
                // (`a[i:j] = ...`) is not supported.
                ast::Expr::Subscript(ast::ExprSubscript {
                    value: base, slice, ..
                }) if !matches!(slice.as_ref(), ast::Expr::Slice(_)) => {
                    let target = lower_expression(base, ctx)?;
                    let index = lower_expression(slice, ctx)?;
                    let value = lower_expression(value, ctx)?;
                    Ok(IRStmt::IndexAssign {
                        target,
                        index,
                        value,
                    })
                }
                _ => Err(LoweringError::UnsupportedStatement(Box::new(stmt.clone()))),
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

            // Extract default values from args
            let num_params = args.args.len();
            let defaults_vec: Vec<_> = args.defaults().collect();
            let num_defaults = defaults_vec.len();
            let mut defaults = vec![None; num_params];

            // Default values apply to the last N parameters
            let defaults_start = num_params - num_defaults;
            for (i, default_expr) in defaults_vec.iter().enumerate() {
                let lowered_default = lower_expression(default_expr, ctx)?;
                defaults[defaults_start + i] = Some(lowered_default);
            }

            let body = lower_block(body, ctx)?;
            Ok(IRStmt::FunctionDef {
                name: name.to_string(),
                params,
                defaults,
                body,
            })
        }
        ast::Stmt::Return(ast::StmtReturn { value, .. }) => {
            let value = value
                .as_ref()
                .ok_or_else(|| LoweringError::UnsupportedStatement(Box::new(stmt.clone())))?;
            let expr = lower_expression(value, ctx)?;
            Ok(IRStmt::Return(expr))
        }
        ast::Stmt::If(ast::StmtIf {
            test, body, orelse, ..
        }) => {
            let condition = lower_expression(test, ctx)?;
            let then_body = lower_block(body, ctx)?;

            // Handle else clause (including elif, which is represented as a nested If in orelse)
            let else_body = if !orelse.is_empty() {
                lower_block(orelse, ctx)?
            } else {
                Vec::new()
            };

            Ok(IRStmt::If {
                condition,
                then_body,
                else_body,
            })
        }
        ast::Stmt::While(ast::StmtWhile { test, body, .. }) => {
            let condition = lower_expression(test, ctx)?;
            let body = lower_block(body, ctx)?;
            Ok(IRStmt::While { condition, body })
        }
        ast::Stmt::AugAssign(ast::StmtAugAssign {
            target, op, value, ..
        }) => {
            // Desugar augmented assignment: x += y => x = x + y
            if let ast::Expr::Name(ast::ExprName { id, .. }) = target.as_ref() {
                let current_value = IRExpr::Variable(id.to_string());
                let new_value = lower_expression(value, ctx)?;
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
        ast::Stmt::Break(_) => Ok(IRStmt::Break),
        ast::Stmt::Continue(_) => Ok(IRStmt::Continue),
        ast::Stmt::For(ast::StmtFor {
            target, iter, body, ..
        }) => {
            // Only support for i in range(...) pattern
            if let ast::Expr::Call(ast::ExprCall { func, args, .. }) = iter.as_ref() {
                if let ast::Expr::Name(ast::ExprName { id, .. }) = func.as_ref() {
                    if id == "range" && !args.is_empty() {
                        // Extract the loop variable
                        let var = if let ast::Expr::Name(ast::ExprName { id, .. }) = target.as_ref()
                        {
                            id.to_string()
                        } else {
                            return Err(LoweringError::UnsupportedStatement(Box::new(
                                stmt.clone(),
                            )));
                        };

                        // Handle range(end) or range(start, end)
                        let (start, end) = if args.len() == 1 {
                            // range(end) - start from 0
                            (IRExpr::Constant(0), lower_expression(&args[0], ctx)?)
                        } else if args.len() == 2 {
                            // range(start, end)
                            (
                                lower_expression(&args[0], ctx)?,
                                lower_expression(&args[1], ctx)?,
                            )
                        } else {
                            // range with step is not supported
                            return Err(LoweringError::UnsupportedStatement(Box::new(
                                stmt.clone(),
                            )));
                        };

                        // Lower the loop body
                        let body = lower_block(body, ctx)?;

                        return Ok(IRStmt::For {
                            var,
                            start,
                            end,
                            body,
                        });
                    }
                }
            }
            Err(LoweringError::UnsupportedStatement(Box::new(stmt.clone())))
        }
        _ => Err(LoweringError::UnsupportedStatement(Box::new(stmt.clone()))),
    }
}

/// Lowers a list of expressions (e.g. call arguments), threading the context.
fn lower_expressions(
    exprs: &[ast::Expr],
    ctx: &LoweringContext,
) -> Result<Vec<IRExpr>, LoweringError> {
    exprs.iter().map(|e| lower_expression(e, ctx)).collect()
}

/// Lowers a single expression.
fn lower_expression(expr: &ast::Expr, ctx: &LoweringContext) -> Result<IRExpr, LoweringError> {
    match expr {
        ast::Expr::Constant(ast::ExprConstant { value, .. }) => match value {
            ast::Constant::Int(n) => Ok(IRExpr::Constant(n.to_i64().unwrap())),
            ast::Constant::Float(f) => Ok(IRExpr::Float(*f)),
            ast::Constant::Str(s) => Ok(IRExpr::StringLiteral(s.to_string())),
            ast::Constant::Bool(b) => Ok(IRExpr::Bool(*b)),
            _ => Err(LoweringError::UnsupportedExpression(Box::new(expr.clone()))),
        },
        ast::Expr::Name(ast::ExprName { id, .. }) => Ok(IRExpr::Variable(id.to_string())),
        ast::Expr::BinOp(ast::ExprBinOp {
            left, op, right, ..
        }) => {
            let left = lower_expression(left, ctx)?;
            let right = lower_expression(right, ctx)?;
            let op = lower_binop(op)?;
            Ok(IRExpr::BinaryOp {
                op,
                left: Box::new(left),
                right: Box::new(right),
            })
        }
        ast::Expr::Call(ast::ExprCall { func, args, .. }) => lower_call(expr, func, args, ctx),
        ast::Expr::Attribute(ast::ExprAttribute { value, attr, .. }) => {
            // `module.name` (a module attribute, e.g. `np.pi`) resolves to a
            // module-level lookup; any other `value.attr` is a plain attribute
            // access on a runtime value.
            if let ast::Expr::Name(ast::ExprName { id, .. }) = value.as_ref() {
                if let Some(module) = ctx.module_for_alias(id) {
                    // Model a module constant as a zero-argument module call so
                    // codegen has a single dispatch path for module members.
                    return Ok(IRExpr::ModuleCall {
                        module: module.to_string(),
                        func: attr.to_string(),
                        args: Vec::new(),
                    });
                }
            }
            let value = lower_expression(value, ctx)?;
            Ok(IRExpr::Attribute {
                value: Box::new(value),
                attr: attr.to_string(),
            })
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

            let left = lower_expression(left, ctx)?;
            let right = lower_expression(&comparators[0], ctx)?;
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
            let operand = lower_expression(operand, ctx)?;
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
        ast::Expr::List(ast::ExprList { elts, .. }) => {
            Ok(IRExpr::List(lower_expressions(elts, ctx)?))
        }
        ast::Expr::Subscript(ast::ExprSubscript { value, slice, .. }) => {
            let base = lower_expression(value, ctx)?;
            match slice.as_ref() {
                // `value[lower:upper]` — slicing (copy). A step is not supported.
                ast::Expr::Slice(ast::ExprSlice {
                    lower, upper, step, ..
                }) => {
                    if step.is_some() {
                        return Err(LoweringError::UnsupportedExpression(Box::new(expr.clone())));
                    }
                    let lower = lower
                        .as_ref()
                        .map(|e| lower_expression(e, ctx))
                        .transpose()?
                        .map(Box::new);
                    let upper = upper
                        .as_ref()
                        .map(|e| lower_expression(e, ctx))
                        .transpose()?
                        .map(Box::new);
                    Ok(IRExpr::Slice {
                        value: Box::new(base),
                        lower,
                        upper,
                    })
                }
                // `a[i, j]` — the subscript is a tuple of indices.
                ast::Expr::Tuple(ast::ExprTuple { elts, .. }) => Ok(IRExpr::IndexND {
                    array: Box::new(base),
                    indices: lower_expressions(elts, ctx)?,
                }),
                _ => {
                    let index = lower_expression(slice, ctx)?;
                    Ok(IRExpr::Index {
                        list: Box::new(base),
                        index: Box::new(index),
                    })
                }
            }
        }
        _ => Err(LoweringError::UnsupportedExpression(Box::new(expr.clone()))),
    }
}

/// Lowers a call expression, distinguishing the several call shapes Rusthon
/// understands:
///
/// - `name(...)`         → built-in (`input`/`len`) or a user function call
/// - `module.func(...)`  → [`IRExpr::ModuleCall`] when `module` is an import alias
/// - `receiver.meth(...)`→ [`IRExpr::MethodCall`] otherwise
fn lower_call(
    expr: &ast::Expr,
    func: &ast::Expr,
    args: &[ast::Expr],
    ctx: &LoweringContext,
) -> Result<IRExpr, LoweringError> {
    match func {
        ast::Expr::Name(ast::ExprName { id, .. }) => {
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
            // Handle len() call
            if id == "len" {
                if args.len() != 1 {
                    return Err(LoweringError::UnsupportedExpression(Box::new(expr.clone())));
                }
                let arg = lower_expression(&args[0], ctx)?;
                return Ok(IRExpr::Len(Box::new(arg)));
            }
            // A name bound by `from <module> import <name>` calls into that module.
            if let Some((module, orig)) = ctx.from_imports.get(id.as_str()) {
                return Ok(IRExpr::ModuleCall {
                    module: module.clone(),
                    func: orig.clone(),
                    args: lower_expressions(args, ctx)?,
                });
            }
            Ok(IRExpr::Call {
                func: id.to_string(),
                args: lower_expressions(args, ctx)?,
            })
        }
        ast::Expr::Attribute(ast::ExprAttribute { value, attr, .. }) => {
            // `alias.func(...)` where `alias` names an imported module is a
            // module function call; anything else is a method call on a value.
            if let ast::Expr::Name(ast::ExprName { id, .. }) = value.as_ref() {
                if let Some(module) = ctx.module_for_alias(id) {
                    return Ok(IRExpr::ModuleCall {
                        module: module.to_string(),
                        func: attr.to_string(),
                        args: lower_expressions(args, ctx)?,
                    });
                }
            }
            let receiver = lower_expression(value, ctx)?;
            Ok(IRExpr::MethodCall {
                receiver: Box::new(receiver),
                method: attr.to_string(),
                args: lower_expressions(args, ctx)?,
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

#[cfg(test)]
mod tests {
    use super::*;
    use rustpython_parser::{ast, Parse};

    #[test]
    fn test_bool_literal() {
        let source = "x = True\ny = False";
        let stmts = ast::Suite::parse(source, "<test>").unwrap();
        let ir = lower_program(&stmts).unwrap();

        assert_eq!(ir.len(), 2);
        if let IRStmt::Assign { target, value } = &ir[0] {
            assert_eq!(target, "x");
            assert_eq!(value, &IRExpr::Bool(true));
        } else {
            panic!("Expected Assign statement");
        }

        if let IRStmt::Assign { target, value } = &ir[1] {
            assert_eq!(target, "y");
            assert_eq!(value, &IRExpr::Bool(false));
        } else {
            panic!("Expected Assign statement");
        }
    }

    #[test]
    fn test_elif_support() {
        let source = r#"
if x == 1:
    print(1)
elif x == 2:
    print(2)
else:
    print(3)
"#;
        let stmts = ast::Suite::parse(source, "<test>").unwrap();
        let ir = lower_program(&stmts);

        // Should not error (previously would error on elif)
        assert!(ir.is_ok());

        let ir = ir.unwrap();
        assert_eq!(ir.len(), 1);

        // Check that it's an If statement with an else body containing another If
        if let IRStmt::If {
            condition: _,
            then_body,
            else_body,
        } = &ir[0]
        {
            assert_eq!(then_body.len(), 1);
            assert_eq!(else_body.len(), 1);
            // The else body should contain the elif as a nested If
            assert!(matches!(else_body[0], IRStmt::If { .. }));
        } else {
            panic!("Expected If statement");
        }
    }

    #[test]
    fn test_break_continue() {
        let source = r#"
while True:
    if x == 1:
        break
    if x == 2:
        continue
    print(x)
"#;
        let stmts = ast::Suite::parse(source, "<test>").unwrap();
        let ir = lower_program(&stmts).unwrap();

        assert_eq!(ir.len(), 1);
        if let IRStmt::While { body, .. } = &ir[0] {
            // Find break and continue in the body
            let has_break = body.iter().any(|stmt| {
                if let IRStmt::If { then_body, .. } = stmt {
                    then_body.iter().any(|s| matches!(s, IRStmt::Break))
                } else {
                    false
                }
            });
            let has_continue = body.iter().any(|stmt| {
                if let IRStmt::If { then_body, .. } = stmt {
                    then_body.iter().any(|s| matches!(s, IRStmt::Continue))
                } else {
                    false
                }
            });
            assert!(has_break, "Should contain break statement");
            assert!(has_continue, "Should contain continue statement");
        } else {
            panic!("Expected While statement");
        }
    }

    #[test]
    fn test_for_range() {
        let source = "for i in range(5):\n    print(i)";
        let stmts = ast::Suite::parse(source, "<test>").unwrap();
        let ir = lower_program(&stmts).unwrap();

        assert_eq!(ir.len(), 1);
        if let IRStmt::For {
            var,
            start,
            end,
            body,
        } = &ir[0]
        {
            assert_eq!(var, "i");
            assert_eq!(start, &IRExpr::Constant(0));
            assert_eq!(end, &IRExpr::Constant(5));
            assert_eq!(body.len(), 1);
        } else {
            panic!("Expected For statement");
        }
    }

    #[test]
    fn test_for_range_start_end() {
        let source = "for j in range(2, 8):\n    print(j)";
        let stmts = ast::Suite::parse(source, "<test>").unwrap();
        let ir = lower_program(&stmts).unwrap();

        assert_eq!(ir.len(), 1);
        if let IRStmt::For {
            var,
            start,
            end,
            body,
        } = &ir[0]
        {
            assert_eq!(var, "j");
            assert_eq!(start, &IRExpr::Constant(2));
            assert_eq!(end, &IRExpr::Constant(8));
            assert_eq!(body.len(), 1);
        } else {
            panic!("Expected For statement");
        }
    }
}
