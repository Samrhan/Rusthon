use inkwell::context::Context;

mod ast;
mod codegen;
mod lowering;
mod parser;

fn main() {
    let source = "x = 10\ny = x + 5\nprint(y)";
    println!("Compiling: {}", source);

    let ast = match parser::parse_program(source) {
        Ok(ast) => ast,
        Err(e) => {
            eprintln!("Parsing failed: {}", e);
            return;
        }
    };

    let ir = match lowering::lower_program(&ast) {
        Ok(ir) => ir,
        Err(e) => {
            eprintln!("Lowering failed: {}", e);
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
            eprintln!("Code generation failed: {}", e);
        }
    }
}
