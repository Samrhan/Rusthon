use inkwell::context::Context;
use python_compiler::*;

#[test]
fn test_multiplication_before_addition() {
    // Should be 1 + (2 * 3) = 7, not (1 + 2) * 3 = 9
    let source = "print(1 + 2 * 3)";
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_division_before_subtraction() {
    // Should be 10 - (6 / 2) = 7, not (10 - 6) / 2 = 2
    let source = "print(10 - 6 / 2)";
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_modulo_same_as_multiplication() {
    // Should be (10 + 7) % 4 = 1
    let source = "print(10 + 7 % 4)";
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_left_to_right_same_precedence() {
    // Should be (10 - 5) + 3 = 8
    let source = "print(10 - 5 + 3)";
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_multiplication_division_left_to_right() {
    // Should be (20 / 4) * 2 = 10
    let source = "print(20 / 4 * 2)";
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_parentheses_override_precedence() {
    // Should be (1 + 2) * 3 = 9
    let source = "print((1 + 2) * 3)";
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_nested_parentheses() {
    // Should be ((2 + 3) * (4 + 5)) = 5 * 9 = 45
    let source = "print((2 + 3) * (4 + 5))";
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_bitwise_precedence_and_before_or() {
    // & has higher precedence than |
    // Should be 8 | (4 & 2) = 8 | 0 = 8
    let source = "print(8 | 4 & 2)";
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_bitwise_precedence_and_before_xor() {
    // & has higher precedence than ^
    // Should be 12 ^ (8 & 4) = 12 ^ 0 = 12
    let source = "print(12 ^ 8 & 4)";
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_shifts_before_bitwise() {
    // Shifts have higher precedence than bitwise &, |, ^
    // Should be (8 << 1) & 4 = 16 & 4 = 0
    let source = "print(8 << 1 & 4)";
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_arithmetic_before_bitwise() {
    // Arithmetic has higher precedence than bitwise
    // Should be (5 + 3) & (10 - 2) = 8 & 8 = 8
    let source = "print(5 + 3 & 10 - 2)";
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_arithmetic_before_shifts() {
    // Arithmetic has higher precedence than shifts
    // Should be (2 + 3) << (1 + 1) = 5 << 2 = 20
    let source = "print(2 + 3 << 1 + 1)";
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_unary_before_binary() {
    // Unary operators have higher precedence than binary
    // Should be (-5) + 3 = -2, not -(5 + 3) = -8
    let source = "print(-5 + 3)";
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_unary_not_before_bitwise() {
    // Should be (~5) & 3
    let source = "print(~5 & 3)";
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_complex_precedence_expression() {
    // Tests: parentheses, unary, multiplication, addition, bitwise
    // Should be (((-2) * 3) + 5) & 7 = (-6 + 5) & 7 = -1 & 7
    let source = "print(-2 * 3 + 5 & 7)";
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_deeply_nested_expression() {
    // Complex expression with multiple levels
    let source = "print(((5 + 3) * 2 - 4) / (2 + 1))";
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_comparison_lower_than_arithmetic() {
    // Comparisons have lower precedence than arithmetic
    // Should be (5 + 3) > (2 * 4) = 8 > 8 = 0 (false)
    let source = r#"
x = 5 + 3 > 2 * 4
print(x)
"#;
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}

#[test]
fn test_not_lower_than_comparison() {
    // 'not' has lower precedence than comparisons
    // Should be not (5 > 3) = not true = false
    let source = "print(not 5 > 3)";
    let ast = parser::parse_program(source).unwrap();
    let ir = lowering::lower_program(&ast).unwrap();
    let context = Context::create();
    let compiler = codegen::Compiler::new(&context);
    let llvm_ir = compiler.compile_program(&ir).unwrap();
    insta::assert_snapshot!(llvm_ir);
}
