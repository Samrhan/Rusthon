use crate::ast::{BinOp, CmpOp, IRExpr, IRStmt, UnaryOp};
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::{Linkage, Module};
use inkwell::values::{FunctionValue, FloatValue, IntValue, PointerValue, StructValue};
use inkwell::FloatPredicate;
use inkwell::types::StructType;
use std::collections::HashMap;
use thiserror::Error;

// Type tags for PyObject discrimination
const TYPE_TAG_INT: u8 = 0;
const TYPE_TAG_FLOAT: u8 = 1;
const TYPE_TAG_BOOL: u8 = 2;
const TYPE_TAG_STRING: u8 = 3;
const TYPE_TAG_LIST: u8 = 4;

#[derive(Debug, Error)]
pub enum CodeGenError {
    #[error("LLVM module verification failed: {0}")]
    ModuleVerification(String),
    #[error("Undefined variable: {0}")]
    UndefinedVariable(String),
}

pub struct Compiler<'ctx> {
    context: &'ctx Context,
    builder: Builder<'ctx>,
    module: Module<'ctx>,
    variables: HashMap<String, PointerValue<'ctx>>,
    functions: HashMap<String, FunctionValue<'ctx>>,
    function_defaults: HashMap<String, Vec<Option<IRExpr>>>,
    // Stack of (continue_target, break_target) basic blocks for nested loops
    loop_stack: Vec<(inkwell::basic_block::BasicBlock<'ctx>, inkwell::basic_block::BasicBlock<'ctx>)>,
    // Arena for string allocations - stores pointers to allocated strings for cleanup
    string_arena: Vec<PointerValue<'ctx>>,
}

impl<'ctx> Compiler<'ctx> {
    pub fn new(context: &'ctx Context) -> Self {
        let builder = context.create_builder();
        let module = context.create_module("main");
        Self {
            context,
            builder,
            module,
            variables: HashMap::new(),
            functions: HashMap::new(),
            function_defaults: HashMap::new(),
            loop_stack: Vec::new(),
            string_arena: Vec::new(),
        }
    }

