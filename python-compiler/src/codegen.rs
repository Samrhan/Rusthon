use crate::ast::{BinOp, CmpOp, IRExpr, IRStmt, UnaryOp};
use crate::compiler::runtime::{FormatStrings, Runtime};
use crate::compiler::values::{
    ValueManager, TYPE_TAG_FLOAT, TYPE_TAG_INT, TYPE_TAG_LIST, TYPE_TAG_STRING,
};
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::passes::PassBuilderOptions;
use inkwell::targets::{CodeModel, InitializationConfig, RelocMode, Target, TargetMachine};
use inkwell::values::{FloatValue, FunctionValue, IntValue, PointerValue};
use inkwell::FloatPredicate;
use inkwell::OptimizationLevel;
use std::collections::HashMap;
use std::sync::Once;
use thiserror::Error;

static INIT_TARGETS: Once = Once::new();

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
    loop_stack: Vec<(
        inkwell::basic_block::BasicBlock<'ctx>,
        inkwell::basic_block::BasicBlock<'ctx>,
    )>,
    // Arena for string allocations - stores pointers to allocated strings for cleanup
    // Only strings allocated in the main entry block are tracked to avoid dominance issues
    string_arena: Vec<PointerValue<'ctx>>,
    // The entry block of the main function (used to check if strings can be safely tracked)
    main_entry_block: Option<inkwell::basic_block::BasicBlock<'ctx>>,
    // Runtime manager for external C functions
    runtime: Runtime<'ctx>,
    // Format strings manager for printf/scanf
    format_strings: FormatStrings<'ctx>,
    // Value manager for NaN-boxing operations
    values: ValueManager<'ctx>,
}

impl<'ctx> Compiler<'ctx> {
    pub fn new(context: &'ctx Context) -> Self {
        let builder = context.create_builder();
        let module = context.create_module("main");
        let runtime = Runtime::new(context);
        let format_strings = FormatStrings::new(context);
        let values = ValueManager::new(context);
        Self {
            context,
            builder,
            module,
            variables: HashMap::new(),
            functions: HashMap::new(),
            function_defaults: HashMap::new(),
            loop_stack: Vec::new(),
            string_arena: Vec::new(),
            main_entry_block: None,
            runtime,
            format_strings,
            values,
        }
    }

