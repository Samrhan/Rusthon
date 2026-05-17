//! Runtime and External Functions
//!
//! This module manages declarations for external C library functions used by the compiler.
//! It handles printf, scanf, malloc, free, strlen, and memcpy.
//!
//! ## Purpose
//! - Centralizes external function management
//! - Provides a clean interface for declaring runtime functions
//! - Respects Single Responsibility Principle (SRP)

use inkwell::context::Context;
use inkwell::module::{Linkage, Module};
use inkwell::values::{FunctionValue, PointerValue};

/// Runtime manager for external C functions
pub struct Runtime<'ctx> {
    context: &'ctx Context,
}

impl<'ctx> Runtime<'ctx> {
    /// Creates a new Runtime manager
    pub fn new(context: &'ctx Context) -> Self {
        Self { context }
    }

    /// Declares printf function if not already declared
    /// Signature: int printf(const char* format, ...)
    pub fn add_printf(&self, module: &Module<'ctx>) -> FunctionValue<'ctx> {
        if let Some(function) = module.get_function("printf") {
            return function;
        }
        let i32_type = self.context.i32_type();
        let i8_ptr_type = self.context.ptr_type(inkwell::AddressSpace::default());
        let printf_type = i32_type.fn_type(&[i8_ptr_type.into()], true);
        module.add_function("printf", printf_type, Some(Linkage::External))
    }

    /// Declares scanf function if not already declared
    /// Signature: int scanf(const char* format, ...)
    pub fn add_scanf(&self, module: &Module<'ctx>) -> FunctionValue<'ctx> {
        if let Some(function) = module.get_function("scanf") {
            return function;
        }
        let i32_type = self.context.i32_type();
        let i8_ptr_type = self.context.ptr_type(inkwell::AddressSpace::default());
        let scanf_type = i32_type.fn_type(&[i8_ptr_type.into()], true);
        module.add_function("scanf", scanf_type, Some(Linkage::External))
    }

    /// Declares malloc function if not already declared
    /// Signature: void* malloc(size_t size)
    pub fn add_malloc(&self, module: &Module<'ctx>) -> FunctionValue<'ctx> {
        if let Some(function) = module.get_function("malloc") {
            return function;
        }
        let i8_ptr_type = self.context.ptr_type(inkwell::AddressSpace::default());
        let size_type = self.context.i64_type(); // size_t is typically i64
        let malloc_type = i8_ptr_type.fn_type(&[size_type.into()], false);
        module.add_function("malloc", malloc_type, Some(Linkage::External))
    }

    /// Declares memcpy function if not already declared
    /// Signature: void* memcpy(void* dest, const void* src, size_t n)
    pub fn add_memcpy(&self, module: &Module<'ctx>) -> FunctionValue<'ctx> {
        if let Some(function) = module.get_function("memcpy") {
            return function;
        }
        let i8_ptr_type = self.context.ptr_type(inkwell::AddressSpace::default());
        let size_type = self.context.i64_type();
        let memcpy_type = i8_ptr_type.fn_type(
            &[i8_ptr_type.into(), i8_ptr_type.into(), size_type.into()],
            false,
        );
        module.add_function("memcpy", memcpy_type, Some(Linkage::External))
    }

    /// Declares free function if not already declared
    /// Signature: void free(void* ptr)
    pub fn add_free(&self, module: &Module<'ctx>) -> FunctionValue<'ctx> {
        if let Some(function) = module.get_function("free") {
            return function;
        }
        let void_type = self.context.void_type();
        let i8_ptr_type = self.context.ptr_type(inkwell::AddressSpace::default());
        let free_type = void_type.fn_type(&[i8_ptr_type.into()], false);
        module.add_function("free", free_type, Some(Linkage::External))
    }

    /// Declares strlen function if not already declared
    /// Signature: size_t strlen(const char* s)
    pub fn add_strlen(&self, module: &Module<'ctx>) -> FunctionValue<'ctx> {
        if let Some(function) = module.get_function("strlen") {
            return function;
        }
        let size_type = self.context.i64_type();
        let i8_ptr_type = self.context.ptr_type(inkwell::AddressSpace::default());
        let strlen_type = size_type.fn_type(&[i8_ptr_type.into()], false);
        module.add_function("strlen", strlen_type, Some(Linkage::External))
    }
}

/// Format string manager for printf/scanf operations
/// This struct doesn't store any state, but uses the lifetime parameter
/// to ensure format strings are created with the correct lifetime.
pub struct FormatStrings<'ctx> {
    _phantom: std::marker::PhantomData<&'ctx ()>,
}

impl<'ctx> FormatStrings<'ctx> {
    /// Creates a new FormatStrings manager
    pub fn new(_context: &'ctx Context) -> Self {
        Self {
            _phantom: std::marker::PhantomData,
        }
    }

    /// Returns a pointer to the "%d\n" format string for integers
    pub fn get_int_format_string(
        &self,
        builder: &inkwell::builder::Builder<'ctx>,
    ) -> PointerValue<'ctx> {
        builder
            .build_global_string_ptr("%d\n", "int_format_string")
            .unwrap()
            .as_pointer_value()
    }

    /// Returns a pointer to the "%f\n" format string for floats
    pub fn get_float_format_string(
        &self,
        builder: &inkwell::builder::Builder<'ctx>,
    ) -> PointerValue<'ctx> {
        builder
            .build_global_string_ptr("%f\n", "float_format_string")
            .unwrap()
            .as_pointer_value()
    }

    /// Returns a pointer to the "%lf" format string for scanf float input
    pub fn get_scanf_float_format_string(
        &self,
        builder: &inkwell::builder::Builder<'ctx>,
    ) -> PointerValue<'ctx> {
        builder
            .build_global_string_ptr("%lf", "scanf_float_format_string")
            .unwrap()
            .as_pointer_value()
    }

    /// Returns a pointer to the "%s\n" format string for strings
    pub fn get_string_format_string(
        &self,
        builder: &inkwell::builder::Builder<'ctx>,
    ) -> PointerValue<'ctx> {
        builder
            .build_global_string_ptr("%s\n", "string_format_string")
            .unwrap()
            .as_pointer_value()
    }

    /// Returns a pointer to the "%d" format string for integers (no newline)
    pub fn get_int_format_string_no_newline(
        &self,
        builder: &inkwell::builder::Builder<'ctx>,
    ) -> PointerValue<'ctx> {
        builder
            .build_global_string_ptr("%d", "int_format_no_nl")
            .unwrap()
            .as_pointer_value()
    }

