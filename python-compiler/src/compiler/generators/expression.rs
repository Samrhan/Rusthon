//! Expression Compilation Module
//!
//! This module contains helper functions for compiling Python expressions to LLVM IR.
//! Each function takes `&mut Compiler` as a parameter to access necessary compilation state.
//!
//! ## Architecture
//! Expression compilation is separated into focused helper functions:
//! - **Simple values**: Constants, literals, variables
//! - **Binary operations**: Arithmetic, bitwise, string concatenation
//! - **Unary operations**: Negation, not, bitwise not
//! - **Complex operations**: Function calls, list operations, indexing
//! - **Comparisons**: Equality, ordering
//!
//! ## Usage
//! These functions are called from `Compiler::compile_expression()` to handle specific
//! expression types while keeping the main compilation logic clean and maintainable.

use crate::ast::{BinOp, CmpOp, IRExpr, UnaryOp};
use crate::codegen::{CodeGenError, Compiler};
use crate::compiler::values::{TYPE_TAG_FLOAT, TYPE_TAG_INT, TYPE_TAG_LIST, TYPE_TAG_STRING};
use inkwell::values::IntValue;
use inkwell::FloatPredicate;

// ============================================================================
// Simple Expression Helpers
// ============================================================================

/// Compiles a constant integer expression
pub fn compile_constant<'ctx>(
    compiler: &Compiler<'ctx>,
    value: i64,
) -> Result<IntValue<'ctx>, CodeGenError> {
    let int_val = compiler.context.i64_type().const_int(value as u64, true);
    Ok(compiler.create_pyobject_int(int_val))
}

/// Compiles a constant float expression
pub fn compile_float<'ctx>(
    compiler: &Compiler<'ctx>,
    value: f64,
) -> Result<IntValue<'ctx>, CodeGenError> {
    let float_val = compiler.context.f64_type().const_float(value);
    Ok(compiler.create_pyobject_float(float_val))
}

/// Compiles a boolean constant expression
pub fn compile_bool<'ctx>(
    compiler: &Compiler<'ctx>,
    value: bool,
) -> Result<IntValue<'ctx>, CodeGenError> {
    let bool_val = compiler.context.bool_type().const_int(value as u64, false);
    Ok(compiler.create_pyobject_bool(bool_val))
}

/// Compiles a variable access expression
pub fn compile_variable<'ctx>(
    compiler: &Compiler<'ctx>,
    name: &str,
) -> Result<IntValue<'ctx>, CodeGenError> {
    let ptr = compiler
        .variables
        .get(name)
        .ok_or_else(|| CodeGenError::UndefinedVariable(name.to_string()))?;

    let pyobject_type = compiler.create_pyobject_type();
    let loaded = compiler
        .builder
        .build_load(pyobject_type, *ptr, name)
        .unwrap();

    Ok(loaded.into_int_value())
}

/// Compiles a string literal expression
pub fn compile_string_literal<'ctx>(
    compiler: &mut Compiler<'ctx>,
    s: &str,
) -> Result<IntValue<'ctx>, CodeGenError> {
    // Calculate string length (including null terminator)
    let str_len = s.len() + 1;
    let size = compiler.context.i64_type().const_int(str_len as u64, false);

    // Call malloc to allocate memory
    let malloc_fn = compiler.runtime.add_malloc(&compiler.module);
    let malloc_result = compiler
        .builder
        .build_call(malloc_fn, &[size.into()], "malloc_str")
        .unwrap();

    // Get the allocated pointer
    use inkwell::values::ValueKind;
    let str_ptr = match malloc_result.try_as_basic_value() {
        ValueKind::Basic(value) => value.into_pointer_value(),
        ValueKind::Instruction(_) => {
            return Err(CodeGenError::UndefinedVariable(
                "malloc did not return a value".to_string(),
            ))
        }
    };

    // Create a global string constant for the literal
    let global_str = compiler
        .builder
        .build_global_string_ptr(s, "str_literal")
        .unwrap();

    // Use memcpy to copy the string to the allocated memory
    let memcpy_fn = compiler.runtime.add_memcpy(&compiler.module);
    compiler
        .builder
        .build_call(
            memcpy_fn,
            &[
                str_ptr.into(),
                global_str.as_pointer_value().into(),
                size.into(),
            ],
            "memcpy_str",
        )
        .unwrap();

    // Track the allocated string in the arena for cleanup only if in main entry block
    if let Some(main_entry) = compiler.main_entry_block {
        if compiler.builder.get_insert_block() == Some(main_entry) {
            compiler.string_arena.push(str_ptr);
        }
    }

    // Wrap the string pointer in a PyObject
    Ok(compiler.create_pyobject_string(str_ptr))
}