    /// Creates the PyObject struct type: {i8 tag, f64 payload}
    fn create_pyobject_type(&self) -> StructType<'ctx> {
        let i8_type = self.context.i8_type();
        let f64_type = self.context.f64_type();
        self.context.struct_type(&[i8_type.into(), f64_type.into()], false)
    }

    /// Creates a PyObject value from an integer
    fn create_pyobject_int(&self, value: IntValue<'ctx>) -> StructValue<'ctx> {
        let pyobject_type = self.create_pyobject_type();
        let tag = self.context.i8_type().const_int(TYPE_TAG_INT as u64, false);

        // Convert i64 to f64 for storage in the payload
        let value_f64 = self.builder
            .build_signed_int_to_float(value, self.context.f64_type(), "int_to_f64")
            .unwrap();

        // Create the struct
        let mut pyobject = pyobject_type.get_undef();
        pyobject = self.builder
            .build_insert_value(pyobject, tag, 0, "insert_tag")
            .unwrap()
            .into_struct_value();
        pyobject = self.builder
            .build_insert_value(pyobject, value_f64, 1, "insert_payload")
            .unwrap()
            .into_struct_value();

        pyobject
    }

    /// Creates a PyObject value from a float
    fn create_pyobject_float(&self, value: FloatValue<'ctx>) -> StructValue<'ctx> {
        let pyobject_type = self.create_pyobject_type();
        let tag = self.context.i8_type().const_int(TYPE_TAG_FLOAT as u64, false);

        // Create the struct
        let mut pyobject = pyobject_type.get_undef();
        pyobject = self.builder
            .build_insert_value(pyobject, tag, 0, "insert_tag")
            .unwrap()
            .into_struct_value();
        pyobject = self.builder
            .build_insert_value(pyobject, value, 1, "insert_payload")
            .unwrap()
            .into_struct_value();

        pyobject
    }

    /// Creates a PyObject value from a boolean (stored as 0.0 or 1.0)
    fn create_pyobject_bool(&self, value: IntValue<'ctx>) -> StructValue<'ctx> {
        let pyobject_type = self.create_pyobject_type();
        let tag = self.context.i8_type().const_int(TYPE_TAG_BOOL as u64, false);

        // Convert i1 to f64 (0.0 or 1.0)
        let value_f64 = self.builder
            .build_unsigned_int_to_float(value, self.context.f64_type(), "bool_to_f64")
            .unwrap();

        // Create the struct
        let mut pyobject = pyobject_type.get_undef();
        pyobject = self.builder
            .build_insert_value(pyobject, tag, 0, "insert_tag")
            .unwrap()
            .into_struct_value();
        pyobject = self.builder
            .build_insert_value(pyobject, value_f64, 1, "insert_payload")
            .unwrap()
            .into_struct_value();

        pyobject
    }

    /// Creates a PyObject value from a string pointer
    /// The pointer is stored in the payload as a pointer-sized integer cast to f64
    fn create_pyobject_string(&self, ptr: PointerValue<'ctx>) -> StructValue<'ctx> {
        let pyobject_type = self.create_pyobject_type();
        let tag = self.context.i8_type().const_int(TYPE_TAG_STRING as u64, false);

        // Convert pointer to integer, then to f64 for storage
        // Note: This is a bit of a hack - we're storing a pointer as a float
        // A better approach would use a union type, but this works for now
        let ptr_as_int = self.builder
            .build_ptr_to_int(ptr, self.context.i64_type(), "ptr_to_int")
            .unwrap();
        let ptr_as_f64 = self.builder
            .build_unsigned_int_to_float(ptr_as_int, self.context.f64_type(), "ptr_to_f64")
            .unwrap();

        // Create the struct
        let mut pyobject = pyobject_type.get_undef();
        pyobject = self.builder
            .build_insert_value(pyobject, tag, 0, "insert_tag")
            .unwrap()
            .into_struct_value();
        pyobject = self.builder
            .build_insert_value(pyobject, ptr_as_f64, 1, "insert_payload")
            .unwrap()
            .into_struct_value();

        pyobject
    }

    /// Extracts a string pointer from a PyObject
    /// Assumes the PyObject has a STRING tag
    fn extract_string_ptr(&self, pyobject: StructValue<'ctx>) -> PointerValue<'ctx> {
        let payload = self.extract_payload(pyobject);

        // Convert f64 back to integer, then to pointer
        let ptr_as_int = self.builder
            .build_float_to_unsigned_int(payload, self.context.i64_type(), "f64_to_int")
            .unwrap();
        self.builder
            .build_int_to_ptr(
                ptr_as_int,
                self.context.ptr_type(inkwell::AddressSpace::default()),
                "int_to_ptr"
            )
            .unwrap()
    }

    /// Creates a PyObject value from a list pointer and length
    /// The pointer is stored in the payload as a pointer-sized integer cast to f64
    /// The length is encoded in the upper 32 bits of the pointer
    fn create_pyobject_list(&self, ptr: PointerValue<'ctx>, len: usize) -> StructValue<'ctx> {
        let pyobject_type = self.create_pyobject_type();
        let tag = self.context.i8_type().const_int(TYPE_TAG_LIST as u64, false);

        // Encode both pointer and length in the payload
        // We'll use a simple encoding: store the pointer as an int, then encode length separately
        // For simplicity, we'll just store the pointer and track length elsewhere if needed
        // For now, we'll store the length in a separate allocation and store that pointer

        // Allocate space for length (i64)
        let len_alloca = self.builder
            .build_alloca(self.context.i64_type(), "list_len_tmp")
            .unwrap();
        let len_val = self.context.i64_type().const_int(len as u64, false);
        self.builder.build_store(len_alloca, len_val).unwrap();

        // Create a struct to hold both pointer and length
        // We'll pack them into a single i64: lower 48 bits for pointer, upper 16 bits for length
        // This limits lists to 65535 elements and pointers to 48 bits (common on x86-64)
        let ptr_as_int = self.builder
            .build_ptr_to_int(ptr, self.context.i64_type(), "ptr_to_int")
            .unwrap();

        let len_i64 = self.context.i64_type().const_int(len as u64, false);
        let len_shifted = self.builder
            .build_left_shift(len_i64, self.context.i64_type().const_int(48, false), "len_shift")
            .unwrap();

        let packed = self.builder
            .build_or(ptr_as_int, len_shifted, "packed")
            .unwrap();

        let packed_as_f64 = self.builder
            .build_unsigned_int_to_float(packed, self.context.f64_type(), "packed_to_f64")
            .unwrap();

        // Create the struct
        let mut pyobject = pyobject_type.get_undef();
        pyobject = self.builder
            .build_insert_value(pyobject, tag, 0, "insert_tag")
            .unwrap()
            .into_struct_value();
        pyobject = self.builder
            .build_insert_value(pyobject, packed_as_f64, 1, "insert_payload")
            .unwrap()
            .into_struct_value();

        pyobject
    }

    /// Extracts a list pointer and length from a PyObject
    /// Assumes the PyObject has a LIST tag
    fn extract_list_ptr_and_len(&self, pyobject: StructValue<'ctx>) -> (PointerValue<'ctx>, IntValue<'ctx>) {
        let payload = self.extract_payload(pyobject);

        // Convert f64 back to integer
        let packed = self.builder
            .build_float_to_unsigned_int(payload, self.context.i64_type(), "f64_to_int")
            .unwrap();

        // Extract pointer (lower 48 bits)
        let ptr_mask = self.context.i64_type().const_int((1u64 << 48) - 1, false);
        let ptr_as_int = self.builder
            .build_and(packed, ptr_mask, "extract_ptr")
            .unwrap();
        let ptr = self.builder
            .build_int_to_ptr(
                ptr_as_int,
                self.context.ptr_type(inkwell::AddressSpace::default()),
                "int_to_ptr"
            )
            .unwrap();

        // Extract length (upper 16 bits)
        let len_shifted = self.builder
            .build_right_shift(packed, self.context.i64_type().const_int(48, false), false, "len_unshift")
            .unwrap();

        (ptr, len_shifted)
    }

    /// Extracts the tag from a PyObject
    fn extract_tag(&self, pyobject: StructValue<'ctx>) -> IntValue<'ctx> {
        self.builder
            .build_extract_value(pyobject, 0, "extract_tag")
            .unwrap()
            .into_int_value()
    }

    /// Extracts the payload (f64) from a PyObject
    fn extract_payload(&self, pyobject: StructValue<'ctx>) -> FloatValue<'ctx> {
        self.builder
            .build_extract_value(pyobject, 1, "extract_payload")
            .unwrap()
            .into_float_value()
    }

    /// Converts a PyObject to a boolean (i1) for conditionals
    /// Returns true if the value is non-zero
    fn pyobject_to_bool(&self, pyobject: StructValue<'ctx>) -> IntValue<'ctx> {
        let payload = self.extract_payload(pyobject);
        let zero = self.context.f64_type().const_float(0.0);
        self.builder
            .build_float_compare(FloatPredicate::ONE, payload, zero, "to_bool")
            .unwrap()
    }

    pub fn compile_program(mut self, program: &[IRStmt]) -> Result<String, CodeGenError> {
        // Separate function definitions from top-level statements
        let (functions, top_level): (Vec<_>, Vec<_>) = program.iter().partition(|stmt| {
            matches!(stmt, IRStmt::FunctionDef { .. })
        });

        // Compile all function definitions first
        for func_stmt in functions {
            if let IRStmt::FunctionDef { name, params, defaults, body } = func_stmt {
                self.compile_function_def(name, params, defaults, body)?;
            }
        }

        // Create the main function and compile top-level statements
        let i32_type = self.context.i32_type();
        let main_fn_type = i32_type.fn_type(&[], false);
        let main_fn = self.module.add_function("main", main_fn_type, None);
        let entry = self.context.append_basic_block(main_fn, "entry");
        self.builder.position_at_end(entry);

        for stmt in top_level {
            self.compile_statement(stmt, main_fn)?;
        }

        // Cleanup: free all allocated strings from the arena
        let free_fn = self.add_free();
        for str_ptr in &self.string_arena {
            self.builder
                .build_call(free_fn, &[(*str_ptr).into()], "free_str")
                .unwrap();
        }

        self.builder
            .build_return(Some(&i32_type.const_int(0, false)))
            .unwrap();

        if !main_fn.verify(true) {
            return Err(CodeGenError::ModuleVerification(
                "Main function verification failed".to_string(),
            ));
        }

        Ok(self.module.print_to_string().to_string())
    }

    fn compile_statement(
        &mut self,
        stmt: &IRStmt,
        current_fn: FunctionValue<'ctx>,
    ) -> Result<(), CodeGenError> {
        match stmt {
            IRStmt::Print(exprs) => {
                // Handle print with multiple arguments
                if exprs.is_empty() {
                    // print() with no arguments just prints a newline
                    let printf = self.add_printf();
                    self.builder
                        .build_call(printf, &[self.get_newline_format_string().into()], "printf_newline")
                        .unwrap();
                } else {
                    // Print each argument
                    for (i, expr) in exprs.iter().enumerate() {
                        let value = self.compile_expression(expr)?;
                        let is_last = i == exprs.len() - 1;

                        // Print the value (with newline only for the last one)
                        self.build_print_value(value, is_last);

                        // Print a space between arguments (but not after the last one)
                        if !is_last {
                            let printf = self.add_printf();
                            self.builder
                                .build_call(printf, &[self.get_space_format_string().into()], "printf_space")
                                .unwrap();
                        }
                    }
                }
            }
            IRStmt::Assign { target, value } => {
                let value = self.compile_expression(value)?;
                let ptr = self.variables.get(target).copied().unwrap_or_else(|| {
                    let ptr = self.create_entry_block_alloca(target, current_fn);
                    self.variables.insert(target.clone(), ptr);
                    ptr
                });
                self.builder.build_store(ptr, value).unwrap();
            }
            IRStmt::Return(expr) => {
                let value = self.compile_expression(expr)?;
                self.builder.build_return(Some(&value)).unwrap();
            }
            IRStmt::FunctionDef { .. } => {
                // Function definitions are handled separately in compile_program
                // This should not be reached during normal statement compilation
            }
            IRStmt::If { condition, then_body, else_body } => {
                // Compile the condition expression
                let cond_pyobj = self.compile_expression(condition)?;

                // Convert PyObject to boolean for branching
                let cond_bool = self.pyobject_to_bool(cond_pyobj);

                // Create basic blocks for then, else, and merge
                let then_bb = self.context.append_basic_block(current_fn, "then");
                let else_bb = self.context.append_basic_block(current_fn, "else");
                let merge_bb = self.context.append_basic_block(current_fn, "ifcont");

                // Build conditional branch
                self.builder
                    .build_conditional_branch(cond_bool, then_bb, else_bb)
                    .unwrap();

                // Compile then block
                self.builder.position_at_end(then_bb);
                for stmt in then_body {
                    self.compile_statement(stmt, current_fn)?;
                }
                // Only add branch if current block doesn't already have a terminator (e.g., return)
                let current_block = self.builder.get_insert_block().unwrap();
                if current_block.get_terminator().is_none() {
                    self.builder.build_unconditional_branch(merge_bb).unwrap();
                }

                // Compile else block
                self.builder.position_at_end(else_bb);
                for stmt in else_body {
                    self.compile_statement(stmt, current_fn)?;
                }
                // Only add branch if current block doesn't already have a terminator
                let current_block = self.builder.get_insert_block().unwrap();
                if current_block.get_terminator().is_none() {
                    self.builder.build_unconditional_branch(merge_bb).unwrap();
                }

                // Continue building in the merge block
                self.builder.position_at_end(merge_bb);
            }
            IRStmt::While { condition, body } => {
                // Create basic blocks for loop condition, body, and exit
                let loop_cond_bb = self.context.append_basic_block(current_fn, "loop_cond");
                let loop_body_bb = self.context.append_basic_block(current_fn, "loop_body");
                let loop_exit_bb = self.context.append_basic_block(current_fn, "loop_exit");

                // Push loop targets onto the stack for break/continue
                self.loop_stack.push((loop_cond_bb, loop_exit_bb));

                // Jump to the condition check
                self.builder.build_unconditional_branch(loop_cond_bb).unwrap();

                // Build the condition block
                self.builder.position_at_end(loop_cond_bb);
                let cond_pyobj = self.compile_expression(condition)?;

                // Convert PyObject to boolean for branching
                let cond_bool = self.pyobject_to_bool(cond_pyobj);

                // Branch based on condition
                self.builder
                    .build_conditional_branch(cond_bool, loop_body_bb, loop_exit_bb)
                    .unwrap();

                // Build the loop body
                self.builder.position_at_end(loop_body_bb);
                for stmt in body {
                    self.compile_statement(stmt, current_fn)?;
                }
                // Only add branch if current block doesn't already have a terminator
                let current_block = self.builder.get_insert_block().unwrap();
                if current_block.get_terminator().is_none() {
                    self.builder.build_unconditional_branch(loop_cond_bb).unwrap();
                }

                // Pop loop targets from the stack
                self.loop_stack.pop();

                // Continue building after the loop
                self.builder.position_at_end(loop_exit_bb);
            }
            IRStmt::For { var, start, end, body } => {
                // Compile for loop as: var = start; while var < end: body; var += 1

                // Initialize loop variable
                let start_val = self.compile_expression(start)?;
                let ptr = self.variables.get(var).copied().unwrap_or_else(|| {
                    let ptr = self.create_entry_block_alloca(var, current_fn);
                    self.variables.insert(var.clone(), ptr);
                    ptr
                });
                self.builder.build_store(ptr, start_val).unwrap();

                // Create basic blocks for loop condition, body, and exit
                let loop_cond_bb = self.context.append_basic_block(current_fn, "for_cond");
                let loop_body_bb = self.context.append_basic_block(current_fn, "for_body");
                let loop_incr_bb = self.context.append_basic_block(current_fn, "for_incr");
                let loop_exit_bb = self.context.append_basic_block(current_fn, "for_exit");

                // Push loop targets onto the stack (continue goes to increment, break to exit)
                self.loop_stack.push((loop_incr_bb, loop_exit_bb));

                // Jump to the condition check
                self.builder.build_unconditional_branch(loop_cond_bb).unwrap();

                // Build the condition block (var < end)
                self.builder.position_at_end(loop_cond_bb);
                let end_val = self.compile_expression(end)?;
                let pyobject_type = self.create_pyobject_type();
                let var_val = self.builder
                    .build_load(pyobject_type, ptr, var)
                    .unwrap()
                    .into_struct_value();

                // Compare var < end
                let var_payload = self.extract_payload(var_val);
                let end_payload = self.extract_payload(end_val);
                let cond_bool = self.builder
                    .build_float_compare(FloatPredicate::OLT, var_payload, end_payload, "for_cond")
                    .unwrap();

                // Branch based on condition
                self.builder
                    .build_conditional_branch(cond_bool, loop_body_bb, loop_exit_bb)
                    .unwrap();

                // Build the loop body
                self.builder.position_at_end(loop_body_bb);
                for stmt in body {
                    self.compile_statement(stmt, current_fn)?;
                }
                // Only add branch if current block doesn't already have a terminator
                let current_block = self.builder.get_insert_block().unwrap();
                if current_block.get_terminator().is_none() {
                    self.builder.build_unconditional_branch(loop_incr_bb).unwrap();
                }

                // Build the increment block (var += 1)
                self.builder.position_at_end(loop_incr_bb);
                let var_val = self.builder
                    .build_load(pyobject_type, ptr, var)
                    .unwrap()
                    .into_struct_value();
                let var_payload = self.extract_payload(var_val);
                let one = self.context.f64_type().const_float(1.0);
                let new_payload = self.builder
                    .build_float_add(var_payload, one, "for_incr")
                    .unwrap();

                // Preserve the tag from the loop variable
                let tag = self.extract_tag(var_val);
                let mut new_val = pyobject_type.get_undef();
                new_val = self.builder
                    .build_insert_value(new_val, tag, 0, "insert_tag")
                    .unwrap()
                    .into_struct_value();
                new_val = self.builder
                    .build_insert_value(new_val, new_payload, 1, "insert_payload")
                    .unwrap()
                    .into_struct_value();

                self.builder.build_store(ptr, new_val).unwrap();
                self.builder.build_unconditional_branch(loop_cond_bb).unwrap();

                // Pop loop targets from the stack
                self.loop_stack.pop();

                // Continue building after the loop
                self.builder.position_at_end(loop_exit_bb);
            }
            IRStmt::Break => {
                // Branch to the exit block of the current loop
                if let Some((_, break_target)) = self.loop_stack.last() {
                    self.builder.build_unconditional_branch(*break_target).unwrap();
                }
                // Note: Any code after break in the same block is unreachable
            }
            IRStmt::Continue => {
                // Branch to the continue target (loop condition or increment) of the current loop
                if let Some((continue_target, _)) = self.loop_stack.last() {
                    self.builder.build_unconditional_branch(*continue_target).unwrap();
                }
                // Note: Any code after continue in the same block is unreachable
            }
        }
        Ok(())
    }

    fn compile_expression(&mut self, expr: &IRExpr) -> Result<StructValue<'ctx>, CodeGenError> {
        match expr {
            IRExpr::Constant(n) => {
                let int_val = self.context.i64_type().const_int(*n as u64, true);
                Ok(self.create_pyobject_int(int_val))
            }
            IRExpr::Float(f) => {
                let float_val = self.context.f64_type().const_float(*f);
                Ok(self.create_pyobject_float(float_val))
            }
            IRExpr::Bool(b) => {
                let bool_val = self.context.bool_type().const_int(*b as u64, false);
                Ok(self.create_pyobject_bool(bool_val))
            }
            IRExpr::Variable(name) => {
                let ptr = self.variables
                    .get(name)
                    .ok_or_else(|| CodeGenError::UndefinedVariable(name.clone()))?;

                // Variables are stored as PyObject structs
                let pyobject_type = self.create_pyobject_type();
                let loaded = self.builder
                    .build_load(pyobject_type, *ptr, name)
                    .unwrap();

                Ok(loaded.into_struct_value())
            }
            IRExpr::BinaryOp { op, left, right } => {
                let lhs_obj = self.compile_expression(left)?;
                let rhs_obj = self.compile_expression(right)?;

                // Extract tags to check types
                let lhs_tag = self.extract_tag(lhs_obj);
                let rhs_tag = self.extract_tag(rhs_obj);
                let string_tag_const = self.context.i8_type().const_int(TYPE_TAG_STRING as u64, false);

                // Handle string concatenation for Add operator
                if matches!(op, BinOp::Add) {
                    let lhs_is_string = self.builder
                        .build_int_compare(inkwell::IntPredicate::EQ, lhs_tag, string_tag_const, "lhs_is_string")
                        .unwrap();
                    let rhs_is_string = self.builder
                        .build_int_compare(inkwell::IntPredicate::EQ, rhs_tag, string_tag_const, "rhs_is_string")
                        .unwrap();
                    let both_strings = self.builder
                        .build_and(lhs_is_string, rhs_is_string, "both_strings")
                        .unwrap();

                    // Get current function for creating basic blocks
                    let current_fn = self.builder
                        .get_insert_block()
                        .unwrap()
                        .get_parent()
                        .unwrap();

                    let concat_block = self.context.append_basic_block(current_fn, "str_concat");
                    let arithmetic_block = self.context.append_basic_block(current_fn, "arithmetic");
                    let merge_block = self.context.append_basic_block(current_fn, "add_merge");

                    // Branch based on whether both are strings
                    self.builder
                        .build_conditional_branch(both_strings, concat_block, arithmetic_block)
                        .unwrap();

                    // String concatenation block
                    self.builder.position_at_end(concat_block);
                    let lhs_str_ptr = self.extract_string_ptr(lhs_obj);
                    let rhs_str_ptr = self.extract_string_ptr(rhs_obj);

                    // Get lengths of both strings using strlen
                    let strlen_fn = self.add_strlen();
                    let lhs_len_result = self.builder
                        .build_call(strlen_fn, &[lhs_str_ptr.into()], "lhs_len")
                        .unwrap();
                    let lhs_len = match lhs_len_result.try_as_basic_value() {
                        inkwell::values::ValueKind::Basic(value) => value.into_int_value(),
                        _ => {
                            return Err(CodeGenError::UndefinedVariable(
                                "strlen did not return a value".to_string()
                            ))
                        }
                    };
                    let rhs_len_result = self.builder
                        .build_call(strlen_fn, &[rhs_str_ptr.into()], "rhs_len")
                        .unwrap();
                    let rhs_len = match rhs_len_result.try_as_basic_value() {
                        inkwell::values::ValueKind::Basic(value) => value.into_int_value(),
                        _ => {
                            return Err(CodeGenError::UndefinedVariable(
                                "strlen did not return a value".to_string()
                            ))
                        }
                    };

                    // Calculate total size (lhs_len + rhs_len + 1 for null terminator)
                    let total_len = self.builder
                        .build_int_add(lhs_len, rhs_len, "total_len")
                        .unwrap();
                    let total_size = self.builder
                        .build_int_add(total_len, self.context.i64_type().const_int(1, false), "total_size")
                        .unwrap();

                    // Allocate memory for concatenated string
                    let malloc_fn = self.add_malloc();
                    let concat_ptr_result = self.builder
                        .build_call(malloc_fn, &[total_size.into()], "malloc_concat")
                        .unwrap();
                    let concat_ptr = match concat_ptr_result.try_as_basic_value() {
                        inkwell::values::ValueKind::Basic(value) => value.into_pointer_value(),
                        _ => {
                            return Err(CodeGenError::UndefinedVariable(
                                "malloc did not return a value".to_string()
                            ))
                        }
                    };

                    // Copy first string
                    let memcpy_fn = self.add_memcpy();
                    self.builder
                        .build_call(
                            memcpy_fn,
                            &[concat_ptr.into(), lhs_str_ptr.into(), lhs_len.into()],
                            "memcpy_lhs",
                        )
                        .unwrap();

                    // Copy second string (offset by lhs_len)
                    let rhs_dest = unsafe {
                        self.builder
                            .build_gep(
                                self.context.i8_type(),
                                concat_ptr,
                                &[lhs_len],
                                "rhs_dest"
                            )
                            .unwrap()
                    };
                    // Copy rhs_len + 1 to include null terminator
                    let rhs_copy_len = self.builder
                        .build_int_add(rhs_len, self.context.i64_type().const_int(1, false), "rhs_copy_len")
                        .unwrap();
                    self.builder
                        .build_call(
                            memcpy_fn,
                            &[rhs_dest.into(), rhs_str_ptr.into(), rhs_copy_len.into()],
                            "memcpy_rhs",
                        )
                        .unwrap();

                    // Track the allocated string in the arena
                    self.string_arena.push(concat_ptr);

                    // Create PyObject for concatenated string
                    let concat_result = self.create_pyobject_string(concat_ptr);
                    self.builder.build_unconditional_branch(merge_block).unwrap();

                    // Arithmetic block (for non-string addition)
                    self.builder.position_at_end(arithmetic_block);
                    let lhs_payload = self.extract_payload(lhs_obj);
                    let rhs_payload = self.extract_payload(rhs_obj);

                    // Check if either operand is a float (tag == TYPE_TAG_FLOAT)
                    let float_tag_const = self.context.i8_type().const_int(TYPE_TAG_FLOAT as u64, false);
                    let lhs_is_float = self.builder
                        .build_int_compare(inkwell::IntPredicate::EQ, lhs_tag, float_tag_const, "lhs_is_float")
                        .unwrap();
                    let rhs_is_float = self.builder
                        .build_int_compare(inkwell::IntPredicate::EQ, rhs_tag, float_tag_const, "rhs_is_float")
                        .unwrap();

                    // If either is float, result should be float
                    let result_is_float = self.builder
                        .build_or(lhs_is_float, rhs_is_float, "result_is_float")
                        .unwrap();

                    let result_payload = self.builder.build_float_add(lhs_payload, rhs_payload, "addtmp").unwrap();

                    // Select the result tag based on whether either operand is float
                    let int_tag = self.context.i8_type().const_int(TYPE_TAG_INT as u64, false);
                    let float_tag = self.context.i8_type().const_int(TYPE_TAG_FLOAT as u64, false);
                    let result_tag = self.builder
                        .build_select(result_is_float, float_tag, int_tag, "result_tag")
                        .unwrap()
                        .into_int_value();

                    // Create result PyObject
                    let pyobject_type = self.create_pyobject_type();
                    let mut arithmetic_result = pyobject_type.get_undef();
                    arithmetic_result = self.builder
                        .build_insert_value(arithmetic_result, result_tag, 0, "insert_tag")
                        .unwrap()
                        .into_struct_value();
                    arithmetic_result = self.builder
                        .build_insert_value(arithmetic_result, result_payload, 1, "insert_payload")
                        .unwrap()
                        .into_struct_value();
                    self.builder.build_unconditional_branch(merge_block).unwrap();

                    // Merge block - phi node to select result
                    self.builder.position_at_end(merge_block);
                    let phi = self.builder.build_phi(pyobject_type, "add_result").unwrap();
                    phi.add_incoming(&[(&concat_result, concat_block), (&arithmetic_result, arithmetic_block)]);
                    return Ok(phi.as_basic_value().into_struct_value());
                }

                // Handle bitwise operations separately (they require integer operands)
                match op {
                    BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor | BinOp::LShift | BinOp::RShift => {
                        // Convert payloads to integers
                        let lhs_payload = self.extract_payload(lhs_obj);
                        let rhs_payload = self.extract_payload(rhs_obj);

                        let lhs_int = self.builder
                            .build_float_to_signed_int(lhs_payload, self.context.i64_type(), "lhs_to_int")
                            .unwrap();
                        let rhs_int = self.builder
                            .build_float_to_signed_int(rhs_payload, self.context.i64_type(), "rhs_to_int")
                            .unwrap();

                        // Perform bitwise operation
                        let result_int = match op {
                            BinOp::BitAnd => self.builder.build_and(lhs_int, rhs_int, "and").unwrap(),
                            BinOp::BitOr => self.builder.build_or(lhs_int, rhs_int, "or").unwrap(),
                            BinOp::BitXor => self.builder.build_xor(lhs_int, rhs_int, "xor").unwrap(),
                            BinOp::LShift => self.builder.build_left_shift(lhs_int, rhs_int, "shl").unwrap(),
                            BinOp::RShift => self.builder.build_right_shift(lhs_int, rhs_int, true, "shr").unwrap(),
                            _ => unreachable!(),
                        };

                        // Convert result back to PyObject (always returns integer type)
                        Ok(self.create_pyobject_int(result_int))
                    }
                    // Arithmetic operations (Add, Sub, Mul, Div, Mod)
                    _ => {
                        // Extract tags and payloads
                        let lhs_tag = self.extract_tag(lhs_obj);
                        let rhs_tag = self.extract_tag(rhs_obj);
                        let lhs_payload = self.extract_payload(lhs_obj);
                        let rhs_payload = self.extract_payload(rhs_obj);

                        // Check if either operand is a float (tag == TYPE_TAG_FLOAT)
                        let float_tag_const = self.context.i8_type().const_int(TYPE_TAG_FLOAT as u64, false);
                        let lhs_is_float = self.builder
                            .build_int_compare(inkwell::IntPredicate::EQ, lhs_tag, float_tag_const, "lhs_is_float")
                            .unwrap();
                        let rhs_is_float = self.builder
                            .build_int_compare(inkwell::IntPredicate::EQ, rhs_tag, float_tag_const, "rhs_is_float")
                            .unwrap();

                        // If either is float, result should be float
                        let result_is_float = self.builder
                            .build_or(lhs_is_float, rhs_is_float, "result_is_float")
                            .unwrap();

                        // Perform the operation on payloads
                        let result_payload = match op {
                            BinOp::Add => self.builder.build_float_add(lhs_payload, rhs_payload, "addtmp").unwrap(),
                            BinOp::Sub => self.builder.build_float_sub(lhs_payload, rhs_payload, "subtmp").unwrap(),
                            BinOp::Mul => self.builder.build_float_mul(lhs_payload, rhs_payload, "multmp").unwrap(),
                            BinOp::Div => self.builder.build_float_div(lhs_payload, rhs_payload, "divtmp").unwrap(),
                            BinOp::Mod => self.builder.build_float_rem(lhs_payload, rhs_payload, "modtmp").unwrap(),
                            _ => unreachable!(),
                        };

                        // Select the result tag based on whether either operand is float
                        let int_tag = self.context.i8_type().const_int(TYPE_TAG_INT as u64, false);
                        let float_tag = self.context.i8_type().const_int(TYPE_TAG_FLOAT as u64, false);
                        let result_tag = self.builder
                            .build_select(result_is_float, float_tag, int_tag, "result_tag")
                            .unwrap()
                            .into_int_value();

                        // Create result PyObject
                        let pyobject_type = self.create_pyobject_type();
                        let mut result_obj = pyobject_type.get_undef();
                        result_obj = self.builder
                            .build_insert_value(result_obj, result_tag, 0, "insert_tag")
                            .unwrap()
                            .into_struct_value();
                        result_obj = self.builder
                            .build_insert_value(result_obj, result_payload, 1, "insert_payload")
                            .unwrap()
                            .into_struct_value();

                        Ok(result_obj)
                    }
                }
            }
            IRExpr::Call { func, args } => {
                let function = self
                    .functions
                    .get(func)
                    .ok_or_else(|| CodeGenError::UndefinedVariable(format!("function '{}'", func)))?;

                // Get defaults for this function
                let defaults = self.function_defaults.get(func).cloned().unwrap_or_default();
                let num_params = defaults.len();
                let num_provided_args = args.len();

                // Build the full argument list by filling in defaults
                let mut compiled_args = Vec::new();
                for (i, arg) in args.iter().enumerate() {
                    let arg_pyobj = self.compile_expression(arg)?;
                    compiled_args.push(arg_pyobj.into());
                }

                // Add default arguments for missing parameters
                for i in num_provided_args..num_params {
                    if let Some(default_expr) = &defaults[i] {
                        let default_pyobj = self.compile_expression(default_expr)?;
                        compiled_args.push(default_pyobj.into());
                    } else {
                        return Err(CodeGenError::UndefinedVariable(
                            format!("Missing required argument {} for function '{}'", i, func)
                        ));
                    }
                }

                let call_result = self
                    .builder
                    .build_call(*function, &compiled_args, "calltmp")
                    .unwrap();

                // Extract the return value from the call (should be a PyObject)
                use inkwell::values::ValueKind;
                match call_result.try_as_basic_value() {
                    ValueKind::Basic(value) => Ok(value.into_struct_value()),
                    ValueKind::Instruction(_) => {
                        Err(CodeGenError::UndefinedVariable(
                            "Function call did not return a value".to_string()
                        ))
                    }
                }
            }
            IRExpr::Input => {
                let scanf = self.add_scanf();
                let format_string = self.get_scanf_float_format_string();

                // Allocate space for the input value
                let input_alloca = self.builder
                    .build_alloca(self.context.f64_type(), "input_tmp")
                    .unwrap();

                // Call scanf
                self.builder
                    .build_call(
                        scanf,
                        &[format_string.into(), input_alloca.into()],
                        "scanf_call",
                    )
                    .unwrap();

                // Load the value from the alloca
                let value = self.builder
                    .build_load(self.context.f64_type(), input_alloca, "input_value")
                    .unwrap()
                    .into_float_value();

                // Wrap in PyObject (as float since input() reads floats)
                Ok(self.create_pyobject_float(value))
            }
            IRExpr::Len(arg) => {
                let arg_obj = self.compile_expression(arg)?;
                let arg_tag = self.extract_tag(arg_obj);

                // Check if the argument is a string
                let string_tag_const = self.context.i8_type().const_int(TYPE_TAG_STRING as u64, false);
                let is_string = self.builder
                    .build_int_compare(inkwell::IntPredicate::EQ, arg_tag, string_tag_const, "is_string")
                    .unwrap();

                // Get current function for creating basic blocks
                let current_fn = self.builder
                    .get_insert_block()
                    .unwrap()
                    .get_parent()
                    .unwrap();

                let string_len_block = self.context.append_basic_block(current_fn, "string_len");
                let other_len_block = self.context.append_basic_block(current_fn, "other_len");
                let merge_block = self.context.append_basic_block(current_fn, "len_merge");

                // Branch based on type
                self.builder
                    .build_conditional_branch(is_string, string_len_block, other_len_block)
                    .unwrap();

                // String length block
                self.builder.position_at_end(string_len_block);
                let str_ptr = self.extract_string_ptr(arg_obj);
                let strlen_fn = self.add_strlen();
                let len_result = self.builder
                    .build_call(strlen_fn, &[str_ptr.into()], "strlen")
                    .unwrap();
                let len_int = match len_result.try_as_basic_value() {
                    inkwell::values::ValueKind::Basic(value) => value.into_int_value(),
                    _ => {
                        return Err(CodeGenError::UndefinedVariable(
                            "strlen did not return a value".to_string()
                        ))
                    }
                };
                let string_len_result = self.create_pyobject_int(len_int);
                self.builder.build_unconditional_branch(merge_block).unwrap();

                // Other types - return 0 for now (could be extended for lists, etc.)
                self.builder.position_at_end(other_len_block);
                let zero_int = self.context.i64_type().const_int(0, false);
                let other_len_result = self.create_pyobject_int(zero_int);
                self.builder.build_unconditional_branch(merge_block).unwrap();

                // Merge block
                self.builder.position_at_end(merge_block);
                let pyobject_type = self.create_pyobject_type();
                let phi = self.builder.build_phi(pyobject_type, "len_result").unwrap();
                phi.add_incoming(&[(&string_len_result, string_len_block), (&other_len_result, other_len_block)]);
                Ok(phi.as_basic_value().into_struct_value())
            }
            IRExpr::Comparison { op, left, right } => {
                let lhs_obj = self.compile_expression(left)?;
                let rhs_obj = self.compile_expression(right)?;

                // Extract payloads (values are already stored as f64)
                let lhs_payload = self.extract_payload(lhs_obj);
                let rhs_payload = self.extract_payload(rhs_obj);

                // Perform the comparison
                let predicate = match op {
                    CmpOp::Eq => FloatPredicate::OEQ,   // Ordered and equal
                    CmpOp::NotEq => FloatPredicate::ONE, // Ordered and not equal
                    CmpOp::Lt => FloatPredicate::OLT,   // Ordered and less than
                    CmpOp::Gt => FloatPredicate::OGT,   // Ordered and greater than
                    CmpOp::LtE => FloatPredicate::OLE,  // Ordered and less than or equal
                    CmpOp::GtE => FloatPredicate::OGE,  // Ordered and greater than or equal
                };

                let cmp_result = self.builder
                    .build_float_compare(predicate, lhs_payload, rhs_payload, "cmptmp")
                    .unwrap();

                // Return as PyObject with bool tag
                Ok(self.create_pyobject_bool(cmp_result))
            }
            IRExpr::StringLiteral(s) => {
                // Calculate string length (including null terminator)
                let str_len = s.len() + 1;
                let size = self.context.i64_type().const_int(str_len as u64, false);

                // Call malloc to allocate memory
                let malloc_fn = self.add_malloc();
                let malloc_result = self.builder
                    .build_call(malloc_fn, &[size.into()], "malloc_str")
                    .unwrap();

                // Get the allocated pointer
                use inkwell::values::ValueKind;
                let str_ptr = match malloc_result.try_as_basic_value() {
                    ValueKind::Basic(value) => value.into_pointer_value(),
                    ValueKind::Instruction(_) => {
                        return Err(CodeGenError::UndefinedVariable(
                            "malloc did not return a value".to_string()
                        ))
                    }
                };

                // Create a global string constant for the literal
                let global_str = self.builder
                    .build_global_string_ptr(s, "str_literal")
                    .unwrap();

                // Use memcpy to copy the string to the allocated memory
                let memcpy_fn = self.add_memcpy();
                self.builder
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

                // Track the allocated string in the arena for cleanup
                self.string_arena.push(str_ptr);

                // Wrap the string pointer in a PyObject
                Ok(self.create_pyobject_string(str_ptr))
            }
            IRExpr::UnaryOp { op, operand } => {
                let operand_obj = self.compile_expression(operand)?;

                match op {
                    UnaryOp::Invert => {
                        // Bitwise NOT (~x)
                        let payload = self.extract_payload(operand_obj);
                        let operand_int = self.builder
                            .build_float_to_signed_int(payload, self.context.i64_type(), "to_int")
                            .unwrap();
                        let result = self.builder.build_not(operand_int, "not").unwrap();
                        Ok(self.create_pyobject_int(result))
                    }
                    UnaryOp::USub => {
                        // Unary minus (-x)
                        let payload = self.extract_payload(operand_obj);
                        let zero = self.context.f64_type().const_float(0.0);
                        let result = self.builder.build_float_sub(zero, payload, "neg").unwrap();

                        // Preserve the type tag from the operand
                        let tag = self.extract_tag(operand_obj);
                        let pyobject_type = self.create_pyobject_type();
                        let mut result_obj = pyobject_type.get_undef();
                        result_obj = self.builder
                            .build_insert_value(result_obj, tag, 0, "insert_tag")
                            .unwrap()
                            .into_struct_value();
                        result_obj = self.builder
                            .build_insert_value(result_obj, result, 1, "insert_payload")
                            .unwrap()
                            .into_struct_value();

                        Ok(result_obj)
                    }
                    UnaryOp::UAdd => {
                        // Unary plus (+x) - just return the operand unchanged
                        Ok(operand_obj)
                    }
                    UnaryOp::Not => {
                        // Logical NOT (not x)
                        let payload = self.extract_payload(operand_obj);
                        let zero = self.context.f64_type().const_float(0.0);

                        // Check if operand is zero
                        let is_zero = self.builder
                            .build_float_compare(FloatPredicate::OEQ, payload, zero, "is_zero")
                            .unwrap();

                        // Return True if operand is zero, False otherwise
                        Ok(self.create_pyobject_bool(is_zero))
                    }
                }
            }
            IRExpr::List(elements) => {
                // Compile all element expressions
                let mut compiled_elements = Vec::new();
                for elem in elements {
                    let elem_pyobj = self.compile_expression(elem)?;
                    compiled_elements.push(elem_pyobj);
                }

                let list_len = elements.len();
                let pyobject_type = self.create_pyobject_type();

                // Allocate memory for the array of PyObjects
                // size = list_len * sizeof(PyObject)
                let pyobject_size = pyobject_type.size_of().unwrap();
                let element_count = self.context.i64_type().const_int(list_len as u64, false);
                let total_size = self.builder
                    .build_int_mul(
                        pyobject_size,
                        element_count,
                        "list_size"
                    )
                    .unwrap();

                // Allocate the list
                let malloc_fn = self.add_malloc();
                let list_ptr_result = self.builder
                    .build_call(malloc_fn, &[total_size.into()], "malloc_list")
                    .unwrap();
                let list_ptr = match list_ptr_result.try_as_basic_value() {
                    inkwell::values::ValueKind::Basic(value) => value.into_pointer_value(),
                    _ => {
                        return Err(CodeGenError::UndefinedVariable(
                            "malloc did not return a value".to_string()
                        ))
                    }
                };

                // Store each element in the array
                for (i, elem_pyobj) in compiled_elements.iter().enumerate() {
                    let index = self.context.i64_type().const_int(i as u64, false);
                    let elem_ptr = unsafe {
                        self.builder
                            .build_in_bounds_gep(
                                pyobject_type,
                                list_ptr,
                                &[index],
                                &format!("elem_ptr_{}", i)
                            )
                            .unwrap()
                    };
                    self.builder.build_store(elem_ptr, *elem_pyobj).unwrap();
                }

                // Create a PyObject with LIST tag and the pointer as payload
                Ok(self.create_pyobject_list(list_ptr, list_len))
            }
            IRExpr::Index { list, index } => {
                let list_obj = self.compile_expression(list)?;
                let index_obj = self.compile_expression(index)?;

                // Extract the list pointer and length from the PyObject
                let (list_ptr, _list_len) = self.extract_list_ptr_and_len(list_obj);

                // Extract the index value
                let index_payload = self.extract_payload(index_obj);
                let index_int = self.builder
                    .build_float_to_signed_int(index_payload, self.context.i64_type(), "index_int")
                    .unwrap();

                // Get the element at the index
                let pyobject_type = self.create_pyobject_type();
                let elem_ptr = unsafe {
                    self.builder
                        .build_in_bounds_gep(
                            pyobject_type,
                            list_ptr,
                            &[index_int],
                            "elem_ptr"
                        )
                        .unwrap()
                };

                // Load and return the element
                let elem = self.builder
                    .build_load(pyobject_type, elem_ptr, "elem")
                    .unwrap()
                    .into_struct_value();

                Ok(elem)
            }
        }
    }

    fn compile_function_def(
        &mut self,
        name: &str,
        params: &[String],
        defaults: &[Option<IRExpr>],
        body: &[IRStmt],
    ) -> Result<(), CodeGenError> {
        let pyobject_type = self.create_pyobject_type();

        // Create function signature: all params are PyObject, return type is PyObject
        let param_types: Vec<_> = params.iter().map(|_| pyobject_type.into()).collect();
        let fn_type = pyobject_type.fn_type(&param_types, false);
        let function = self.module.add_function(name, fn_type, None);

        // Store function and defaults in the maps
        self.functions.insert(name.to_string(), function);
        self.function_defaults.insert(name.to_string(), defaults.to_vec());

        // Create entry block
        let entry = self.context.append_basic_block(function, "entry");
        self.builder.position_at_end(entry);

        // Save current variable scope (for nested functions, though we don't support them yet)
        let saved_variables = self.variables.clone();
        self.variables.clear();

        // Set up parameters as local variables
        for (i, param_name) in params.iter().enumerate() {
            let param_value = function.get_nth_param(i as u32).unwrap();
            let alloca = self.create_entry_block_alloca(param_name, function);
            self.builder.build_store(alloca, param_value).unwrap();
            self.variables.insert(param_name.clone(), alloca);
        }

        // Compile function body
        for stmt in body {
            self.compile_statement(stmt, function)?;
        }

        // Restore variable scope
        self.variables = saved_variables;

        // Verify function
        if !function.verify(true) {
            return Err(CodeGenError::ModuleVerification(format!(
                "Function '{}' verification failed",
                name
            )));
        }

        Ok(())
    }

    fn create_entry_block_alloca(
        &self,
        name: &str,
        function: FunctionValue<'ctx>,
    ) -> PointerValue<'ctx> {
        let builder = self.context.create_builder();
        let entry = function.get_first_basic_block().unwrap();

        match entry.get_first_instruction() {
            Some(first_instr) => builder.position_before(&first_instr),
            None => builder.position_at_end(entry),
        }

        // Allocate space for PyObject struct
        let pyobject_type = self.create_pyobject_type();
        builder
            .build_alloca(pyobject_type, name)
            .unwrap()
    }

    fn add_printf(&self) -> FunctionValue<'ctx> {
        if let Some(function) = self.module.get_function("printf") {
            return function;
        }
        let i32_type = self.context.i32_type();
        let i8_ptr_type = self.context.ptr_type(inkwell::AddressSpace::default());
        let printf_type = i32_type.fn_type(&[i8_ptr_type.into()], true);
        self.module
            .add_function("printf", printf_type, Some(Linkage::External))
    }

    fn add_scanf(&self) -> FunctionValue<'ctx> {
        if let Some(function) = self.module.get_function("scanf") {
            return function;
        }
        let i32_type = self.context.i32_type();
        let i8_ptr_type = self.context.ptr_type(inkwell::AddressSpace::default());
        let scanf_type = i32_type.fn_type(&[i8_ptr_type.into()], true);
        self.module
            .add_function("scanf", scanf_type, Some(Linkage::External))
    }

    fn add_malloc(&self) -> FunctionValue<'ctx> {
        if let Some(function) = self.module.get_function("malloc") {
            return function;
        }
        let i8_ptr_type = self.context.ptr_type(inkwell::AddressSpace::default());
        let size_type = self.context.i64_type(); // size_t is typically i64
        let malloc_type = i8_ptr_type.fn_type(&[size_type.into()], false);
        self.module
            .add_function("malloc", malloc_type, Some(Linkage::External))
    }

    fn add_memcpy(&self) -> FunctionValue<'ctx> {
        if let Some(function) = self.module.get_function("memcpy") {
            return function;
        }
        let i8_ptr_type = self.context.ptr_type(inkwell::AddressSpace::default());
        let size_type = self.context.i64_type();
        // void* memcpy(void* dest, const void* src, size_t n)
        let memcpy_type = i8_ptr_type.fn_type(&[i8_ptr_type.into(), i8_ptr_type.into(), size_type.into()], false);
        self.module
            .add_function("memcpy", memcpy_type, Some(Linkage::External))
    }

    fn add_free(&self) -> FunctionValue<'ctx> {
        if let Some(function) = self.module.get_function("free") {
            return function;
        }
        let void_type = self.context.void_type();
        let i8_ptr_type = self.context.ptr_type(inkwell::AddressSpace::default());
        let free_type = void_type.fn_type(&[i8_ptr_type.into()], false);
        self.module
            .add_function("free", free_type, Some(Linkage::External))
    }

    fn add_strlen(&self) -> FunctionValue<'ctx> {
        if let Some(function) = self.module.get_function("strlen") {
            return function;
        }
        let size_type = self.context.i64_type();
        let i8_ptr_type = self.context.ptr_type(inkwell::AddressSpace::default());
        let strlen_type = size_type.fn_type(&[i8_ptr_type.into()], false);
        self.module
            .add_function("strlen", strlen_type, Some(Linkage::External))
    }

    fn get_int_format_string(&self) -> PointerValue<'ctx> {
        self.builder
            .build_global_string_ptr("%d\n", "int_format_string")
            .unwrap()
            .as_pointer_value()
    }

    fn get_float_format_string(&self) -> PointerValue<'ctx> {
        self.builder
            .build_global_string_ptr("%f\n", "float_format_string")
            .unwrap()
            .as_pointer_value()
    }

    fn get_scanf_float_format_string(&self) -> PointerValue<'ctx> {
        self.builder
            .build_global_string_ptr("%lf", "scanf_float_format_string")
            .unwrap()
            .as_pointer_value()
    }

    fn get_string_format_string(&self) -> PointerValue<'ctx> {
        self.builder
            .build_global_string_ptr("%s\n", "string_format_string")
            .unwrap()
            .as_pointer_value()
    }

    fn get_int_format_string_no_newline(&self) -> PointerValue<'ctx> {
        self.builder
            .build_global_string_ptr("%d", "int_format_no_nl")
            .unwrap()
            .as_pointer_value()
    }

    fn get_float_format_string_no_newline(&self) -> PointerValue<'ctx> {
        self.builder
            .build_global_string_ptr("%f", "float_format_no_nl")
            .unwrap()
            .as_pointer_value()
    }

    fn get_string_format_string_no_newline(&self) -> PointerValue<'ctx> {
        self.builder
            .build_global_string_ptr("%s", "string_format_no_nl")
            .unwrap()
            .as_pointer_value()
    }

    fn get_space_format_string(&self) -> PointerValue<'ctx> {
        self.builder
            .build_global_string_ptr(" ", "space_format")
            .unwrap()
            .as_pointer_value()
    }

    fn get_newline_format_string(&self) -> PointerValue<'ctx> {
        self.builder
            .build_global_string_ptr("\n", "newline_format")
            .unwrap()
            .as_pointer_value()
    }

    fn build_print_value(&mut self, pyobject: StructValue<'ctx>, with_newline: bool) {
        let printf = self.add_printf();

        // Extract tag and payload
        let tag = self.extract_tag(pyobject);
        let payload = self.extract_payload(pyobject);

        // Check the tag to determine print type
        let int_tag = self.context.i8_type().const_int(TYPE_TAG_INT as u64, false);
        let string_tag = self.context.i8_type().const_int(TYPE_TAG_STRING as u64, false);
        let list_tag = self.context.i8_type().const_int(TYPE_TAG_LIST as u64, false);

        let is_int = self.builder
            .build_int_compare(inkwell::IntPredicate::EQ, tag, int_tag, "is_int")
            .unwrap();
        let is_string = self.builder
            .build_int_compare(inkwell::IntPredicate::EQ, tag, string_tag, "is_string")
            .unwrap();
        let is_list = self.builder
            .build_int_compare(inkwell::IntPredicate::EQ, tag, list_tag, "is_list")
            .unwrap();

        // Get current function for creating basic blocks
        let current_fn = self.builder
            .get_insert_block()
            .unwrap()
            .get_parent()
            .unwrap();

        // Create basic blocks for type dispatch
        let check_string_block = self.context.append_basic_block(current_fn, "check_string");
        let check_int_block = self.context.append_basic_block(current_fn, "check_int");
        let int_block = self.context.append_basic_block(current_fn, "print_int");
        let float_block = self.context.append_basic_block(current_fn, "print_float");
        let string_block = self.context.append_basic_block(current_fn, "print_string");
        let list_block = self.context.append_basic_block(current_fn, "print_list");
        let end_block = self.context.append_basic_block(current_fn, "print_end");

        // First, check if it's a list
        self.builder
            .build_conditional_branch(is_list, list_block, check_string_block)
            .unwrap();

        // If not list, check if it's a string
        self.builder.position_at_end(check_string_block);
        self.builder
            .build_conditional_branch(is_string, string_block, check_int_block)
            .unwrap();

        // If not string, check if it's int
        self.builder.position_at_end(check_int_block);
        self.builder
            .build_conditional_branch(is_int, int_block, float_block)
            .unwrap();

        // Print int
        self.builder.position_at_end(int_block);
        let int_val = self.builder
            .build_float_to_signed_int(payload, self.context.i64_type(), "to_int")
            .unwrap();
        let int_format = if with_newline {
            self.get_int_format_string()
        } else {
            self.get_int_format_string_no_newline()
        };
        self.builder
            .build_call(
                printf,
                &[int_format.into(), int_val.into()],
                "printf_int",
            )
            .unwrap();
        self.builder.build_unconditional_branch(end_block).unwrap();

        // Float block
        self.builder.position_at_end(float_block);
        let float_format = if with_newline {
            self.get_float_format_string()
        } else {
            self.get_float_format_string_no_newline()
        };
        self.builder
            .build_call(
                printf,
                &[float_format.into(), payload.into()],
                "printf_float",
            )
            .unwrap();
        self.builder.build_unconditional_branch(end_block).unwrap();

        // String block
        self.builder.position_at_end(string_block);
        let str_ptr = self.extract_string_ptr(pyobject);
        let string_format = if with_newline {
            self.get_string_format_string()
        } else {
            self.get_string_format_string_no_newline()
        };
        self.builder
            .build_call(
                printf,
                &[string_format.into(), str_ptr.into()],
                "printf_string",
            )
            .unwrap();
        self.builder.build_unconditional_branch(end_block).unwrap();

        // List block
        self.builder.position_at_end(list_block);
        let (list_ptr, list_len) = self.extract_list_ptr_and_len(pyobject);

        // Print opening bracket
        let open_bracket = self.builder.build_global_string_ptr("[", "open_bracket").unwrap();
        self.builder
            .build_call(printf, &[open_bracket.as_pointer_value().into()], "print_open")
            .unwrap();

        // Loop through elements and print them
        let pyobject_type = self.create_pyobject_type();
        let zero = self.context.i64_type().const_zero();
        let one = self.context.i64_type().const_int(1, false);

        // Create loop blocks
        let loop_cond = self.context.append_basic_block(current_fn, "list_loop_cond");
        let loop_body = self.context.append_basic_block(current_fn, "list_loop_body");
        let loop_inc = self.context.append_basic_block(current_fn, "list_loop_inc");
        let loop_end = self.context.append_basic_block(current_fn, "list_loop_end");

        // Allocate counter variable
        let counter_ptr = self.builder
            .build_alloca(self.context.i64_type(), "counter")
            .unwrap();
        self.builder.build_store(counter_ptr, zero).unwrap();

        self.builder.build_unconditional_branch(loop_cond).unwrap();

        // Loop condition: counter < list_len
        self.builder.position_at_end(loop_cond);
        let counter = self.builder
            .build_load(self.context.i64_type(), counter_ptr, "counter_val")
            .unwrap()
            .into_int_value();
        let cond = self.builder
            .build_int_compare(inkwell::IntPredicate::ULT, counter, list_len, "list_cond")
            .unwrap();
        self.builder
            .build_conditional_branch(cond, loop_body, loop_end)
            .unwrap();

        // Loop body: print element
        self.builder.position_at_end(loop_body);

        // Load element at counter
        let elem_ptr = unsafe {
            self.builder
                .build_in_bounds_gep(pyobject_type, list_ptr, &[counter], "list_elem_ptr")
                .unwrap()
        };
        let elem = self.builder
            .build_load(pyobject_type, elem_ptr, "list_elem")
            .unwrap()
            .into_struct_value();

        // Print element (no newline)
        self.build_print_value(elem, false);

        // Print comma and space if not last element
        let next_counter = self.builder
            .build_int_add(counter, one, "next_counter")
            .unwrap();
        let is_last = self.builder
            .build_int_compare(inkwell::IntPredicate::EQ, next_counter, list_len, "is_last")
            .unwrap();

        let print_comma_block = self.context.append_basic_block(current_fn, "print_comma");
        self.builder
            .build_conditional_branch(is_last, loop_inc, print_comma_block)
            .unwrap();

        self.builder.position_at_end(print_comma_block);
        let comma = self.builder.build_global_string_ptr(", ", "comma").unwrap();
        self.builder
            .build_call(printf, &[comma.as_pointer_value().into()], "print_comma_call")
            .unwrap();
        self.builder.build_unconditional_branch(loop_inc).unwrap();

        // Loop increment
        self.builder.position_at_end(loop_inc);
        let counter = self.builder
            .build_load(self.context.i64_type(), counter_ptr, "counter_val")
            .unwrap()
            .into_int_value();
        let next_counter = self.builder
            .build_int_add(counter, one, "next_counter")
            .unwrap();
        self.builder.build_store(counter_ptr, next_counter).unwrap();
        self.builder.build_unconditional_branch(loop_cond).unwrap();

        // After loop, print closing bracket
        self.builder.position_at_end(loop_end);
        let close_bracket = if with_newline {
            self.builder.build_global_string_ptr("]\n", "close_bracket").unwrap()
        } else {
            self.builder.build_global_string_ptr("]", "close_bracket").unwrap()
        };
        self.builder
            .build_call(printf, &[close_bracket.as_pointer_value().into()], "print_close")
            .unwrap();
        self.builder.build_unconditional_branch(end_block).unwrap();

        // Continue at end block
        self.builder.position_at_end(end_block);
    }
}
