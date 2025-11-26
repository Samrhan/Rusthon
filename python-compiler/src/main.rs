use inkwell::context::Context;
use std::env;
use std::fs;
use std::path::Path;
use std::process::{self, Command};

mod ast;
mod codegen;
mod lowering;
mod parser;
mod error;

fn main() {
    let args: Vec<String> = env::args().collect();
    
    if args.len() != 2 {
        eprintln!("Usage: {} <python_file.py>", args[0]);
        eprintln!("Example: {} example.py", args[0]);
        process::exit(1);
    }

    let filename = &args[1];
    
    let source = match fs::read_to_string(filename) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("Error reading file '{}': {}", filename, e);
            process::exit(1);
        }
    };

    println!("Compiling: {}", filename);

    let ast = match parser::parse_program(&source) {
        Ok(ast) => ast,
        Err(e) => {
            error::display_parse_error(&source, filename, &e);
            process::exit(1);
        }
    };

    let ir = match lowering::lower_program(&ast) {
        Ok(ir) => ir,
        Err(e) => {
            error::display_lowering_error(&source, filename, &e);
            process::exit(1);
        }
    };

    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);

    let llvm_ir = match compiler.compile_program(&ir) {
        Ok(llvm_ir) => llvm_ir,
        Err(e) => {
            error::display_codegen_error(&source, filename, &e);
            process::exit(1);
        }
    };

    // Generate output filenames
    let path = Path::new(filename);
    let stem = path.file_stem().unwrap().to_str().unwrap();
    let ll_file = format!("{}.ll", stem);
    let output_file = stem.to_string();

    // Write LLVM IR to .ll file
    if let Err(e) = fs::write(&ll_file, llvm_ir) {
        eprintln!("Error writing LLVM IR file '{}': {}", ll_file, e);
        process::exit(1);
    }
    println!("Generated LLVM IR: {}", ll_file);

    // Compile LLVM IR to executable using clang
    println!("Compiling to executable...");
    let clang_output = Command::new("clang")
        .arg(&ll_file)
        .arg("-o")
        .arg(&output_file)
        .arg("-lm")
        .output();

    match clang_output {
        Ok(output) => {
            if output.status.success() {
                println!("✓ Successfully compiled to: {}", output_file);
            } else {
                eprintln!("Clang compilation failed:");
                eprintln!("{}", String::from_utf8_lossy(&output.stderr));
                process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("Error running clang: {}", e);
            eprintln!("Make sure clang is installed on your system.");
            process::exit(1);
        }
    }
}