// ============================================================================
// Comparison Operations
// ============================================================================

/// Compiles a comparison expression (==, !=, <, >, <=, >=)
pub fn compile_comparison<'ctx>(
    compiler: &mut Compiler<'ctx>,
    op: &CmpOp,
    left: &IRExpr,
    right: &IRExpr,
) -> Result<IntValue<'ctx>, CodeGenError> {
    let lhs_obj = compiler.compile_expression(left)?;
    let rhs_obj = compiler.compile_expression(right)?;

    // Extract payloads (values are already stored as f64)
    let lhs_payload = compiler.extract_payload(lhs_obj);
    let rhs_payload = compiler.extract_payload(rhs_obj);

    // Perform the comparison
    let predicate = match op {
        CmpOp::Eq => FloatPredicate::OEQ,    // Ordered and equal
        CmpOp::NotEq => FloatPredicate::ONE, // Ordered and not equal
        CmpOp::Lt => FloatPredicate::OLT,    // Ordered and less than
        CmpOp::Gt => FloatPredicate::OGT,    // Ordered and greater than
        CmpOp::LtE => FloatPredicate::OLE,   // Ordered and less than or equal
        CmpOp::GtE => FloatPredicate::OGE,   // Ordered and greater than or equal
    };

    let cmp_result = compiler
        .builder
        .build_float_compare(predicate, lhs_payload, rhs_payload, "cmptmp")
        .unwrap();

    // Return as PyObject with bool tag
    Ok(compiler.create_pyobject_bool(cmp_result))
}

// ============================================================================
// Unary Operations
// ============================================================================

/// Compiles a unary operation expression (-, +, ~, not)
pub fn compile_unary_op<'ctx>(
    compiler: &mut Compiler<'ctx>,
    op: &UnaryOp,
    operand: &IRExpr,
) -> Result<IntValue<'ctx>, CodeGenError> {
    let operand_obj = compiler.compile_expression(operand)?;

    match op {
        UnaryOp::Invert => {
            // Bitwise NOT (~x)
            let payload = compiler.extract_payload(operand_obj);
            let operand_int = compiler
                .builder
                .build_float_to_signed_int(payload, compiler.context.i64_type(), "to_int")
                .unwrap();
            let result = compiler.builder.build_not(operand_int, "not").unwrap();
            Ok(compiler.create_pyobject_int(result))
        }
        UnaryOp::USub => {
            // Unary minus (-x)
            let payload = compiler.extract_payload(operand_obj);
            let zero = compiler.context.f64_type().const_float(0.0);
            let result = compiler
                .builder
                .build_float_sub(zero, payload, "neg")
                .unwrap();

            // Preserve the type tag from the operand
            let tag = compiler.extract_tag(operand_obj);
            let result_obj = compiler.create_pyobject_from_tag_and_payload(tag, result);

            Ok(result_obj)
        }
        UnaryOp::UAdd => {
            // Unary plus (+x) - just return the operand unchanged
            Ok(operand_obj)
        }
        UnaryOp::Not => {
            // Logical NOT (not x)
            let payload = compiler.extract_payload(operand_obj);
            let zero = compiler.context.f64_type().const_float(0.0);

            // Check if operand is zero
            let is_zero = compiler
                .builder
                .build_float_compare(FloatPredicate::OEQ, payload, zero, "is_zero")
                .unwrap();

            // Return True if operand is zero, False otherwise
            Ok(compiler.create_pyobject_bool(is_zero))
        }
    }
}