    /// Returns a pointer to the "%f" format string for floats (no newline)
    pub fn get_float_format_string_no_newline(
        &self,
        builder: &inkwell::builder::Builder<'ctx>,
    ) -> PointerValue<'ctx> {
        builder
            .build_global_string_ptr("%f", "float_format_no_nl")
            .unwrap()
            .as_pointer_value()
    }

    /// Returns a pointer to the "%s" format string for strings (no newline)
    pub fn get_string_format_string_no_newline(
        &self,
        builder: &inkwell::builder::Builder<'ctx>,
    ) -> PointerValue<'ctx> {
        builder
            .build_global_string_ptr("%s", "string_format_no_nl")
            .unwrap()
            .as_pointer_value()
    }

    /// Returns a pointer to the " " format string for spaces
    pub fn get_space_format_string(
        &self,
        builder: &inkwell::builder::Builder<'ctx>,
    ) -> PointerValue<'ctx> {
        builder
            .build_global_string_ptr(" ", "space_format")
            .unwrap()
            .as_pointer_value()
    }

    /// Returns a pointer to the "\n" format string for newlines
    pub fn get_newline_format_string(
        &self,
        builder: &inkwell::builder::Builder<'ctx>,
    ) -> PointerValue<'ctx> {
        builder
            .build_global_string_ptr("\n", "newline_format")
            .unwrap()
            .as_pointer_value()
    }
}
