use inkwell::context::Context;

mod ast;
mod codegen;
mod compiler;
mod lowering;
mod parser;

fn main() {
    let source = "print(42)";
    println!("Compiling: {}", source);

    let context = Context::create();
    match codegen::compile_print_42(&context) {
        Ok(ir) => {
            println!("Successfully generated LLVM IR:");
            println!("{}", ir);
        }
        Err(e) => {
            eprintln!("Code generation failed: {}", e);
        }
    }
}
