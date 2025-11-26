use inkwell::context::Context;
use python_compiler::*;

#[test]
fn test_minimal_function_no_defaults() {
    let source = r#"
def add(a, b):
    return a + b

x = add(1, 2)
print(x)
"#;
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();

    // Check the IR structure
    eprintln!("IR: {:#?}", ir);

    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let result = compiler.compile_program(&ir);

    match result {
        Ok(llvm_ir) => {
            eprintln!("Success! Generated {} bytes of LLVM IR", llvm_ir.len());
        }
        Err(e) => {
            panic!("Compilation failed: {}", e);
        }
    }
}