    /// Returns the PyObject type: i64 (NaN-boxed value)
    /// PyObjects are now single 64-bit values using NaN-boxing for 50% memory reduction
    fn create_pyobject_type(&self) -> inkwell::types::IntType<'ctx> {
        self.values.pyobject_type()
    }

    /// Creates a PyObject value from an integer using NaN-boxing
    fn create_pyobject_int(&self, value: IntValue<'ctx>) -> IntValue<'ctx> {
        self.values.create_int(&self.builder, value)
    }

    /// Creates a PyObject value from a float using NaN-boxing
    /// Floats are stored as-is in their canonical IEEE 754 representation
    fn create_pyobject_float(&self, value: FloatValue<'ctx>) -> IntValue<'ctx> {
        self.values.create_float(&self.builder, value)
    }

    /// Creates a PyObject value from a boolean using NaN-boxing
    fn create_pyobject_bool(&self, value: IntValue<'ctx>) -> IntValue<'ctx> {
        self.values.create_bool(&self.builder, value)
    }

    /// Creates a PyObject value from a string pointer using NaN-boxing
    fn create_pyobject_string(&self, ptr: PointerValue<'ctx>) -> IntValue<'ctx> {
        self.values.create_string(&self.builder, ptr)
    }

    /// Extracts a string pointer from a PyObject
    /// Assumes the PyObject has a STRING tag
    fn extract_string_ptr(&self, pyobject: IntValue<'ctx>) -> PointerValue<'ctx> {
        self.values.extract_string_ptr(&self.builder, pyobject)
    }

    /// Creates a PyObject value from a list pointer and length using NaN-boxing
    /// The pointer should point to a memory layout: [length: i64][element_0: i64]...[element_n: i64]
    /// The length is stored at offset 0 in the allocation
    fn create_pyobject_list(&self, ptr: PointerValue<'ctx>, _len: usize) -> IntValue<'ctx> {
        self.values.create_list(&self.builder, ptr, _len)
    }

    /// Extracts a list pointer and length from a PyObject
    /// Assumes the PyObject has a LIST tag
    /// The pointer points to: [length: i64][element_0: i64]...[element_n: i64]
    fn extract_list_ptr_and_len(
        &self,
        pyobject: IntValue<'ctx>,
    ) -> (PointerValue<'ctx>, IntValue<'ctx>) {
        self.values
            .extract_list_ptr_and_len(&self.builder, pyobject)
    }

    /// Reconstructs a PyObject from a tag and payload
    /// tag: IntValue (i64) representing the type tag (0=INT, 1=FLOAT, 2=BOOL, 3=STRING, 4=LIST)
    /// payload: FloatValue representing the payload as f64
    /// Returns: IntValue (i64) representing the NaN-boxed PyObject
    fn create_pyobject_from_tag_and_payload(
        &self,
        tag: IntValue<'ctx>,
        payload: FloatValue<'ctx>,
    ) -> IntValue<'ctx> {
        self.values
            .create_from_tag_and_payload(&self.builder, tag, payload)
    }

    /// Checks if a PyObject is a float (not NaN-boxed)
    #[allow(dead_code)]
    fn is_float(&self, pyobject: IntValue<'ctx>) -> IntValue<'ctx> {
        self.values.is_float(&self.builder, pyobject)
    }

    /// Extracts the tag from a NaN-boxed PyObject
    /// Returns tag as i64 for compatibility (0=INT, 1=FLOAT, 2=BOOL, 3=STRING, 4=LIST)
    fn extract_tag(&self, pyobject: IntValue<'ctx>) -> IntValue<'ctx> {
        self.values.extract_tag(&self.builder, pyobject)
    }

    /// Extracts the payload as f64 from a PyObject
    /// For floats: bitcast i64 to f64
    /// For integers/bools: extract and convert to f64
    /// For pointers: extract as integer and convert to f64
    fn extract_payload(&self, pyobject: IntValue<'ctx>) -> FloatValue<'ctx> {
        self.values.extract_payload(&self.builder, pyobject)
    }

    /// Converts a PyObject to a boolean (i1) for conditionals
    /// Returns true if the value is non-zero
    fn pyobject_to_bool(&self, pyobject: IntValue<'ctx>) -> IntValue<'ctx> {
        self.values.to_bool(&self.builder, pyobject)
    }

    /// Initializes LLVM targets (only once per program execution)
    fn init_targets() {
        INIT_TARGETS.call_once(|| {
            Target::initialize_all(&InitializationConfig::default());
        });
    }

    /// Runs LLVM optimization passes using the new pass manager (LLVM 18+)
    /// Uses a moderate optimization pipeline (O2) for good performance without excessive compile time
    fn run_optimization_passes(&self) -> Result<(), CodeGenError> {
        // Initialize targets (required for run_passes)
        Self::init_targets();

        // Create target machine
        let triple = TargetMachine::get_default_triple();
        let target = Target::from_triple(&triple).map_err(|e| {
            CodeGenError::ModuleVerification(format!("Failed to get target: {}", e))
        })?;

        let machine = target
            .create_target_machine(
                &triple,
                "generic",
                "",
                OptimizationLevel::Default,
                RelocMode::Default,
                CodeModel::Default,
            )
            .ok_or_else(|| {
                CodeGenError::ModuleVerification("Failed to create target machine".to_string())
            })?;

        // Configure pass builder options
        let pass_options = PassBuilderOptions::create();
        pass_options.set_verify_each(true);
        pass_options.set_loop_vectorization(true);
        pass_options.set_loop_slp_vectorization(true);
        pass_options.set_loop_unrolling(true);
        pass_options.set_merge_functions(true);

        // Run the optimization pipeline
        // "default<O2>" runs the default optimization pipeline at O2 level
        // This includes common optimizations like:
        // - Instruction combining
        // - Dead code elimination
        // - GVN (global value numbering)
        // - Memory to register promotion
        // - Loop optimizations
        // - Inlining
        self.module
            .run_passes("default<O2>", &machine, pass_options)
            .map_err(|e| {
                CodeGenError::ModuleVerification(format!("Optimization passes failed: {}", e))
            })?;

        Ok(())
    }

    pub fn compile_program(mut self, program: &[IRStmt]) -> Result<String, CodeGenError> {
        // Separate function definitions from top-level statements
        let (functions, top_level): (Vec<_>, Vec<_>) = program
            .iter()
            .partition(|stmt| matches!(stmt, IRStmt::FunctionDef { .. }));

        // Two-pass compilation for mutual recursion support:

        // Pass 1: Declare all function signatures
        for func_stmt in &functions {
            if let IRStmt::FunctionDef {
                name,
                params,
                defaults,
                ..
            } = func_stmt
            {
                self.declare_function(name, params, defaults);
            }
        }

        // Pass 2: Compile all function bodies
        for func_stmt in &functions {
            if let IRStmt::FunctionDef {
                name, params, body, ..
            } = func_stmt
            {
                self.compile_function_body(name, params, body)?;
            }
        }

        // Create the main function and compile top-level statements
        let i32_type = self.context.i32_type();
        let main_fn_type = i32_type.fn_type(&[], false);
        let main_fn = self.module.add_function("main", main_fn_type, None);
        let entry = self.context.append_basic_block(main_fn, "entry");
        self.builder.position_at_end(entry);

        // Store the main entry block to track which strings can be safely freed
        self.main_entry_block = Some(entry);

        for stmt in top_level {
            self.compile_statement(stmt, main_fn)?;
        }

        // String cleanup: Free all allocated strings
        // Note: We accept that strings allocated in functions may leak, as we only
        // track strings allocated in main. A full solution would require reference
        // counting or garbage collection, which is beyond the scope of this compiler.
        let free_fn = self.runtime.add_free(&self.module);
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

        // Run optimization passes using the new pass manager (LLVM 18+)
        // This optimizes all functions in the module at once
        self.run_optimization_passes()?;

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
                    let printf = self.runtime.add_printf(&self.module);
                    self.builder
                        .build_call(
                            printf,
                            &[self
                                .format_strings
                                .get_newline_format_string(&self.builder)
                                .into()],
                            "printf_newline",
                        )
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
                            let printf = self.runtime.add_printf(&self.module);
                            self.builder
                                .build_call(
                                    printf,
                                    &[self
                                        .format_strings
                                        .get_space_format_string(&self.builder)
                                        .into()],
                                    "printf_space",
                                )
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
            IRStmt::ExprStmt(expr) => {
                // Evaluate the expression and discard the result
                // This is used for function calls that are executed for their side effects
                self.compile_expression(expr)?;
            }
            IRStmt::Return(expr) => {
                let value = self.compile_expression(expr)?;
                self.builder.build_return(Some(&value)).unwrap();
            }
            IRStmt::FunctionDef { .. } => {
                // Function definitions are handled separately in compile_program
                // This should not be reached during normal statement compilation
            }
            IRStmt::If {
                condition,
                then_body,
                else_body,
            } => {
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
                self.builder
                    .build_unconditional_branch(loop_cond_bb)
                    .unwrap();

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
                    self.builder
                        .build_unconditional_branch(loop_cond_bb)
                        .unwrap();
                }

                // Pop loop targets from the stack
                self.loop_stack.pop();

                // Continue building after the loop
                self.builder.position_at_end(loop_exit_bb);
            }
            IRStmt::For {
                var,
                start,
                end,
                body,
            } => {
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
                self.builder
                    .build_unconditional_branch(loop_cond_bb)
                    .unwrap();

                // Build the condition block (var < end)
                self.builder.position_at_end(loop_cond_bb);
                let end_val = self.compile_expression(end)?;
                let pyobject_type = self.create_pyobject_type();
                let var_val = self
                    .builder
                    .build_load(pyobject_type, ptr, var)
                    .unwrap()
                    .into_int_value();

                // Compare var < end
                let var_payload = self.extract_payload(var_val);
                let end_payload = self.extract_payload(end_val);
                let cond_bool = self
                    .builder
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
                    self.builder
                        .build_unconditional_branch(loop_incr_bb)
                        .unwrap();
                }

                // Build the increment block (var += 1)
                self.builder.position_at_end(loop_incr_bb);
                let var_val = self
                    .builder
                    .build_load(pyobject_type, ptr, var)
                    .unwrap()
                    .into_int_value();
                let var_payload = self.extract_payload(var_val);
                let one = self.context.f64_type().const_float(1.0);
                let new_payload = self
                    .builder
                    .build_float_add(var_payload, one, "for_incr")
                    .unwrap();

                // Preserve the tag from the loop variable
                let tag = self.extract_tag(var_val);
                let new_val = self.create_pyobject_from_tag_and_payload(tag, new_payload);

                self.builder.build_store(ptr, new_val).unwrap();
                self.builder
                    .build_unconditional_branch(loop_cond_bb)
                    .unwrap();

                // Pop loop targets from the stack
                self.loop_stack.pop();

                // Continue building after the loop
                self.builder.position_at_end(loop_exit_bb);
            }
            IRStmt::Break => {
                // Branch to the exit block of the current loop
                if let Some((_, break_target)) = self.loop_stack.last() {
                    self.builder
                        .build_unconditional_branch(*break_target)
                        .unwrap();
                }
                // Note: Any code after break in the same block is unreachable
            }
            IRStmt::Continue => {
                // Branch to the continue target (loop condition or increment) of the current loop
                if let Some((continue_target, _)) = self.loop_stack.last() {
                    self.builder
                        .build_unconditional_branch(*continue_target)
                        .unwrap();
                }
                // Note: Any code after continue in the same block is unreachable
            }
        }
        Ok(())
    }

    fn compile_expression(&mut self, expr: &IRExpr) -> Result<IntValue<'ctx>, CodeGenError> {
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
                let ptr = self
                    .variables
                    .get(name)
                    .ok_or_else(|| CodeGenError::UndefinedVariable(name.clone()))?;

                // Variables are stored as PyObject (i64)
                let pyobject_type = self.create_pyobject_type();
                let loaded = self.builder.build_load(pyobject_type, *ptr, name).unwrap();

                Ok(loaded.into_int_value())
            }
            IRExpr::BinaryOp { op, left, right } => {
                let lhs_obj = self.compile_expression(left)?;
                let rhs_obj = self.compile_expression(right)?;

                // Extract tags to check types
                let lhs_tag = self.extract_tag(lhs_obj);
                let rhs_tag = self.extract_tag(rhs_obj);
                let string_tag_const = self
                    .context
                    .i64_type()
                    .const_int(TYPE_TAG_STRING as u64, false);

                // Handle string concatenation for Add operator
                if matches!(op, BinOp::Add) {
                    let lhs_is_string = self
                        .builder
                        .build_int_compare(
                            inkwell::IntPredicate::EQ,
                            lhs_tag,
                            string_tag_const,
                            "lhs_is_string",
                        )
                        .unwrap();
                    let rhs_is_string = self
                        .builder
                        .build_int_compare(
                            inkwell::IntPredicate::EQ,
                            rhs_tag,
                            string_tag_const,
                            "rhs_is_string",
                        )
                        .unwrap();
                    let both_strings = self
                        .builder
                        .build_and(lhs_is_string, rhs_is_string, "both_strings")
                        .unwrap();

                    // Get current function for creating basic blocks
                    let current_fn = self
                        .builder
                        .get_insert_block()
                        .unwrap()
                        .get_parent()
                        .unwrap();

                    let concat_block = self.context.append_basic_block(current_fn, "str_concat");
                    let arithmetic_block =
                        self.context.append_basic_block(current_fn, "arithmetic");
                    let merge_block = self.context.append_basic_block(current_fn, "add_merge");

                    let pyobject_type = self.create_pyobject_type();

                    // Branch based on whether both are strings
                    self.builder
                        .build_conditional_branch(both_strings, concat_block, arithmetic_block)
                        .unwrap();

                    // String concatenation block
                    self.builder.position_at_end(concat_block);
                    let lhs_str_ptr = self.extract_string_ptr(lhs_obj);
                    let rhs_str_ptr = self.extract_string_ptr(rhs_obj);

                    // Get lengths of both strings using strlen
                    let strlen_fn = self.runtime.add_strlen(&self.module);
                    let lhs_len_result = self
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
                    let rhs_len_result = self
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
                    let total_len = self
                        .builder
                        .build_int_add(lhs_len, rhs_len, "total_len")
                        .unwrap();
                    let total_size = self
                        .builder
                        .build_int_add(
                            total_len,
                            self.context.i64_type().const_int(1, false),
                            "total_size",
                        )
                        .unwrap();

                    // Allocate memory for concatenated string
                    let malloc_fn = self.runtime.add_malloc(&self.module);
                    let concat_ptr_result = self
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
                    let memcpy_fn = self.runtime.add_memcpy(&self.module);
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
                            .build_gep(self.context.i8_type(), concat_ptr, &[lhs_len], "rhs_dest")
                            .unwrap()
                    };
                    // Copy rhs_len + 1 to include null terminator
                    let rhs_copy_len = self
                        .builder
                        .build_int_add(
                            rhs_len,
                            self.context.i64_type().const_int(1, false),
                            "rhs_copy_len",
                        )
                        .unwrap();
                    self.builder
                        .build_call(
                            memcpy_fn,
                            &[rhs_dest.into(), rhs_str_ptr.into(), rhs_copy_len.into()],
                            "memcpy_rhs",
                        )
                        .unwrap();

                    // Track the allocated string in the arena only if in main entry block
                    // This avoids dominance issues with strings allocated in conditional branches
                    if let Some(main_entry) = self.main_entry_block {
                        if self.builder.get_insert_block() == Some(main_entry) {
                            self.string_arena.push(concat_ptr);
                        }
                    }

                    // Create PyObject for concatenated string
                    let concat_result = self.create_pyobject_string(concat_ptr);
                    self.builder
                        .build_unconditional_branch(merge_block)
                        .unwrap();

                    // Arithmetic block (for non-string addition)
                    self.builder.position_at_end(arithmetic_block);
                    let lhs_payload = self.extract_payload(lhs_obj);
                    let rhs_payload = self.extract_payload(rhs_obj);

                    // Check if either operand is a float (tag == TYPE_TAG_FLOAT)
                    let float_tag_const = self
                        .context
                        .i64_type()
                        .const_int(TYPE_TAG_FLOAT as u64, false);
                    let lhs_is_float = self
                        .builder
                        .build_int_compare(
                            inkwell::IntPredicate::EQ,
                            lhs_tag,
                            float_tag_const,
                            "lhs_is_float",
                        )
                        .unwrap();
                    let rhs_is_float = self
                        .builder
                        .build_int_compare(
                            inkwell::IntPredicate::EQ,
                            rhs_tag,
                            float_tag_const,
                            "rhs_is_float",
                        )
                        .unwrap();

                    // If either is float, result should be float
                    let result_is_float = self
                        .builder
                        .build_or(lhs_is_float, rhs_is_float, "result_is_float")
                        .unwrap();

                    let result_payload = self
                        .builder
                        .build_float_add(lhs_payload, rhs_payload, "addtmp")
                        .unwrap();

                    // Select the result tag based on whether either operand is float
                    let int_tag = self
                        .context
                        .i64_type()
                        .const_int(TYPE_TAG_INT as u64, false);
                    let float_tag = self
                        .context
                        .i64_type()
                        .const_int(TYPE_TAG_FLOAT as u64, false);
                    let result_tag = self
                        .builder
                        .build_select(result_is_float, float_tag, int_tag, "result_tag")
                        .unwrap()
                        .into_int_value();

                    // Create result PyObject
                    let arithmetic_result =
                        self.create_pyobject_from_tag_and_payload(result_tag, result_payload);
                    self.builder
                        .build_unconditional_branch(merge_block)
                        .unwrap();

                    // Merge block - phi node to select result
                    self.builder.position_at_end(merge_block);
                    let phi = self.builder.build_phi(pyobject_type, "add_result").unwrap();
                    phi.add_incoming(&[
                        (&concat_result, concat_block),
                        (&arithmetic_result, arithmetic_block),
                    ]);
                    return Ok(phi.as_basic_value().into_int_value());
                }

                // Handle bitwise operations separately (they require integer operands)
                match op {
                    BinOp::BitAnd
                    | BinOp::BitOr
                    | BinOp::BitXor
                    | BinOp::LShift
                    | BinOp::RShift => {
                        // Convert payloads to integers
                        let lhs_payload = self.extract_payload(lhs_obj);
                        let rhs_payload = self.extract_payload(rhs_obj);

                        let lhs_int = self
                            .builder
                            .build_float_to_signed_int(
                                lhs_payload,
                                self.context.i64_type(),
                                "lhs_to_int",
                            )
                            .unwrap();
                        let rhs_int = self
                            .builder
                            .build_float_to_signed_int(
                                rhs_payload,
                                self.context.i64_type(),
                                "rhs_to_int",
                            )
                            .unwrap();

                        // Perform bitwise operation
                        let result_int = match op {
                            BinOp::BitAnd => {
                                self.builder.build_and(lhs_int, rhs_int, "and").unwrap()
                            }
                            BinOp::BitOr => self.builder.build_or(lhs_int, rhs_int, "or").unwrap(),
                            BinOp::BitXor => {
                                self.builder.build_xor(lhs_int, rhs_int, "xor").unwrap()
                            }
                            BinOp::LShift => self
                                .builder
                                .build_left_shift(lhs_int, rhs_int, "shl")
                                .unwrap(),
                            BinOp::RShift => self
                                .builder
                                .build_right_shift(lhs_int, rhs_int, true, "shr")
                                .unwrap(),
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
                        let float_tag_const = self
                            .context
                            .i64_type()
                            .const_int(TYPE_TAG_FLOAT as u64, false);
                        let lhs_is_float = self
                            .builder
                            .build_int_compare(
                                inkwell::IntPredicate::EQ,
                                lhs_tag,
                                float_tag_const,
                                "lhs_is_float",
                            )
                            .unwrap();
                        let rhs_is_float = self
                            .builder
                            .build_int_compare(
                                inkwell::IntPredicate::EQ,
                                rhs_tag,
                                float_tag_const,
                                "rhs_is_float",
                            )
                            .unwrap();

                        // If either is float, result should be float
                        let result_is_float = self
                            .builder
                            .build_or(lhs_is_float, rhs_is_float, "result_is_float")
                            .unwrap();

                        // Perform the operation on payloads
                        let result_payload = match op {
                            BinOp::Add => self
                                .builder
                                .build_float_add(lhs_payload, rhs_payload, "addtmp")
                                .unwrap(),
                            BinOp::Sub => self
                                .builder
                                .build_float_sub(lhs_payload, rhs_payload, "subtmp")
                                .unwrap(),
                            BinOp::Mul => self
                                .builder
                                .build_float_mul(lhs_payload, rhs_payload, "multmp")
                                .unwrap(),
                            BinOp::Div => self
                                .builder
                                .build_float_div(lhs_payload, rhs_payload, "divtmp")
                                .unwrap(),
                            BinOp::Mod => self
                                .builder
                                .build_float_rem(lhs_payload, rhs_payload, "modtmp")
                                .unwrap(),
                            _ => unreachable!(),
                        };

                        // Select the result tag based on whether either operand is float
                        let int_tag = self
                            .context
                            .i64_type()
                            .const_int(TYPE_TAG_INT as u64, false);
                        let float_tag = self
                            .context
                            .i64_type()
                            .const_int(TYPE_TAG_FLOAT as u64, false);
                        let result_tag = self
                            .builder
                            .build_select(result_is_float, float_tag, int_tag, "result_tag")
                            .unwrap()
                            .into_int_value();

                        // Create result PyObject
                        let result_obj =
                            self.create_pyobject_from_tag_and_payload(result_tag, result_payload);

                        Ok(result_obj)
                    }
                }
            }
            IRExpr::Call { func, args } => {
                // Clone the function value to avoid borrow checker issues
                let function = *self.functions.get(func).ok_or_else(|| {
                    CodeGenError::UndefinedVariable(format!("function '{}'", func))
                })?;

                // Get defaults for this function
                let defaults = self
                    .function_defaults
                    .get(func)
                    .cloned()
                    .unwrap_or_default();
                let num_provided_args = args.len();

                // Compile provided arguments
                let mut compiled_args = Vec::new();
                for arg in args.iter() {
                    let arg_pyobj = self.compile_expression(arg)?;
                    compiled_args.push(arg_pyobj.into());
                }

                // Add default arguments for missing parameters
                // Only iterate through defaults that correspond to parameters we didn't provide
                if num_provided_args < defaults.len() {
                    for (i, default_opt) in defaults.iter().enumerate().skip(num_provided_args) {
                        if let Some(default_expr) = default_opt {
                            let default_pyobj = self.compile_expression(default_expr)?;
                            compiled_args.push(default_pyobj.into());
                        } else {
                            return Err(CodeGenError::UndefinedVariable(format!(
                                "Missing required argument {} for function '{}'",
                                i, func
                            )));
                        }
                    }
                }

                let call_result = self
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
            IRExpr::Input => {
                let scanf = self.runtime.add_scanf(&self.module);
                let format_string = self
                    .format_strings
                    .get_scanf_float_format_string(&self.builder);

                // Allocate space for the input value
                let input_alloca = self
                    .builder
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
                let value = self
                    .builder
                    .build_load(self.context.f64_type(), input_alloca, "input_value")
                    .unwrap()
                    .into_float_value();

                // Wrap in PyObject (as float since input() reads floats)
                Ok(self.create_pyobject_float(value))
            }
            IRExpr::Len(arg) => {
                let arg_obj = self.compile_expression(arg)?;
                let arg_tag = self.extract_tag(arg_obj);

                // Check if the argument is a string or list
                let string_tag_const = self
                    .context
                    .i64_type()
                    .const_int(TYPE_TAG_STRING as u64, false);
                let list_tag_const = self
                    .context
                    .i64_type()
                    .const_int(TYPE_TAG_LIST as u64, false);

                let is_string = self
                    .builder
                    .build_int_compare(
                        inkwell::IntPredicate::EQ,
                        arg_tag,
                        string_tag_const,
                        "is_string",
                    )
                    .unwrap();
                let is_list = self
                    .builder
                    .build_int_compare(
                        inkwell::IntPredicate::EQ,
                        arg_tag,
                        list_tag_const,
                        "is_list",
                    )
                    .unwrap();

                // Get current function for creating basic blocks
                let current_fn = self
                    .builder
                    .get_insert_block()
                    .unwrap()
                    .get_parent()
                    .unwrap();

                let string_len_block = self.context.append_basic_block(current_fn, "string_len");
                let list_len_block = self.context.append_basic_block(current_fn, "list_len");
                let other_len_block = self.context.append_basic_block(current_fn, "other_len");
                let merge_block = self.context.append_basic_block(current_fn, "len_merge");

                // Branch: is_string ? string_len : check_list
                let check_list_block = self.context.append_basic_block(current_fn, "check_list");
                self.builder
                    .build_conditional_branch(is_string, string_len_block, check_list_block)
                    .unwrap();

                // Check if it's a list
                self.builder.position_at_end(check_list_block);
                self.builder
                    .build_conditional_branch(is_list, list_len_block, other_len_block)
                    .unwrap();

                // String length block
                self.builder.position_at_end(string_len_block);
                let str_ptr = self.extract_string_ptr(arg_obj);
                let strlen_fn = self.runtime.add_strlen(&self.module);
                let len_result = self
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
                let string_len_result = self.create_pyobject_int(len_int);
                self.builder
                    .build_unconditional_branch(merge_block)
                    .unwrap();

                // List length block
                self.builder.position_at_end(list_len_block);
                let (_list_ptr, list_len) = self.extract_list_ptr_and_len(arg_obj);
                let list_len_result = self.create_pyobject_int(list_len);
                self.builder
                    .build_unconditional_branch(merge_block)
                    .unwrap();

                // Other types - return 0 for now
                self.builder.position_at_end(other_len_block);
                let zero_int = self.context.i64_type().const_int(0, false);
                let other_len_result = self.create_pyobject_int(zero_int);
                self.builder
                    .build_unconditional_branch(merge_block)
                    .unwrap();

                // Merge block
                self.builder.position_at_end(merge_block);
                let pyobject_type = self.create_pyobject_type();
                let phi = self.builder.build_phi(pyobject_type, "len_result").unwrap();
                phi.add_incoming(&[
                    (&string_len_result, string_len_block),
                    (&list_len_result, list_len_block),
                    (&other_len_result, other_len_block),
                ]);
                Ok(phi.as_basic_value().into_int_value())
            }
            IRExpr::Comparison { op, left, right } => {
                let lhs_obj = self.compile_expression(left)?;
                let rhs_obj = self.compile_expression(right)?;

                // Extract payloads (values are already stored as f64)
                let lhs_payload = self.extract_payload(lhs_obj);
                let rhs_payload = self.extract_payload(rhs_obj);

                // Perform the comparison
                let predicate = match op {
                    CmpOp::Eq => FloatPredicate::OEQ,    // Ordered and equal
                    CmpOp::NotEq => FloatPredicate::ONE, // Ordered and not equal
                    CmpOp::Lt => FloatPredicate::OLT,    // Ordered and less than
                    CmpOp::Gt => FloatPredicate::OGT,    // Ordered and greater than
                    CmpOp::LtE => FloatPredicate::OLE,   // Ordered and less than or equal
                    CmpOp::GtE => FloatPredicate::OGE,   // Ordered and greater than or equal
                };

                let cmp_result = self
                    .builder
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
                let malloc_fn = self.runtime.add_malloc(&self.module);
                let malloc_result = self
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
                let global_str = self
                    .builder
                    .build_global_string_ptr(s, "str_literal")
                    .unwrap();

                // Use memcpy to copy the string to the allocated memory
                let memcpy_fn = self.runtime.add_memcpy(&self.module);
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

                // Track the allocated string in the arena for cleanup only if in main entry block
                // This avoids dominance issues with strings allocated in conditional branches
                if let Some(main_entry) = self.main_entry_block {
                    if self.builder.get_insert_block() == Some(main_entry) {
                        self.string_arena.push(str_ptr);
                    }
                }

                // Wrap the string pointer in a PyObject
                Ok(self.create_pyobject_string(str_ptr))
            }
            IRExpr::UnaryOp { op, operand } => {
                let operand_obj = self.compile_expression(operand)?;

                match op {
                    UnaryOp::Invert => {
                        // Bitwise NOT (~x)
                        let payload = self.extract_payload(operand_obj);
                        let operand_int = self
                            .builder
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
                        let result_obj = self.create_pyobject_from_tag_and_payload(tag, result);

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
                        let is_zero = self
                            .builder
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

                // Allocate memory for: [length: i64][element_0: i64]...[element_n: i64]
                // Total size = (1 + list_len) * sizeof(i64)
                let pyobject_size = pyobject_type.size_of();
                let element_count = self
                    .context
                    .i64_type()
                    .const_int((list_len + 1) as u64, false); // +1 for length header
                let total_size = self
                    .builder
                    .build_int_mul(pyobject_size, element_count, "list_size")
                    .unwrap();

                // Allocate the list
                let malloc_fn = self.runtime.add_malloc(&self.module);
                let list_ptr_result = self
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
                let len_value = self.context.i64_type().const_int(list_len as u64, false);
                let len_ptr = unsafe {
                    self.builder
                        .build_in_bounds_gep(
                            pyobject_type,
                            list_ptr,
                            &[self.context.i64_type().const_int(0, false)],
                            "len_ptr",
                        )
                        .unwrap()
                };
                self.builder.build_store(len_ptr, len_value).unwrap();

                // Store each element in the array (starting at offset 1)
                for (i, elem_pyobj) in compiled_elements.iter().enumerate() {
                    let index = self.context.i64_type().const_int((i + 1) as u64, false); // +1 to skip length header
                    let elem_ptr = unsafe {
                        self.builder
                            .build_in_bounds_gep(
                                pyobject_type,
                                list_ptr,
                                &[index],
                                &format!("elem_ptr_{}", i),
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
                let index_int = self
                    .builder
                    .build_float_to_signed_int(index_payload, self.context.i64_type(), "index_int")
                    .unwrap();

                // Add 1 to the index to skip the length header
                // List layout: [length: i64][element_0: i64]...[element_n: i64]
                let adjusted_index = self
                    .builder
                    .build_int_add(
                        index_int,
                        self.context.i64_type().const_int(1, false),
                        "adjusted_index",
                    )
                    .unwrap();

                // Get the element at the adjusted index
                let pyobject_type = self.create_pyobject_type();
                let elem_ptr = unsafe {
                    self.builder
                        .build_in_bounds_gep(pyobject_type, list_ptr, &[adjusted_index], "elem_ptr")
                        .unwrap()
                };

                // Load and return the element
                let elem = self
                    .builder
                    .build_load(pyobject_type, elem_ptr, "elem")
                    .unwrap()
                    .into_int_value();

                Ok(elem)
            }
        }
    }

    /// Declares a function signature without compiling the body.
    /// This is the first pass for supporting mutual recursion.
    fn declare_function(
        &mut self,
        name: &str,
        params: &[String],
        defaults: &[Option<IRExpr>],
    ) -> FunctionValue<'ctx> {
        let pyobject_type = self.create_pyobject_type();

        // Create function signature: all params are PyObject, return type is PyObject
        let param_types: Vec<_> = params.iter().map(|_| pyobject_type.into()).collect();
        let fn_type = pyobject_type.fn_type(&param_types, false);
        let function = self.module.add_function(name, fn_type, None);

        // Store function and defaults in the maps
        self.functions.insert(name.to_string(), function);
        self.function_defaults
            .insert(name.to_string(), defaults.to_vec());

        function
    }

    /// Compiles the body of a previously declared function.
    /// This is the second pass for supporting mutual recursion.
    fn compile_function_body(
        &mut self,
        name: &str,
        params: &[String],
        body: &[IRStmt],
    ) -> Result<(), CodeGenError> {
        let function = *self
            .functions
            .get(name)
            .ok_or_else(|| CodeGenError::UndefinedVariable(format!("function '{}'", name)))?;

        // Create entry block
        let entry = self.context.append_basic_block(function, "entry");
        self.builder.position_at_end(entry);

        // Save current variable scope
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
        builder.build_alloca(pyobject_type, name).unwrap()
    }

    fn build_print_value(&mut self, pyobject: IntValue<'ctx>, with_newline: bool) {
        let printf = self.runtime.add_printf(&self.module);

        // Extract tag and payload
        let tag = self.extract_tag(pyobject);
        let payload = self.extract_payload(pyobject);

        // Check the tag to determine print type
        let int_tag = self
            .context
            .i64_type()
            .const_int(TYPE_TAG_INT as u64, false);
        let string_tag = self
            .context
            .i64_type()
            .const_int(TYPE_TAG_STRING as u64, false);

        let is_int = self
            .builder
            .build_int_compare(inkwell::IntPredicate::EQ, tag, int_tag, "is_int")
            .unwrap();
        let is_string = self
            .builder
            .build_int_compare(inkwell::IntPredicate::EQ, tag, string_tag, "is_string")
            .unwrap();

        // Get current function for creating basic blocks
        let current_fn = self
            .builder
            .get_insert_block()
            .unwrap()
            .get_parent()
            .unwrap();

        // Create basic blocks for type dispatch
        let check_int_block = self.context.append_basic_block(current_fn, "check_int");
        let int_block = self.context.append_basic_block(current_fn, "print_int");
        let float_block = self.context.append_basic_block(current_fn, "print_float");
        let string_block = self.context.append_basic_block(current_fn, "print_string");
        let end_block = self.context.append_basic_block(current_fn, "print_end");

        // First, check if it's a string
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
        let int_val = self
            .builder
            .build_float_to_signed_int(payload, self.context.i64_type(), "to_int")
            .unwrap();
        let int_format = if with_newline {
            self.format_strings.get_int_format_string(&self.builder)
        } else {
            self.format_strings
                .get_int_format_string_no_newline(&self.builder)
        };
        self.builder
            .build_call(printf, &[int_format.into(), int_val.into()], "printf_int")
            .unwrap();
        self.builder.build_unconditional_branch(end_block).unwrap();

        // Float block
        self.builder.position_at_end(float_block);
        let float_format = if with_newline {
            self.format_strings.get_float_format_string(&self.builder)
        } else {
            self.format_strings
                .get_float_format_string_no_newline(&self.builder)
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
            self.format_strings.get_string_format_string(&self.builder)
        } else {
            self.format_strings
                .get_string_format_string_no_newline(&self.builder)
        };
        self.builder
            .build_call(
                printf,
                &[string_format.into(), str_ptr.into()],
                "printf_string",
            )
            .unwrap();
        self.builder.build_unconditional_branch(end_block).unwrap();

        // Continue at end block
        self.builder.position_at_end(end_block);
    }
}
