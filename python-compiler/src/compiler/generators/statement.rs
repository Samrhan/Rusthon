//! Statement Compilation Module
//!
//! This module contains helper functions for compiling Python statements to LLVM IR.
//! Each function takes `&mut Compiler` as a parameter to access necessary compilation state.
//!
//! ## Architecture
//! Statement compilation is separated into focused helper functions:
//! - **Simple statements**: Print, Assign, ExprStmt, Return
//! - **Control flow**: If, While, For, Break, Continue (in control.rs)
//!
//! ## Usage
//! These functions are called from `Compiler::compile_statement()` to handle specific
//! statement types while keeping the main compilation logic clean and maintainable.

use crate::ast::IRExpr;
use crate::codegen::{CodeGenError, Compiler};
use inkwell::values::FunctionValue;

// ============================================================================
// Simple Statement Helpers
// ============================================================================

/// Compiles a print statement: print(expr1, expr2, ...)
pub fn compile_print<'ctx>(
    compiler: &mut Compiler<'ctx>,
    exprs: &[IRExpr],
) -> Result<(), CodeGenError> {
    // Handle print with multiple arguments
    if exprs.is_empty() {
        // print() with no arguments just prints a newline
        let printf = compiler.runtime.add_printf(&compiler.module);
        compiler
            .builder
            .build_call(
                printf,
                &[compiler
                    .format_strings
                    .get_newline_format_string(&compiler.builder)
                    .into()],
                "printf_newline",
            )
            .unwrap();
    } else {
        // Print each argument
        for (i, expr) in exprs.iter().enumerate() {
            let value = compiler.compile_expression(expr)?;
            let is_last = i == exprs.len() - 1;

            // Print the value (with newline only for the last one)
            compiler.build_print_value(value, is_last);

            // Print a space between arguments (but not after the last one)
            if !is_last {
                let printf = compiler.runtime.add_printf(&compiler.module);
                compiler
                    .builder
                    .build_call(
                        printf,
                        &[compiler
                            .format_strings
                            .get_space_format_string(&compiler.builder)
                            .into()],
                        "printf_space",
                    )
                    .unwrap();
            }
        }
    }
    Ok(())
}

/// Compiles an assignment statement: target = value
pub fn compile_assign<'ctx>(
    compiler: &mut Compiler<'ctx>,
    target: &str,
    value: &IRExpr,
    current_fn: FunctionValue<'ctx>,
) -> Result<(), CodeGenError> {
    let value = compiler.compile_expression(value)?;
    let ptr = compiler.variables.get(target).copied().unwrap_or_else(|| {
        let ptr = compiler.create_entry_block_alloca(target, current_fn);
        compiler.variables.insert(target.to_string(), ptr);
        ptr
    });
    compiler.builder.build_store(ptr, value).unwrap();
    Ok(())
}

/// Compiles an expression statement (expression evaluated for side effects)
pub fn compile_expr_stmt<'ctx>(
    compiler: &mut Compiler<'ctx>,
    expr: &IRExpr,
) -> Result<(), CodeGenError> {
    // Evaluate the expression and discard the result
    // This is used for function calls that are executed for their side effects
    compiler.compile_expression(expr)?;
    Ok(())
}

/// Compiles a return statement: return expr
pub fn compile_return<'ctx>(
    compiler: &mut Compiler<'ctx>,
    expr: &IRExpr,
) -> Result<(), CodeGenError> {
    let value = compiler.compile_expression(expr)?;
    compiler.builder.build_return(Some(&value)).unwrap();
    Ok(())
}
