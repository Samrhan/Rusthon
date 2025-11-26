use inkwell::context::Context;

mod ast;
mod codegen;
mod lowering;
mod parser;
mod error;

fn main() {
    let source = "x = 10\ny = x + 5\nprint(y)";
    let filename = "<input>";
    println!("Compiling: {}", source);

    let ast = match parser::parse_program(source) {
        Ok(ast) => ast,
        Err(e) => {
            error::display_parse_error(source, filename, &e);
            return;
        }
    };

    let ir = match lowering::lower_program(&ast) {
        Ok(ir) => ir,
        Err(e) => {
            error::display_lowering_error(source, filename, &e);
            return;
        }
    };

    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);

    match compiler.compile_program(&ir) {
        Ok(llvm_ir) => {
            println!("Successfully generated LLVM IR:");
            println!("{}", llvm_ir);
        }
        Err(e) => {
            error::display_codegen_error(source, filename, &e);
        }
    }
}