// ============================================================================
// List Operations
// ============================================================================

/// Compiles a list literal expression [a, b, c]
pub fn compile_list<'ctx>(
    compiler: &mut Compiler<'ctx>,
    elements: &[IRExpr],
) -> Result<IntValue<'ctx>, CodeGenError> {
    // Compile all element expressions
    let mut compiled_elements = Vec::new();
    for elem in elements {
        let elem_pyobj = compiler.compile_expression(elem)?;
        compiled_elements.push(elem_pyobj);
    }

    let list_len = elements.len();
    let pyobject_type = compiler.create_pyobject_type();

    // Allocate memory for: [length: i64][element_0: i64]...[element_n: i64]
    // Total size = (1 + list_len) * sizeof(i64)
    let pyobject_size = pyobject_type.size_of();
    let element_count = compiler
        .context
        .i64_type()
        .const_int((list_len + 1) as u64, false); // +1 for length header
    let total_size = compiler
        .builder
        .build_int_mul(pyobject_size, element_count, "list_size")
        .unwrap();

    // Allocate the list
    let malloc_fn = compiler.runtime.add_malloc(&compiler.module);
    let list_ptr_result = compiler
        .builder
        .build_call(malloc_fn, &[total_size.into()], "malloc_list")
        .unwrap();
    let list_ptr = match list_ptr_result.try_as_basic_value() {
        inkwell::values::ValueKind::Basic(value) => value.into_pointer_value(),
        _ => {
            return Err(CodeGenError::UndefinedVariable(
                "malloc did not return a value".to_string(),
            ))
        }
    };

    // Store the length at offset 0
    let len_value = compiler
        .context
        .i64_type()
        .const_int(list_len as u64, false);
    let len_ptr = unsafe {
        compiler
            .builder
            .build_in_bounds_gep(
                pyobject_type,
                list_ptr,
                &[compiler.context.i64_type().const_int(0, false)],
                "len_ptr",
            )
            .unwrap()
    };
    compiler.builder.build_store(len_ptr, len_value).unwrap();

    // Store each element in the array (starting at offset 1)
    for (i, elem_pyobj) in compiled_elements.iter().enumerate() {
        let index = compiler.context.i64_type().const_int((i + 1) as u64, false); // +1 to skip length header
        let elem_ptr = unsafe {
            compiler
                .builder
                .build_in_bounds_gep(
                    pyobject_type,
                    list_ptr,
                    &[index],
                    &format!("elem_ptr_{}", i),
                )
                .unwrap()
        };
        compiler.builder.build_store(elem_ptr, *elem_pyobj).unwrap();
    }

    // Create a PyObject with LIST tag and the pointer as payload
    Ok(compiler.create_pyobject_list(list_ptr, list_len))
}

/// Compiles a list indexing expression list[index]
pub fn compile_index<'ctx>(
    compiler: &mut Compiler<'ctx>,
    list: &IRExpr,
    index: &IRExpr,
) -> Result<IntValue<'ctx>, CodeGenError> {
    let list_obj = compiler.compile_expression(list)?;
    let index_obj = compiler.compile_expression(index)?;

    // Extract the list pointer and length from the PyObject
    let (list_ptr, _list_len) = compiler.extract_list_ptr_and_len(list_obj);

    // Extract the index value
    let index_payload = compiler.extract_payload(index_obj);
    let index_int = compiler
        .builder
        .build_float_to_signed_int(index_payload, compiler.context.i64_type(), "index_int")
        .unwrap();

    // Add 1 to the index to skip the length header
    // List layout: [length: i64][element_0: i64]...[element_n: i64]
    let adjusted_index = compiler
        .builder
        .build_int_add(
            index_int,
            compiler.context.i64_type().const_int(1, false),
            "adjusted_index",
        )
        .unwrap();

    // Get the element at the adjusted index
    let pyobject_type = compiler.create_pyobject_type();
    let elem_ptr = unsafe {
        compiler
            .builder
            .build_in_bounds_gep(pyobject_type, list_ptr, &[adjusted_index], "elem_ptr")
            .unwrap()
    };

    // Load and return the element
    let elem = compiler
        .builder
        .build_load(pyobject_type, elem_ptr, "elem")
        .unwrap()
        .into_int_value();

    Ok(elem)
}

