use crate::ast::{BinOp, IRExpr, IRStmt};
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::{Linkage, Module};
use inkwell::values::{BasicValueEnum, FunctionValue, FloatValue, PointerValue};
use std::collections::HashMap;
use thiserror::Error;

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
        }
    }

    pub fn compile_program(mut self, program: &[IRStmt]) -> Result<String, CodeGenError> {
        // Separate function definitions from top-level statements
        let (functions, top_level): (Vec<_>, Vec<_>) = program.iter().partition(|stmt| {
            matches!(stmt, IRStmt::FunctionDef { .. })
        });

        // Compile all function definitions first
        for func_stmt in functions {
            if let IRStmt::FunctionDef { name, params, body } = func_stmt {
                self.compile_function_def(name, params, body)?;
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
            IRStmt::Print(expr) => {
                let value = self.compile_expression(expr)?;
                self.build_print(value);
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
        }
        Ok(())
    }

    fn compile_expression(&self, expr: &IRExpr) -> Result<BasicValueEnum<'ctx>, CodeGenError> {
        match expr {
            IRExpr::Constant(n) => {
                Ok(self.context.i64_type().const_int(*n as u64, true).into())
            }
            IRExpr::Float(f) => {
                Ok(self.context.f64_type().const_float(*f).into())
            }
            IRExpr::Variable(name) => {
                self.variables
                    .get(name)
                    .map(|ptr| {
                        self.builder
                            .build_load(self.context.f64_type(), *ptr, name)
                            .unwrap()
                    })
                    .ok_or_else(|| CodeGenError::UndefinedVariable(name.clone()))
            }
            IRExpr::BinaryOp { op, left, right } => {
                let lhs = self.compile_expression(left)?;
                let rhs = self.compile_expression(right)?;

                // Promote to float if either operand is float
                let (lhs_float, rhs_float) = self.promote_to_float(lhs, rhs);

                let result = match op {
                    BinOp::Add => self.builder.build_float_add(lhs_float, rhs_float, "addtmp").unwrap(),
                    BinOp::Sub => self.builder.build_float_sub(lhs_float, rhs_float, "subtmp").unwrap(),
                    BinOp::Mul => self.builder.build_float_mul(lhs_float, rhs_float, "multmp").unwrap(),
                    BinOp::Div => self.builder.build_float_div(lhs_float, rhs_float, "divtmp").unwrap(),
                };
                Ok(result.into())
            }
            IRExpr::Call { func, args } => {
                let function = self
                    .functions
                    .get(func)
                    .ok_or_else(|| CodeGenError::UndefinedVariable(format!("function '{}'", func)))?;

                let mut compiled_args = Vec::new();
                for arg in args {
                    let arg_value = self.compile_expression(arg)?;
                    // Convert to f64 for function calls
                    let arg_f64 = if arg_value.is_int_value() {
                        self.builder
                            .build_signed_int_to_float(
                                arg_value.into_int_value(),
                                self.context.f64_type(),
                                "arg_to_float"
                            )
                            .unwrap()
                            .into()
                    } else {
                        arg_value
                    };
                    compiled_args.push(arg_f64.into());
                }

                let call_result = self
                    .builder
                    .build_call(*function, &compiled_args, "calltmp")
                    .unwrap();

                // Extract the return value from the call
                // try_as_basic_value returns ValueKind enum
                use inkwell::values::ValueKind;
                match call_result.try_as_basic_value() {
                    ValueKind::Basic(value) => Ok(value),
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
                    .unwrap();

                Ok(value)
            }
        }
    }

    /// Promote int values to float for mixed arithmetic
    fn promote_to_float(&self, lhs: BasicValueEnum<'ctx>, rhs: BasicValueEnum<'ctx>) -> (FloatValue<'ctx>, FloatValue<'ctx>) {
        let lhs_float = if lhs.is_int_value() {
            self.builder
                .build_signed_int_to_float(lhs.into_int_value(), self.context.f64_type(), "inttofloat")
                .unwrap()
        } else {
            lhs.into_float_value()
        };

        let rhs_float = if rhs.is_int_value() {
            self.builder
                .build_signed_int_to_float(rhs.into_int_value(), self.context.f64_type(), "inttofloat")
                .unwrap()
        } else {
            rhs.into_float_value()
        };

        (lhs_float, rhs_float)
    }

    fn compile_function_def(
        &mut self,
        name: &str,
        params: &[String],
        body: &[IRStmt],
    ) -> Result<(), CodeGenError> {
        let f64_type = self.context.f64_type();

        // Create function signature: all params are f64, return type is f64
        let param_types: Vec<_> = params.iter().map(|_| f64_type.into()).collect();
        let fn_type = f64_type.fn_type(&param_types, false);
        let function = self.module.add_function(name, fn_type, None);

        // Store function in the functions map
        self.functions.insert(name.to_string(), function);

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

        builder
            .build_alloca(self.context.f64_type(), name)
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

    fn build_print(&self, value: BasicValueEnum<'ctx>) {
        let printf = self.add_printf();

        // Determine the correct format string based on the value type
        let (format_string, print_value) = if value.is_int_value() {
            (self.get_int_format_string(), value)
        } else {
            (self.get_float_format_string(), value)
        };

        self.builder
            .build_call(
                printf,
                &[format_string.into(), print_value.into()],
                "printf_call",
            )
            .unwrap();
    }
}
