use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::{Linkage, Module};
use inkwell::values::{FunctionValue, PointerValue};

/// Adds a declaration for the `printf` function to the module.
fn add_printf<'ctx>(context: &'ctx Context, module: &Module<'ctx>) -> FunctionValue<'ctx> {
    let i32_type = context.i32_type();
    let i8_ptr_type = context.ptr_type(inkwell::AddressSpace::default());
    let printf_type = i32_type.fn_type(&[i8_ptr_type.into()], true);
    module.add_function("printf", printf_type, Some(Linkage::External))
}

/// Creates a global string pointer for the given string.
fn create_global_string<'ctx>(
    builder: &Builder<'ctx>,
    string: &str,
    name: &str,
) -> PointerValue<'ctx> {
    builder.build_global_string_ptr(string, name).unwrap().as_pointer_value()
}

/// Compiles a single `print(42)` statement.
pub fn compile_print_42(context: &Context) -> Result<String, String> {
    let module = context.create_module("main");
    let builder = context.create_builder();

    let printf = add_printf(context, &module);

    let i32_type = context.i32_type();
    let main_fn_type = i32_type.fn_type(&[], false);
    let main_fn = module.add_function("main", main_fn_type, None);
    let entry = context.append_basic_block(main_fn, "entry");
    builder.position_at_end(entry);

    let format_string = create_global_string(&builder, "%d\n", "format_string");
    let value = i32_type.const_int(42, false);

    builder
        .build_call(
            printf,
            &[format_string.into(), value.into()],
            "printf_call",
        )
        .unwrap();

    builder
        .build_return(Some(&i32_type.const_int(0, false)))
        .unwrap();

    Ok(module.print_to_string().to_string())
}