/// Compiles a len() expression for strings and lists
pub fn compile_len<'ctx>(
    compiler: &mut Compiler<'ctx>,
    arg: &IRExpr,
) -> Result<IntValue<'ctx>, CodeGenError> {
    let arg_obj = compiler.compile_expression(arg)?;
    let arg_tag = compiler.extract_tag(arg_obj);

    // Check if the argument is a string or list
    let string_tag_const = compiler
        .context
        .i64_type()
        .const_int(TYPE_TAG_STRING as u64, false);
    let list_tag_const = compiler
        .context
        .i64_type()
        .const_int(TYPE_TAG_LIST as u64, false);

    let is_string = compiler
        .builder
        .build_int_compare(
            inkwell::IntPredicate::EQ,
            arg_tag,
            string_tag_const,
            "is_string",
        )
        .unwrap();
    let is_list = compiler
        .builder
        .build_int_compare(
            inkwell::IntPredicate::EQ,
            arg_tag,
            list_tag_const,
            "is_list",
        )
        .unwrap();

    // Get current function for creating basic blocks
    let current_fn = compiler
        .builder
        .get_insert_block()
        .unwrap()
        .get_parent()
        .unwrap();

    let string_len_block = compiler
        .context
        .append_basic_block(current_fn, "string_len");
    let list_len_block = compiler.context.append_basic_block(current_fn, "list_len");
    let other_len_block = compiler.context.append_basic_block(current_fn, "other_len");
    let merge_block = compiler.context.append_basic_block(current_fn, "len_merge");

    // Branch: is_string ? string_len : check_list
    let check_list_block = compiler
        .context
        .append_basic_block(current_fn, "check_list");
    compiler
        .builder
        .build_conditional_branch(is_string, string_len_block, check_list_block)
        .unwrap();

    // Check if it's a list
    compiler.builder.position_at_end(check_list_block);
    compiler
        .builder
        .build_conditional_branch(is_list, list_len_block, other_len_block)
        .unwrap();

    // String length block
    compiler.builder.position_at_end(string_len_block);
    let str_ptr = compiler.extract_string_ptr(arg_obj);
    let strlen_fn = compiler.runtime.add_strlen(&compiler.module);
    let len_result = compiler
        .builder
        .build_call(strlen_fn, &[str_ptr.into()], "strlen")
        .unwrap();
    let len_int = match len_result.try_as_basic_value() {
        inkwell::values::ValueKind::Basic(value) => value.into_int_value(),
        _ => {
            return Err(CodeGenError::UndefinedVariable(
                "strlen did not return a value".to_string(),
            ))
        }
    };
    let string_len_result = compiler.create_pyobject_int(len_int);
    compiler
        .builder
        .build_unconditional_branch(merge_block)
        .unwrap();

    // List length block
    compiler.builder.position_at_end(list_len_block);
    let (_list_ptr, list_len) = compiler.extract_list_ptr_and_len(arg_obj);
    let list_len_result = compiler.create_pyobject_int(list_len);
    compiler
        .builder
        .build_unconditional_branch(merge_block)
        .unwrap();

    // Other types - return 0 for now
    compiler.builder.position_at_end(other_len_block);
    let zero_int = compiler.context.i64_type().const_int(0, false);
    let other_len_result = compiler.create_pyobject_int(zero_int);
    compiler
        .builder
        .build_unconditional_branch(merge_block)
        .unwrap();

    // Merge block
    compiler.builder.position_at_end(merge_block);
    let pyobject_type = compiler.create_pyobject_type();
    let phi = compiler
        .builder
        .build_phi(pyobject_type, "len_result")
        .unwrap();
    phi.add_incoming(&[
        (&string_len_result, string_len_block),
        (&list_len_result, list_len_block),
        (&other_len_result, other_len_block),
    ]);
    Ok(phi.as_basic_value().into_int_value())
}

