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
use crate::compiler::generators::ndarray;
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
            let is_last = i == exprs.len() - 1;

            // Arrays print as `[e0 e1 ...]` (formatted per dtype); everything
            // else via the scalar dispatcher. Gating on `expr_may_be_array`
            // keeps scalar prints (and their snapshots) unchanged.
            if compiler.expr_may_be_array(expr) {
                let dtype = compiler.require_known_array_dtype(expr)?;
                let value = compiler.compile_expression(expr)?;
                ndarray::print_array(compiler, value, dtype, is_last);
            } else {
                let value = compiler.compile_expression(expr)?;
                // Print the value (with newline only for the last one)
                compiler.build_print_value(value, is_last);
            }

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
    // Track (conservatively) whether this variable may now hold an array, and
    // of which dtype, so later uses emit the right array-aware code. Computed on
    // the IR using the arrayness of variables assigned so far.
    let dtype = compiler.expr_array_dtype(value);

    let value = compiler.compile_expression(value)?;
    let ptr = compiler.variables.get(target).copied().unwrap_or_else(|| {
        let ptr = compiler.create_entry_block_alloca(target, current_fn);
        compiler.variables.insert(target.to_string(), ptr);
        ptr
    });
    compiler.builder.build_store(ptr, value).unwrap();

    match dtype {
        Some(dt) => {
            compiler.array_vars.insert(target.to_string(), dt);
        }
        None => {
            compiler.array_vars.remove(target);
        }
    }
    Ok(())
}

/// Compiles an item assignment `target[index] = value`.
///
/// Dispatches on whether the target might be an array: arrays store a raw `f64`
/// element, lists store a boxed element after the length header. Gating on
/// `expr_may_be_array` keeps this coherent with how indexing reads back.
pub fn compile_index_assign<'ctx>(
    compiler: &mut Compiler<'ctx>,
    target: &IRExpr,
    index: &IRExpr,
    value: &IRExpr,
) -> Result<(), CodeGenError> {
    let array_dtype = compiler.expr_array_dtype(target);
    let obj = compiler.compile_expression(target)?;
    let index_obj = compiler.compile_expression(index)?;
    let value_obj = compiler.compile_expression(value)?;

    if array_dtype.is_some() {
        let dtype = compiler.require_known_array_dtype(target)?;
        ndarray::store_index(compiler, obj, index_obj, value_obj, dtype);
        return Ok(());
    }

    // List store: elements live at offset `index + 1` (after the length header).
    let (list_ptr, _len) = compiler.extract_list_ptr_and_len(obj);
    let index_int = compiler
        .builder
        .build_float_to_signed_int(
            compiler.extract_payload(index_obj),
            compiler.context.i64_type(),
            "index_int",
        )
        .unwrap();
    let adjusted_index = compiler
        .builder
        .build_int_add(
            index_int,
            compiler.context.i64_type().const_int(1, false),
            "adjusted_index",
        )
        .unwrap();
    let pyobject_type = compiler.create_pyobject_type();
    let elem_ptr = unsafe {
        compiler
            .builder
            .build_in_bounds_gep(pyobject_type, list_ptr, &[adjusted_index], "elem_ptr")
            .unwrap()
    };
    compiler.builder.build_store(elem_ptr, value_obj).unwrap();
    Ok(())
}

/// Compiles a tuple-unpacking assignment `t0, t1, ... = value`.
///
/// The value evaluates to a tuple/sequence (list heap layout); each element is
/// loaded and stored into the corresponding target variable. Unpacked values
/// are treated as scalars (arrays do not flow through tuple unpacking).
pub fn compile_unpack<'ctx>(
    compiler: &mut Compiler<'ctx>,
    targets: &[String],
    value: &IRExpr,
    current_fn: FunctionValue<'ctx>,
) -> Result<(), CodeGenError> {
    let seq = compiler.compile_expression(value)?;
    let (seq_ptr, _len) = compiler.extract_list_ptr_and_len(seq);
    let pyobject_type = compiler.create_pyobject_type();

    for (i, target) in targets.iter().enumerate() {
        // Elements start at offset 1 (offset 0 is the length header).
        let index = compiler.context.i64_type().const_int((i + 1) as u64, false);
        let elem_ptr = unsafe {
            compiler
                .builder
                .build_in_bounds_gep(pyobject_type, seq_ptr, &[index], "unpack_elem_ptr")
                .unwrap()
        };
        let elem = compiler
            .builder
            .build_load(pyobject_type, elem_ptr, "unpack_elem")
            .unwrap()
            .into_int_value();

        let ptr = compiler.variables.get(target).copied().unwrap_or_else(|| {
            let ptr = compiler.create_entry_block_alloca(target, current_fn);
            compiler.variables.insert(target.to_string(), ptr);
            ptr
        });
        compiler.builder.build_store(ptr, elem).unwrap();
        compiler.array_vars.remove(target);
    }
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
