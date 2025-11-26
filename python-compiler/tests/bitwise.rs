use inkwell::context::Context;
use python_compiler::*;

#[test]
fn test_bitwise_and() {
    let source = "print(12 & 10)";
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_bitwise_or() {
    let source = "print(12 | 10)";
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_bitwise_xor() {
    let source = "print(12 ^ 10)";
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_left_shift() {
    let source = "print(5 << 2)";
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_right_shift() {
    let source = "print(20 >> 2)";
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_bitwise_complex_expression() {
    let source = "print((15 & 7) | (3 << 2))";
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_bitwise_with_variables() {
    let source = r#"
x = 12
y = 10
print(x & y)
print(x | y)
print(x ^ y)
"#;
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_shifts_with_variables() {
    let source = r#"
x = 8
y = 2
print(x << y)
print(x >> y)
"#;
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_bitwise_precedence() {
    // Test that bitwise operators have correct precedence
    // & has higher precedence than |
    let source = "print(8 | 4 & 2)";
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_bitwise_with_negatives() {
    let source = r#"
x = -5
y = 3
print(x & y)
print(x | y)
"#;
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_bitwise_zero() {
    let source = r#"
print(0 & 255)
print(0 | 255)
print(0 ^ 255)
"#;
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_bitwise_all_ones() {
    let source = r#"
x = 255
print(x & 255)
print(x | 0)
print(x ^ x)
"#;
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}