// ============================================================================
// Input/Output Operations
// ============================================================================

/// Compiles an input() expression for reading user input
pub fn compile_input<'ctx>(compiler: &mut Compiler<'ctx>) -> Result<IntValue<'ctx>, CodeGenError> {
    let scanf = compiler.runtime.add_scanf(&compiler.module);
    let format_string = compiler
        .format_strings
        .get_scanf_float_format_string(&compiler.builder);

    // Allocate space for the input value
    let input_alloca = compiler
        .builder
        .build_alloca(compiler.context.f64_type(), "input_tmp")
        .unwrap();

    // Call scanf
    compiler
        .builder
        .build_call(
            scanf,
            &[format_string.into(), input_alloca.into()],
            "scanf_call",
        )
        .unwrap();

    // Load the value from the alloca
    let value = compiler
        .builder
        .build_load(compiler.context.f64_type(), input_alloca, "input_value")
        .unwrap()
        .into_float_value();

    // Wrap in PyObject (as float since input() reads floats)
    Ok(compiler.create_pyobject_float(value))
}

// ============================================================================
// Function Call Operations
// ============================================================================

/// Compiles a function call expression func(arg1, arg2, ...)
pub fn compile_call<'ctx>(
    compiler: &mut Compiler<'ctx>,
    func: &str,
    args: &[IRExpr],
) -> Result<IntValue<'ctx>, CodeGenError> {
    // Clone the function value to avoid borrow checker issues
    let function = *compiler
        .functions
        .get(func)
        .ok_or_else(|| CodeGenError::UndefinedVariable(format!("function '{}'", func)))?;

    // Get defaults for this function
    let defaults = compiler
        .function_defaults
        .get(func)
        .cloned()
        .unwrap_or_default();
    let num_provided_args = args.len();

    // Compile provided arguments
    let mut compiled_args = Vec::new();
    for arg in args.iter() {
        let arg_pyobj = compiler.compile_expression(arg)?;
        compiled_args.push(arg_pyobj.into());
    }

    // Add default arguments for missing parameters
    if num_provided_args < defaults.len() {
        for (i, default_opt) in defaults.iter().enumerate().skip(num_provided_args) {
            if let Some(default_expr) = default_opt {
                let default_pyobj = compiler.compile_expression(default_expr)?;
                compiled_args.push(default_pyobj.into());
            } else {
                return Err(CodeGenError::UndefinedVariable(format!(
                    "Missing required argument {} for function '{}'",
                    i, func
                )));
            }
        }
    }

    let call_result = compiler
        .builder
        .build_call(function, &compiled_args, "calltmp")
        .unwrap();

    // Extract the return value from the call (should be a PyObject)
    use inkwell::values::ValueKind;
    match call_result.try_as_basic_value() {
        ValueKind::Basic(value) => Ok(value.into_int_value()),
        ValueKind::Instruction(_) => Err(CodeGenError::UndefinedVariable(
            "Function call did not return a value".to_string(),
        )),
    }
}

// ============================================================================
// Binary Operations
// ============================================================================

