use crate::ast::{BinOp, IRExpr, IRStmt};
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::{Linkage, Module};
use inkwell::values::{FunctionValue, IntValue, PointerValue};
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
        }
    }

    pub fn compile_program(mut self, program: &[IRStmt]) -> Result<String, CodeGenError> {
        let i32_type = self.context.i32_type();
        let main_fn_type = i32_type.fn_type(&[], false);
        let main_fn = self.module.add_function("main", main_fn_type, None);
        let entry = self.context.append_basic_block(main_fn, "entry");
        self.builder.position_at_end(entry);

        for stmt in program {
            self.compile_statement(stmt)?;
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

    fn compile_statement(&mut self, stmt: &IRStmt) -> Result<(), CodeGenError> {
        match stmt {
            IRStmt::Print(expr) => {
                let value = self.compile_expression(expr)?;
                self.build_print(value);
            }
            IRStmt::Assign { target, value } => {
                let value = self.compile_expression(value)?;
                let ptr = self.variables.get(target).copied().unwrap_or_else(|| {
                    let ptr = self.create_entry_block_alloca(target);
                    self.variables.insert(target.clone(), ptr);
                    ptr
                });
                self.builder.build_store(ptr, value).unwrap();
            }
        }
        Ok(())
    }

    fn compile_expression(&self, expr: &IRExpr) -> Result<IntValue<'ctx>, CodeGenError> {
        match expr {
            IRExpr::Constant(n) => Ok(self.context.i64_type().const_int(*n as u64, true)),
            IRExpr::Variable(name) => self
                .variables
                .get(name)
                .map(|ptr| {
                    self.builder
                        .build_load(self.context.i64_type(), *ptr, name)
                        .unwrap()
                        .into_int_value()
                })
                .ok_or_else(|| CodeGenError::UndefinedVariable(name.clone())),
            IRExpr::BinaryOp { op, left, right } => {
                let lhs = self.compile_expression(left)?;
                let rhs = self.compile_expression(right)?;
                let result = match op {
                    BinOp::Add => self.builder.build_int_add(lhs, rhs, "addtmp").unwrap(),
                    BinOp::Sub => self.builder.build_int_sub(lhs, rhs, "subtmp").unwrap(),
                    BinOp::Mul => self.builder.build_int_mul(lhs, rhs, "multmp").unwrap(),
                    BinOp::Div => self
                        .builder
                        .build_int_signed_div(lhs, rhs, "divtmp")
                        .unwrap(),
                };
                Ok(result)
            }
        }
    }

    fn create_entry_block_alloca(&self, name: &str) -> PointerValue<'ctx> {
        let builder = self.context.create_builder();
        let entry = self
            .builder
            .get_insert_block()
            .unwrap()
            .get_parent()
            .unwrap()
            .get_first_basic_block()
            .unwrap();

        match entry.get_first_instruction() {
            Some(first_instr) => builder.position_before(&first_instr),
            None => builder.position_at_end(entry),
        }

        builder
            .build_alloca(self.context.i64_type(), name)
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

    fn get_format_string(&self) -> PointerValue<'ctx> {
        self.builder
            .build_global_string_ptr("%d\n", "format_string")
            .unwrap()
            .as_pointer_value()
    }

    fn build_print(&self, value: IntValue<'ctx>) {
        let printf = self.add_printf();
        let format_string = self.get_format_string();

        self.builder
            .build_call(
                printf,
                &[format_string.into(), value.into()],
                "printf_call",
            )
            .unwrap();
    }
}
