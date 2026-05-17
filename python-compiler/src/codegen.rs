use crate::ast::{IRExpr, IRStmt};
use crate::compiler::generators::{expression, statement};
use crate::compiler::runtime::{FormatStrings, Runtime};
use crate::compiler::values::{ValueManager, TYPE_TAG_INT, TYPE_TAG_STRING};
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
    pub(crate) context: &'ctx Context,
    pub(crate) builder: Builder<'ctx>,
    pub(crate) module: Module<'ctx>,
    pub(crate) variables: HashMap<String, PointerValue<'ctx>>,
    pub(crate) functions: HashMap<String, FunctionValue<'ctx>>,
    pub(crate) function_defaults: HashMap<String, Vec<Option<IRExpr>>>,
    // Stack of (continue_target, break_target) basic blocks for nested loops
    pub(crate) loop_stack: Vec<(
        inkwell::basic_block::BasicBlock<'ctx>,
        inkwell::basic_block::BasicBlock<'ctx>,
    )>,
    // Arena for string allocations - stores pointers to allocated strings for cleanup
    // Only strings allocated in the main entry block are tracked to avoid dominance issues
    pub(crate) string_arena: Vec<PointerValue<'ctx>>,
    // The entry block of the main function (used to check if strings can be safely tracked)
    pub(crate) main_entry_block: Option<inkwell::basic_block::BasicBlock<'ctx>>,
    // Runtime manager for external C functions
    pub(crate) runtime: Runtime<'ctx>,
    // Format strings manager for printf/scanf
    pub(crate) format_strings: FormatStrings<'ctx>,
    // Value manager for NaN-boxing operations
    pub(crate) values: ValueManager<'ctx>,
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
    pub(crate) fn create_pyobject_type(&self) -> inkwell::types::IntType<'ctx> {
        self.values.pyobject_type()
    }

    /// Creates a PyObject value from an integer using NaN-boxing
    pub(crate) fn create_pyobject_int(&self, value: IntValue<'ctx>) -> IntValue<'ctx> {
        self.values.create_int(&self.builder, value)
    }

    /// Creates a PyObject value from a float using NaN-boxing
    /// Floats are stored as-is in their canonical IEEE 754 representation
    pub(crate) fn create_pyobject_float(&self, value: FloatValue<'ctx>) -> IntValue<'ctx> {
        self.values.create_float(&self.builder, value)
    }

    /// Creates a PyObject value from a boolean using NaN-boxing
    pub(crate) fn create_pyobject_bool(&self, value: IntValue<'ctx>) -> IntValue<'ctx> {
        self.values.create_bool(&self.builder, value)
    }

    /// Creates a PyObject value from a string pointer using NaN-boxing
    pub(crate) fn create_pyobject_string(&self, ptr: PointerValue<'ctx>) -> IntValue<'ctx> {
        self.values.create_string(&self.builder, ptr)
    }

    /// Extracts a string pointer from a PyObject
    /// Assumes the PyObject has a STRING tag
    pub(crate) fn extract_string_ptr(&self, pyobject: IntValue<'ctx>) -> PointerValue<'ctx> {
        self.values.extract_string_ptr(&self.builder, pyobject)
    }

    /// Creates a PyObject value from a list pointer and length using NaN-boxing
    /// The pointer should point to a memory layout: [length: i64][element_0: i64]...[element_n: i64]
    /// The length is stored at offset 0 in the allocation
    pub(crate) fn create_pyobject_list(
        &self,
        ptr: PointerValue<'ctx>,
        _len: usize,
    ) -> IntValue<'ctx> {
        self.values.create_list(&self.builder, ptr, _len)
    }

    /// Extracts a list pointer and length from a PyObject
    /// Assumes the PyObject has a LIST tag
    /// The pointer points to: [length: i64][element_0: i64]...[element_n: i64]
    pub(crate) fn extract_list_ptr_and_len(
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
    pub(crate) fn create_pyobject_from_tag_and_payload(
        &self,
        tag: IntValue<'ctx>,
        payload: FloatValue<'ctx>,
    ) -> IntValue<'ctx> {
        self.values
            .create_from_tag_and_payload(&self.builder, tag, payload)
    }

    /// Checks if a PyObject is a float (not NaN-boxed)
    #[allow(dead_code)]
    pub(crate) fn is_float(&self, pyobject: IntValue<'ctx>) -> IntValue<'ctx> {
        self.values.is_float(&self.builder, pyobject)
    }

    /// Extracts the tag from a NaN-boxed PyObject
    /// Returns tag as i64 for compatibility (0=INT, 1=FLOAT, 2=BOOL, 3=STRING, 4=LIST)
    pub(crate) fn extract_tag(&self, pyobject: IntValue<'ctx>) -> IntValue<'ctx> {
        self.values.extract_tag(&self.builder, pyobject)
    }

    /// Extracts the payload as f64 from a PyObject
    /// For floats: bitcast i64 to f64
    /// For integers/bools: extract and convert to f64
    /// For pointers: extract as integer and convert to f64
    pub(crate) fn extract_payload(&self, pyobject: IntValue<'ctx>) -> FloatValue<'ctx> {
        self.values.extract_payload(&self.builder, pyobject)
    }

    /// Converts a PyObject to a boolean (i1) for conditionals
    /// Returns true if the value is non-zero
    pub(crate) fn pyobject_to_bool(&self, pyobject: IntValue<'ctx>) -> IntValue<'ctx> {
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
            IRStmt::Print(exprs) => statement::compile_print(self, exprs)?,
            IRStmt::Assign { target, value } => {
                statement::compile_assign(self, target, value, current_fn)?
            }
            IRStmt::ExprStmt(expr) => statement::compile_expr_stmt(self, expr)?,
            IRStmt::Return(expr) => statement::compile_return(self, expr)?,
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

    pub(crate) fn compile_expression(
        &mut self,
        expr: &IRExpr,
    ) -> Result<IntValue<'ctx>, CodeGenError> {
        match expr {
            IRExpr::Constant(n) => expression::compile_constant(self, *n),
            IRExpr::Float(f) => expression::compile_float(self, *f),
            IRExpr::Bool(b) => expression::compile_bool(self, *b),
            IRExpr::Variable(name) => expression::compile_variable(self, name),
            IRExpr::BinaryOp { op, left, right } => {
                expression::compile_binary_op(self, op, left, right)
            }
            IRExpr::Call { func, args } => expression::compile_call(self, func, args),
            IRExpr::Input => expression::compile_input(self),
            IRExpr::Len(arg) => expression::compile_len(self, arg),
            IRExpr::Comparison { op, left, right } => {
                expression::compile_comparison(self, op, left, right)
            }
            IRExpr::StringLiteral(s) => expression::compile_string_literal(self, s),
            IRExpr::UnaryOp { op, operand } => expression::compile_unary_op(self, op, operand),
            IRExpr::List(elements) => expression::compile_list(self, elements),
            IRExpr::Index { list, index } => expression::compile_index(self, list, index),
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

    pub(crate) fn create_entry_block_alloca(
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

    pub(crate) fn build_print_value(&mut self, pyobject: IntValue<'ctx>, with_newline: bool) {
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