/// Compiles a binary operation expression (arithmetic, bitwise, string concatenation)
pub fn compile_binary_op<'ctx>(
    compiler: &mut Compiler<'ctx>,
    op: &BinOp,
    left: &IRExpr,
    right: &IRExpr,
) -> Result<IntValue<'ctx>, CodeGenError> {
    let lhs_obj = compiler.compile_expression(left)?;
    let rhs_obj = compiler.compile_expression(right)?;

    // Extract tags to check types
    let lhs_tag = compiler.extract_tag(lhs_obj);
    let rhs_tag = compiler.extract_tag(rhs_obj);
    let string_tag_const = compiler
        .context
        .i64_type()
        .const_int(TYPE_TAG_STRING as u64, false);

    // Handle string concatenation for Add operator
    if matches!(op, BinOp::Add) {
        let lhs_is_string = compiler
            .builder
            .build_int_compare(
                inkwell::IntPredicate::EQ,
                lhs_tag,
                string_tag_const,
                "lhs_is_string",
            )
            .unwrap();
        let rhs_is_string = compiler
            .builder
            .build_int_compare(
                inkwell::IntPredicate::EQ,
                rhs_tag,
                string_tag_const,
                "rhs_is_string",
            )
            .unwrap();
        let both_strings = compiler
            .builder
            .build_and(lhs_is_string, rhs_is_string, "both_strings")
            .unwrap();

        // Get current function for creating basic blocks
        let current_fn = compiler
            .builder
            .get_insert_block()
            .unwrap()
            .get_parent()
            .unwrap();

        let concat_block = compiler
            .context
            .append_basic_block(current_fn, "str_concat");
        let arithmetic_block = compiler
            .context
            .append_basic_block(current_fn, "arithmetic");
        let merge_block = compiler.context.append_basic_block(current_fn, "add_merge");

        let pyobject_type = compiler.create_pyobject_type();

        // Branch based on whether both are strings
        compiler
            .builder
            .build_conditional_branch(both_strings, concat_block, arithmetic_block)
            .unwrap();

        // String concatenation block
        compiler.builder.position_at_end(concat_block);
        let lhs_str_ptr = compiler.extract_string_ptr(lhs_obj);
        let rhs_str_ptr = compiler.extract_string_ptr(rhs_obj);

        // Get lengths of both strings using strlen
        let strlen_fn = compiler.runtime.add_strlen(&compiler.module);
        let lhs_len_result = compiler
            .builder
            .build_call(strlen_fn, &[lhs_str_ptr.into()], "lhs_len")
            .unwrap();
        let lhs_len = match lhs_len_result.try_as_basic_value() {
            inkwell::values::ValueKind::Basic(value) => value.into_int_value(),
            _ => {
                return Err(CodeGenError::UndefinedVariable(
                    "strlen did not return a value".to_string(),
                ))
            }
        };
        let rhs_len_result = compiler
            .builder
            .build_call(strlen_fn, &[rhs_str_ptr.into()], "rhs_len")
            .unwrap();
        let rhs_len = match rhs_len_result.try_as_basic_value() {
            inkwell::values::ValueKind::Basic(value) => value.into_int_value(),
            _ => {
                return Err(CodeGenError::UndefinedVariable(
                    "strlen did not return a value".to_string(),
                ))
            }
        };

        // Calculate total size (lhs_len + rhs_len + 1 for null terminator)
        let total_len = compiler
            .builder
            .build_int_add(lhs_len, rhs_len, "total_len")
            .unwrap();
        let total_size = compiler
            .builder
            .build_int_add(
                total_len,
                compiler.context.i64_type().const_int(1, false),
                "total_size",
            )
            .unwrap();

        // Allocate memory for concatenated string
        let malloc_fn = compiler.runtime.add_malloc(&compiler.module);
        let concat_ptr_result = compiler
            .builder
            .build_call(malloc_fn, &[total_size.into()], "malloc_concat")
            .unwrap();
        let concat_ptr = match concat_ptr_result.try_as_basic_value() {
            inkwell::values::ValueKind::Basic(value) => value.into_pointer_value(),
            _ => {
                return Err(CodeGenError::UndefinedVariable(
                    "malloc did not return a value".to_string(),
                ))
            }
        };

        // Copy first string
        let memcpy_fn = compiler.runtime.add_memcpy(&compiler.module);
        compiler
            .builder
            .build_call(
                memcpy_fn,
                &[concat_ptr.into(), lhs_str_ptr.into(), lhs_len.into()],
                "memcpy_lhs",
            )
            .unwrap();

        // Copy second string (offset by lhs_len)
        let rhs_dest = unsafe {
            compiler
                .builder
                .build_gep(
                    compiler.context.i8_type(),
                    concat_ptr,
                    &[lhs_len],
                    "rhs_dest",
                )
                .unwrap()
        };
        // Copy rhs_len + 1 to include null terminator
        let rhs_copy_len = compiler
            .builder
            .build_int_add(
                rhs_len,
                compiler.context.i64_type().const_int(1, false),
                "rhs_copy_len",
            )
            .unwrap();
        compiler
            .builder
            .build_call(
                memcpy_fn,
                &[rhs_dest.into(), rhs_str_ptr.into(), rhs_copy_len.into()],
                "memcpy_rhs",
            )
            .unwrap();

        // Track the allocated string in the arena only if in main entry block
        if let Some(main_entry) = compiler.main_entry_block {
            if compiler.builder.get_insert_block() == Some(main_entry) {
                compiler.string_arena.push(concat_ptr);
            }
        }

        // Create PyObject for concatenated string
        let concat_result = compiler.create_pyobject_string(concat_ptr);
        compiler
            .builder
            .build_unconditional_branch(merge_block)
            .unwrap();

        // Arithmetic block (for non-string addition)
        compiler.builder.position_at_end(arithmetic_block);
        let lhs_payload = compiler.extract_payload(lhs_obj);
        let rhs_payload = compiler.extract_payload(rhs_obj);

        // Check if either operand is a float (tag == TYPE_TAG_FLOAT)
        let float_tag_const = compiler
            .context
            .i64_type()
            .const_int(TYPE_TAG_FLOAT as u64, false);
        let lhs_is_float = compiler
            .builder
            .build_int_compare(
                inkwell::IntPredicate::EQ,
                lhs_tag,
                float_tag_const,
                "lhs_is_float",
            )
            .unwrap();
        let rhs_is_float = compiler
            .builder
            .build_int_compare(
                inkwell::IntPredicate::EQ,
                rhs_tag,
                float_tag_const,
                "rhs_is_float",
            )
            .unwrap();

        // If either is float, result should be float
        let result_is_float = compiler
            .builder
            .build_or(lhs_is_float, rhs_is_float, "result_is_float")
            .unwrap();

        let result_payload = compiler
            .builder
            .build_float_add(lhs_payload, rhs_payload, "addtmp")
            .unwrap();

        // Select the result tag based on whether either operand is float
        let int_tag = compiler
            .context
            .i64_type()
            .const_int(TYPE_TAG_INT as u64, false);
        let float_tag = compiler
            .context
            .i64_type()
            .const_int(TYPE_TAG_FLOAT as u64, false);
        let result_tag = compiler
            .builder
            .build_select(result_is_float, float_tag, int_tag, "result_tag")
            .unwrap()
            .into_int_value();

        // Create result PyObject
        let arithmetic_result =
            compiler.create_pyobject_from_tag_and_payload(result_tag, result_payload);
        compiler
            .builder
            .build_unconditional_branch(merge_block)
            .unwrap();

        // Merge block - phi node to select result
        compiler.builder.position_at_end(merge_block);
        let phi = compiler
            .builder
            .build_phi(pyobject_type, "add_result")
            .unwrap();
        phi.add_incoming(&[
            (&concat_result, concat_block),
            (&arithmetic_result, arithmetic_block),
        ]);
        return Ok(phi.as_basic_value().into_int_value());
    }

    // Handle bitwise operations separately (they require integer operands)
    match op {
        BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor | BinOp::LShift | BinOp::RShift => {
            // Convert payloads to integers
            let lhs_payload = compiler.extract_payload(lhs_obj);
            let rhs_payload = compiler.extract_payload(rhs_obj);

            let lhs_int = compiler
                .builder
                .build_float_to_signed_int(lhs_payload, compiler.context.i64_type(), "lhs_to_int")
                .unwrap();
            let rhs_int = compiler
                .builder
                .build_float_to_signed_int(rhs_payload, compiler.context.i64_type(), "rhs_to_int")
                .unwrap();

            // Perform bitwise operation
            let result_int = match op {
                BinOp::BitAnd => compiler.builder.build_and(lhs_int, rhs_int, "and").unwrap(),
                BinOp::BitOr => compiler.builder.build_or(lhs_int, rhs_int, "or").unwrap(),
                BinOp::BitXor => compiler.builder.build_xor(lhs_int, rhs_int, "xor").unwrap(),
                BinOp::LShift => compiler
                    .builder
                    .build_left_shift(lhs_int, rhs_int, "shl")
                    .unwrap(),
                BinOp::RShift => compiler
                    .builder
                    .build_right_shift(lhs_int, rhs_int, true, "shr")
                    .unwrap(),
                _ => unreachable!(),
            };

            // Convert result back to PyObject (always returns integer type)
            Ok(compiler.create_pyobject_int(result_int))
        }
        // Arithmetic operations (Add, Sub, Mul, Div, Mod)
        _ => {
            // Extract tags and payloads
            let lhs_tag = compiler.extract_tag(lhs_obj);
            let rhs_tag = compiler.extract_tag(rhs_obj);
            let lhs_payload = compiler.extract_payload(lhs_obj);
            let rhs_payload = compiler.extract_payload(rhs_obj);

            // Check if either operand is a float (tag == TYPE_TAG_FLOAT)
            let float_tag_const = compiler
                .context
                .i64_type()
                .const_int(TYPE_TAG_FLOAT as u64, false);
            let lhs_is_float = compiler
                .builder
                .build_int_compare(
                    inkwell::IntPredicate::EQ,
                    lhs_tag,
                    float_tag_const,
                    "lhs_is_float",
                )
                .unwrap();
            let rhs_is_float = compiler
                .builder
                .build_int_compare(
                    inkwell::IntPredicate::EQ,
                    rhs_tag,
                    float_tag_const,
                    "rhs_is_float",
                )
                .unwrap();

            // If either is float, result should be float
            let result_is_float = compiler
                .builder
                .build_or(lhs_is_float, rhs_is_float, "result_is_float")
                .unwrap();

            // Perform the operation on payloads
            let result_payload = match op {
                BinOp::Add => compiler
                    .builder
                    .build_float_add(lhs_payload, rhs_payload, "addtmp")
                    .unwrap(),
                BinOp::Sub => compiler
                    .builder
                    .build_float_sub(lhs_payload, rhs_payload, "subtmp")
                    .unwrap(),
                BinOp::Mul => compiler
                    .builder
                    .build_float_mul(lhs_payload, rhs_payload, "multmp")
                    .unwrap(),
                BinOp::Div => compiler
                    .builder
                    .build_float_div(lhs_payload, rhs_payload, "divtmp")
                    .unwrap(),
                BinOp::Mod => compiler
                    .builder
                    .build_float_rem(lhs_payload, rhs_payload, "modtmp")
                    .unwrap(),
                _ => unreachable!(),
            };

            // Select the result tag based on whether either operand is float
            let int_tag = compiler
                .context
                .i64_type()
                .const_int(TYPE_TAG_INT as u64, false);
            let float_tag = compiler
                .context
                .i64_type()
                .const_int(TYPE_TAG_FLOAT as u64, false);
            let result_tag = compiler
                .builder
                .build_select(result_is_float, float_tag, int_tag, "result_tag")
                .unwrap()
                .into_int_value();

            // Create result PyObject
            let result_obj =
                compiler.create_pyobject_from_tag_and_payload(result_tag, result_payload);

            Ok(result_obj)
        }
    }
}
